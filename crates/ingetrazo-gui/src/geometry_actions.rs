// SPDX-License-Identifier: GPL-3.0-or-later
//! Prepared geometric menu mutations commit exactly once after validation.
use crate::app::IngeTrazoApp;
use ingetrazo_core::{
    intersect::{self, IntersectMode},
    planar,
    scene::SelectionItem,
};
impl IngeTrazoApp {
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
}
