//! Runtime vertex holds matching original dimension binding and fallback.
use crate::{dimensions::Dimension, groups::Group, mesh::Mesh, scene::Scene};
use glam::{DMat4, DVec3};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq)]
pub struct VertexAnchor {
    pub vertex: Uuid,
    pub path: Vec<Uuid>,
}
// Source admission looks up the local registry first, then checks the
// QVector3D tolerance. Native double payloads are never rewritten by lookup.
fn exact_qt_matrix(m: DMat4) -> bool {
    m.to_cols_array().iter().all(|x| (*x as f32) as f64 == *x)
}
fn map(m: DMat4, p: DVec3) -> DVec3 {
    if exact_qt_matrix(m) && p.as_vec3().as_dvec3() == p {
        crate::change_axes::map(m, p)
    } else {
        m.transform_point3(p)
    }
}
fn local_inverse(m: DMat4) -> Option<DMat4> {
    // QMatrix4x4 preserves its Scale|Translation flags through composition.
    // Its inverse reciprocals and translated products are float operations;
    // a generic double inverse narrowed afterwards differs at far placements.
    let a = m.to_cols_array_2d();
    if (0..3).all(|c| (0..3).all(|r| c == r || a[c][r] == 0.))
        && a[0][3] == 0.
        && a[1][3] == 0.
        && a[2][3] == 0.
        && a[3][3] == 1.
    {
        let mut v = DMat4::IDENTITY.to_cols_array_2d();
        for i in 0..3 {
            let scale = a[i][i] as f32;
            if scale == 0. {
                return None;
            }
            let inv = 1f32 / scale;
            v[i][i] = f64::from(inv);
            v[3][i] = f64::from(-(a[3][i] as f32) * inv);
        }
        let inv = DMat4::from_cols_array_2d(&v);
        return inv.is_finite().then_some(inv);
    }
    crate::change_axes::inverse(m).ok()
}
fn at(mesh: &Mesh, m: DMat4, p: DVec3, path: &[Uuid]) -> Option<VertexAnchor> {
    if !p.is_finite() || !m.is_finite() {
        return None;
    }
    let local = if m == DMat4::IDENTITY {
        p
    } else if exact_qt_matrix(m) && p.as_vec3().as_dvec3() == p {
        map(local_inverse(m)?, p)
    } else {
        let inv = m.inverse();
        if !inv.is_finite() {
            return None;
        }
        inv.transform_point3(p)
    };
    let id = mesh.vertex_at_position(local)?;
    let v = mesh.vertices.get(id)?;
    let distance =
        if local.as_vec3().as_dvec3() == local && v.position.as_vec3().as_dvec3() == v.position {
            f64::from((v.position.as_vec3() - local.as_vec3()).length())
        } else {
            (v.position - local).length()
        };
    if v.edges.is_empty() || !distance.is_finite() || distance > 1e-4 {
        return None;
    }
    Some(VertexAnchor {
        vertex: v.uid,
        path: path.to_vec(),
    })
}
fn find(groups: &[Group], parent: DMat4, p: DVec3, path: &mut Vec<Uuid>) -> Option<VertexAnchor> {
    for group in groups {
        // Original skips the entire billboard root; nested billboard members
        // are visited by iter_placements of an ordinary root.
        if path.is_empty() && group.billboard {
            continue;
        }
        let m = if exact_qt_matrix(parent) && exact_qt_matrix(group.transform) {
            crate::change_axes::multiply(parent, group.transform)
        } else {
            parent * group.transform
        };
        path.push(group.id);
        let anchor = at(&group.mesh, m, p, path).or_else(|| find(&group.children, m, p, path));
        path.pop();
        if anchor.is_some() {
            return anchor;
        }
    }
    None
}
impl VertexAnchor {
    pub fn position(&self, scene: &Scene) -> Option<DVec3> {
        let mut mesh = &scene.mesh;
        let mut groups = &scene.groups;
        let mut matrix = DMat4::IDENTITY;
        for id in &self.path {
            let group = groups.iter().find(|g| g.id == *id)?;
            matrix = if exact_qt_matrix(matrix) && exact_qt_matrix(group.transform) {
                crate::change_axes::multiply(matrix, group.transform)
            } else {
                matrix * group.transform
            };
            mesh = &group.mesh;
            groups = &group.children;
        }
        let vertex = mesh
            .vertices
            .iter()
            .find(|v| v.uid == self.vertex && !v.edges.is_empty())?;
        Some(map(matrix, vertex.position))
    }
}
impl Scene {
    pub fn resolve_dimension_anchor(&self, point: DVec3) -> Option<VertexAnchor> {
        at(&self.mesh, DMat4::IDENTITY, point, &[])
            .or_else(|| find(&self.groups, DMat4::IDENTITY, point, &mut vec![]))
    }
    pub fn bind_dimensions(&mut self) {
        let mut dimensions = std::mem::take(&mut self.dimensions);
        for dim in &mut dimensions {
            dim.measurement_frame = self.dimension_frame;
            dim.anchor_a = self.resolve_dimension_anchor(dim.start);
            dim.anchor_b = self.resolve_dimension_anchor(dim.end);
        }
        self.dimensions = dimensions;
    }
    pub fn dimension_points(&self, dim: &Dimension) -> (DVec3, DVec3) {
        (
            dim.anchor_a
                .as_ref()
                .and_then(|a| a.position(self))
                .unwrap_or(dim.start),
            dim.anchor_b
                .as_ref()
                .and_then(|a| a.position(self))
                .unwrap_or(dim.end),
        )
    }
    pub fn refresh_dimensions(&mut self) {
        let mut dimensions = std::mem::take(&mut self.dimensions);
        for dim in &mut dimensions {
            dim.measurement_frame = self.dimension_frame;
            for (point, anchor) in [
                (&mut dim.start, &mut dim.anchor_a),
                (&mut dim.end, &mut dim.anchor_b),
            ] {
                if let Some(held) = anchor.as_ref() {
                    if let Some(live) = held.position(self) {
                        *point = live;
                    } else {
                        *anchor = self.resolve_dimension_anchor(*point);
                        if let Some(live) = anchor.as_ref().and_then(|a| a.position(self)) {
                            *point = live;
                        }
                    }
                }
            }
        }
        self.dimensions = dimensions;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::SelectionItem;
    #[test]
    fn anchors_follow_uid_through_movement_grouping_transform_explode_and_deletion() {
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::ZERO);
        let b = scene.mesh.add_vertex(DVec3::X);
        scene.mesh.add_edge(a, b);
        scene
            .dimensions
            .push(Dimension::new(DVec3::ZERO, DVec3::X, DVec3::Y));
        scene.bind_dimensions();
        assert!(scene.dimensions[0].anchor_a.is_some());
        scene.mesh.vertices[b].position.x = 3.;
        scene.refresh_dimensions();
        assert_eq!(scene.dimensions[0].end.x, 3.);
        scene.select(SelectionItem::Edge(0));
        scene.group_selection("part").unwrap();
        scene.refresh_dimensions();
        assert_eq!(
            scene.dimensions[0].anchor_b.as_ref().unwrap().path,
            vec![scene.groups[0].id]
        );
        scene.groups[0].transform = DMat4::from_scale(DVec3::splat(2.));
        scene.refresh_dimensions();
        assert_eq!(scene.dimensions[0].end.x, 6.);
        scene.explode_selected_groups().unwrap();
        scene.refresh_dimensions();
        assert!(scene.dimensions[0]
            .anchor_b
            .as_ref()
            .unwrap()
            .path
            .is_empty());
        scene.mesh = Mesh::new();
        scene.refresh_dimensions();
        assert!(scene.dimensions[0].anchor_b.is_none());
        assert_eq!(scene.dimensions[0].end.x, 6.);
        scene.mesh.add_vertex(DVec3::new(100., 0., 0.));
        scene.refresh_dimensions();
        assert_eq!(scene.dimensions[0].end.x, 6.);
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    #[test]
    fn local_registry_tolerance_and_native_precision_preservation() {
        let mut s = Scene::new();
        let a = s.mesh.add_vertex(DVec3::ZERO);
        let b = s.mesh.add_vertex(DVec3::X);
        s.mesh.add_edge(a, b);
        let near = DVec3::new(f64::from(2e-5f32), 0., 0.);
        assert!(s.resolve_dimension_anchor(near).is_some());
        assert!(s
            .resolve_dimension_anchor(DVec3::new(2e-4, 0., 0.))
            .is_none());
        let precise = 1. + 2f64.powi(-40);
        let mut mesh = Mesh::new();
        let a = mesh.add_vertex(DVec3::new(precise, 0., 0.));
        let b = mesh.add_vertex(DVec3::new(2., 0., 0.));
        mesh.add_edge(a, b);
        s.mesh = Mesh::new();
        let mut g = Group::with_mesh("native", mesh);
        g.transform = DMat4::from_translation(DVec3::new(2., 0., 0.));
        s.groups.push(g);
        let p = DVec3::new(precise + 2., 0., 0.);
        let anchor = s.resolve_dimension_anchor(p).unwrap();
        assert_eq!(anchor.position(&s), Some(p));
        assert_eq!(s.groups[0].mesh.vertices[0].position.x, precise);
        s.groups[0].billboard = true;
        assert!(s.resolve_dimension_anchor(p).is_none());
        s.groups[0].billboard = false;
        s.groups[0].transform = DMat4::from_scale(DVec3::new(0., 1., 1.));
        assert!(s.resolve_dimension_anchor(p).is_none());
    }
}
