// SPDX-License-Identifier: GPL-3.0-or-later
//! Borrowed source SavedView visibility/cut state for paper geometry. Applying a
//! view never clones or mutates the document, its placements, or model revisions.
use glam::DVec3;
use ingetrazo_core::{
    groups::Group, mesh::Mesh, saved_views::SavedView, scene::Scene, section::SectionPlane,
};

pub struct PaperSceneView<'a> {
    pub scene: &'a Scene,
    pub saved: Option<&'a SavedView>,
}
impl<'a> PaperSceneView<'a> {
    pub fn new(scene: &'a Scene, saved: Option<&'a SavedView>) -> Self {
        Self { scene, saved }
    }
    pub fn is_layer_visible(&self, name: Option<&str>) -> bool {
        let Some(saved) = self.saved else {
            return self.scene.is_layer_visible(name);
        };
        let layer = if let Some(name) = name {
            self.scene.layers.iter().find(|l| l.name == name)
        } else {
            self.scene
                .layers
                .iter()
                .find(|l| matches!(l.name.as_str(), "Capa 0" | "Layer0" | "Layer 0"))
        };
        layer.is_none_or(|l| !saved.hidden_layers.contains(&l.name))
    }
    pub fn group_visible(&self, group: &Group) -> bool {
        let hidden = self
            .saved
            .and_then(|s| s.hidden_objects.as_ref())
            .map_or(group.hidden, |ids| ids.contains(&group.native_uid()));
        !hidden && self.is_layer_visible(group.layer.as_deref())
    }
    pub fn mesh_face_visible(&self, mesh: &Mesh, id: usize) -> bool {
        let Some(face) = mesh.faces.get(id) else {
            return false;
        };
        face.attrs.get("hidden").and_then(|v| v.as_bool()) != Some(true)
            && self.is_layer_visible(
                face.layer
                    .as_deref()
                    .or_else(|| face.attrs.get("layer").and_then(|v| v.as_str())),
            )
    }
    pub fn mesh_edge_visible(&self, mesh: &Mesh, id: usize) -> bool {
        let Some(edge) = mesh.edges.get(id) else {
            return false;
        };
        !edge.hidden
            && self.is_layer_visible(edge.layer.as_deref())
            && (edge.faces.is_empty()
                || edge.faces.iter().any(|&f| self.mesh_face_visible(mesh, f)))
    }
    pub fn active_section(&self) -> Option<&'a SectionPlane> {
        let Some(section) = self.saved.and_then(|v| v.section.as_ref()) else {
            return self.scene.active_section();
        };
        if section.get("cuts_shown").and_then(|v| v.as_bool()) == Some(false) {
            return None;
        }
        let active = section.get("active")?;
        self.scene
            .section_planes
            .iter()
            .find(|p| p.native_uid() == *active)
    }
    pub fn section_segment(&self, a: DVec3, b: DVec3) -> Option<(DVec3, DVec3)> {
        let Some(plane) = self.active_section() else {
            return Some((a, b));
        };
        let normal = plane.normal.as_vec3().as_dvec3();
        let point = plane.point.as_vec3().as_dvec3();
        let c = normal.x * point.x + normal.y * point.y + normal.z * point.z;
        let distance = |p: DVec3| p.x * normal.x + p.y * normal.y + p.z * normal.z - c;
        let da = distance(a);
        let db = distance(b);
        if da > 1e-9 && db > 1e-9 {
            return None;
        }
        if da <= 1e-9 && db <= 1e-9 {
            return Some((a, b));
        }
        let cut = a.lerp(b, da / (da - db));
        Some(if da > 1e-9 { (cut, b) } else { (a, cut) })
    }
}
impl std::ops::Deref for PaperSceneView<'_> {
    type Target = Scene;
    fn deref(&self) -> &Scene {
        self.scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::{camera::OrbitCamera, layers::Layer};
    #[test]
    fn borrowed_visibility_matches_applied_layers_and_nested_objects_without_mutation() {
        let mut scene = Scene::new();
        scene.layers.push(Layer::new("Hidden"));
        let mut parent = Group::new("parent");
        let child = Group::new("child");
        let child_id = child.native_uid();
        parent.children.push(child);
        parent.hidden = true;
        scene.groups.push(parent);
        let mut saved = SavedView::new("paper");
        saved.hidden_objects = Some(vec![child_id]);
        saved.hidden_layers = vec!["Hidden".into()];
        saved.hidden_shown = Some(serde_json::json!({"objects":true,"geometry":true}));
        let before = scene.clone();
        let view = PaperSceneView::new(&scene, Some(&saved));
        assert!(view.group_visible(&scene.groups[0]));
        assert!(!view.group_visible(&scene.groups[0].children[0]));
        assert!(!view.is_layer_visible(Some("Hidden")));
        assert!(view.is_layer_visible(Some("unknown later layer")));
        let mut applied = scene.clone();
        saved.apply_scene(&mut applied, &mut OrbitCamera::default());
        assert_eq!(
            view.group_visible(&scene.groups[0]),
            !applied.groups[0].hidden
        );
        assert_eq!(
            view.group_visible(&scene.groups[0].children[0]),
            !applied.groups[0].children[0].hidden
        );
        assert_eq!(scene, before);
    }
    #[test]
    fn saved_section_overrides_live_active_cut_and_off_state() {
        let mut scene = Scene::new();
        let mut a = SectionPlane::new(DVec3::ZERO, DVec3::X, "A", "A");
        a.active = true;
        let b = SectionPlane::new(DVec3::new(0., 1., 0.), DVec3::Y, "B", "B");
        let b_id = b.native_uid();
        scene.section_planes = vec![a, b];
        scene.show_section_cuts = false;
        let mut saved = SavedView::new("cut");
        saved.section =
            Some(serde_json::json!({"active":b_id,"cuts_shown":true,"planes_shown":false}));
        let view = PaperSceneView::new(&scene, Some(&saved));
        assert_eq!(
            view.active_section().unwrap().id,
            scene.section_planes[1].id
        );
        assert!(view
            .section_segment(DVec3::new(0., 2., 0.), DVec3::new(1., 2., 0.))
            .is_none());
        let mut applied = scene.clone();
        saved.apply_scene(&mut applied, &mut OrbitCamera::default());
        assert_eq!(
            view.active_section().unwrap().id,
            applied.active_section().unwrap().id
        );
        saved.section = Some(serde_json::json!({"active":null,"cuts_shown":true}));
        assert!(PaperSceneView::new(&scene, Some(&saved))
            .active_section()
            .is_none());
        saved.section = Some(serde_json::json!({"active":b_id,"cuts_shown":false}));
        assert!(PaperSceneView::new(&scene, Some(&saved))
            .active_section()
            .is_none());
    }
}
