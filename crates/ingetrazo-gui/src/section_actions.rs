//! Section controls operate on document planes, including inside editing contexts.
use crate::IngeTrazoApp;
#[derive(Clone)]
pub enum SectionAction {
    Activate(uuid::Uuid),
    Deactivate,
    Reverse(uuid::Uuid),
    Delete(uuid::Uuid),
    Rename(uuid::Uuid, String, String),
    Display(bool, bool, bool),
}
impl IngeTrazoApp {
    pub fn apply_section_action(&mut self, action: SectionAction) {
        let mut candidate = self.scene.clone();
        match action {
            SectionAction::Display(cuts, planes, fill) => {
                candidate.show_section_cuts = cuts;
                candidate.show_section_planes = planes;
                candidate.show_section_fill = fill;
            }
            SectionAction::Deactivate => {
                for plane in &mut candidate.section_planes {
                    plane.active = false;
                }
            }
            SectionAction::Activate(id) => {
                if !candidate.section_planes.iter().any(|p| p.id == id) {
                    return;
                }
                for plane in &mut candidate.section_planes {
                    plane.active = plane.id == id;
                }
            }
            SectionAction::Reverse(id) => {
                let Some(plane) = candidate.section_planes.iter_mut().find(|p| p.id == id) else {
                    return;
                };
                plane.flip();
            }
            SectionAction::Delete(id) => {
                let Some(removed)=candidate.section_planes.iter().position(|plane|plane.id==id)else{return;};candidate.section_planes.remove(removed);
                fn remap(item:ingetrazo_core::scene::SelectionItem,index:usize)->Option<ingetrazo_core::scene::SelectionItem>{use ingetrazo_core::scene::SelectionItem;match item{SelectionItem::SectionPlane(id)if id==index=>None,SelectionItem::SectionPlane(id)if id>index=>Some(SelectionItem::SectionPlane(id-1)),item=>Some(item)}}
                candidate.selection=std::mem::take(&mut candidate.selection).into_iter().filter_map(|item|remap(item,removed)).collect();candidate.selection_order=std::mem::take(&mut candidate.selection_order).into_iter().filter_map(|item|remap(item,removed)).collect();
            },
            SectionAction::Rename(id, name, symbol) => {
                let Some(plane) = candidate.section_planes.iter_mut().find(|p| p.id == id) else {
                    return;
                };
                plane.name = name;
                plane.symbol = symbol;
            }
        }
        if candidate != self.scene {
            self.save_undo_state();
            candidate.bump_version();
            self.scene = candidate;
        }
    }
}
pub fn render_section_controls(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    let mut rgb=[app.style.section_fill_color[0],app.style.section_fill_color[1],app.style.section_fill_color[2]];ui.horizontal(|ui|{ui.label("Section Fill Color");if ui.color_edit_button_rgb(&mut rgb).changed(){app.style.section_fill_color=[rgb[0],rgb[1],rgb[2],1.];app.scene.bump_view_version();}});
    let mut cuts = app.scene.show_section_cuts;
    let mut planes = app.scene.show_section_planes;
    let mut fill = app.scene.show_section_fill;
    let changed = ui.checkbox(&mut cuts, "Section Cuts").changed()
        | ui.checkbox(&mut planes, "Section Planes").changed()
        | ui.checkbox(&mut fill, "Section Fill").changed();
    if changed {
        app.apply_section_action(SectionAction::Display(cuts, planes, fill));
    }
    for plane in app.scene.section_planes.clone() {
        ui.menu_button(format!("{} {}", plane.symbol, plane.name), |ui| {
            if ui.selectable_label(plane.active, "Active Cut").clicked() {
                app.apply_section_action(if plane.active {
                    SectionAction::Deactivate
                } else {
                    SectionAction::Activate(plane.id)
                });
            }
            if ui.button("Reverse").clicked() {
                app.apply_section_action(SectionAction::Reverse(plane.id));
            }
            let draft_id = ui.id().with(plane.id);
            let (mut name, mut symbol) = ui
                .ctx()
                .data_mut(|data| data.get_temp::<(String, String)>(draft_id))
                .unwrap_or((plane.name.clone(), plane.symbol.clone()));
            let a = ui.text_edit_singleline(&mut name);
            let b = ui.text_edit_singleline(&mut symbol);
            ui.ctx()
                .data_mut(|data| data.insert_temp(draft_id, (name.clone(), symbol.clone())));
            if (a.lost_focus() || b.lost_focus()) && (name != plane.name || symbol != plane.symbol)
            {
                app.apply_section_action(SectionAction::Rename(plane.id, name, symbol));
                ui.ctx()
                    .data_mut(|data| data.remove::<(String, String)>(draft_id));
            }
            if ui.button("Delete").clicked() {
                app.apply_section_action(SectionAction::Delete(plane.id));
                ui.close_menu();
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, DVec3};
    use ingetrazo_core::{groups::Group, scene::SelectionItem, section::SectionPlane};
    #[test]
    fn sections_in_rotated_scaled_edit_context_keep_world_plane_and_outer_undo() {
        let mut app = IngeTrazoApp::new();
        let mut group = Group::new("part");
        group.transform = DMat4::from_translation(DVec3::new(1., 2., 3.))
            * DMat4::from_rotation_y(0.4)
            * DMat4::from_scale(DVec3::new(2., 3., 4.));
        app.scene.groups.push(group);
        app.scene.section_planes.push(SectionPlane::new(
            DVec3::new(2., 3., 4.),
            DVec3::new(1., 2., 3.),
            "section",
            "A",
        ));
        let before = app.scene.clone();
        let id = app.scene.section_planes[0].id;
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        app.apply_section_action(SectionAction::Reverse(id));
        app.finish_component_edit();
        assert_eq!(app.scene.section_planes[0].id, id);
        assert!(
            (app.scene.section_planes[0].point - before.section_planes[0].point).length() < 1e-10
        );
        assert!(
            (app.scene.section_planes[0].normal + before.section_planes[0].normal).length() < 1e-10
        );
        assert_eq!(app.undo_stack.len(), 1);
        app.undo();
        assert_eq!(app.scene.section_planes, before.section_planes);
    }
}

impl IngeTrazoApp {
    pub(crate) fn queue_section_name_prompt(&mut self) {
        if self
            .document_session
            .preferences
            .extra
            .get("section_ask_name")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
        {
            if let Some(plane) = self.scene.section_planes.last() {
                self.section_name_prompt =
                    Some((plane.id, plane.name.clone(), plane.symbol.clone(), false));
            }
        }
    }
    pub fn finish_section_name_prompt(&mut self, accepted: bool) {
        let Some((id, name, symbol, no_more)) = self.section_name_prompt.take() else {
            return;
        };
        if !accepted {
            return;
        }
        let Some(plane) = self.scene.section_planes.iter_mut().find(|p| p.id == id) else {
            return;
        };
        if !name.trim().is_empty() {
            plane.name = name.trim().to_owned();
        }
        if !symbol.trim().is_empty() {
            plane.symbol = symbol.trim().chars().take(3).collect();
        }
        self.scene.bump_version();
        if no_more {
            self.document_session
                .preferences
                .extra
                .insert("section_ask_name".into(), serde_json::json!(false));
            self.persist_preferences();
        }
    }
}
pub fn render_section_name_prompt(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        app.finish_section_name_prompt(false);
        return;
    }
    let mut accept = None;
    if let Some((id, name, symbol, no_more)) = &mut app.section_name_prompt {
        if !app.scene.section_planes.iter().any(|plane| plane.id == *id) {
            app.section_name_prompt = None;
            return;
        }
        egui::Window::new("Name Section Plane")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(name);
                ui.label("Symbol");
                ui.text_edit_singleline(symbol);
                ui.checkbox(no_more, "Don't ask again — keep defaults");
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        accept = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        accept = Some(false);
                    }
                });
            });
    }
    if let Some(accepted) = accept {
        app.finish_section_name_prompt(accepted);
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::math::Ray;
    use ingetrazo_tools::base::{ToolContext, ToolType};
    #[test]
    fn placement_prompt_cancel_rename_optout_and_undo_follow_original_history() {
        let mut app = IngeTrazoApp::new();
        app.set_tool(ToolType::SectionPlane);
        let context = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::Z, -DVec3::Z));
        app.dispatch_tool_click(&context);
        assert!(app.section_name_prompt.is_some());
        assert_eq!(app.undo_stack.len(), 1);
        let before = app.scene.clone();
        app.dispatch_tool_click(&context);
        assert!(app.dispatch_tool_vcb("1").is_err());
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack.len(), 1);
        app.finish_section_name_prompt(false);
        assert_eq!(app.scene.section_planes.len(), 1);
        assert_eq!(app.undo_stack.len(), 1);
        app.dispatch_tool_click(&context);
        let prompt = app.section_name_prompt.as_mut().unwrap();
        prompt.1 = " New section ".into();
        prompt.2 = "ABCD".into();
        prompt.3 = true;
        app.finish_section_name_prompt(true);
        assert_eq!(app.scene.section_planes[1].name, "New section");
        assert_eq!(app.scene.section_planes[1].symbol, "ABC");
        assert_eq!(app.undo_stack.len(), 2);
        assert!(!app.scene.section_planes[0].active);
        app.undo();
        assert_eq!(app.scene.section_planes.len(), 1);
        app.redo();
        assert_eq!(app.scene.section_planes[1].name, "New section");
        app.dispatch_tool_click(&context);
        assert!(app.section_name_prompt.is_none());
    }
}

