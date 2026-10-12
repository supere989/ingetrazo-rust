//! Selected saved-view operations with identity-preserving updates.
use crate::IngeTrazoApp;use ingetrazo_core::saved_views::SavedView;#[cfg(test)]use ingetrazo_render::style::VisualStyle;
impl IngeTrazoApp {
    fn capture_live_view(&self,name:String)->SavedView {
        let mut view=SavedView::capture_scene(name,&self.scene,&self.camera);
        let(style,shadows)=self.live_presentation();view.style=Some(serde_json::to_value(style).unwrap());view.shadows=Some(serde_json::to_value(shadows).unwrap());view
    }
    pub fn add_saved_view(&mut self,name:String){let view=self.capture_live_view(name);self.seal_view_history();self.scene.saved_views.push(view);self.selected_saved_view=Some(self.scene.saved_views.len()-1);self.scene.bump_version();}
    pub fn update_selected_saved_view(&mut self){
        let Some(i)=self.selected_saved_view.filter(|i|*i<self.scene.saved_views.len()) else{return;};
        let old=self.scene.saved_views[i].clone();let mut view=self.capture_live_view(old.name.clone());
        view.history_id=old.history_id;view.native_document=old.native_document;view.native_baseline=old.native_baseline;
        fn merge(old:Option<serde_json::Value>,new:Option<serde_json::Value>)->Option<serde_json::Value>{match(old,new){(Some(mut old),Some(new))if old.is_object()&&new.is_object()=>{for(k,v)in new.as_object().unwrap(){old[k]=v.clone();}Some(old)},(_,new)=>new}}
        view.style=merge(old.style,view.style);view.section=merge(old.section,view.section);view.shadows=merge(old.shadows,view.shadows);
        view.georef=merge(old.georef,view.georef);view.hidden_shown=merge(old.hidden_shown,view.hidden_shown);
        if view!=self.scene.saved_views[i]{self.seal_view_history();self.refresh_history_view(&view);self.scene.saved_views[i]=view;self.scene.bump_version();}
    }
    pub fn remove_selected_saved_view(&mut self){if let Some(i)=self.selected_saved_view.filter(|i|*i<self.scene.saved_views.len()){self.seal_view_history();self.scene.saved_views.remove(i);self.selected_saved_view=if self.scene.saved_views.is_empty(){None}else{Some(i.min(self.scene.saved_views.len()-1))};self.scene.bump_version();}}
    pub fn rename_selected_saved_view(&mut self,name:String){if let Some(i)=self.selected_saved_view.filter(|i|*i<self.scene.saved_views.len()){if !name.trim().is_empty()&&name!=self.scene.saved_views[i].name{self.seal_view_history();self.scene.saved_views[i].name=name;let view=self.scene.saved_views[i].clone();self.refresh_history_view(&view);self.scene.bump_version();}}}
    pub fn apply_saved_view(&mut self,i:usize){
        let Some(view)=self.scene.saved_views.get(i).cloned() else{return;};self.selected_saved_view=Some(i);if let Some(visible)=view.georef.as_ref().and_then(|g|g.get("terrain")).and_then(|v|v.as_bool()){self.set_terrain_visibility(visible);}view.apply_scene(&mut self.scene,&mut self.camera);
        self.apply_document_presentation();
        self.status_message=format!("Applied scene: {}",view.name);
    }
}
#[cfg(test)]mod tests{use super::*;use glam::DVec3;use ingetrazo_core::{groups::Group,section::SectionPlane};
    #[test]fn selected_update_preserves_metadata_and_recalls_hidden_tree_section_style(){
        let mut app=IngeTrazoApp::new().without_startup_figure_for_test();let mut parent=Group::new("parent");let mut child=Group::new("child");child.hidden=true;parent.children.push(child);app.scene.groups.push(parent);
        app.scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::Z,"cut","A"));app.style.visual_style=VisualStyle::XRay;app.shadows_enabled=true;app.camera.two_point=true;
        app.add_saved_view("first".into());app.add_saved_view("second".into());app.scene.saved_views[1].native_document=serde_json::json!({"opaque":17});app.scene.saved_views[1].style.as_mut().unwrap()["opaque_style"]=serde_json::json!(19);
        let first=app.scene.saved_views[0].clone();app.camera.distance=33.;app.update_selected_saved_view();assert_eq!(app.scene.saved_views[0],first);assert_eq!(app.scene.saved_views[1].native_document["opaque"],17);assert_eq!(app.scene.saved_views[1].style.as_ref().unwrap()["opaque_style"],19);
        app.scene.groups[0].children[0].hidden=false;app.scene.section_planes[0].active=false;app.style.visual_style=VisualStyle::Wireframe;app.shadows_enabled=false;app.camera.two_point=false;
        app.apply_saved_view(1);assert!(app.scene.groups[0].children[0].hidden);assert!(app.scene.section_planes[0].active);assert!(app.camera.two_point&&app.shadows_enabled);assert_eq!(app.camera.distance,33.);assert_eq!(app.style.visual_style,VisualStyle::XRay);
        let history=app.undo_stack.len();app.remove_selected_saved_view();assert_eq!(app.scene.saved_views.len(),1);app.undo();assert_eq!(app.scene.saved_views.len(),1);assert_eq!(app.undo_stack.len(),history);
    }
}

#[cfg(test)]mod ghost_view_tests {
 use super::*;
 #[test]fn ghost_saved_view_update_keeps_unknown_fields_and_recalls_current_flags(){
  let mut app=IngeTrazoApp::new();app.scene.set_hidden_view(true,true);
  app.add_saved_view("ghost".into());
  app.scene.saved_views[0].hidden_shown.as_mut().unwrap()["future"]=serde_json::json!({"owner":7});
  app.scene.set_hidden_view(true,false);app.scene.set_hidden_view(false,true);
  app.update_selected_saved_view();
  assert_eq!(app.scene.saved_views[0].hidden_shown,Some(serde_json::json!({"objects":false,"geometry":true,"future":{"owner":7}})));
  app.scene.set_hidden_view(true,true);app.scene.set_hidden_view(false,false);app.apply_saved_view(0);
  assert!(!app.scene.show_hidden_objects&&app.scene.show_hidden_geometry);
 }
}
