//! Shared immutable source annotation layout, independent of native GPU backend.
use std::sync::Arc;
use ingetrazo_render::annotations::{self,Builder,DimensionInput,LeaderInput};
use crate::annotation_overlay_geometry::Cache;
fn map(matrix:&[f32;16],v:[f32;4])->[f32;4]{std::array::from_fn(|row|matrix[row]*v[0]+matrix[4+row]*v[1]+matrix[8+row]*v[2]+matrix[12+row]*v[3])}
// One planner shared by production and bounded CPU-only engineering inspection.
pub(crate) fn build_plan(app:&crate::IngeTrazoApp,rect:egui::Rect,matrix:&[f32;16],cache:&mut Cache,include_annotations:bool)->Result<annotations::Plan,String>{
 let raw=app.scene.dimension_style();
 let hover=app.active_tool.annotation_hover_target(&app.scene);
  let project=|point:glam::Vec3|{
   let q=map(&matrix,[point.x,point.y,point.z,1.]);
   if !q.iter().all(|v|v.is_finite())||q[3]<=0.{return None;}
   Some([rect.min.x as f64+(q[0] as f64/q[3] as f64+1.)*0.5*rect.width() as f64,
    rect.min.y as f64+(1.-q[1] as f64/q[3] as f64)*0.5*rect.height() as f64])
  };
  let mut builder=Builder::new(project,annotations::Limits{queries:200_000,operations:200_000,text_bytes:16*1024*1024});
  let mut style=annotations::Style::default();
  if let Some(v)=raw["color"].as_array(){if v.len()>=3{for(i,c)in style.color[..3].iter_mut().enumerate(){*c=v[i].as_u64().and_then(|x|u8::try_from(x).ok()).ok_or("Invalid annotation style color")?;}}}
  if let Some(v)=raw["font_size"].as_i64(){style.font_size=i32::try_from(v).map_err(|_|"Annotation style size overflow")?;}
  if let Some(v)=raw["ends"].as_str(){style.ends=v.into();}
  if let Some(v)=raw["units"].as_str(){style.units=v.into();}
  if let Some(v)=raw["decimals"].as_i64(){style.decimals=i32::try_from(v).map_err(|_|"Annotation decimal overflow")?;}
  if include_annotations{
  for(index,dim)in app.scene.dimensions.iter().enumerate(){
   let frame=app.scene.dimension_frame;
   builder.dimension(DimensionInput{a:frame.transform_point3(dim.start).as_vec3(),b:frame.transform_point3(dim.end).as_vec3(),offset:frame.transform_vector3(dim.offset).as_vec3(),axis:dim.axis.as_deref(),text:dim.text_override.as_deref(),highlighted:app.selected_dimension==Some(index)||app.scene.is_selected(&ingetrazo_core::scene::SelectionItem::Dimension(index))||hover==Some(ingetrazo_tools::base::AnnotationEditTarget::Dimension(dim.id)),hidden:dim.native_document["hidden"].as_bool().unwrap_or(false)||!app.scene.is_layer_visible(dim.layer.as_deref())},&style)?;
  }
  let leader_style=style.clone();
  for(index,label)in app.scene.text_labels.iter().enumerate(){
   let(height,_)=cache.leader_metrics("",leader_style.font_size,96.)?;
   builder.leader(LeaderInput{anchor:label.anchor.as_vec3(),offset:label.offset.as_vec3(),text:&label.text,highlighted:app.scene.is_selected(&ingetrazo_core::scene::SelectionItem::TextLabel(index))||hover==Some(ingetrazo_tools::base::AnnotationEditTarget::Label(label.history_id)),hidden:!app.scene.is_layer_visible(label.layer.as_deref())},&leader_style,height,|text|cache.leader_metrics(text,leader_style.font_size,96.).map(|(_,advance)|advance))?;
  }
  }
  let mut plan=builder.finish()?;
  if let Some((distance,point))=app.active_tool.distance_label(){
   if !distance.is_finite()||!point.is_finite(){return Err("Tool label outside finite admission".into());}
   if let Some(pixel)=project(point.as_vec3()){
    let(unit,precision)=label_units(&app.scene)?;
    let text:Arc<str>=if app.vcb_text.is_empty(){annotations::format_length(distance,unit,precision)?}else{typed_distance_text(&app.vcb_text,unit)}.into();
    let shadow=if app.vcb_text.is_empty(){[255,255,255,220]}else{[255,220,130,235]};
    for(offset,color)in [([12.,-7.],shadow),([11.,-8.],[15,20,27,255])]{
     let position=[pixel[0]+offset[0],pixel[1]+offset[1]];
     if position.iter().any(|x|!x.is_finite()){return Err("Tool label projection overflow".into());}
     plan.operations.try_reserve(1).map_err(|_|"Tool label operation admission failed")?;
     plan.operations.push(annotations::Operation{primitive:annotations::Primitive::Text{position,text:text.clone(),size_pt:10,color},gate:annotations::Gate::Always});
    }
   }
  }
  Ok(plan)

}
pub(crate) fn label_units(scene:&ingetrazo_core::scene::Scene)->Result<(&'static str,i32),String>{
 use ingetrazo_core::units::LengthUnit;
 let fallback=match scene.unit{LengthUnit::Meters=>"m",LengthUnit::Centimeters=>"cm",LengthUnit::Millimeters=>"mm",LengthUnit::Inches=>"in",LengthUnit::Feet=>"ft"};
 let unit=match scene.native_document["units"]["length"].as_str().unwrap_or(fallback){
  "m"=>"m","cm"=>"cm","mm"=>"mm","in"=>"in","ft"=>"ft","ft-in"=>"ft-in","in-frac"=>"in-frac","ft-in-frac"=>"ft-in-frac",_=>return Err("Unknown tool label unit".into())};
 let precision=scene.native_document["units"]["precision"].as_i64().unwrap_or(2);
 Ok((unit,i32::try_from(precision).map_err(|_|"Tool label precision overflow")?))
}