impl IngeTrazoApp{pub(crate) fn handle_section_prompt_close(&mut self,ctx:&egui::Context){if self.section_name_prompt.is_some()&&ctx.input(|input|input.viewport().close_requested()){self.finish_section_name_prompt(false);}}}
#[cfg(test)]mod native_close_tests{use super::*;use glam::DVec3;use ingetrazo_tools::base::{ToolType,ToolContext};use ingetrazo_core::math::Ray;#[test]fn native_close_cancels_naming_then_enters_unsaved_guard(){let mut app=IngeTrazoApp::new();app.set_tool(ToolType::SectionPlane);app.dispatch_tool_click(&ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z)));assert!(app.section_name_prompt.is_some());let ctx=egui::Context::default();let mut input=egui::RawInput::default();input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().events.push(egui::ViewportEvent::Close);let output=ctx.run(input,|ctx|{app.handle_section_prompt_close(ctx);app.document_session_ui(ctx);});assert!(app.section_name_prompt.is_none());assert!(app.document_session.confirm_unsaved);assert!(!app.document_session.close_allowed);assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|cmd|matches!(cmd,egui::ViewportCommand::CancelClose)));assert_eq!(app.scene.section_planes.len(),1);}}

#[cfg(test)]mod selection_identity_tests{use super::*;use ingetrazo_core::{scene::SelectionItem,section::SectionPlane};use glam::DVec3;#[test]fn deleting_earlier_plane_keeps_selection_on_same_persisted_identity(){let mut app=IngeTrazoApp::new();for symbol in["A","B","C"]{app.scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::Z,symbol,symbol));}let remove=app.scene.section_planes[0].id;let selected=app.scene.section_planes[2].id;app.scene.select(SelectionItem::SectionPlane(2));let before=app.scene.clone();app.apply_section_action(SectionAction::Delete(remove));assert!(app.scene.selection.contains(&SelectionItem::SectionPlane(1)));assert_eq!(app.scene.section_planes[1].id,selected);assert_eq!(app.scene.selection_order,vec![SelectionItem::SectionPlane(1)]);app.undo();assert_eq!(app.scene,before);}}

#[cfg(test)]mod color_tests{use super::*;#[test]fn section_color_saved_view_and_isolated_restart_keep_rgb(){use crate::session_state::SessionStore;let root=std::env::temp_dir().join(format!("ingetrazo-section-color-{}",uuid::Uuid::new_v4()));let mut app=IngeTrazoApp::new();assert_eq!(app.style.section_fill_color,[0.35,0.37,0.41,1.]);app.attach_session_store(SessionStore::isolated(&root));app.style.section_fill_color=[0.2,0.4,0.6,1.];app.add_saved_view("Color".into());app.style.section_fill_color=[1.,0.,0.,1.];app.apply_saved_view(0);assert_eq!(app.style.section_fill_color,[0.2,0.4,0.6,1.]);app.persist_preferences();let mut restarted=IngeTrazoApp::new();restarted.attach_session_store(SessionStore::isolated(&root));assert_eq!(restarted.style.section_fill_color,[0.2,0.4,0.6,1.]);std::fs::remove_dir_all(root).unwrap();}}
