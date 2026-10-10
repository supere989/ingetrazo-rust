//! Document lifecycle, persistent settings and recoverable autosaves.
use std::{path::PathBuf,time::{Duration,Instant}};
use crate::{app::IngeTrazoApp,session_state::{SessionStore,Preferences,RecoveryRecord}};
use ingetrazo_core::scene::Scene;
use ingetrazo_render::style::VisualStyle;
#[derive(Clone,Debug)]
pub enum DocumentAction {New,Open(PathBuf),Close,Recover(RecoveryRecord)}
pub struct DocumentSession {
    pub store:Option<SessionStore>,pub preferences:Preferences,
    pub clean:Option<Scene>,pub autosaved:Option<Scene>,
    pub last_autosave:Instant,pub pending:Option<DocumentAction>,
    pub confirm_unsaved:bool,pub close_allowed:bool,
    pub recovery:Vec<RecoveryRecord>,pub show_recovery:bool,pub recovered_slot:Option<PathBuf>,
}
impl Default for DocumentSession {fn default()->Self {Self{store:None,preferences:Preferences::default(),clean:None,
    autosaved:None,last_autosave:Instant::now(),pending:None,confirm_unsaved:false,close_allowed:false,
    recovery:vec![],show_recovery:false,recovered_slot:None}}}
