// SPDX-License-Identifier: GPL-3.0-or-later
//! Prepared geometric menu mutations commit exactly once after validation.
use crate::app::IngeTrazoApp;
use ingetrazo_core::{
    intersect::{self, IntersectMode},
    planar,
    scene::SelectionItem,
};
impl IngeTrazoApp {
    pub fn split_into_pieces(&mut self) {
        if !self.component_edit_stack.is_empty() {
            self.status_message = "Leave component edit before splitting pieces".into();
            return;
        }
        let selected: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|i| {
                if let SelectionItem::Group(id) = i {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        if selected.len() != 1 {
            self.status_message = "Select one group or component to split into pieces".into();
            return;
        }
        let Some(group) = self.scene.groups.get(selected[0]) else {
            self.status_message = "Invalid selected group".into();
            return;
        };
        if group.billboard {
            self.status_message = "Billboards cannot split into physical pieces".into();
            return;
        }
        let result = ingetrazo_core::pieces::split_into_pieces(group);
        match result {
            Ok(pieces) => {
                if pieces.is_empty() {
                    self.status_message = "Nothing to split: already in physical pieces".into();
                    return;
                }
                let count = pieces.len();
                let mut candidate = self.scene.clone();
                candidate.camera = self.camera.clone();
                if let Some(definition) = group.component_definition {
                    if let Err(error) = candidate.replace_component_definition(
                        definition,
                        ingetrazo_core::components::ComponentDefinition {
                            mesh: ingetrazo_core::mesh::Mesh::new(),
                            children: pieces,
                        },
                    ) {
                        self.status_message = format!("Split into Pieces: {error}");
                        return;
                    }
                } else {
                    let group = &mut candidate.groups[selected[0]];
                    group.mesh = ingetrazo_core::mesh::Mesh::new();
                    group.children = pieces;
                    candidate.bump_version();
                }
                if let Some(doc) = candidate.groups[selected[0]]
                    .native_document
                    .as_object_mut()
                {
                    if doc.contains_key("exploded") {
                        doc.insert("exploded".into(), serde_json::Value::Null);
                    }
                }
                self.save_undo_state();
                self.scene = candidate;
                self.status_message =
                    format!("Split into {count} pieces; Explode Group sets them free");
            }
            Err(error) => self.status_message = format!("Split into Pieces: {error}"),
        }
    }
    pub fn orient_faces(&mut self) {
        match ingetrazo_core::heal::orient_faces(&self.scene.mesh) {
            Ok(mesh) => {
                if !planar::equivalent(&self.scene.mesh, &mesh) {
                    self.save_undo_state();
                    self.scene.mesh = mesh;
                    self.scene.bump_version();
                }
                self.status_message = "Oriented face winding".into();
            }
            Err(error) => self.status_message = format!("Orient Faces: {error}"),
        }
    }

    pub fn heal_overlaps(&mut self) {
        match ingetrazo_core::heal::heal_faces(&self.scene.mesh, 0.5, None) {
            Ok((mesh, removed)) => {
                if !planar::equivalent(&self.scene.mesh, &mesh) {
                    self.save_undo_state();
                    self.scene.mesh = mesh;
                    self.scene.clear_selection();
                    self.scene.bump_version();
                }
                self.status_message = format!(
                    "Healed overlapping geometry: removed {} redundant face(s)",
                    removed.len()
                );
            }
            Err(error) => self.status_message = format!("Heal: {error}"),
        }
    }

    pub fn intersect_faces(&mut self, mode: IntersectMode) {
        let mut prepared = self.scene.clone();
        prepared.camera = self.camera.clone();
        match intersect::intersect_faces(&mut prepared, mode) {
            Ok(count) => {
                if count > 0 && !planar::equivalent(&self.scene.mesh, &prepared.mesh) {
                    self.save_undo_state();
                    self.scene = prepared;
                }
                self.status_message = format!("Intersect Faces: {count} crossing segment(s)");
            }
            Err(error) => self.status_message = format!("Intersect Faces: {error}"),
        }
    }
    pub fn rebuild_planar_faces(&mut self) {
        let selected: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|i| {
                if let SelectionItem::Face(id) = i {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        match planar::rebuild_faces(&self.scene.mesh, &selected) {
            Ok(mesh) => {
                if !planar::equivalent(&self.scene.mesh, &mesh) {
                    self.save_undo_state();
                    self.scene.mesh = mesh;
                    self.scene.clear_selection();
                    self.scene.bump_version();
                }
                self.status_message = "Rebuilt planar face regions".into();
            }
            Err(error) => self.status_message = format!("Rebuild Faces: {error}"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    fn face(app: &mut IngeTrazoApp, points: &[DVec3]) -> usize {
        let ring = points
            .iter()
            .map(|&p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ring, vec![]).unwrap()
    }
    fn cube(mesh: &mut ingetrazo_core::mesh::Mesh, x: f64) {
        let mut vertices = vec![];
        for z in [0., 1.] {
            for y in [0., 1.] {
                for dx in [0., 1.] {
                    vertices.push(mesh.add_vertex(DVec3::new(x + dx, y, z)));
                }
            }
        }
        for q in [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ] {
            mesh.add_face(q.iter().map(|&i| vertices[i]).collect(), vec![]);
        }
    }
    #[test]
    fn placed_component_pieces_update_shared_definition_and_undo_once() {
        use ingetrazo_core::{groups::Group, mesh::Mesh};
        let mut app = IngeTrazoApp::new();
        app.scene.mesh.clear();
        let mut mesh = Mesh::new();
        cube(&mut mesh, 0.);
        cube(&mut mesh, 5.);
        mesh.faces[0]
            .attrs
            .insert("color".into(), serde_json::json!([0.8, 0.1, 0.1]));
        let mut group = Group::with_mesh("stool", mesh);
        group.transform = glam::DMat4::from_translation(DVec3::new(3., 4., 0.))
            * glam::DMat4::from_rotation_z(30_f64.to_radians());
        group.layer = Some("furniture".into());
        let original = group.clone();
        let id = group.id;
        app.scene.groups.push(group);
        app.scene.make_component(id).unwrap();
        let other = app.scene.groups[0].duplicate_instance();
        app.scene.groups.push(other);
        let before = app.scene.groups.clone();
        let definitions = app.scene.component_definitions.clone();
        app.scene.selection.insert(SelectionItem::Group(0));
        app.split_into_pieces();
        assert_eq!(app.undo_stack.len(), 1);
        for group in &app.scene.groups {
            assert_eq!(group.children.len(), 2);
            assert!(group.mesh.faces.is_empty());
            assert_eq!(group.transform, original.transform);
            assert_eq!(group.name, "stool");
            assert_eq!(group.layer, original.layer);
            assert_eq!(
                group
                    .children
                    .iter()
                    .flat_map(|g| &g.mesh.faces)
                    .filter(|f| f.attrs.contains_key("color"))
                    .count(),
                1
            );
        }
        let after = app.scene.groups.clone();
        app.undo();
        assert_eq!(app.scene.groups, before);
        assert_eq!(app.scene.component_definitions, definitions);
        app.redo();
        assert_eq!(app.scene.groups, after);
        app.split_into_pieces();
        assert_eq!(app.undo_stack.len(), 1);
    }

    #[test]
    fn heal_menu_holes_overlap_and_undo_restores_both_faces() {
        let mut app = IngeTrazoApp::new();
        app.scene.mesh.clear();
        face(
            &mut app,
            &[
                DVec3::ZERO,
                DVec3::new(10., 0., 0.),
                DVec3::new(10., 10., 0.),
                DVec3::new(0., 10., 0.),
            ],
        );
        face(
            &mut app,
            &[
                DVec3::new(2., 2., 0.),
                DVec3::new(4., 2., 0.),
                DVec3::new(4., 4., 0.),
                DVec3::new(2., 4., 0.),
            ],
        );
        let before = app.scene.mesh.clone();
        app.heal_overlaps();
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.scene.mesh.faces.len(), 2);
        assert!(app.scene.mesh.faces.iter().any(|f| f.holes.len() == 1));
        let after = app.scene.mesh.clone();
        app.undo();
        assert_eq!(app.scene.mesh, before);
        app.redo();
        assert_eq!(app.scene.mesh, after);
        app.heal_overlaps();
        assert_eq!(app.undo_stack.len(), 1);
    }

    #[test]
    fn menu_intersect_rebuild_and_failed_plane_have_single_atomic_history() {
        let mut app = IngeTrazoApp::new();
        app.scene.mesh.clear();
        face(
            &mut app,
            &[
                DVec3::ZERO,
                DVec3::new(2., 0., 0.),
                DVec3::new(2., 2., 0.),
                DVec3::new(0., 2., 0.),
            ],
        );
        face(
            &mut app,
            &[
                DVec3::new(1., 1., 0.),
                DVec3::new(3., 1., 0.),
                DVec3::new(3., 3., 0.),
                DVec3::new(1., 3., 0.),
            ],
        );
        let before = app.scene.mesh.clone();
        app.rebuild_planar_faces();
        assert_eq!(app.scene.mesh.faces.len(), 3);
        assert_eq!(app.undo_stack.len(), 1);
        app.undo();
        assert_eq!(app.scene.mesh, before);
        app.redo();
        assert_eq!(app.scene.mesh.faces.len(), 3);
        app.scene.mesh.clear();
        app.undo_stack.clear();
        app.redo_stack.clear();
        face(
            &mut app,
            &[
                DVec3::ZERO,
                DVec3::new(2., 0., 0.),
                DVec3::new(2., 2., 0.),
                DVec3::new(0., 2., 0.),
            ],
        );
        face(
            &mut app,
            &[
                DVec3::new(1., -1., -1.),
                DVec3::new(1., 3., -1.),
                DVec3::new(1., 3., 1.),
                DVec3::new(1., -1., 1.),
            ],
        );
        app.scene.selection.insert(SelectionItem::Face(0));
        app.scene.selection.insert(SelectionItem::Face(1));
        let before = app.scene.mesh.clone();
        app.rebuild_planar_faces();
        assert_eq!(app.scene.mesh, before);
        assert!(app.undo_stack.is_empty());
        assert!(app.status_message.contains("one plane"));
        app.intersect_faces(IntersectMode::WithSelection);
        assert!(app.scene.mesh.faces.len() > 2);
        assert_eq!(app.undo_stack.len(), 1);
        app.undo();
        assert_eq!(app.scene.mesh, before);
    }
    #[test]
    fn orient_partition_marking_flip_and_second_pass_noop_have_one_undo() {
        let mut app=IngeTrazoApp::new();app.scene.mesh.clear();
        for x in [0.,1.] {
            let p=[DVec3::new(x,0.,0.),DVec3::new(x+1.,0.,0.),DVec3::new(x+1.,1.,0.),DVec3::new(x,1.,0.),DVec3::new(x,0.,1.),DVec3::new(x+1.,0.,1.),DVec3::new(x+1.,1.,1.),DVec3::new(x,1.,1.)];
            for (k,f) in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]].into_iter().enumerate(){if x==1.&&k==5{continue;}face(&mut app,&f.map(|i|p[i]));}
        }
        app.scene.mesh.faces[0].vertices.reverse();let before=app.scene.mesh.clone();
        app.orient_faces();assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.mesh.faces.iter().filter(|f|f.interior).count(),1);let after=app.scene.mesh.clone();
        app.orient_faces();assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.mesh,after);
        app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,after);
    }

}
