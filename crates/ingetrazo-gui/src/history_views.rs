//! Source ScenesPanel actions mutate identity-held objects outside model History.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    saved_views::{SavedView, ViewMembershipDelta},
    scene::Scene,
};
pub(crate) fn restore(current: &Scene, target: &mut Scene) -> Option<ViewMembershipDelta> {
    let delta = target.history_views.0.take();
    target.saved_views = delta.as_ref().map_or_else(
        || current.saved_views.clone(),
        |d| d.restore(&current.saved_views),
    );
    delta.map(|d| d.inverse())
}
impl IngeTrazoApp {
    pub(crate) fn seal_view_history(&mut self) {
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_context == self.scene.history_context
                && before.history_views.0.is_none()
            {
                before.history_views.0 = Some(ViewMembershipDelta::between(
                    &before.saved_views,
                    &self.scene.saved_views,
                ));
            }
        }
    }
    pub(crate) fn seal_view_history_before(&mut self, current:&Scene){if let Some(before)=self.undo_stack.last_mut(){if before.history_views.0.is_none(){before.history_views.0=Some(ViewMembershipDelta::between(&before.saved_views,&current.saved_views));}}}
    pub(crate) fn refresh_history_view(&mut self, view: &SavedView) {
        for s in self.undo_stack.iter_mut().chain(&mut self.redo_stack) {
            if let Some(d) = &mut s.history_views.0 {
                d.refresh(view);
            }
        }
        for session in &mut self.component_edit_stack {
            for s in session
                .original_undo
                .iter_mut()
                .chain(&mut session.original_redo)
            {
                if let Some(d) = &mut s.history_views.0 {
                    d.refresh(view);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_view_owner_survives_update_delete_and_redo_object_lifetime() {
        let mut app = IngeTrazoApp::new();
        app.scene = Scene::new();
        app.save_undo_state();
        let imported = SavedView::new("Imported");
        let id = imported.history_id;
        app.scene.saved_views.push(imported);
        app.scene.bump_version();
        app.selected_saved_view = Some(0);
        app.rename_selected_saved_view("Renamed".into());
        assert_eq!(app.undo_stack.len(), 1);
        app.remove_selected_saved_view();
        assert!(app.scene.saved_views.is_empty());
        app.undo();
        assert!(app.scene.saved_views.is_empty());
        app.redo();
        assert_eq!(app.scene.saved_views.len(), 1);
        assert_eq!(app.scene.saved_views[0].history_id, id);
        assert_eq!(app.scene.saved_views[0].name, "Renamed");
        app.undo();
        app.add_saved_view("UI only".into());
        assert_eq!(app.redo_stack.len(), 1);
        app.redo();
        assert_eq!(
            app.scene
                .saved_views
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            ["UI only", "Renamed"]
        );
    }
    #[test]
    fn nonhistory_view_actions_do_not_hide_geometry_branch() {
        let mut a = IngeTrazoApp::new();
        a.scene = Scene::new();
        a.save_undo_state();
        a.scene.mesh.add_vertex(glam::DVec3::X);
        a.scene.bump_version();
        a.add_saved_view("UI".into());
        a.undo();
        assert!(a.scene.mesh.vertices.is_empty());
        assert_eq!(a.scene.saved_views.len(), 1);
        a.rename_selected_saved_view("Changed on undone branch".into());
        assert_eq!(a.redo_stack.len(), 1);
        a.redo();
        assert_eq!(a.scene.mesh.vertices.len(), 1);
        assert_eq!(a.scene.saved_views[0].name, "Changed on undone branch");
        let value = serde_json::to_value(&a.scene).unwrap();
        assert!(value.get("history_views").is_none());
        assert!(value["saved_views"][0].get("history_id").is_none());
    }
}
