//! Selected saved-view operations with identity-preserving updates.
use crate::IngeTrazoApp;use ingetrazo_core::saved_views::SavedView;use ingetrazo_render::style::VisualStyle;
impl IngeTrazoApp {
    fn capture_live_view(&self,name:String)->SavedView {
        let mut view=SavedView::capture_scene(name,&self.scene,&self.camera);
        let s=&self.style;
        view.style=Some(serde_json::json!({"face_mode":match s.visual_style{VisualStyle::Wireframe=>"wireframe",VisualStyle::HiddenLine=>"hidden_line",VisualStyle::Monochrome=>"monochrome",VisualStyle::XRay=>"xray",_=>"textures"},
            "edges":s.show_edges,"profiles":s.show_profiles,"edge_color":&s.edge_color[..3],"front_color":&s.front_color[..3],"back_color":&s.back_color[..3],"background":&s.background_color[..3],"section_fill":self.scene.show_section_fill}));
        view.shadows=Some(serde_json::json!({"enabled":self.shadows_enabled}));view
    }
    pub fn add_saved_view(&mut self,name:String){let view=self.capture_live_view(name);self.save_undo_state();self.scene.saved_views.push(view);self.selected_saved_view=Some(self.scene.saved_views.len()-1);self.scene.bump_version();}
    pub fn update_selected_saved_view(&mut self){
        let Some(i)=self.selected_saved_view.filter(|i|*i<self.scene.saved_views.len()) else{return;};
        let old=self.scene.saved_views[i].clone();let mut view=self.capture_live_view(old.name.clone());
        view.native_document=old.native_document;view.native_baseline=old.native_baseline;
        fn merge(old:Option<serde_json::Value>,new:Option<serde_json::Value>)->Option<serde_json::Value>{match(old,new){(Some(mut old),Some(new))if old.is_object()&&new.is_object()=>{for(k,v)in new.as_object().unwrap(){old[k]=v.clone();}Some(old)},(_,new)=>new}}
        view.style=merge(old.style,view.style);view.section=merge(old.section,view.section);view.shadows=merge(old.shadows,view.shadows);
        view.georef=merge(old.georef,view.georef);view.hidden_shown=old.hidden_shown;
        if view!=self.scene.saved_views[i]{self.save_undo_state();self.scene.saved_views[i]=view;self.scene.bump_version();}
    }
    pub fn remove_selected_saved_view(&mut self){if let Some(i)=self.selected_saved_view.filter(|i|*i<self.scene.saved_views.len()){self.save_undo_state();self.scene.saved_views.remove(i);self.selected_saved_view=if self.scene.saved_views.is_empty(){None}else{Some(i.min(self.scene.saved_views.len()-1))};self.scene.bump_version();}}
    pub fn rename_selected_saved_view(&mut self,name:String){if let Some(i)=self.selected_saved_view.filter(|i|*i<self.scene.saved_views.len()){if !name.trim().is_empty()&&name!=self.scene.saved_views[i].name{self.save_undo_state();self.scene.saved_views[i].name=name;self.scene.bump_version();}}}
    pub fn apply_saved_view(&mut self,i:usize){
        let Some(view)=self.scene.saved_views.get(i).cloned() else{return;};self.selected_saved_view=Some(i);view.apply_scene(&mut self.scene,&mut self.camera);
        if let Some(s)=&view.style{
            if let Some(mode)=s.get("face_mode").and_then(|v|v.as_str()){self.style.visual_style=match mode{"wireframe"=>VisualStyle::Wireframe,"hidden_line"=>VisualStyle::HiddenLine,"monochrome"=>VisualStyle::Monochrome,"xray"=>VisualStyle::XRay,_=>VisualStyle::ShadedWithTextures};}
            if let Some(v)=s.get("edges").and_then(|v|v.as_bool()){self.style.show_edges=v;}if let Some(v)=s.get("profiles").and_then(|v|v.as_bool()){self.style.show_profiles=v;}
            fn color(value:Option<&serde_json::Value>,target:&mut [f32;4]){if let Some(a)=value.and_then(|v|v.as_array()).filter(|a|a.len()>=3){for i in 0..3{if let Some(v)=a[i].as_f64(){target[i]=v as f32;}}}}
            color(s.get("edge_color"),&mut self.style.edge_color);color(s.get("front_color"),&mut self.style.front_color);color(s.get("back_color"),&mut self.style.back_color);color(s.get("background"),&mut self.style.background_color);
            if let Some(v)=s.get("section_fill").and_then(|v|v.as_bool()){self.scene.show_section_fill=v;}
        }
        if let Some(v)=view.shadows.as_ref().and_then(|s|s.get("enabled")).and_then(|v|v.as_bool()){self.shadows_enabled=v;}
        self.status_message=format!("Applied scene: {}",view.name);
    }
}
#[cfg(test)]mod tests{use super::*;use glam::DVec3;use ingetrazo_core::{groups::Group,section::SectionPlane};
    #[test]fn selected_update_preserves_metadata_and_recalls_hidden_tree_section_style(){
        let mut app=IngeTrazoApp::new();let mut parent=Group::new("parent");let mut child=Group::new("child");child.hidden=true;parent.children.push(child);app.scene.groups.push(parent);
        app.scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::Z,"cut","A"));app.style.visual_style=VisualStyle::XRay;app.shadows_enabled=true;app.camera.two_point=true;
        app.add_saved_view("first".into());app.add_saved_view("second".into());app.scene.saved_views[1].native_document=serde_json::json!({"opaque":17});app.scene.saved_views[1].style.as_mut().unwrap()["opaque_style"]=serde_json::json!(19);
        let first=app.scene.saved_views[0].clone();app.camera.distance=33.;app.update_selected_saved_view();assert_eq!(app.scene.saved_views[0],first);assert_eq!(app.scene.saved_views[1].native_document["opaque"],17);assert_eq!(app.scene.saved_views[1].style.as_ref().unwrap()["opaque_style"],19);
        app.scene.groups[0].children[0].hidden=false;app.scene.section_planes[0].active=false;app.style.visual_style=VisualStyle::Wireframe;app.shadows_enabled=false;app.camera.two_point=false;
        app.apply_saved_view(1);assert!(app.scene.groups[0].children[0].hidden);assert!(app.scene.section_planes[0].active);assert!(app.camera.two_point&&app.shadows_enabled);assert_eq!(app.camera.distance,33.);assert_eq!(app.style.visual_style,VisualStyle::XRay);
        let before=app.scene.clone();app.remove_selected_saved_view();assert_eq!(app.scene.saved_views.len(),1);app.undo();assert_eq!(app.scene,before);
    }
}
