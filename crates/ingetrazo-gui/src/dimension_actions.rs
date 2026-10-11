//! Dimension property editing with one model transaction per action.
use crate::IngeTrazoApp;
impl IngeTrazoApp{
    pub fn edit_dimension(&mut self,index:usize,text:Option<String>,axis:Option<String>,layer:Option<String>)->bool{
        if axis.as_deref().map(|a|!["x","y","z"].contains(&a)).unwrap_or(false){return false;}
        let Some(old)=self.scene.dimensions.get(index)else{return false;};if old.text_override==text&&old.axis==axis&&old.layer==layer{return false;}
        self.save_undo_state();self.undo_stack.last_mut().unwrap().history_annotation_edit=true;let dimension=&mut self.scene.dimensions[index];dimension.text_override=text;dimension.axis=axis;dimension.layer=layer;self.scene.bump_version();self.bump_model_revision();true
    }
    pub fn remove_dimension(&mut self,index:usize)->bool{if index>=self.scene.dimensions.len(){return false;}self.save_undo_state();self.scene.dimensions.remove(index);self.selected_dimension=None;self.scene.bump_version();self.bump_model_revision();true}
}
pub fn render_dimensions(app:&mut IngeTrazoApp,ui:&mut egui::Ui){
    egui::CollapsingHeader::new("Dimensions").show(ui,|ui|{
        for(i,d)in app.scene.dimensions.iter().enumerate(){if ui.selectable_label(app.selected_dimension==Some(i),d.display_text(app.scene.unit,2)).clicked(){app.selected_dimension=Some(i);}}
        if let Some(i)=app.selected_dimension.filter(|i|*i<app.scene.dimensions.len()){
            let d=&app.scene.dimensions[i];let mut text=app.dimension_text_input.as_ref().filter(|(idx,_)|*idx==i).map(|(_,text)|text.clone()).unwrap_or_else(||d.text_override.clone().unwrap_or_default());let mut axis=d.axis.clone();let mut layer=d.layer.clone();
            ui.label("Custom text (<> inserts measurement)");let edit=ui.text_edit_singleline(&mut text);
            let mut changed=edit.lost_focus()&&text!=app.scene.dimensions[i].text_override.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt("dimension_axis").selected_text(axis.as_deref().unwrap_or("Aligned")).show_ui(ui,|ui|{changed|=ui.selectable_value(&mut axis,None,"Aligned").changed();for a in["x","y","z"]{changed|=ui.selectable_value(&mut axis,Some(a.to_owned()),a).changed();}});
            egui::ComboBox::from_id_salt("dimension_layer").selected_text(layer.as_deref().unwrap_or("Default")).show_ui(ui,|ui|{changed|=ui.selectable_value(&mut layer,None,"Default").changed();for l in &app.scene.layers{changed|=ui.selectable_value(&mut layer,Some(l.name.clone()),&l.name).changed();}});
            // Text edits use their own buffer so typing is retained until Apply.
            if edit.changed(){app.dimension_text_input=Some((i,text.clone()));}
            if let Some((idx,value))=&app.dimension_text_input{if *idx==i{text=value.clone();}}
            if changed||ui.button("Apply properties").clicked(){app.edit_dimension(i,if text.is_empty(){None}else{Some(text)},axis,layer);app.dimension_text_input=None;}
            if ui.button("Delete dimension").clicked(){app.remove_dimension(i);}
        }
    });
}
#[cfg(test)]mod tests{use super::*;use glam::DVec3;use ingetrazo_core::dimensions::Dimension;
    #[test]fn property_edit_delete_invalid_axis_and_undo_preserve_dimension_identity(){let mut app=IngeTrazoApp::new();let mut dim=Dimension::new(DVec3::ZERO,DVec3::new(3.,4.,0.),DVec3::Y);dim.native_document=serde_json::json!({"opaque":19});app.scene.dimensions.push(dim);let before=app.scene.clone();assert!(app.edit_dimension(0,Some("width <>".into()),Some("x".into()),Some("Annotations".into())));assert_eq!(app.scene.dimensions[0].native_document["opaque"],19);assert_eq!(app.scene.dimensions[0].length(),3.);assert!(!app.edit_dimension(0,None,Some("bad".into()),None));app.undo();assert_eq!(app.scene,before);assert!(app.remove_dimension(0));app.undo();assert_eq!(app.scene,before);}
}
