// SPDX-License-Identifier: GPL-3.0-or-later
//! Face partitions and selected/picked planar edge-run offsets.
use crate::base::{Tool, ToolContext, ToolType};
use glam::DVec3;
use ingetrazo_core::{
    math::newell_vector,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
#[derive(Debug, Default, Clone)]
pub struct OffsetTool {
    pub hovered_face: Option<usize>,
    pub selected_face: Option<usize>,
    pub start_point: Option<DVec3>,
    pub offset_distance: f64,
    pub is_dragging: bool,
    hovered_edge: Option<usize>,
    run: Vec<DVec3>,
    normal: DVec3,
    closed: bool,
    armed: bool,
    reference: Option<(DVec3, DVec3)>,
    error: Option<String>,
}
impl OffsetTool {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn offset_polygon(points: &[DVec3], normal: DVec3, dist: f64) -> Option<Vec<DVec3>> {
        let regions = ingetrazo_core::offset::offset_regions(points, normal, dist).ok()?;
        if regions.len() == 1 {
            Some(regions[0].outer.clone())
        } else {
            None
        }
    }
    pub fn apply_offset(scene: &mut Scene, face_id: usize, dist: f64) -> Result<(), String> {
        let result = ingetrazo_core::offset::offset_face(&scene.mesh, face_id, dist)?;
        scene.mesh = result;
        scene.bump_version();
        Ok(())
    }
    fn take_run(&mut self, mesh: &Mesh, ids: &[usize]) -> Result<(), String> {
        let (points, closed) = crate::followme::order_path_edges(mesh, ids)?;
        let normal = newell_vector(&points).normalize_or_zero();
        if normal.length_squared() < 0.5 {
            return Err("Offset needs connected edges that are not in a straight line".into());
        }
        // Reuse the validated chain API for coplanarity even for closed runs.
        ingetrazo_core::offset::offset_chain(&points, normal, 0.)?;
        self.run = points;
        self.normal = normal;
        self.closed = closed;
        self.selected_face = None;
        Ok(())
    }
    fn picked_run(mesh: &Mesh, id: usize) -> Result<Vec<usize>, String> {
        let seed = mesh.edges.get(id).ok_or("Invalid Offset edge")?;
        let mut out = vec![id];
        let mut seen = std::collections::HashSet::from([id]);
        for start in [seed.v0, seed.v1] {
            let mut v = start;
            let mut previous = id;
            loop {
                let adjacent: Vec<_> = mesh
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.v0 == v || e.v1 == v)
                    .map(|(i, _)| i)
                    .collect();
                if adjacent.len() != 2 {
                    break;
                }
                let next = *adjacent.iter().find(|&&i| i != previous).unwrap();
                if !seen.insert(next) {
                    break;
                }
                out.push(next);
                let e = &mesh.edges[next];
                v = if e.v0 == v { e.v1 } else { e.v0 };
                previous = next;
            }
        }
        Ok(out)
    }
    fn start(&mut self, scene: &Scene, ctx: &ToolContext) -> Result<(), String> {
        if self.armed {
            self.armed = false;
        } else if let Some(id) = self
            .hovered_face
            .or_else(|| crate::base::pick_scene_face(scene, &ctx.ray).map(|p| p.0))
        {
            let face = &scene.mesh.faces[id];
            self.run = face
                .vertices
                .iter()
                .map(|&v| scene.mesh.vertices[v].position)
                .collect();
            self.normal = scene.mesh.face_normal(id);
            self.closed = true;
            self.selected_face = Some(id);
        } else if let Some(id) = self
            .hovered_edge
            .or_else(|| crate::base::pick_scene_edge(scene, &ctx.ray, 0.2))
        {
            self.take_run(&scene.mesh, &Self::picked_run(&scene.mesh, id)?)?;
        } else {
            return Err("Click a face or connected edges to offset".into());
        }
        let count = if self.closed {
            self.run.len()
        } else {
            self.run.len() - 1
        };
        let closest = (0..count)
            .map(|i| {
                let a = self.run[i];
                let b = self.run[(i + 1) % self.run.len()];
                let dir = (b - a).normalize();
                let t = (ctx.world - a).dot(dir).clamp(0., (b - a).length());
                let p = a + dir * t;
                (
                    (ctx.world - p).length_squared(),
                    p,
                    self.normal.cross(dir).normalize(),
                )
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .ok_or("No offset boundary")?;
        self.reference = Some((closest.1, closest.2));
        self.start_point = Some(ctx.world);
        self.offset_distance = 0.;
        self.is_dragging = true;
        self.error = None;
        Ok(())
    }
    fn commit(&mut self, scene: &mut Scene) -> Result<(), String> {
        if !self.offset_distance.is_finite() || self.offset_distance.abs() < 1e-9 {
            return Err("Offset distance must be finite and nonzero".into());
        }
        if let Some(id) = self.selected_face {
            Self::apply_offset(scene, id, self.offset_distance)?;
        } else {
            let runs: Vec<Vec<DVec3>> = if self.closed {
                let regions = ingetrazo_core::offset::offset_regions(
                    &self.run,
                    self.normal,
                    self.offset_distance,
                )?;
                if regions.is_empty() {
                    return Err("This distance consumes the run; use a smaller offset".into());
                }
                regions.into_iter().map(|r| r.outer).collect()
            } else {
                vec![ingetrazo_core::offset::offset_chain(
                    &self.run,
                    self.normal,
                    self.offset_distance,
                )?]
            };
            let mut candidate = scene.mesh.clone();
            for r in runs {
                let count = if self.closed { r.len() } else { r.len() - 1 };
                for i in 0..count {
                    let a = candidate.add_vertex(r[i]);
                    let b = candidate.add_vertex(r[(i + 1) % r.len()]);
                    candidate
                        .add_edge(a, b)
                        .ok_or("Offset edge collapsed at document precision")?;
                }
            }
            // Detect faces and split crossings only on this run's plane.
            let selected: Vec<_> = candidate
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| {
                    f.vertices.iter().all(|&v| {
                        (candidate.vertices[v].position - self.run[0])
                            .dot(self.normal)
                            .abs()
                            < 1e-6
                    })
                })
                .map(|(i, _)| i)
                .collect();
            if !selected.is_empty() || candidate.faces.is_empty() {
                candidate = ingetrazo_core::planar::rebuild_faces(&candidate, &selected)?;
            }
            ingetrazo_core::planar::validate_mesh(&candidate)?;
            scene.mesh = candidate;
            scene.bump_version();
        }
        self.reset();
        Ok(())
    }
}
impl Tool for OffsetTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Offset
    }
    fn name(&self) -> &'static str {
        "Offset"
    }
    fn shortcut(&self) -> Option<&'static str> {
        Some("F")
    }
    fn description(&self) -> &'static str {
        "Offset a face boundary or connected planar edges."
    }
    fn vcb_label(&self) -> &'static str {
        "Offset"
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn on_activate(&mut self, scene: &mut Scene) {
        self.reset();
        let mut ids: Vec<_> = scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::Edge(i) = s {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();
        ids.sort_unstable();
        if ids.len() >= 2 && ids.len() == scene.selection.len() {
            match self.take_run(&scene.mesh, &ids) {
                Ok(()) => self.armed = true,
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn on_deactivate(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_cancel(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.is_dragging {
            if let Some((p, n)) = self.reference {
                self.offset_distance = (ctx.world - p).dot(n);
            }
        } else if !self.armed {
            self.hovered_face = crate::base::pick_scene_face(scene, &ctx.ray).map(|p| p.0);
            self.hovered_edge = if self.hovered_face.is_none() {
                crate::base::pick_scene_edge(scene, &ctx.ray, 0.2)
            } else {
                None
            };
        }
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let result = if self.is_dragging {
            self.commit(scene)
        } else {
            self.start(scene, ctx)
        };
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let d=ingetrazo_core::units::vcb::parse_single(text,ingetrazo_core::units::model_scale(scene))?;
        if !d.is_finite() || d <= 0. {
            return Err("Enter a finite positive Offset distance".into());
        }
        if !self.is_dragging {
            return Err("Click to start the offset before entering a distance".into());
        }
        self.offset_distance = if self.offset_distance < 0. { -d } else { d };
        let result = self.commit(scene);
        if let Err(e) = &result {
            self.error = Some(e.clone());
        }
        result
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        if !self.is_dragging || self.offset_distance.abs() < 1e-9 {
            return vec![];
        }
        let loops = if self.closed {
            ingetrazo_core::offset::offset_regions(&self.run, self.normal, self.offset_distance)
                .map(|rs| rs.into_iter().map(|r| r.outer).collect::<Vec<_>>())
        } else {
            ingetrazo_core::offset::offset_chain(&self.run, self.normal, self.offset_distance)
                .map(|r| vec![r])
        };
        loops
            .unwrap_or_default()
            .iter()
            .flat_map(|r| {
                let count = if self.closed {
                    r.len()
                } else {
                    r.len().saturating_sub(1)
                };
                (0..count).map(move |i| (r[i], r[(i + 1) % r.len()]))
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_offset_tool_inward() {
        let mut scene = Scene::new();
        // 10x10 square
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(10.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(10.0, 10.0, 0.0));
        let v3 = scene.mesh.add_vertex(DVec3::new(0.0, 10.0, 0.0));
        let f0 = scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]).unwrap();

        let mut tool = OffsetTool::new();
        let ray = Ray::new(DVec3::new(5.0, 5.0, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(5.0, 5.0, 0.0), ray);

        tool.on_click(&mut scene, &ctx);
        assert_eq!(tool.selected_face, Some(f0));

        // Offset inward by 1.0 via VCB
        assert!(tool.on_vcb_input(&mut scene, "1.0").is_ok());

        // Should have added an 8x8 inner face
        assert_eq!(scene.mesh.faces.len(), 2);
        let inner_face_id = 1;
        let inner_area = scene.mesh.face_area(inner_face_id);
        assert!((inner_area - 64.0).abs() < 1e-4);
    }
    #[test]
    fn selected_chain_preview_cancel_and_outward_positive_vcb() {
        use ingetrazo_core::scene::SelectionItem;
        let mut scene = Scene::new();
        let ids: Vec<_> = [
            DVec3::ZERO,
            DVec3::new(10., 0., 0.),
            DVec3::new(10., 10., 0.),
        ]
        .into_iter()
        .map(|p| scene.mesh.add_vertex(p))
        .collect();
        for pair in ids.windows(2) {
            let e = scene.mesh.add_edge(pair[0], pair[1]).unwrap();
            scene.select(SelectionItem::Edge(e));
        }
        let before = scene.mesh.clone();
        let mut tool = OffsetTool::new();
        tool.on_activate(&mut scene);
        assert!(tool.armed);
        let ctx = ToolContext::new(
            DVec3::new(5., 0., 0.),
            Ray::new(DVec3::new(5., 0., 5.), -DVec3::Z),
        );
        tool.on_click(&mut scene, &ctx);
        let moved = ToolContext::new(DVec3::new(7., -1., 0.), ctx.ray.clone());
        tool.on_move(&mut scene, &moved);
        assert_eq!(tool.offset_distance, -1.);
        assert_eq!(tool.rubber_band_lines().len(), 2);
        assert_eq!(scene.mesh, before);
        tool.on_cancel(&mut scene);
        assert_eq!(scene.mesh, before);
        tool.on_activate(&mut scene);
        tool.on_click(&mut scene, &ctx);
        tool.on_move(&mut scene, &moved);
        tool.on_vcb_input(&mut scene, "2").unwrap();
        assert!(scene
            .mesh
            .vertices
            .iter()
            .any(|v| v.position.distance(DVec3::new(0., -2., 0.)) < 1e-8));
        assert_eq!(scene.mesh.edges.len(), 4);
        assert_eq!(scene.mesh.faces.len(), 0);
    }
    #[test]
    fn straight_run_reports_failure_without_mutating_and_drag_is_perpendicular() {
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::ZERO);
        let b = scene.mesh.add_vertex(DVec3::X);
        let c = scene.mesh.add_vertex(DVec3::X * 2.);
        for (a, b) in [(a, b), (b, c)] {
            let e = scene.mesh.add_edge(a, b).unwrap();
            scene.select(SelectionItem::Edge(e));
        }
        let before = scene.mesh.clone();
        let mut tool = OffsetTool::new();
        tool.on_activate(&mut scene);
        assert!(tool.error_message().is_some());
        assert_eq!(scene.mesh, before);
        let mut scene = Scene::new();
        let ids = [
            DVec3::ZERO,
            DVec3::X * 10.,
            DVec3::new(10., 10., 0.),
            DVec3::Y * 10.,
        ]
        .into_iter()
        .map(|p| scene.mesh.add_vertex(p))
        .collect();
        scene.mesh.add_face(ids, vec![]);
        let click = ToolContext::new(
            DVec3::new(3., 0., 0.),
            Ray::new(DVec3::new(3., 0., 5.), -DVec3::Z),
        );
        tool.reset();
        tool.on_click(&mut scene, &click);
        let moved = ToolContext::new(DVec3::new(8., 1., 0.), click.ray.clone());
        tool.on_move(&mut scene, &moved);
        assert_eq!(tool.offset_distance, 1.);
        let before = scene.mesh.clone();
        assert!(tool.on_vcb_input(&mut scene, "NaN").is_err());
        assert_eq!(scene.mesh, before);
    }
}
