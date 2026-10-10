//! Runtime terrain visibility is presentation state; a model command may own
//! replacing/clearing the terrain (e.g. original model-to-north reanchoring).
use crate::IngeTrazoApp;
use ingetrazo_core::scene::Scene;
pub(crate) fn inverse_snapshot(current:&mut Scene)->Scene{let terrain=current.terrain.take();let inverse=current.clone();current.terrain=terrain;inverse}
pub(crate) fn restore(current: &mut Scene, target: &mut Scene, inverse: &mut Scene) {
    use ingetrazo_core::georef::{HeldTerrain, TerrainOwnership};
    match target
        .history_terrain
        .0
        .take()
        .unwrap_or(TerrainOwnership::Unowned)
    {
        TerrainOwnership::Unowned => {
            std::mem::swap(&mut target.terrain, &mut current.terrain);
            inverse.terrain=current.terrain.take();
            inverse.history_terrain.0 = Some(TerrainOwnership::Unowned);
        }
        TerrainOwnership::Owned { after } => {
            inverse.terrain = after.map(|t| t.instantiate());
            inverse.history_terrain.0 = Some(TerrainOwnership::Owned {
                after: target.terrain.as_ref().map(HeldTerrain::capture),
            });
        }
    }
}
impl IngeTrazoApp {
    pub(crate) fn seal_terrain_history(&mut self) {
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_terrain.0.is_none() {
                before.history_terrain.0 = Some(ingetrazo_core::georef::TerrainHistory::seal(
                    &before.terrain,
                    &self.scene.terrain,
                ));
            }
        }
    }
    pub(crate) fn seal_terrain_history_before(&mut self, current: &Scene) {
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_terrain.0.is_none() {
                before.history_terrain.0 = Some(ingetrazo_core::georef::TerrainHistory::seal(
                    &before.terrain,
                    &current.terrain,
                ));
            }
        }
    }
    pub fn publish_runtime_terrain(
        &mut self,
        terrain: Option<ingetrazo_core::georef::TerrainObject>,
    ) {
        self.seal_view_history();
        self.seal_terrain_history();
        self.scene.terrain = terrain;
        self.scene.bump_view_version();
    }
    pub fn set_terrain_visibility(&mut self, visible: bool) -> bool {
        let Some(terrain) = self.scene.terrain.as_ref() else {
            return false;
        };
        if terrain.visible == visible {
            return false;
        }
        let id = terrain.history_id;
        self.seal_view_history();
        self.seal_terrain_history();
        fn update(s: &mut Scene, id: uuid::Uuid, visible: bool) {
            s.history_terrain.refresh_visibility(id, visible);
            if let Some(t) = &mut s.terrain {
                if t.history_id == id {
                    t.visible = visible;
                }
            }
        }
        update(&mut self.scene, id, visible);
        for s in self.undo_stack.iter_mut().chain(&mut self.redo_stack) {
            update(s, id, visible);
        }
        for session in &mut self.component_edit_stack {
            update(&mut session.original_scene, id, visible);
            update(&mut session.cancel_scene, id, visible);
            for s in session
                .original_undo
                .iter_mut()
                .chain(&mut session.original_redo)
            {
                update(s, id, visible);
            }
        }
        self.scene.bump_view_version();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::georef::TerrainObject;
    fn terrain() -> TerrainObject {
        TerrainObject {
            history_id: uuid::Uuid::new_v4(),
            vertices: vec![DVec3::ZERO, DVec3::X, DVec3::Y],
            uvs: vec![(0., 0.), (1., 0.), (0., 1.)],
            triangles: vec![(0, 1, 2)],
            tile_range: (0, 0, 1, 1, 0),
            nx: 0,
            ny: 0,
            bbox: (0., 0., 0., 0.),
            visible: true,
        }
    }
    #[test]
    fn visibility_is_nonhistory_and_noop_has_no_revision_or_copy() {
        let mut a = IngeTrazoApp::new();
        a.scene = Scene::new();
        assert!(!a.set_terrain_visibility(false));
        a.publish_runtime_terrain(Some(terrain()));
        a.save_undo_state();
        a.scene.mesh.add_vertex(DVec3::X);
        a.scene.bump_version();
        let id = a.scene.terrain.as_ref().unwrap().history_id;
        let pointer = a.scene.terrain.as_ref().unwrap().vertices.as_ptr();
        let depth = a.undo_stack.len();
        assert!(a.set_terrain_visibility(false));
        assert_eq!(a.undo_stack.len(), depth);
        assert_eq!(a.scene.terrain.as_ref().unwrap().vertices.as_ptr(), pointer);
        let version = a.scene.version;
        let revision = a.model_revision;
        assert!(!a.set_terrain_visibility(false));
        assert_eq!(a.scene.version, version);
        assert_eq!(a.model_revision, revision);
        a.undo();
        assert!(a.scene.mesh.vertices.is_empty());
        assert_eq!(a.scene.terrain.as_ref().unwrap().history_id, id);
        assert!(!a.scene.terrain.as_ref().unwrap().visible);
        assert!(a.set_terrain_visibility(true));
        assert_eq!(a.redo_stack.len(), 1);
        a.redo();
        assert!(a.scene.terrain.as_ref().unwrap().visible);
        let json = serde_json::to_value(&a.scene).unwrap();
        assert!(json.get("history_terrain").is_none());
        assert!(json["terrain"].get("history_id").is_none());
    }
    #[test]
    fn same_geometry_rebuild_is_different_runtime_owner_and_survives_unrelated_undo() {
        let mut a = IngeTrazoApp::new();
        a.scene = Scene::new();
        a.publish_runtime_terrain(Some(terrain()));
        let old = a.scene.terrain.as_ref().unwrap().history_id;
        a.save_undo_state();
        a.scene.mesh.add_vertex(DVec3::X);
        a.scene.bump_version();
        a.publish_runtime_terrain(Some(terrain()));
        let fresh = a.scene.terrain.as_ref().unwrap().history_id;
        assert_ne!(old, fresh);
        a.set_terrain_visibility(false);
        a.undo();
        assert_eq!(a.scene.terrain.as_ref().unwrap().history_id, fresh);
        assert!(!a.scene.terrain.as_ref().unwrap().visible);
        a.redo();
        assert_eq!(a.scene.terrain.as_ref().unwrap().history_id, fresh);
        assert!(!a.scene.terrain.as_ref().unwrap().visible);
    }
}
