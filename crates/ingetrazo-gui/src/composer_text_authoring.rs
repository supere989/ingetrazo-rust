//! Original Texto placement and InlineTextEditor command boundaries.
use crate::app::IngeTrazoApp;
use ingetrazo_core::composition::{SheetItem,TextItem};
#[derive(Debug,Clone)]pub struct InlineText{pub sheet:uuid::Uuid,pub item:uuid::Uuid,pub original:String,pub draft:String,pub focus:bool}
impl IngeTrazoApp{
 pub fn place_composer_text(&mut self,sheet_idx:usize,point:(f64,f64))->Result<uuid::Uuid,String>{
  if !point.0.is_finite()||!point.1.is_finite(){return Err("Text position must be finite".into());}
  if self.scene.sheets.get(sheet_idx).is_none(){return Err("Text sheet is no longer available".into());}
  self.finish_composer_edit();self.begin_composer_edit();let item=TextItem{id:uuid::Uuid::new_v4(),pos:point,text:ingetrazo_core::i18n::tr("Text"),font_size_pt:14.,bold:false};let id=item.id;
  let sheet=&mut self.scene.sheets[sheet_idx];let z=sheet.items.iter().map(|i|match i{SheetItem::Shape(t)=>t.shape.z,_=>sheet.native_items.get(&i.id()).and_then(|v|v.get("z")).and_then(|v|v.as_f64()).unwrap_or(0.)}).reduce(f64::max).map(|z|z+1.).unwrap_or(0.);sheet.native_items.insert(id,serde_json::json!({"z":z}));sheet.add_item(SheetItem::Text(item));self.finish_composer_edit();self.composer_state.text_tool=false;Ok(id)
 }
 pub fn begin_composer_text_edit(&mut self,sheet_idx:usize,item_id:uuid::Uuid)->Result<(),String>{
  let sheet=self.scene.sheets.get(sheet_idx).ok_or("Text sheet is no longer available")?;
  if sheet.native_items.get(&item_id).and_then(|v|v.get("locked")).and_then(|v|v.as_bool()).unwrap_or(false){return Err("Locked text cannot be edited".into());}
  let item=sheet.items.iter().find_map(|i|if let SheetItem::Text(t)=i{(t.id==item_id).then_some(t)}else{None}).ok_or("Text item is no longer available")?;
  self.composer_state.inline_text=Some(InlineText{sheet:sheet.id,item:item.id,original:item.text.clone(),draft:item.text.clone(),focus:true});Ok(())
 }
 pub fn finish_composer_text_edit(&mut self,commit:bool)->Result<(),String>{
  let Some(edit)=self.composer_state.inline_text.as_ref()else{return Ok(());};
  if !commit{self.composer_state.inline_text=None;return Ok(());}
  let sheet_idx=self.scene.sheets.iter().position(|s|s.id==edit.sheet).ok_or("Text edit target sheet changed")?;
  let item_idx=self.scene.sheets[sheet_idx].items.iter().position(|i|i.id()==edit.item).ok_or("Text edit target was removed")?;
  let SheetItem::Text(item)=&self.scene.sheets[sheet_idx].items[item_idx]else{return Err("Text edit target type changed".into());};
  if item.text!=edit.original{return Err("Text changed while the editor was active".into());}
  let text=edit.draft.trim_end_matches('\n').to_owned();if text==item.text{self.composer_state.inline_text=None;return Ok(());}
  self.finish_composer_edit();self.begin_composer_edit();if let SheetItem::Text(item)=&mut self.scene.sheets[sheet_idx].items[item_idx]{item.text=text;}
  self.composer_state.inline_text=None;self.finish_composer_edit();Ok(())
 }
}
pub(super)fn render(app:&mut IngeTrazoApp,ui:&mut egui::Ui,_painter:&egui::Painter,index:usize,paper:egui::Rect,toolbar:egui::Rect,response:&egui::Response,zoom:f32)->bool{
 if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some()&&(response.clicked()||response.double_clicked()||response.triple_clicked()) {let value=serde_json::json!({"at":ui.input(|i|i.time),"clicked":response.clicked(),"double":response.double_clicked(),"triple":response.triple_clicked(),"pos":response.interact_pointer_pos().map(|p|[p.x,p.y]),"tool":app.composer_state.text_tool,"inline":app.composer_state.inline_text.is_some()});ui.ctx().data_mut(|d|{let mut events=d.get_temp::<Vec<serde_json::Value>>(egui::Id::new("composer-text-pointer-events")).unwrap_or_default();events.push(value);if events.len()>16{events.remove(0);}d.insert_temp(egui::Id::new("composer-text-pointer-events"),events);});}
 if app.composer_state.text_tool{
  if ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::Escape)){app.composer_state.text_tool=false;return true;}
  if response.clicked(){if let Some(pos)=response.interact_pointer_pos().filter(|p|paper.contains(*p)&&!toolbar.contains(*p)){if let Err(e)=app.place_composer_text(index,(f64::from((pos.x-paper.min.x)/zoom),f64::from((pos.y-paper.min.y)/zoom))){app.status_message=e;}}}return true;
 }
 if app.composer_state.inline_text.is_none()&&(response.double_clicked()||response.triple_clicked()){if let Some(pos)=response.interact_pointer_pos(){
  let sheet=&app.scene.sheets[index];let target=sheet.visible_item_indices().into_iter().rev().find_map(|n|if let SheetItem::Text(t)=&sheet.items[n]{let r=t_rect(sheet,t);egui::Rect::from_min_size(paper.min+egui::vec2(r.0 as f32,r.1 as f32)*zoom,egui::vec2(r.2 as f32,r.3 as f32)*zoom).contains(pos).then_some(t.id)}else{None});
  if let Some(id)=target{if let Err(e)=app.begin_composer_text_edit(index,id){app.status_message=e;}}
 }}
 // Operative ComposerCanvasView handles Escape before InlineTextEditor:
 // clear selection while keeping its draft/focus (source line5217).
 if app.composer_state.inline_text.is_some()&&ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::Escape)){app.scene.sheets[index].selected_item=None;}
 let Some(edit)=app.composer_state.inline_text.as_ref()else{return false;};
 if app.scene.sheets[index].id!=edit.sheet{app.composer_state.inline_text=None;return false;}
 let Some(item)=app.scene.sheets[index].items.iter().find_map(|i|if let SheetItem::Text(t)=i{(t.id==edit.item).then_some(t)}else{None})else{return true;};
 let (x,y,w,_)=t_rect(&app.scene.sheets[index],item);let font_size=item.font_size_pt as f32*(25.4/72.)*0.75*zoom;
 // Original empty-sheet press is intercepted for box selection (line4256),
 // never sent to QGraphicsScene; the inline editor therefore keeps focus.
 let pointer_position=ui.input(|i|i.pointer.interact_pos());let window_has_focus=ui.input(|i|i.focused);

 let cancel=false;let enter=ui.input_mut(|i|i.consume_key(egui::Modifiers::CTRL,egui::Key::Enter));let edit=app.composer_state.inline_text.as_mut().unwrap();let first=edit.focus;let mut outside=false;
 egui::Area::new(egui::Id::new(("composer-inline-text",edit.item))).order(egui::Order::Foreground).fixed_pos(paper.min+egui::vec2(x as f32,y as f32)*zoom).show(ui.ctx(),|ui|{
  egui::Frame::none().fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1.,egui::Color32::from_rgb(58,110,165))).inner_margin(0.).show(ui,|ui|{let r=ui.add(egui::TextEdit::multiline(&mut edit.draft).desired_width(w as f32*zoom).font(egui::FontId::proportional(font_size)).margin(egui::Vec2::ZERO));if first{r.request_focus();edit.focus=false;}let blank=window_has_focus&&r.lost_focus()&&pointer_position.is_some_and(|pos|response.rect.contains(pos)&&!toolbar.contains(pos)&&!app.scene.sheets[index].visible_item_indices().into_iter().any(|n|{let q=app.scene.sheets[index].items[n].rect();egui::Rect::from_min_size(paper.min+egui::vec2(q.x as f32,q.y as f32)*zoom,egui::vec2(q.w as f32,q.h as f32)*zoom).contains(pos)}));let retained=window_has_focus&&r.lost_focus()&&(blank||pointer_position.is_some_and(|pos|r.rect.contains(pos)));outside=!first&&r.lost_focus()&&!retained;if retained{r.request_focus();}if blank{app.scene.sheets[index].selected_item=None;}if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-inline-text-rect"),r.rect));}});
 });
 if cancel||enter||outside{if let Err(e)=app.finish_composer_text_edit(!cancel){app.status_message=e;}}true
}
fn t_rect(sheet:&ingetrazo_core::composition::Sheet,t:&TextItem)->(f64,f64,f64,f64){let w=sheet.native_items.get(&t.id).and_then(|v|v.get("w_mm")).and_then(|v|v.as_f64()).unwrap_or(80.);(t.pos.0,t.pos.1,w,(t.font_size_pt*25.4/72.*1.4*(t.text.matches('\n').count()+1)as f64).max(6.))}
