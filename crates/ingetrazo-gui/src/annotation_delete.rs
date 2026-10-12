//! Atomic mixed annotation deletion. History uses the existing command snapshot;
//! entity IDs and list order survive its Undo/Redo. No native state is published
//! until every selected reference/layer and candidate topology was admitted.
use crate::IngeTrazoApp;
use ingetrazo_core::scene::SelectionItem;
impl IngeTrazoApp {
    /// Source Select Delete gives real selection priority over transient hover.
    pub fn delete_selection_or_annotation_hover(&mut self) {
        if !self.scene.selection.is_empty() {
            self.delete_selection();
            return;
        }
        let target = self.active_tool.annotation_hover_target(&self.scene);
        self.active_tool.clear_annotation_hover();
        let item = match target {
            Some(ingetrazo_tools::base::AnnotationEditTarget::Label(id)) => self
                .scene
                .text_labels
                .iter()
                .position(|t| t.history_id == id)
                .map(SelectionItem::TextLabel),
            Some(ingetrazo_tools::base::AnnotationEditTarget::Dimension(id)) => self
                .scene
                .dimensions
                .iter()
                .position(|d| d.id == id)
                .map(SelectionItem::Dimension),
            None => None,
        };
        if let Some(item) = item {
            if let Err(error) = self.delete_annotation_targets(&[item]) {
                self.status_message = error;
            }
        }
    }
    pub(crate) fn delete_annotation_selection(&mut self) -> Result<(), String> {
        let targets: Vec<_> = self.scene.selection.iter().cloned().collect();
        self.delete_annotation_targets(&targets)
    }

