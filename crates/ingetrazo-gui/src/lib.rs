// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! # IngeTrazo GUI
//!
//! Native desktop application built on `eframe` and `egui`.
//! Includes menu bar, toolbar, status bar with VCB, inspector side tray,
//! and interactive 3D viewport canvas.

pub mod app;
pub mod blender;
pub mod composer;
pub mod dialogs;
pub mod icons;
pub mod menu;
pub mod statusbar;
pub mod toolbar;
mod tool_history;
pub mod tray;
pub mod viewport;

pub use app::IngeTrazoApp;

use std::path::PathBuf;

/// Launch the desktop GUI application.
pub fn run_gui(initial_file: Option<PathBuf>) -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("IngeTrazo 0.3.15 (Rust native)"),
        ..Default::default()
    };

    eframe::run_native(
        "IngeTrazo",
        native_options,
        Box::new(move |_cc| {
            let mut app=IngeTrazoApp::new();
            app.system_clipboard=arboard::Clipboard::new().ok();
            app.attach_session_store(crate::session_state::SessionStore::user_default());
            if let Some(path)=initial_file {app.open_document(&path);}
            Ok(Box::new(app))
        }),
    )

}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::camera::StandardView;
    use ingetrazo_core::scene::SelectionItem;
    use ingetrazo_render::style::VisualStyle;
    use ingetrazo_tools::base::ToolType;

    #[test]
    fn test_app_initialization() {
        let app = IngeTrazoApp::new();
        assert_eq!(app.active_tool_type, ToolType::Select);
        assert!(app.show_toolbar);
        assert!(app.show_tray);
        assert_eq!(app.style.visual_style, VisualStyle::ShadedWithTextures);
    }

    #[test]
    fn test_app_tool_switching() {
        let mut app = IngeTrazoApp::new();
        app.set_tool(ToolType::Line);
        assert_eq!(app.active_tool_type, ToolType::Line);
        assert_eq!(app.active_tool.name(), "Line");
        assert_eq!(app.active_tool.vcb_label(), "Length");

        app.set_tool(ToolType::Rectangle);
        assert_eq!(app.active_tool_type, ToolType::Rectangle);
        assert_eq!(app.active_tool.vcb_label(), "Dimensions");
    }

    #[test]
    fn test_app_undo_redo() {
        let mut app = IngeTrazoApp::new();
        assert_eq!(app.scene.mesh.vertices.len(), 0);

        app.save_undo_state();
        app.scene.mesh.add_vertex(DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(app.scene.mesh.vertices.len(), 1);

        app.undo();
        assert_eq!(app.scene.mesh.vertices.len(), 0);

        app.redo();
        assert_eq!(app.scene.mesh.vertices.len(), 1);
    }

    #[test]
    fn test_app_delete_and_select_all() {
        let mut app = IngeTrazoApp::new();
        let v0 = app.scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = app.scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = app.scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let f = app.scene.mesh.add_face(vec![v0, v1, v2], vec![]).unwrap();

        app.select_all();
        assert!(app.scene.is_selected(&SelectionItem::Face(f)));

        app.delete_selection();
        assert_eq!(app.scene.mesh.faces.len(), 0);
    }

    #[test]
    fn test_app_camera_presets() {
        let mut app = IngeTrazoApp::new();
        app.camera.set_standard_view(StandardView::Top);
        assert!(app.camera.pitch > 1.5); // close to 90 degrees
    }
}

pub mod component_edit;

pub mod session_state;

pub mod document_session;

pub mod clipboard;

pub mod saved_view_actions;

pub mod outliner;

mod geometry_actions;
pub mod dimension_actions;

pub mod section_actions;