// Pinned typed_value_text's unit mark is added only to bare numeric/list input.
pub(crate) fn typed_distance_text(buffer:&str,unit:&str)->String{
 let text=buffer.trim();
 let bare=!text.is_empty()&&text.split([',',';']).all(|part|{
  let p=part.trim();let p=p.strip_prefix(['-','+']).unwrap_or(p);
  !p.is_empty()&&p.chars().all(|c|c.is_ascii_digit()||c=='.')
 });
 if !bare{return text.into();}
 let mark=match unit{"m"=>" m","cm"=>" cm","mm"=>" mm","ft"=>"'",_=>"\""};
 format!("{text}{mark}")
}

#[cfg(test)]mod document_tests {
 use super::*;
 #[test]fn persisted_dimensions_share_source_plan_and_hidden_clear(){
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(glam::DVec3::ZERO,glam::DVec3::X,glam::DVec3::Y));
  app.scene.native_document["dimension_style"]=serde_json::json!({"units":"mm","decimals":2,"font_size":9,"color":[45,55,75]});
  let mut cache=Cache::new(ingetrazo_formats::annotation_font::Limits{faces:2,retained_font_bytes:1024*1024},crate::annotation_overlay_geometry::Limits{cells:1000,spans:1000,geometry_bytes:1024*1024,glyphs:2,glyph_bytes:1024*1024}).unwrap();
  let rect=egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.));let matrix=glam::Mat4::IDENTITY.to_cols_array();
  let plan=build_plan(&app,rect,&matrix,&mut cache,true).unwrap();assert!(!plan.queries.is_empty());assert!(plan.operations.iter().any(|op|matches!(&op.primitive,annotations::Primitive::Text{text,..}if text.as_ref()=="1000.00 mm")));
  app.scene.dimensions[0].native_document["hidden"]=serde_json::json!(true);assert!(build_plan(&app,rect,&matrix,&mut cache,true).unwrap().operations.is_empty());
 }
}