fn same_model(a:&Scene,b:&Scene)->bool {
    a.mesh==b.mesh &&
    a.groups==b.groups &&
    a.dimensions==b.dimensions &&
    a.guides==b.guides &&
    a.layers==b.layers &&
    a.materials==b.materials &&
    a.saved_views==b.saved_views &&
    a.unit==b.unit &&
    a.embedded_textures==b.embedded_textures &&
    a.text_labels==b.text_labels &&
    a.section_planes==b.section_planes &&
    a.show_section_cuts==b.show_section_cuts &&
    a.show_section_planes==b.show_section_planes &&
    a.show_section_fill==b.show_section_fill &&
    a.geo_datum==b.geo_datum &&
    a.geo_points==b.geo_points &&
    a.geo_paths==b.geo_paths &&
    a.terrain==b.terrain &&
    a.sheets==b.sheets &&
    a.native_document==b.native_document &&
    a.native_baseline==b.native_baseline &&
    a.component_definitions==b.component_definitions
}
impl IngeTrazoApp {
    pub fn mark_document_clean(&mut self) {self.document_session.clean=Some(self.scene.clone());self.document_session.autosaved=None;self.document_session.last_autosave=Instant::now();}
    pub fn document_dirty(&self)->bool {!self.document_session.clean.as_ref().map(|s|same_model(&self.scene,s)).unwrap_or(false)}
    pub fn attach_session_store(&mut self,mut store:SessionStore) {
        match store.load_preferences() {Ok(p)=>self.document_session.preferences=p,Err(e)=>self.status_message=e}
        let p=&self.document_session.preferences;
        if let Some(lang)=p.language.clone() {self.set_language(&lang);}
        let p=&self.document_session.preferences;
        self.scene.unit=p.new_document_unit;self.recent_files=p.recent_files.clone();self.style.line_width=p.line_width;self.style.profile_width=p.profile_width;
        self.style.visual_style=match p.visual_style.as_str(){"wireframe"=>VisualStyle::Wireframe,"hidden"=>VisualStyle::HiddenLine,"mono"=>VisualStyle::Monochrome,"xray"=>VisualStyle::XRay,_=>VisualStyle::ShadedWithTextures};
        if let Some(rgb)=p.extra.get("section_fill_color").and_then(|v|v.as_array()).filter(|v|v.len()==3){for(i,value)in rgb.iter().enumerate(){if let Some(value)=value.as_f64().filter(|value|value.is_finite()){self.style.section_fill_color[i]=value.clamp(0.,1.)as f32;}}}
        self.shadows_enabled=p.shadows;self.show_toolbar=p.show_toolbar;self.show_tray=p.show_tray;
        match store.recovery_candidates(false) {Ok(candidates)=>{self.document_session.show_recovery=!candidates.is_empty();self.document_session.recovery=candidates;},Err(e)=>self.status_message=e}
        self.document_session.store=Some(store);self.mark_document_clean();
    }
    pub fn persist_preferences(&mut self) {
        let p=&mut self.document_session.preferences;p.language=Some(self.language.clone());p.new_document_unit=self.scene.unit;
        p.recent_files=self.recent_files.clone();p.line_width=self.style.line_width;p.profile_width=self.style.profile_width;
        p.visual_style=match self.style.visual_style{VisualStyle::Wireframe=>"wireframe",VisualStyle::HiddenLine=>"hidden",VisualStyle::Monochrome=>"mono",VisualStyle::XRay=>"xray",_=>"shaded"}.into();
        p.extra.insert("section_fill_color".into(),serde_json::json!(&self.style.section_fill_color[..3]));
        p.shadows=self.shadows_enabled;p.show_toolbar=self.show_toolbar;p.show_tray=self.show_tray;
        if let Some(store)=&self.document_session.store {if let Err(e)=store.save_preferences(p){self.status_message=e;}}
    }
    pub fn request_document_action(&mut self,action:DocumentAction) {
        if !self.component_edit_stack.is_empty(){self.status_message="Finish or cancel the component edit first".into();return;}
        self.document_session.pending=Some(action);
        if self.document_dirty(){self.document_session.confirm_unsaved=true;}else{self.finish_document_action();}
    }
    pub fn cancel_document_action(&mut self) {self.document_session.pending=None;self.document_session.confirm_unsaved=false;}
    pub fn discard_document_action(&mut self) {self.retire_current_recovery();self.finish_document_action();}
    pub fn finish_document_action(&mut self) {
        self.document_session.confirm_unsaved=false;
        match self.document_session.pending.take() {
            Some(DocumentAction::New)=>self.new_document_unchecked(),
            Some(DocumentAction::Open(path))=>self.open_document_unchecked(&path),
            Some(DocumentAction::Close)=>{self.retire_current_recovery();self.persist_preferences();self.document_session.close_allowed=true;},
            Some(DocumentAction::Recover(record))=>match ingetrazo_formats::igz::load_igz(&record.slot){
                Ok(scene)=>{self.retire_current_recovery();self.composer_state=Default::default();self.scene=scene;self.bump_model_revision();self.clipboard_document=uuid::Uuid::new_v4();self.camera=self.scene.camera.clone();self.current_file=None;self.undo_stack.clear();self.redo_stack.clear();self.textures.clear();
                    self.document_session.recovered_slot=Some(record.slot);self.document_session.clean=None;
                    self.document_session.show_recovery=false;self.status_message="Recovered unsaved copy; save it to a new file".into();},
                Err(e)=>self.status_message=format!("Recovery failed: {e}"),
            },None=>{},
        }
    }
    pub fn retire_current_recovery(&mut self) {
        if let Some(store)=&self.document_session.store {
            if let Err(e)=store.retire_document(self.current_file.as_deref()){self.status_message=e;}
            if let Some(slot)=self.document_session.recovered_slot.take(){if let Err(e)=store.retire(&slot){self.status_message=e;}}
        }
    }
    pub fn autosave_if_due(&mut self,now:Instant,gesture:bool)->Result<bool,String> {
        let state=&self.document_session;
        if state.store.is_none()||!state.preferences.autosave||gesture||!self.component_edit_stack.is_empty()
            ||now.saturating_duration_since(state.last_autosave)<Duration::from_secs(state.preferences.autosave_minutes as u64*60){return Ok(false);}
        if state.clean.as_ref().map(|s|same_model(&self.scene,s)).unwrap_or(false)||state.autosaved.as_ref().map(|s|same_model(&self.scene,s)).unwrap_or(false){self.document_session.last_autosave=now;return Ok(false);}
        let store=state.store.as_ref().unwrap();
        let mut snapshot=self.scene.clone();snapshot.camera=self.camera.clone();
        store.autosave(&snapshot,self.current_file.as_deref())?;
        self.document_session.autosaved=Some(self.scene.clone());self.document_session.last_autosave=now;Ok(true)
    }
    pub fn show_discarded_recovery(&mut self) {
        if let Some(store)=&self.document_session.store {match store.recovery_candidates(true){Ok(c)=>{self.document_session.recovery=c;self.document_session.show_recovery=true;},Err(e)=>self.status_message=e}}
    }
    pub(crate) fn begin_document_frame(&mut self,ctx:&egui::Context){
        self.handle_section_prompt_close(ctx);
        self.scene.refresh_dimensions();
        self.document_session_ui(ctx);
    }
    pub fn document_session_ui(&mut self,ctx:&egui::Context) {
        if ctx.input(|i|i.viewport().close_requested())&&!self.document_session.close_allowed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self.document_session.confirm_unsaved {self.request_document_action(DocumentAction::Close);}
        }
        if self.document_session.close_allowed{ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
        let gesture=ctx.input(|i|i.pointer.any_down());
        if let Err(e)=self.autosave_if_due(Instant::now(),gesture){self.status_message=format!("Autosave failed: {e}");self.document_session.last_autosave=Instant::now();}
        ctx.request_repaint_after(Duration::from_secs(30));
        if self.document_session.confirm_unsaved {
            egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).show(ctx,|ui|{
                ui.label("Save changes before continuing?");ui.horizontal(|ui|{
                    if ui.button("Save").clicked(){self.document_session.confirm_unsaved=false;self.save_document();}
                    if ui.button("Discard").clicked(){self.discard_document_action();}
                    if ui.button("Cancel").clicked(){self.cancel_document_action();}
                });
            });
        }
        if self.document_session.show_recovery {
            let records=self.document_session.recovery.clone();
            egui::Window::new("Recover an autosave").collapsible(false).show(ctx,|ui|{
                if records.is_empty(){ui.label("No recovery copies found.");}
                for record in records {
                    let label=record.document_path.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"Untitled".into());
                    if ui.button(format!("{label} — {}",record.updated_ms)).clicked(){self.request_document_action(DocumentAction::Recover(record));}
                }
                if ui.button("Later").clicked(){self.document_session.show_recovery=false;}
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;use glam::DVec3;
    fn root()->PathBuf {std::env::temp_dir().join(format!("ingetrazo-lifecycle-{}",uuid::Uuid::new_v4()))}
    fn triangle(app:&mut IngeTrazoApp,p:DVec3) {let ids=[p,p+DVec3::X,p+DVec3::Y].map(|v|app.scene.mesh.add_vertex(v));app.scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();}
    #[test]
    fn unsaved_actions_cancel_failed_save_and_successful_save_are_atomic() {
        let root=root();std::fs::create_dir_all(&root).unwrap();
        let mut app=IngeTrazoApp::new();app.attach_session_store(SessionStore::isolated(&root));
        triangle(&mut app,DVec3::new(7.,8.,9.));
        app.save_to_path(&root.join("preview.obj"));assert!(app.document_dirty());assert!(app.current_file.is_none());
        app.new_document();assert!(app.document_session.confirm_unsaved);assert_eq!(app.scene.mesh.vertices.len(),3);
        app.cancel_document_action();assert!(app.document_dirty());assert_eq!(app.scene.mesh.vertices.len(),3);
        app.request_document_action(DocumentAction::Close);
        app.save_to_path(&root.join("missing/failed.igz"));
        assert!(!app.document_session.close_allowed);assert!(app.document_session.confirm_unsaved);assert!(app.document_dirty());
        app.save_to_path(&root.join("blocked.obj"));assert!(!root.join("blocked.obj").exists());assert!(!app.document_session.close_allowed);
        app.save_to_path(&root.join("saved.igz"));assert!(app.document_session.close_allowed);assert!(!app.document_dirty());
        let loaded=ingetrazo_formats::igz::load_igz(&root.join("saved.igz")).unwrap();assert_eq!(loaded.mesh.vertices[0].position,DVec3::new(7.,8.,9.));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn naming_modal_keeps_document_autosave_timer_and_pending_prompt(){
        let root=root();std::fs::create_dir_all(&root).unwrap();let mut app=IngeTrazoApp::new();app.attach_session_store(SessionStore::isolated(&root));
        let plane=ingetrazo_core::section::SectionPlane::new(DVec3::ZERO,DVec3::Z,"Section","A");let id=plane.id;app.scene.section_planes.push(plane);app.section_name_prompt=Some((id,"Section".into(),"A".into(),false));
        app.document_session.last_autosave=Instant::now()-Duration::from_secs(301);let ctx=egui::Context::default();let _=ctx.run(Default::default(),|ctx|app.begin_document_frame(ctx));
        assert!(app.section_name_prompt.is_some());let slot=app.document_session.store.as_ref().unwrap().slot_for(None).unwrap();assert!(slot.is_file());let recovered=ingetrazo_formats::igz::load_igz(&slot).unwrap();assert_eq!(recovered.section_planes.len(),1);assert_eq!(recovered.section_planes[0].name,"Section");std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn timer_skips_gestures_cleans_and_duplicates_then_discard_keeps_recovery() {
        let root=root();std::fs::create_dir_all(&root).unwrap();let mut app=IngeTrazoApp::new();app.attach_session_store(SessionStore::isolated(&root));
        let now=app.document_session.last_autosave+Duration::from_secs(301);
        assert!(!app.autosave_if_due(now,false).unwrap());
        let now=now+Duration::from_secs(301);
        triangle(&mut app,DVec3::new(3.,2.,1.));
        app.camera.target=DVec3::new(12.,13.,14.);app.camera.distance=37.;
        assert!(!app.autosave_if_due(now,true).unwrap());assert!(app.autosave_if_due(now,false).unwrap());
        let slot=app.document_session.store.as_ref().unwrap().slot_for(None).unwrap();assert!(slot.is_file());
        assert!(!app.autosave_if_due(now+Duration::from_secs(301),false).unwrap());
        app.new_document();app.discard_document_action();assert!(!slot.exists());assert!(!app.document_dirty());
        let records=app.document_session.store.as_ref().unwrap().recovery_candidates(true).unwrap();assert_eq!(records.len(),1);
        app.camera.target=DVec3::ZERO;app.camera.distance=20.;
        app.request_document_action(DocumentAction::Recover(records[0].clone()));
        assert_eq!(app.camera.target,DVec3::new(12.,13.,14.));assert_eq!(app.camera.distance,37.);
        assert!(app.current_file.is_none());assert!(app.document_dirty());assert_eq!(app.scene.mesh.vertices[0].position,DVec3::new(3.,2.,1.));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn camera_selection_do_not_dirty_and_preferences_restart_keeps_recent_files() {
        let root=root();std::fs::create_dir_all(&root).unwrap();let mut app=IngeTrazoApp::new();app.attach_session_store(SessionStore::isolated(&root));
        app.scene.camera.target=DVec3::new(4.,5.,6.);app.scene.bump_view_version();app.scene.active_sheet_index=Some(0);
        assert!(!app.document_dirty());
        app.document_session.preferences.autosave_minutes=13;app.document_session.preferences.undo_steps=17;
        app.style.visual_style=VisualStyle::XRay;app.set_language("en");app.save_to_path(&root.join("saved.igz"));
        let mut restarted=IngeTrazoApp::new();restarted.attach_session_store(SessionStore::isolated(&root));
        assert_eq!(restarted.document_session.preferences.autosave_minutes,13);assert_eq!(restarted.document_session.preferences.undo_steps,17);
        assert_eq!(restarted.style.visual_style,VisualStyle::XRay);assert_eq!(restarted.recent_files,vec![root.join("saved.igz")]);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_close_requests_offer_cancel_then_close_after_discard() {
        let mut app=IngeTrazoApp::new();app.scene.mesh.add_vertex(DVec3::X);
        let ctx=egui::Context::default();let mut input=egui::RawInput::default();
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().events.push(egui::ViewportEvent::Close);
        let out=ctx.run(input,|ctx|app.document_session_ui(ctx));
        assert!(app.document_session.confirm_unsaved);assert!(!app.document_session.close_allowed);
        assert!(out.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c|matches!(c,egui::ViewportCommand::CancelClose)));
        app.cancel_document_action();assert_eq!(app.scene.mesh.vertices.len(),1);
        app.request_document_action(DocumentAction::Close);app.discard_document_action();
        let out=ctx.run(egui::RawInput::default(),|ctx|app.document_session_ui(ctx));
        assert!(out.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c|matches!(c,egui::ViewportCommand::Close)));
    }
}
