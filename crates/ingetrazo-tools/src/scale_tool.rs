// SPDX-License-Identifier: GPL-3.0-or-later
//! Original grip-box Scale: grab-time transforms, modifier taps and hot retype.
pub mod grips;
mod targets;
use crate::base::{ScaleBoxState, Tool, ToolContext, ToolType};
use glam::{DVec2, DVec3};
use grips::{Grip, GripBox, MIN_FACTOR};
use ingetrazo_core::{
    image_plane::{ImageFrameCommand, ImageFrameOperation, ScaleMatrix},
    scene::{Scene, SelectionItem},
    screen_pick::PixelViewport,
    snap::{SnapKind, SnapResult},
};
use targets::Targets;

#[derive(Debug, Clone)]
struct Operation {
    bounds: GripBox,
    grip: Grip,
    anchor: DVec3,
    factors: DVec3,
    targets: Targets,
    original_version: u64,
    screen: Option<DVec2>,
    moved: bool,
}
#[derive(Debug, Clone)]
struct Last {
    operation: Operation,
    uniform: bool,
    revision: Option<u64>,
    unfolded: Option<std::sync::Arc<ingetrazo_core::mesh::Mesh>>,
}
#[derive(Clone, Default)]
pub struct ScaleTool {
    pub bounds: Option<GripBox>,
    pub about_center: bool,
    pub uniform: bool,
    box_version: Option<u64>,
    hover: Option<Grip>,
    active: Option<Operation>,
    last: Option<Last>,
    control_down: bool,
    shift_down: bool,
    viewport: Option<PixelViewport>,
    snap: Option<DVec3>,
    image_command: Option<ImageFrameCommand>,
    replace_history: bool,
    status: Option<String>,
    error: Option<String>,
}
impl ScaleTool {
    pub fn new() -> Self {
        Self::default()
    }
    fn refresh_box(&mut self, scene: &Scene) {
        if self.active.is_some() || self.box_version == Some(scene.version) {
            return;
        }
        self.bounds = targets::selection_box(scene);
        self.box_version = Some(scene.version);
        self.hover = None;
    }
    fn grip_under(&self, ctx: &ToolContext) -> Option<Grip> {
        let bounds = self.bounds.as_ref()?;
        if let (Some(viewport), Some(cursor)) = (&self.viewport, ctx.screen) {
            let mut best = None;
            let mut distance = 8.;
            for &grip in &bounds.grips {
                if let Some(p) = viewport.project_for_snapping(bounds.position(grip)) {
                    let d = p.distance(cursor);
                    if d < distance {
                        distance = d;
                        best = Some(grip);
                    }
                }
            }
            best
        } else {
            // Headless callers identify a grip by its actual world position.
            bounds
                .grips
                .iter()
                .copied()
                .find(|&g| bounds.position(g).distance(ctx.world) < 1e-6)
        }
    }
    fn grab(&mut self, scene: &Scene, grip: Grip, screen: Option<DVec2>) {
        let bounds = self.bounds.as_ref().unwrap().clone();
        let anchor = bounds.anchor(grip, self.about_center);
        let targets = Targets::capture(scene);
        if targets.is_empty() {
            return;
        }
        self.active = Some(Operation {
            bounds,
            grip,
            anchor,
            factors: DVec3::ONE,
            targets,
            original_version: scene.version,
            screen,
            moved: false,
        });
    }
    fn factors(&self, ctx: &ToolContext) -> Option<DVec3> {
        let op = self.active.as_ref()?;
        self.snap
            .or_else(|| ctx.screen.is_none().then_some(ctx.world))
            .and_then(|p| {
                op.bounds
                    .factors_from_point(op.grip, op.anchor, p, self.uniform)
            })
            .or_else(|| {
                op.bounds
                    .factors_from_ray(op.grip, op.anchor, &ctx.ray, self.uniform)
            })
    }
    fn finite_matrix(op: &Operation, matrix: &ScaleMatrix) -> bool {
        matrix.matrix().is_finite()
            && op
                .bounds
                .grips
                .iter()
                .all(|&g| matrix.point(op.bounds.position(g)).is_finite())
    }
    fn preview(&mut self, scene: &mut Scene, factors: DVec3) {
        if !factors.is_finite() || factors.abs().min_element() < MIN_FACTOR {
            return;
        }
        let Some(op) = self.active.as_mut() else {
            return;
        };
        if (factors - op.factors).abs().max_element() < 1e-12 {
            return;
        }
        let matrix = ScaleMatrix::new(op.anchor, factors, op.bounds.frame.map(|f| f.axes));
        if !Self::finite_matrix(op, &matrix) {
            return;
        }
        let identity = (factors - DVec3::ONE).abs().max_element() < 1e-12;
        op.targets
            .apply(scene, (!identity).then_some(&matrix), false);
        op.factors = factors;
    }
    fn commit(&mut self, scene: &mut Scene, factors: DVec3) {
        if let Some(op) = &self.active {
            let axes = if factors.x == factors.y && factors.y == factors.z {
                None
            } else {
                op.bounds.frame.map(|f| f.axes)
            };
            let matrix = ScaleMatrix::new(op.anchor, factors, axes);
            if !Self::finite_matrix(op, &matrix) || !op.targets.shared_commit_finite(&matrix) {
                self.error = Some("Scale exceeds the finite coordinate range".into());
                return;
            }
        }
        let Some(mut op) = self.active.take() else {
            return;
        };
        op.targets.apply(scene, None, false);
        if factors.is_finite()
            && factors.abs().min_element() >= MIN_FACTOR
            && (factors - DVec3::ONE).abs().max_element() > 1e-9
        {
            let axes = if factors.x == factors.y && factors.y == factors.z {
                None
            } else {
                op.bounds.frame.map(|f| f.axes)
            };
            let matrix = ScaleMatrix::new(op.anchor, factors, axes);
            op.targets.apply(scene, Some(&matrix), true);
            let unfolded = if op.targets.has_loose_vertices()
                && ingetrazo_core::autofold::needs_fold(&scene.mesh)
            {
                // Only a genuinely warped commit retains a topology baseline;
                // mouse motion and planar commands carry target rows only.
                let mut original = scene.mesh.clone();
                op.targets.apply_cold_mesh(&mut original);
                if let Err(error) = ingetrazo_core::autofold::fold_nonplanar_faces(&mut scene.mesh)
                {
                    op.targets.apply(scene, None, true);
                    self.error = Some(error);
                    self.active = Some(op);
                    return;
                }
                Some(std::sync::Arc::new(original))
            } else {
                None
            };
            if !op.targets.images.is_empty() {
                self.image_command = Some(ImageFrameCommand {
                    images: op.targets.images.clone(),
                    operation: ImageFrameOperation::Scale {
                        anchor: op.anchor,
                        factor: factors,
                        axes,
                    },
                });
            }
            op.factors = factors;
            self.last = Some(Last {
                operation: op,
                uniform: self.uniform,
                revision: None,
                unfolded,
            });
        }
        self.box_version = None;
        self.refresh_box(scene);
        self.snap = None;
    }
    fn pick_selection(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let item = crate::base::pick_scene_group(scene, &ctx.ray, 0.2)
            .map(|(i, _)| SelectionItem::Group(i))
            .or_else(|| {
                scene
                    .image_plane_hit(
                        &ctx.ray,
                        ingetrazo_core::image_plane::ImagePickMode::Selectable,
                    )
                    .map(|(i, _)| SelectionItem::ImagePlane(i))
            })
            .or_else(|| crate::base::pick_scene_edge(scene, &ctx.ray, 0.2).map(SelectionItem::Edge))
            .or_else(|| {
                crate::base::pick_scene_face(scene, &ctx.ray)
                    .map(|(i, _, _)| SelectionItem::Face(i))
            });
        if let Some(item) = item {
            scene.select(item);
            self.box_version = None;
            self.refresh_box(scene);
        } else {
            self.status = Some("Select (or click) the geometry to scale first".into());
        }
    }
    fn parse(text: &str) -> Result<(Vec<f64>, bool), String> {
        let text = text.trim();
        let absolute = text.ends_with(['m', 't', 'n', '"', '\'']);
        let values = text
            .split(';')
            .map(|s| {
                if absolute {
                    ingetrazo_core::units::parse_length(
                        s,
                        ingetrazo_core::units::LengthUnit::Meters,
                    )
                } else {
                    s.trim()
                        .replace(',', ".")
                        .parse::<f64>()
                        .map_err(|_| "Invalid scale factor".into())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((values, absolute))
    }
    fn axes_caption(&self) -> &'static str {
        let Some(bounds) = self.bounds.as_ref() else {
            return "Scale";
        };
        let Some(grip) = self.active.as_ref().map(|o| o.grip).or(self.hover) else {
            return "Scale";
        };
        if self.uniform || grip.is_corner(bounds.active_axes()) {
            return "Scale";
        }
        match grip.mask {
            [true, false, false] => "Red Scale",
            [false, true, false] => "Green Scale",
            [false, false, true] => "Blue Scale",
            [true, true, false] => "Red, Green Scale",
            [true, false, true] => "Red, Blue Scale",
            [false, true, true] => "Green, Blue Scale",
            _ => "Scale",
        }
    }
}
impl Tool for ScaleTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Scale
    }
    fn name(&self) -> &'static str {
        "Scale"
    }
    fn shortcut(&self) -> Option<&'static str> {
        Some("S")
    }
    fn description(&self) -> &'static str {
        "Resize the selection by dragging its box grips; corners keep its proportions."
    }
    fn vcb_label(&self) -> &'static str {
        self.axes_caption()
    }
    fn on_activate(&mut self, scene: &mut Scene) {
        *self = Self::default();
        self.refresh_box(scene);
    }
    fn on_deactivate(&mut self, scene: &mut Scene) {
        self.on_cancel(scene);
        self.bounds = None;
        self.box_version = None;
    }
    fn on_cancel(&mut self, scene: &mut Scene) {
        if let Some(mut op) = self.active.take() {
            op.targets.apply(scene, None, false);
        }
        self.last = None;
        self.snap = None;
        self.box_version = None;
        self.refresh_box(scene);
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.error = None;
        self.last = None;
        self.on_control_state(scene, ctx.ctrl_held);
        self.note_shift_state(ctx.shift_held);
        self.refresh_box(scene);
        if self.active.is_some() {
            if let Some(f) = self.factors(ctx) {
                self.commit(scene, f);
            }
            return;
        }
        if let Some(grip) = self.grip_under(ctx) {
            self.grab(scene, grip, ctx.screen);
        } else if scene.selection.is_empty() {
            self.pick_selection(scene, ctx);
        }
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.on_control_state(scene, ctx.ctrl_held);
        self.note_shift_state(ctx.shift_held);
        if self.active.is_none() {
            self.refresh_box(scene);
            self.hover = self.grip_under(ctx);
            return;
        }
        if let Some(f) = self.factors(ctx) {
            if f.abs().min_element() < MIN_FACTOR || !f.is_finite() {
                return;
            }
            if let Some(op) = self.active.as_mut() {
                if let (Some(a), Some(b)) = (op.screen, ctx.screen) {
                    op.moved |= a.distance(b) > 4.;
                }
            }
            self.preview(scene, f);
        }
    }
    fn on_release(&mut self, scene: &mut Scene, _ctx: &ToolContext, drag: f64) {
        if let Some(op) = &self.active {
            if (op.moved || drag > 4.) && (op.factors - DVec3::ONE).abs().max_element() > 1e-9 {
                self.commit(scene, op.factors);
            }
        }
    }
    fn on_control_state(&mut self, scene: &mut Scene, held: bool) {
        if held && !self.control_down {
            self.about_center = !self.about_center;
            if let Some(op) = self.active.as_mut() {
                op.targets.apply(scene, None, false);
                op.anchor = op.bounds.anchor(op.grip, self.about_center);
                let matrix =
                    ScaleMatrix::new(op.anchor, op.factors, op.bounds.frame.map(|f| f.axes));
                op.targets.apply(scene, Some(&matrix), false);
            }
            self.status = Some(format!(
                "About Center: {}",
                if self.about_center { "on" } else { "off" }
            ));
        }
        self.control_down = held;
    }
    fn note_shift_state(&mut self, held: bool) {
        if held && !self.shift_down {
            self.uniform = !self.uniform;
            self.status = Some(format!(
                "Scale Uniformly: {}",
                if self.uniform { "on" } else { "off" }
            ));
        }
        self.shift_down = held;
    }
    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        self.error = None;
        let (values, absolute) = Self::parse(text)?;
        if let Some(op) = &self.active {
            let factors = op
                .bounds
                .typed_factors(op.grip, self.uniform, &values, absolute)
                .ok_or("Invalid factors for this grip")?;
            self.commit(scene, factors);
            return self.error.clone().map_or(Ok(()), Err);
        }
        let last = self.last.as_mut().ok_or("Grab a scale grip first")?;
        let factors = last
            .operation
            .bounds
            .typed_factors(last.operation.grip, last.uniform, &values, absolute)
            .ok_or("Invalid factors for this grip")?;
        let op = &mut last.operation;
        let axes = if factors.x == factors.y && factors.y == factors.z {
            None
        } else {
            op.bounds.frame.map(|f| f.axes)
        };
        let matrix = ScaleMatrix::new(op.anchor, factors, axes);
        if !Self::finite_matrix(op, &matrix) || !op.targets.shared_commit_finite(&matrix) {
            return Err("Scale exceeds the finite coordinate range".into());
        }
        if let Some(unfolded) = &last.unfolded {
            scene.mesh = (**unfolded).clone();
        }
        op.targets.apply(scene, None, true);
        op.targets.apply(scene, Some(&matrix), true);
        if op.targets.has_loose_vertices() && ingetrazo_core::autofold::needs_fold(&scene.mesh) {
            if last.unfolded.is_none() {
                let mut original = scene.mesh.clone();
                op.targets.apply_cold_mesh(&mut original);
                last.unfolded = Some(std::sync::Arc::new(original));
            }
            ingetrazo_core::autofold::fold_nonplanar_faces(&mut scene.mesh)?;
        }
        if !op.targets.images.is_empty() {
            self.image_command = Some(ImageFrameCommand {
                images: op.targets.images.clone(),
                operation: ImageFrameOperation::Scale {
                    anchor: op.anchor,
                    factor: factors,
                    axes,
                },
            });
        }
        op.factors = factors;
        self.replace_history = true;
        self.box_version = None;
        self.refresh_box(scene);
        Ok(())
    }
    fn note_history_revision(&mut self, revision: u64, committed: bool) {
        if committed {
            if let Some(last) = &mut self.last {
                last.revision = Some(revision);
            }
        } else if self
            .last
            .as_ref()
            .is_some_and(|l| l.revision.is_some_and(|r| r != revision))
        {
            self.last = None;
        }
    }
    fn take_history_replacement(&mut self) -> bool {
        std::mem::take(&mut self.replace_history)
    }
    fn pending_model_preview(&self) -> bool {
        self.active.is_some()
    }
    fn placement_only_preview(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|o| o.targets.only_placements())
    }
    fn restore_transient_model(&self, scene: &mut Scene) {
        if let Some(op) = &self.active {
            op.targets.apply_cold(scene, None, false);
            scene.version = op.original_version;
        }
    }
    fn restore_current_preview_model(&self, scene: &mut Scene) {
        if let Some(op) = &self.active {
            let m = ScaleMatrix::new(op.anchor, op.factors, op.bounds.frame.map(|f| f.axes));
            op.targets.apply_cold(scene, Some(&m), false);
        }
    }
    fn end_transient_before_history(&mut self, scene: &mut Scene) {
        self.on_cancel(scene);
    }
    fn preserves_global_preview_on_context_exit(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|o| o.targets.only_images())
    }
    fn visit_image_metadata_roots(
        &self,
        visit: &mut dyn FnMut(&ingetrazo_core::image_plane::ImagePlane),
    ) {
        for op in self
            .active
            .iter()
            .chain(self.last.iter().map(|l| &l.operation))
        {
            for image in &op.targets.images {
                visit(image);
            }
        }
    }
    fn take_image_frame_command(&mut self) -> Option<ImageFrameCommand> {
        self.image_command.take()
    }
    fn note_pick_viewport(&mut self, viewport: PixelViewport, _eye: DVec3, _revision: u64) {
        self.viewport = Some(viewport);
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.active.as_ref().map(|o| o.anchor)
    }
    fn snap_exclusions(&self) -> Option<&ingetrazo_core::snap_screen::SnapExclusions> {
        self.active.as_ref().map(|o| &o.targets.exclusions)
    }
    fn uses_snap(&self) -> bool {
        self.active.is_some()
    }
    fn note_snap_result(&mut self, result: &SnapResult) {
        self.snap = matches!(
            result.kind,
            SnapKind::Endpoint
                | SnapKind::Midpoint
                | SnapKind::Intersection
                | SnapKind::Center
                | SnapKind::Origin
                | SnapKind::OnEdge
                | SnapKind::FromPoint
                | SnapKind::Close
                | SnapKind::Guide
        )
        .then_some(result.point);
    }
    fn value_label(&self) -> Option<(String, DVec3)> {
        let op = self.active.as_ref()?;
        let text = if self.uniform || op.grip.is_corner(op.bounds.active_axes()) {
            format!("{:.2}", op.factors[op.grip.axes().next()?])
        } else {
            op.grip
                .axes()
                .map(|i| format!("{:.2}", op.factors[i]))
                .collect::<Vec<_>>()
                .join("; ")
        };
        Some((text, op.bounds.position(op.grip)))
    }
    fn scale_box_state(&self) -> Option<ScaleBoxState> {
        let bounds = self.bounds.as_ref()?;
        let matrix = self
            .active
            .as_ref()
            .map(|op| ScaleMatrix::new(op.anchor, op.factors, op.bounds.frame.map(|f| f.axes)));
        let map = |p| matrix.as_ref().map_or(p, |m| m.point(p));
        let segments = bounds
            .segments()
            .into_iter()
            .map(|(a, b)| (map(a), map(b)))
            .collect();
        let mut grips = bounds
            .grips
            .iter()
            .map(|&g| {
                (
                    map(bounds.position(g)),
                    self.active
                        .as_ref()
                        .map_or(self.hover == Some(g), |o| o.grip == g),
                )
            })
            .collect::<Vec<_>>();
        if let Some(op) = &self.active {
            grips.push((op.anchor, true));
        }
        Some(ScaleBoxState { segments, grips })
    }
    fn take_status_message(&mut self) -> Option<String> {
        self.status.take()
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
