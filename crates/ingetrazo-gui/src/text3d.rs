// SPDX-License-Identifier: GPL-3.0-or-later
//! Dialog, placement and history ownership for native editable 3D text.
use crate::IngeTrazoApp;
use ingetrazo_core::{groups::Group, scene::SelectionItem};
use ingetrazo_formats::text3d::{self, Params};
use ingetrazo_tools::base::ToolType;
use uuid::Uuid;

pub struct Dialog {
    pub params: Params,
    target: Option<(Uuid, Uuid)>,
    error: Option<String>,
}

fn find(groups: &[Group], id: Uuid) -> Option<&Group> {
    for group in groups {
        if group.id == id {
            return Some(group);
        }
        if let Some(found) = find(&group.children, id) {
            return Some(found);
        }
    }
    None
}
fn find_mut(groups: &mut [Group], id: Uuid) -> Option<&mut Group> {
    for group in groups {
        if group.id == id {
            return Some(group);
        }
        if let Some(found) = find_mut(&mut group.children, id) {
            return Some(found);
        }
    }
    None
}

impl IngeTrazoApp {
    pub fn open_insert_text3d(&mut self) {
        self.text3d_dialog = Some(Dialog {
            params: Params::default(),
            target: None,
            error: None,
        });
    }

    /// A selected letter in the open text container addresses its owner.
    /// This lookup does not regenerate glyphs or hash geometry each frame.
    pub fn selected_text3d(&self) -> Option<Uuid> {
        let mut groups = self.scene.selection.iter().filter_map(|item| match item {
            SelectionItem::Group(index) => self.scene.groups.get(*index),
            _ => None,
        });
        let group = groups.next()?;
        if groups.next().is_some() {
            return None;
        }
        if text3d::params(group).is_some() {
            return Some(group.id);
        }
        let context = self.component_edit_stack.last()?;
        find(&context.original_scene.groups, context.group_id)
            .filter(|owner| text3d::params(owner).is_some())
            .map(|owner| owner.id)
    }

    fn text3d_candidate(&self, id: Uuid) -> Result<Group, String> {
        let mut baseline = self.scene.clone();
        self.active_tool.restore_transient_model(&mut baseline);
        if let Some(group) = find(&baseline.groups, id) {
            return Ok(group.clone());
        }
        let context = self
            .component_edit_stack
            .last()
            .filter(|s| s.group_id == id)
            .ok_or("The selected text no longer exists")?;
        let (candidate, _) = crate::component_edit::component_candidate(context, &baseline)?;
        find(&candidate.groups, id)
            .cloned()
            .ok_or_else(|| "The selected text no longer exists".into())
    }

    pub fn open_edit_text3d(&mut self) {
        let Some(id) = self.selected_text3d() else {
            return;
        };
        match self.text3d_candidate(id) {
            Ok(group) if !text3d::text_is_pristine(&group) => {
                self.status_message = "This text's letters were edited by hand, so it can no longer be regenerated as text.".into();
            }
            Ok(group) => {
                if let Some(params) = text3d::params(&group) {
                    self.text3d_dialog = Some(Dialog {
                        params,
                        target: Some((id, group.history_id)),
                        error: None,
                    });
                }
            }
            Err(error) => self.status_message = error,
        }
    }

    /// Build once before changing document, context, tool or history state.
    pub fn insert_text3d(&mut self, params: &Params) -> Result<(), String> {
        let group = text3d::make_text_group(params)?;
        if !self.finish_contexts_for_document() {
            return Err(self.status_message.clone());
        }
        self.set_tool(ToolType::PlaceGroup);
        self.active_tool = Box::new(ingetrazo_tools::place_group::PlaceGroupTool::new(
            group, true,
        ));
        self.status_message = "3D Text — move to position; click to place; Esc to cancel".into();
        Ok(())
    }

