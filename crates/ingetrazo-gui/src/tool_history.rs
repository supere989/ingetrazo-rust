// SPDX-License-Identifier: GPL-3.0-or-later
//! Tool commits share one undo transaction whether entered by click or VCB.
use crate::app::IngeTrazoApp;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::base::ToolContext;

fn changed_model(before: &Scene, after: &Scene) -> bool {
    before.mesh != after.mesh
        || before.groups != after.groups
        || before.dimensions != after.dimensions
        || before.guides != after.guides
        || before.section_planes != after.section_planes
        || before.geo_points != after.geo_points
        || before.geo_paths != after.geo_paths
        || before.terrain != after.terrain
        || before.materials != after.materials
        || before.layers != after.layers
        || before.text_labels != after.text_labels
        || before.sheets != after.sheets
}

impl IngeTrazoApp {
    fn tool_snapshot(&self) -> Scene {
        let mut before = self.scene.clone();
        before.camera = self.camera.clone();
        before
    }

    fn record_tool_commit(&mut self, before: Scene) {
        if changed_model(&before, &self.scene) {
            self.undo_stack.push(before);
            self.redo_stack.clear();
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
    }

    pub fn dispatch_tool_click(&mut self, context: &ToolContext) {
        let before = self.tool_snapshot();
        self.active_tool.on_click(&mut self.scene, context);
        self.record_tool_commit(before);
    }

    pub fn dispatch_tool_double_click(&mut self, context: &ToolContext) {
        let before = self.tool_snapshot();
        self.active_tool.on_double_click(&mut self.scene, context);
        self.record_tool_commit(before);
    }

    pub fn dispatch_tool_vcb(&mut self, text: &str) -> Result<(), String> {
        let before = self.tool_snapshot();
        match self.active_tool.on_vcb_input(&mut self.scene, text) {
            Ok(()) => {
                self.record_tool_commit(before);
                Ok(())
            }
            Err(error) => {
                self.scene = before;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::{math::Ray, scene::SelectionItem};
    use ingetrazo_tools::base::{create_tool, ToolType};

    fn app() -> IngeTrazoApp {
        let mut app = IngeTrazoApp::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.), DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ids, vec![]);
        app.active_tool = create_tool(ToolType::PushPull);
        app.active_tool_type = ToolType::PushPull;
        app
    }

    fn context(z: f64) -> ToolContext {
        ToolContext::new(
            DVec3::new(0.5, 0.5, z),
            Ray::new(DVec3::new(0.5, 0.5, 5.), -DVec3::Z),
        )
    }

    #[test]
    fn click_and_vcb_pushpull_each_create_one_undo_commit() {
        for vcb in [false, true] {
            let mut app = app();
            let before = app.scene.mesh.clone();
            app.dispatch_tool_click(&context(0.));
            assert!(app.undo_stack.is_empty());
            if vcb {
                app.dispatch_tool_vcb("1").unwrap();
            } else {
                app.active_tool.on_move(&mut app.scene, &context(1.));
                assert!(app.undo_stack.is_empty());
                app.dispatch_tool_click(&context(1.));
            }
            assert_eq!(app.undo_stack.len(), 1);
            assert!((app.scene.mesh.volume().unwrap() - 1.).abs() < 1e-8);
            let after = app.scene.mesh.clone();
            app.undo();
            assert_eq!(app.scene.mesh, before);
            app.redo();
            assert_eq!(app.scene.mesh, after);
        }
    }

    #[test]
    fn cancel_invalid_vcb_and_selection_do_not_create_undo_commits() {
        let mut app = app();
        let before = app.scene.mesh.clone();
        app.dispatch_tool_click(&context(0.));
        assert!(app.dispatch_tool_vcb("NaN").is_err());
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.scene.mesh, before);
        app.active_tool.on_cancel(&mut app.scene);
        assert!(app.undo_stack.is_empty());
        app.active_tool = create_tool(ToolType::Select);
        app.scene.select(SelectionItem::Face(0));
        app.dispatch_tool_click(&context(0.));
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.scene.mesh, before);
    }
}
