//! Cold, non-mutating full-document projection while geometry is being edited.
use crate::app::IngeTrazoApp;
use ingetrazo_core::scene::Scene;

#[derive(Default)]
pub(super) struct ProjectionCache {
    key: Option<(uuid::Uuid, u64, u64, u64, usize)>,
    pub(super) scene: Option<std::sync::Arc<Scene>>,
    builds: u64,
}

impl IngeTrazoApp {
    /// Outside an edit, callers borrow the real Scene directly. Inside an edit,
    /// project from deepest to outermost without Finish, tool cancellation or
    /// history publication. Camera changes never invalidate this cached geometry.
    pub(super) fn prepare_blender_projection(&mut self) -> Result<(), String> {
        let Some(root) = self.component_edit_stack.first() else {
            self.blender_state.projection.key = None;
            self.blender_state.projection.scene = None;
            return Ok(());
        };
        let key = (
            self.clipboard_document,
            self.native_geometry_revision,
            self.scene.view_version,
            root.original_scene.view_version,
            self.component_edit_stack.len(),
        );
        if self.blender_state.projection.key == Some(key) {
            return Ok(());
        }
        let mut candidate = None;
        for session in self.component_edit_stack.iter().rev() {
            let active = candidate.as_ref().unwrap_or(&self.scene);
            candidate = Some(crate::component_edit::component_candidate(session, active)?.0);
        }
        let mut candidate = candidate.expect("Nonempty component edit stack");
        // Plugin commands target the parked document, independently of the
        // working geometry Scene. Never overwrite their authoritative payload.
        candidate.plugin_data = root.original_scene.plugin_data.clone();
        self.blender_state.projection.scene = Some(std::sync::Arc::new(candidate));
        self.blender_state.projection.key = Some(key);
        self.blender_state.projection.builds += 1;
        Ok(())
    }

    pub fn start_blender_render(&mut self) {
        if self.blender_state.pending_document_controls && !self.commit_blender_document_controls()
        {
            return;
        }
        if let Err(error) = self.prepare_blender_projection() {
            self.blender_state.status_line = format!("Error: {error}");
            return;
        }
        self.with_blender_projection(|state, scene, camera, revision| {
            state.start_render_cached(scene, camera, Some(revision));
        });
    }

    pub(super) fn with_blender_projection(
        &mut self,
        call: impl FnOnce(
            &mut super::BlenderRenderState,
            &Scene,
            &ingetrazo_core::camera::OrbitCamera,
            u64,
        ),
    ) {
        // A shared immutable owner permits borrowing the complete render state
        // without moving or cloning the cached Scene, including on unwinding.
        let projected = self.blender_state.projection.scene.clone();
        call(
            &mut self.blender_state,
            projected.as_deref().unwrap_or(&self.scene),
            &self.camera,
            self.native_geometry_revision,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};

    #[test]
    fn parked_document_and_live_edit_are_projected_once_without_finish() {
        let mut app = IngeTrazoApp::new();
        app.scene = Scene::new();
        app.scene.mesh.add_vertex(DVec3::new(30., 0., 0.));
        let mut mesh = Mesh::new();
        let ring = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(ring.to_vec(), vec![]).unwrap();
        app.scene
            .groups
            .push(Group::with_mesh("edited", mesh.clone()));
        app.scene.groups.push(Group::with_mesh("sibling", mesh));
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        app.scene.mesh.vertices[0].position.z = 7.;
        app.bump_model_revision();
        let history = (app.undo_stack.len(), app.redo_stack.len());
        let depth = app.component_edit_stack.len();
        let live = app.scene.clone();
        app.prepare_blender_projection().unwrap();
        let cached = app.blender_state.projection.scene.as_ref().unwrap();
        assert_eq!(cached.mesh.vertices[0].position.x, 30.);
        assert_eq!(cached.groups.len(), 2);
        assert_eq!(cached.groups[0].mesh.vertices[0].position.z, 7.);
        assert_eq!(cached.groups[1].mesh.vertices[0].position.z, 0.);
        let ptr = cached.groups.as_ptr();
        app.camera.distance += 1.;
        app.prepare_blender_projection().unwrap();
        assert_eq!(app.blender_state.projection.builds, 1);
        assert_eq!(
            app.blender_state
                .projection
                .scene
                .as_ref()
                .unwrap()
                .groups
                .as_ptr(),
            ptr
        );
        assert_eq!(app.scene, live);
        assert_eq!(app.component_edit_stack.len(), depth);
        assert_eq!((app.undo_stack.len(), app.redo_stack.len()), history);
        app.cancel_component_edit();
        app.prepare_blender_projection().unwrap();
        assert!(app.blender_state.projection.scene.is_none());
    }
}