#[cfg(test)]
mod hover_tests {
    use super::*;
    use ingetrazo_tools::base::{AnnotationEditTarget, AnnotationHits, ToolContext, ToolType};

    #[test]
    fn hover_identity_context_stale_and_transient_ownership() {
        let mut app = crate::IngeTrazoApp::new();
        app.system_clipboard = None;
        app.scene.text_labels.push(ingetrazo_core::textlabel::TextLabel::new(
            glam::DVec3::ZERO, glam::DVec3::X, "hover"));
        let id = app.scene.text_labels[0].history_id;
        app.set_tool(ToolType::Select);
        let ctx = ToolContext::new(glam::DVec3::ZERO,
            ingetrazo_core::math::Ray::new(glam::DVec3::Z, -glam::DVec3::Z));
        let parent = uuid::Uuid::new_v4();
        let child = uuid::Uuid::new_v4();
        app.scene.history_context = vec![parent, child];
        app.active_tool.note_annotation_hits(AnnotationHits {label:Some(id),dimension:None});
        let before = app.scene.clone();
        let revision = app.model_revision;
        app.dispatch_tool_move(&ctx);
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),Some(AnnotationEditTarget::Label(id)));
        app.dispatch_tool_move(&ctx);
        app.active_tool.on_cancel(&mut app.scene);
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),Some(AnnotationEditTarget::Label(id)));
        assert_eq!(app.scene,before);
        assert_eq!(app.model_revision,revision);
        assert!(app.undo_stack.is_empty() && app.redo_stack.is_empty());
        // Equal depth and shared last child do not identify the same placement context.
        app.scene.history_context[0]=uuid::Uuid::new_v4();
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),None);
        app.scene.history_context[0]=parent;
        let held=app.scene.text_labels.remove(0);
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),None);
        app.scene.text_labels.push(ingetrazo_core::textlabel::TextLabel::new(
            glam::DVec3::ZERO,glam::DVec3::X,"hover"));
        assert_ne!(app.scene.text_labels[0].history_id,id);
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),None);
        app.scene.text_labels[0]=held;
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),Some(AnnotationEditTarget::Label(id)));
        app.begin_annotation_edit(AnnotationEditTarget::Label(id)).unwrap();
        let egui=egui::Context::default();
        let frame=|app:&mut crate::IngeTrazoApp,point:egui::Pos2|{
            let raw=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.))),events:vec![egui::Event::PointerMoved(point)],..Default::default()};
            let _=egui.run(raw,|ctx|{egui::CentralPanel::default().show(ctx,|ui|crate::viewport::render_viewport(app,ui));});
        };
        frame(&mut app,egui::pos2(400.,300.));
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),Some(AnnotationEditTarget::Label(id)),"modal freezes existing hover");
        frame(&mut app,egui::pos2(900.,700.));
        assert_eq!(app.active_tool.annotation_hover_target(&app.scene),None,"actual viewport departure retires hover");
        assert!(app.undo_stack.is_empty()&&app.redo_stack.is_empty());
    }
}

/// CPU-only engineering inspection of the same native annotation plan, no depth/pixel claim.
pub fn annotation_plan_for_probe(app:&crate::IngeTrazoApp,rect:egui::Rect)->Result<annotations::Plan,String>{
    let snapshot=crate::navigation_ray::camera_snapshot(&app.camera).ok_or("Invalid diagnostic camera")?;
    let matrix=std::array::from_fn(|i|snapshot.matrix[i/4][i%4]);
    let mut cache=Cache::new(ingetrazo_formats::annotation_font::Limits{faces:32,retained_font_bytes:32*1024*1024},crate::annotation_overlay_geometry::Limits{cells:2_000_000,spans:200_000,geometry_bytes:32*1024*1024,glyphs:4096,glyph_bytes:16*1024*1024})?;
    build_plan(app,rect,&matrix,&mut cache,true)
}
