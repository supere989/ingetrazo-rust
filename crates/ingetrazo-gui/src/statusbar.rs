// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Bottom status bar with sheet tabs, prompt hints, and Value Control Box (VCB).

use ingetrazo_core::i18n::tr;
use crate::app::IngeTrazoApp;

pub fn render_status_bar(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        // Sheet Tabs
        let is_model_active = app.scene.active_sheet_index.is_none();
        if ui.selectable_label(is_model_active, tr("Model")).clicked() {
            app.scene.active_sheet_index = None;
        }

        let mut switch_to_sheet = None;
        let mut duplicate_sheet_idx = None;
        let mut delete_sheet_idx = None;

        for (idx, sheet) in app.scene.sheets.iter().enumerate() {
            let is_active = app.scene.active_sheet_index == Some(idx);
            let tab_resp = ui.selectable_label(is_active, &sheet.name);
            if tab_resp.clicked() {
                switch_to_sheet = Some(idx);
            }
            tab_resp.context_menu(|ui| {
                if ui.button(tr("Duplicate Sheet")).clicked() {
                    duplicate_sheet_idx = Some(idx);
                    ui.close_menu();
                }
                if ui.button(tr("Delete Sheet")).clicked() {
                    delete_sheet_idx = Some(idx);
                    ui.close_menu();
                }
            });
        }

        if let Some(idx) = switch_to_sheet {
            app.scene.active_sheet_index = Some(idx);
        }

        if let Some(dup_idx) = duplicate_sheet_idx {
            if dup_idx < app.scene.sheets.len() {
                let mut dup = app.scene.sheets[dup_idx].clone();
                dup.name = format!("{} (Copy)", dup.name);
                dup.id = uuid::Uuid::new_v4();
                app.scene.sheets.push(dup);
                app.scene.bump_version();
            }
        }

        if let Some(del_idx) = delete_sheet_idx {
            if del_idx < app.scene.sheets.len() {
                app.scene.sheets.remove(del_idx);
                if app.scene.active_sheet_index == Some(del_idx) {
                    app.scene.active_sheet_index = if !app.scene.sheets.is_empty() {
                        Some(del_idx.saturating_sub(1))
                    } else {
                        None
                    };
                }
                app.scene.bump_version();
            }
        }

        if ui.button("+").on_hover_text(tr("New sheet")).clicked() {
            let count = app.scene.sheets.len() + 1;
            let new_sheet = ingetrazo_core::composition::Sheet::new(
                format!("Sheet {count}"),
                ingetrazo_core::composition::PaperSize::A3,
                ingetrazo_core::composition::Orientation::Landscape,
            );
            app.scene.sheets.push(new_sheet);
            app.scene.active_sheet_index = Some(app.scene.sheets.len() - 1);
            app.scene.bump_version();
            app.status_message = format!("Created Sheet {count}");
        }
        ui.separator();

        // Prompt hint
        let default_hint = tr("Select — Shift = add/remove, Ctrl = add. Double-click a group to edit it. F3 = search commands.");
        let hint = if !app.status_message.is_empty() {
            &app.status_message
        } else {
            &default_hint
        };
        ui.label(egui::RichText::new(hint).color(egui::Color32::from_rgb(200, 200, 210)));

        // Right side: Measurements / VCB
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(tr("Units: Meters"));
            ui.separator();

            let vcb_label = app.active_tool.vcb_label();
            let prompt = if !vcb_label.is_empty() { tr(vcb_label) } else { tr("Measurements") };

            let edit_resp = ui.add(
                egui::TextEdit::singleline(&mut app.vcb_text)
                    .hint_text("0.00")
                    .desired_width(70.0),
            );

            if edit_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Err(err) = app.active_tool.on_vcb_input(&mut app.scene, &app.vcb_text) {
                    app.status_message = format!("{}: {err}", tr("Error"));
                } else {
                    app.status_message.clear();
                }
                app.vcb_text.clear();
            }

            ui.label(format!("{prompt}:"));
        });
    });
}