    pub fn edit_text3d(&mut self, id: Uuid, params: &Params) -> Result<(), String> {
        let direct = self.scene.groups.iter().any(|g| g.id == id);
        let open_owner = self.component_edit_stack.len() == 1
            && self
                .component_edit_stack
                .last()
                .is_some_and(|s| s.group_id == id);
        if !direct && !open_owner {
            return Err("Finish editing this group, then select the text container in its parent to edit its text".into());
        }
        let old = self.text3d_candidate(id)?;
        if !text3d::text_is_pristine(&old) {
            return Err("This text's letters were edited by hand, so it can no longer be regenerated as text.".into());
        }
        if text3d::params(&old).as_ref() == Some(params) {
            return Ok(());
        }
        let children = text3d::rebuild_text_group(&old, params)?;
        let state = text3d::text_state(params, &children)?;
        if self
            .component_edit_stack
            .last()
            .is_some_and(|s| s.group_id == id)
            && !self.finish_contexts_for_document()
        {
            return Err(self.status_message.clone());
        }
        // Context publication may move the group, but cannot change its owner.
        let current = find(&self.scene.groups, id).ok_or("The selected text no longer exists")?;
        if current.history_id != old.history_id {
            return Err("The selected text was replaced".into());
        }
        // Source transform previews are not document commands. Retire the
        // live Rust preview after admission, before taking the edit snapshot.
        self.set_tool(ToolType::Select);
        self.save_undo_state();
        let group = find_mut(&mut self.scene.groups, id).expect("validated text owner");
        group.children = children;
        group.native_document["text3d"] = state;
        let name = params.text.trim().chars().take(24).collect::<String>();
        if !name.is_empty() {
            group.name = name;
        }
        self.scene.clear_selection();
        if let Some(index) = self.scene.groups.iter().position(|g| g.id == id) {
            self.scene.select(SelectionItem::Group(index));
        }
        self.scene.bump_version();
        self.status_message = "Edited 3D Text".into();
        Ok(())
    }
}

pub fn render_dialog(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.text3d_dialog.take() else {
        return;
    };
    let mut open = true;
    let mut accept = false;
    let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Window::new("3D Text")
        .id(egui::Id::new("text3d_dialog"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Grid::new("text3d_params")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Text:");
                    ui.text_edit_singleline(&mut dialog.params.text);
                    ui.end_row();
                    ui.label("Font:");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.params.font)
                            .hint_text("System default"),
                    );
                    ui.end_row();
                    ui.label("Bold:");
                    ui.checkbox(&mut dialog.params.bold, "");
                    ui.end_row();
                    ui.label("Italic:");
                    ui.checkbox(&mut dialog.params.italic, "");
                    ui.end_row();
                    ui.label("Height:");
                    ui.add(
                        egui::DragValue::new(&mut dialog.params.height)
                            .range(0.001..=1000.)
                            .speed(0.05)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    );
                    ui.end_row();
                    ui.label("Extruded:");
                    ui.add(
                        egui::DragValue::new(&mut dialog.params.thickness)
                            .range(0.0..=1000.)
                            .speed(0.01)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    )
                    .on_hover_text("0 leaves flat faces (no extrusion)");
                    ui.end_row();
                });
            if let Some(error) = &dialog.error {
                ui.colored_label(egui::Color32::RED, error);
            }
            ui.horizontal(|ui| {
                accept = ui
                    .add_enabled(
                        !dialog.params.text.trim().is_empty(),
                        egui::Button::new("OK"),
                    )
                    .clicked();
                cancel |= ui.button("Cancel").clicked();
            });
        });
    if !open || cancel {
        return;
    }
    if accept {
        dialog.params.text = dialog.params.text.trim().to_owned();
        let result = match dialog.target {
            Some((id, identity)) => match app.text3d_candidate(id) {
                Ok(group) if group.history_id == identity => app.edit_text3d(id, &dialog.params),
                _ => Err("The selected text was replaced".into()),
            },
            None => app.insert_text3d(&dialog.params),
        };
        match result {
            Ok(()) => return,
            Err(error) => dialog.error = Some(error),
        }
    }
    app.text3d_dialog = Some(dialog);
}
