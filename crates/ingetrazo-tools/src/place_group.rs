// SPDX-License-Identifier: GPL-3.0-or-later
//! Source PlaceGroup: a privately owned candidate, cached capped outlines, one insert.
use crate::base::{Tool, ToolContext, ToolType};
use glam::{DMat4, DVec3, Vec3};
use ingetrazo_core::{
    groups::Group,
    scene::{Scene, SelectionItem},
};

const MAX_EDGES: usize = 2000;
#[derive(Default)]
pub struct PlaceGroupTool {
    group: Option<Group>,
    instance: bool,
    anchor: Vec3,
    offset: Vec3,
    align: bool,
    normal: Option<Vec3>,
    segments: Vec<(Vec3, Vec3)>,
    preview: Vec<(DVec3, DVec3)>,
    completed: bool,
    status: Option<String>,
    error: Option<String>,
}
fn mul(a: DMat4, b: DMat4) -> DMat4 {
    let a = a.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    let b = b.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    let mut out = [[0.; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            out[c][r] =
                (a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3])
                    as f64;
        }
    }
    DMat4::from_cols_array_2d(&out)
}
fn mapped(m: DMat4, p: Vec3) -> Vec3 {
    let a = m.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    Vec3::from_array(std::array::from_fn(|r| {
        a[0][r] * p.x + a[1][r] * p.y + a[2][r] * p.z + a[3][r]
    }))
}
fn walk(group: &Group, parent: DMat4, depth: usize, visit: &mut impl FnMut(&Group, DMat4)) -> bool {
    if depth > 128 || !group.transform.is_finite() {
        return false;
    }
    let world = mul(parent, group.transform);
    if !world.is_finite() {
        return false;
    }
    visit(group, world);
    group
        .children
        .iter()
        .all(|g| walk(g, world, depth + 1, visit))
}
fn normalized(p: Vec3) -> Vec3 {
    let q = p.as_dvec3();
    let len = q.length_squared();
    if (len - 1.).abs() <= 1e-12 {
        p
    } else if len.abs() > 1e-12 {
        (q / len.sqrt()).as_vec3()
    } else {
        Vec3::ZERO
    }
}
fn frame(normal: Vec3) -> [Vec3; 3] {
    let n = normalized(normal);
    let hint = if n.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
    let mut right = hint.cross(n);
    if right.length() < 1e-9 {
        right = Vec3::X;
    }
    right = normalized(right);
    [right, -n, normalized(n.cross(right))]
}
impl PlaceGroupTool {
    pub fn new(group: Group, align_to_face: bool) -> Self {
        Self::construct(group, align_to_face, None)
    }
    pub fn with_anchor(group: Group, align_to_face: bool, anchor: DVec3) -> Self {
        Self::construct(group, align_to_face, Some(anchor))
    }
    fn construct(group: Group, align: bool, anchor: Option<DVec3>) -> Self {
        let mut tool = Self {
            instance: group.component_definition.is_some()
                || !group.children.is_empty()
                || group.transform != DMat4::IDENTITY
                || group
                    .native_document
                    .get("xform")
                    .is_some_and(|v| v.is_array()),
            align,
            ..Self::default()
        };
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        let mut count = 0usize;
        let mut valid = true;
        let structure_valid = walk(&group, DMat4::IDENTITY, 0, &mut |g, m| {
            count = count.saturating_add(g.mesh.edges.len());
            for v in &g.mesh.vertices {
                valid &= v.position.as_vec3().is_finite();
                let p = m.transform_point3(v.position.as_vec3().as_dvec3());
                lo = lo.min(p);
                hi = hi.max(p);
            }
            valid &= g
                .mesh
                .edges
                .iter()
                .all(|e| e.v0 < g.mesh.vertices.len() && e.v1 < g.mesh.vertices.len());
        });
        valid &= structure_valid;
        tool.anchor = anchor.map(|p| p.as_vec3()).unwrap_or_else(|| {
            if lo.is_finite() {
                DVec3::new((lo.x + hi.x) / 2., (lo.y + hi.y) / 2., lo.z).as_vec3()
            } else {
                Vec3::ZERO
            }
        });
        valid &= tool.anchor.is_finite();
        if valid {
            if group.children.is_empty() {
                tool.segments
                    .extend(group.mesh.edges.iter().take(MAX_EDGES).map(|e| {
                        (
                            group.mesh.vertices[e.v0].position.as_vec3() - tool.anchor,
                            group.mesh.vertices[e.v1].position.as_vec3() - tool.anchor,
                        )
                    }));
            } else if count <= MAX_EDGES {
                walk(&group, DMat4::IDENTITY, 0, &mut |g, m| {
                    for e in &g.mesh.edges {
                        tool.segments.push((
                            mapped(m, g.mesh.vertices[e.v0].position.as_vec3()) - tool.anchor,
                            mapped(m, g.mesh.vertices[e.v1].position.as_vec3()) - tool.anchor,
                        ));
                    }
                });
            }
            if tool.segments.is_empty() && lo.is_finite() {
                let a = lo.as_vec3() - tool.anchor;
                let b = hi.as_vec3() - tool.anchor;
                let corners: [Vec3; 8] = std::array::from_fn(|i| {
                    Vec3::new(
                        if i & 1 == 0 { a.x } else { b.x },
                        if i & 2 == 0 { a.y } else { b.y },
                        if i & 4 == 0 { a.z } else { b.z },
                    )
                });
                for (a, b) in [
                    (0, 1),
                    (1, 3),
                    (3, 2),
                    (2, 0),
                    (4, 5),
                    (5, 7),
                    (7, 6),
                    (6, 4),
                    (0, 4),
                    (1, 5),
                    (2, 6),
                    (3, 7),
                ] {
                    tool.segments.push((corners[a], corners[b]));
                }
            }
            tool.refresh_preview();
        } else {
            tool.error = Some("Invalid component placement geometry".into());
        }
        tool.group = Some(group);
        tool
    }
    fn rotate(&self, p: Vec3) -> Vec3 {
        self.normal.map_or(p, |n| {
            let [r, y, u] = frame(n);
            r * p.x + y * p.y + u * p.z
        })
    }
    fn refresh_preview(&mut self) {
        self.preview.clear();
        self.preview.reserve(self.segments.len());
        for &(a, b) in &self.segments {
            self.preview.push((
                (self.rotate(a + self.anchor) + self.offset).as_dvec3(),
                (self.rotate(b + self.anchor) + self.offset).as_dvec3(),
            ));
        }
    }
    fn update_alignment(&mut self, scene: &Scene, ctx: &ToolContext) {
        if self.align {
            // Source instance chunks return the prototype Face, hence its local normal.
            self.normal = ingetrazo_core::screen_pick::hover_face_placement(scene, &ctx.ray)
                .map(|h| h.mesh.face_normal(h.face).as_vec3());
        }
    }
    fn pose(&self, shift: Vec3) -> DMat4 {
        let [r, y, u] = self
            .normal
            .map(frame)
            .unwrap_or([Vec3::X, Vec3::Y, Vec3::Z]);
        DMat4::from_cols(
            r.as_dvec3().extend(0.),
            y.as_dvec3().extend(0.),
            u.as_dvec3().extend(0.),
            shift.as_dvec3().extend(1.),
        )
    }
    pub fn place_at(&mut self, scene: &mut Scene, world: DVec3) -> Result<(), String> {
        if self.group.is_none() {
            return Ok(());
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if !world.as_vec3().is_finite() {
            return Err("Nonfinite placement point".into());
        }
        let shift = world.as_vec3() - self.rotate(self.anchor);
        let pose = self.pose(shift);
        let group = self.group.as_ref().unwrap();
        if !shift.is_finite()
            || !pose.is_finite()
            || (self.instance && !mul(pose, group.transform).is_finite())
        {
            return Err("Nonfinite placement transform".into());
        }
        let positions = if self.instance {
            None
        } else {
            let mut out = Vec::new();
            out.try_reserve_exact(group.mesh.vertices.len())
                .map_err(|_| "Placement positions refused")?;
            for v in &group.mesh.vertices {
                let p = v.position.as_vec3();
                let target = self.rotate(p) + shift;
                let delta = target - p;
                if !target.is_finite() || !delta.is_finite() {
                    return Err("Nonfinite placed vertex".into());
                }
                out.push(if delta.length() > 1e-9 {
                    (p + delta).as_dvec3()
                } else {
                    v.position
                });
            }
            Some(out)
        };
        let prepared = positions
            .as_ref()
            .map(|p| ingetrazo_core::mesh::Mesh::prepare_spatial_index(p))
            .transpose()
            .map_err(str::to_owned)?;
        scene
            .groups
            .try_reserve(1)
            .map_err(|_| "Placement group capacity refused")?;
        scene
            .selection
            .try_reserve(1)
            .map_err(|_| "Placement selection capacity refused")?;
        scene
            .selection_order
            .try_reserve(1)
            .map_err(|_| "Placement selection order refused")?;
        let mut group = self.group.take().unwrap();
        if self.instance {
            group.transform = mul(pose, group.transform);
        } else {
            let positions = positions.unwrap();
            let index = prepared.unwrap();
            for (v, p) in group.mesh.vertices.iter_mut().zip(positions) {
                v.position = p;
            }
            group
                .mesh
                .install_prepared_spatial_index(index)
                .expect("Prepared exact placement positions");
            let inverse = pose.inverse().as_mat4().as_dmat4();
            for face in &mut group.mesh.faces {
                if let Some(t) = face
                    .attrs
                    .get_mut("texture")
                    .and_then(|v| v.as_object_mut())
                {
                    let values = t
                        .get("uvw")
                        .and_then(|v| v.as_array())
                        .filter(|a| a.len() == 8)
                        .and_then(|a| a.iter().map(|v| v.as_f64()).collect::<Option<Vec<_>>>());
                    if let Some(mut values) = values {
                        for base in [0, 4] {
                            let g = inverse.transpose().transform_vector3(DVec3::new(
                                values[base],
                                values[base + 1],
                                values[base + 2],
                            ));
                            let c = values[base + 3] - g.dot(shift.as_dvec3());
                            values[base..base + 4].copy_from_slice(&[g.x, g.y, g.z, c]);
                        }
                        t.insert("uvw".into(), serde_json::json!(values));
                    }
                }
            }
            let axes = mul(pose, group.axes_matrix().unwrap_or(DMat4::IDENTITY));
            if !group.native_document.is_object() {
                group.native_document = serde_json::json!({});
            }
            group.native_document["axes"] = serde_json::json!(axes.to_cols_array());
        }
        let index = scene.groups.len();
        scene.groups.push(group);
        scene.clear_selection();
        scene.select(SelectionItem::Group(index));
        scene.bump_version();
        self.preview.clear();
        self.completed = true;
        self.status = Some("Component placed — Move (M) adjusts it".into());
        Ok(())
    }
}
impl Tool for PlaceGroupTool {
    fn tool_type(&self) -> ToolType {
        ToolType::PlaceGroup
    }
    fn name(&self) -> &'static str {
        "Place component"
    }
    fn completed(&self) -> bool {
        self.completed
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn take_status_message(&mut self) -> Option<String> {
        self.status.take()
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.group.is_some() && self.error.is_none() && ctx.world.as_vec3().is_finite() {
            self.update_alignment(scene, ctx);
            self.offset = ctx.world.as_vec3() - self.rotate(self.anchor);
            self.refresh_preview();
        }
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.group.is_some() {
            self.update_alignment(scene, ctx);
            if let Err(error) = self.place_at(scene, ctx.world) {
                self.error = Some(error);
            }
        }
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.group = None;
        self.preview.clear();
        self.completed = true;
    }
    fn on_deactivate(&mut self, scene: &mut Scene) {
        self.on_cancel(scene);
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        self.preview.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Group {
        let mut g = Group::new("sign");
        let a = g.mesh.add_vertex(DVec3::new(0., 0., 0.));
        let b = g.mesh.add_vertex(DVec3::new(2., 0., 2.));
        g.mesh.add_edge(a, b);
        g
    }
    #[test]
    fn placement_and_cancel_ownership() {
        let mut scene = Scene::default();
        let mut t = PlaceGroupTool::new(candidate(), false);
        assert_eq!(t.anchor, Vec3::new(1., 0., 0.));
        assert_eq!(t.preview.len(), 1);
        t.place_at(&mut scene, DVec3::new(10., 20., 30.)).unwrap();
        assert_eq!(
            scene.groups[0].mesh.vertices[0].position,
            DVec3::new(9., 20., 30.)
        );
        assert_eq!(
            scene.groups[0].mesh.vertices[1].position,
            DVec3::new(11., 20., 32.)
        );
        assert!(t.completed());
        assert_eq!(scene.selection.len(), 1);
        let before = scene.clone();
        let mut cancel = PlaceGroupTool::new(candidate(), false);
        cancel.on_cancel(&mut scene);
        assert!(cancel.completed());
        assert_eq!(scene, before);
    }
    #[test]
    fn source_front_alignment_and_instance() {
        let mut scene = Scene::default();
        let mut t = PlaceGroupTool::new(candidate(), true);
        t.normal = Some(Vec3::Z);
        t.place_at(&mut scene, DVec3::new(10., 20., 30.)).unwrap();
        assert_eq!(
            scene.groups[0].mesh.vertices[0].position,
            DVec3::new(9., 20., 30.)
        );
        assert_eq!(
            scene.groups[0].mesh.vertices[1].position,
            DVec3::new(11., 22., 30.)
        );
        let mut g = candidate();
        g.transform = DMat4::from_translation(DVec3::new(5., 0., 0.));
        let old = g.mesh.clone();
        let mut t = PlaceGroupTool::new(g, false);
        t.place_at(&mut scene, DVec3::new(10., 20., 30.)).unwrap();
        assert_eq!(scene.groups[1].mesh, old);
        assert_eq!(
            scene.groups[1].transform.w_axis.truncate(),
            DVec3::new(9., 20., 30.)
        );
    }
    #[test]
    fn invalid_pose_retains_document_and_candidate() {
        let mut scene = Scene::default();
        let before = scene.clone();
        let mut t = PlaceGroupTool::new(candidate(), false);
        assert!(t.place_at(&mut scene, DVec3::splat(f64::INFINITY)).is_err());
        assert_eq!(scene, before);
        assert!(t.group.is_some());
        let mut bad = candidate();
        bad.transform = DMat4::from_scale(DVec3::splat(1e300));
        let mut t = PlaceGroupTool::new(bad, false);
        assert!(t.place_at(&mut scene, DVec3::ZERO).is_err());
        assert_eq!(scene, before);
    }
    #[test]
    fn pinned_source_controller_rows() {
        let rows: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/place-group/original.json"
        ))
        .unwrap();
        for row in rows.as_array().unwrap().iter().take(4) {
            let mut g = candidate();
            let name = row["name"].as_str().unwrap();
            if name == "instance" {
                g.transform = DMat4::from_translation(DVec3::new(5., 0., 0.));
            }
            let mut t = PlaceGroupTool::new(g, true);
            t.normal = match name {
                "slab" => Some(Vec3::Z),
                "wall" => Some(Vec3::Y),
                _ => None,
            };
            assert_eq!(serde_json::json!(t.anchor.to_array()), row["anchor"]);
            let mut scene = Scene::default();
            t.place_at(&mut scene, DVec3::new(10., 20., 30.)).unwrap();
            assert_eq!(
                serde_json::json!(scene.groups[0]
                    .mesh
                    .vertices
                    .iter()
                    .map(|v| v.position.to_array())
                    .collect::<Vec<_>>()),
                row["vertices"]
            );
            if name == "instance" {
                assert_eq!(
                    serde_json::json!(scene.groups[0].transform.to_cols_array()),
                    row["matrix"]
                );
            }
            assert!(!t.group.is_some());
        }
    }
}