    pub(crate) fn delete_annotation_targets(
        &mut self,
        targets: &[SelectionItem],
    ) -> Result<(), String> {
        if targets.is_empty() {
            return Ok(());
        }
        let scene = &self.scene;
        let mut guides = Vec::new();
        let mut dimensions = Vec::new();
        let mut labels = Vec::new();
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        let mut groups = Vec::new();
        let mut sections = Vec::new();
        for target in targets {
            let selectable = match *target {
                SelectionItem::Guide(i) => {
                    scene.guides.get(i).ok_or("Guide no longer resolves")?;
                    guides.push(i);
                    scene.is_layer_selectable(None)
                }
                SelectionItem::Dimension(i) => {
                    let d = scene
                        .dimensions
                        .get(i)
                        .ok_or("Dimension no longer resolves")?;
                    dimensions.push(i);
                    scene.is_layer_selectable(d.layer.as_deref())
                }
                SelectionItem::TextLabel(i) => {
                    let t = scene
                        .text_labels
                        .get(i)
                        .ok_or("Text label no longer resolves")?;
                    labels.push(i);
                    scene.is_layer_selectable(t.layer.as_deref())
                }
                SelectionItem::Face(i) => {
                    scene.mesh.faces.get(i).ok_or("Face no longer resolves")?;
                    faces.push(i);
                    scene.mesh_face_selectable(&scene.mesh, i)
                }
                SelectionItem::Edge(i) => {
                    scene.mesh.edges.get(i).ok_or("Edge no longer resolves")?;
                    edges.push(i);
                    scene.mesh_edge_selectable(&scene.mesh, i)
                }
                SelectionItem::Group(i) => {
                    let g = scene.groups.get(i).ok_or("Group no longer resolves")?;
                    groups.push(i);
                    scene.group_selectable(g)
                }
                SelectionItem::SectionPlane(i) => {
                    scene
                        .section_planes
                        .get(i)
                        .ok_or("Section no longer resolves")?;
                    sections.push(i);
                    true
                }
                _ => return Err("Selected entity deletion is not yet supported".into()),
            };
            if !selectable {
                return Err("Cannot delete a hidden or locked target".into());
            }
        }
        let geometry_changed = !faces.is_empty() || !edges.is_empty();
        for &edge in &edges {
            let radial = &scene.mesh.edges[edge].faces;
            if radial.len() > 1 {
                return Err(
                    "Shared-edge healing must be completed before mixed annotation deletion".into(),
                );
            }
            for &face in radial {
                scene
                    .mesh
                    .faces
                    .get(face)
                    .ok_or("Deleted edge has an invalid radial face")?;
                if !scene.mesh_face_selectable(&scene.mesh, face) {
                    return Err("Cannot delete a hidden or locked incident face".into());
                }
                faces.push(face);
            }
        }
        faces.sort_unstable();
        faces.dedup();
        let mut candidate = scene.clone();
        fn remove<T>(items: &mut Vec<T>, mut indices: Vec<usize>) {
            indices.sort_unstable_by(|a, b| b.cmp(a));
            for index in indices {
                items.remove(index);
            }
        }
        remove(&mut candidate.guides, guides);
        remove(&mut candidate.dimensions, dimensions);
        remove(&mut candidate.text_labels, labels);
        remove(&mut candidate.mesh.faces, faces);
        remove(&mut candidate.mesh.edges, edges);
        remove(&mut candidate.groups, groups);
        remove(&mut candidate.section_planes, sections);
        candidate.clear_selection();
        if geometry_changed {
            for edge in &candidate.mesh.edges {
                if edge.v0 >= candidate.mesh.vertices.len()
                    || edge.v1 >= candidate.mesh.vertices.len()
                {
                    return Err("Invalid surviving edge endpoint".into());
                }
            }
            for face in &candidate.mesh.faces {
                for &vertex in face.vertices.iter().chain(face.holes.iter().flatten()) {
                    if vertex >= candidate.mesh.vertices.len() {
                        return Err("Invalid surviving face vertex".into());
                    }
                }
            }
            candidate.mesh.rebuild_incidence();
            candidate
                .mesh
                .edges
                .retain(|e| !e.soft || !e.faces.is_empty());
            let mut used = vec![false; candidate.mesh.vertices.len()];
            for edge in &candidate.mesh.edges {
                used[edge.v0] = true;
                used[edge.v1] = true;
            }
            for face in &candidate.mesh.faces {
                for &v in face.vertices.iter().chain(face.holes.iter().flatten()) {
                    used[v] = true;
                }
            }
            let mut ids = vec![0usize; used.len()];
            let mut next = 0;
            for (i, &keep) in used.iter().enumerate() {
                if keep {
                    ids[i] = next;
                    next += 1;
                }
            }
            let mut index = 0;
            candidate.mesh.vertices.retain(|_| {
                let keep = used[index];
                index += 1;
                keep
            });
            for edge in &mut candidate.mesh.edges {
                edge.v0 = ids[edge.v0];
                edge.v1 = ids[edge.v1];
            }
            for face in &mut candidate.mesh.faces {
                for v in face
                    .vertices
                    .iter_mut()
                    .chain(face.holes.iter_mut().flatten())
                {
                    *v = ids[*v];
                }
            }
            candidate.mesh.rebuild_spatial_index();
            candidate.mesh.rebuild_incidence();
        }
        candidate.bump_version();
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Delete history allocation refused")?;
        self.save_undo_state();
        // Source Delete commands discard targets and do not reselect on Undo.
        if let Some(before) = self.undo_stack.last_mut() {
            before.clear_selection();
            before.history_annotation_delete = true;
        }
        self.scene = candidate;
        self.seal_snapshot_visibility();
        self.bump_model_revision();
        self.status_message = "Deleted selection".into();
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    #[test]
    fn refusal_is_atomic_for_locked_or_stale_annotation() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene
            .guides
            .push(ingetrazo_core::guides::Guide::point(DVec3::ZERO));
        a.scene.select(SelectionItem::Guide(0));
        a.scene.select(SelectionItem::Dimension(99));
        let before = a.scene.clone();
        let version = a.model_revision;
        a.delete_selection();
        assert_eq!(a.scene, before);
        assert_eq!(a.model_revision, version);
        assert!(a.undo_stack.is_empty());
        a.scene.deselect(&SelectionItem::Dimension(99));
        a.scene.layers[0].locked = true;
        let before = a.scene.clone();
        a.delete_selection();
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
    }
    #[test]
    fn duplicate_labels_have_independent_runtime_identity_and_serde_is_semantic() {
        let a = ingetrazo_core::textlabel::TextLabel::new(DVec3::ZERO, DVec3::X, "same");
        let b = ingetrazo_core::textlabel::TextLabel::new(DVec3::ZERO, DVec3::X, "same");
        assert_eq!(a, b);
        assert_ne!(a.history_id, b.history_id);
        assert_eq!(a.clone().history_id, a.history_id);
        let c: ingetrazo_core::textlabel::TextLabel =
            serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
        assert_eq!(a, c);
        assert_ne!(a.history_id, c.history_id);
    }
    #[test]
    fn mixed_delete_shareback_preserves_native_f64_under_exact_frames() {
        use ingetrazo_core::{groups::Group, guides::Guide, mesh::Mesh};
        for frame in [
            glam::DMat4::IDENTITY,
            glam::DMat4::from_translation(DVec3::new(2., 0., 0.)),
        ] {
            let mut a = IngeTrazoApp::new().without_startup_figure_for_test();
            a.system_clipboard = None;
            let mut mesh = Mesh::new();
            let p = DVec3::new(1. + 2f64.powi(-40), 0., 0.);
            let vertices =
                [p, DVec3::new(2., 0., 0.), DVec3::new(1., 1., 0.)].map(|p| mesh.add_vertex(p));
            mesh.add_face(vertices.to_vec(), vec![]).unwrap();
            let mut group = Group::with_mesh("native", mesh);
            group.transform = frame;
            a.scene.groups.push(group);
            a.scene.guides.push(Guide::point(DVec3::ZERO));
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Face(0));
            a.scene.select(SelectionItem::Guide(0));
            a.delete_selection();
            a.finish_component_edit();
            assert!(a.scene.groups[0].mesh.faces.is_empty());
            assert_eq!(a.scene.groups[0].mesh.vertices[0].position, p);
        }
    }
    #[test]
    fn parked_global_undo_survives_nested_cancel() {
        use ingetrazo_core::{groups::Group, guides::Guide, mesh::Mesh};
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene.guides.push(Guide::point(DVec3::ZERO));
        let mut mesh = Mesh::new();
        let vertices = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(vertices.to_vec(), vec![]).unwrap();
        let mut parent = Group::with_mesh("parent", Mesh::new());
        parent.children.push(Group::with_mesh("child", mesh));
        a.scene.groups.push(parent);
        let id = a.scene.guides[0].id;
        a.scene.select(SelectionItem::Guide(0));
        a.delete_selection();
        assert!(a.scene.guides.is_empty());
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        a.undo();
        assert_eq!(a.scene.guides[0].id, id);
        a.cancel_component_edit();
        assert_eq!(a.scene.guides[0].id, id);
        a.cancel_component_edit();
        assert_eq!(a.scene.guides[0].id, id);
        a.redo();
        assert!(a.scene.guides.is_empty());
    }
}
