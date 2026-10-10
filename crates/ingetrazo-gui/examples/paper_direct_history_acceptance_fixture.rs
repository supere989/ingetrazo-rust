use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_core::composition::{Sheet,SheetItem,TextItem,PaperSize,Orientation};
fn snapshot(app:&IngeTrazoApp,label:&str)->serde_json::Value{
 let s=&app.scene.sheets[0];let counts=app.composer_state.sheet_history.get(&s.id).map(|(a,b)|[a.len(),b.len()]).unwrap_or([0,0]);let border=s.effective_source_border();
 serde_json::json!({"label":label,"undo":counts[0],"redo":counts[1],"positions":s.items.iter().filter_map(|i|if let SheetItem::Text(t)=i{Some([t.pos.0,t.pos.1])}else{None}).collect::<Vec<_>>(),"paper":format!("{:?}",s.paper_size),"landscape":s.orientation==Orientation::Landscape,"border":{"on":border.on,"color":border.color,"radius_mm":border.radius_mm}})
}
fn main(){
 let mut app=IngeTrazoApp::new();let mut sheet=Sheet::new("Source",PaperSize::A4,Orientation::Landscape);sheet.items.clear();app.scene.sheets=vec![sheet];app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);let mut rows=vec![];
 let mut b=app.scene.sheets[0].effective_source_border();b.color="#ffffff".into();app.scene.sheets[0].set_source_border(b).unwrap();app.undo();app.redo();rows.push(snapshot(&app,"empty_property_history"));
 app.begin_composer_edit();app.scene.sheets[0].items.push(SheetItem::Text(TextItem{id:uuid::Uuid::new_v4(),pos:(20.,30.),text:"A".into(),font_size_pt:14.,bold:false}));app.finish_composer_edit();app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(33.,44.);app.finish_composer_edit();
 let mut b=app.scene.sheets[0].effective_source_border();b.on=true;b.radius_mm=5.;app.scene.sheets[0].set_source_border(b).unwrap();app.scene.sheets[0].paper_size=PaperSize::A1;app.scene.sheets[0].orientation=Orientation::Portrait;app.undo();rows.push(snapshot(&app,"undo_item_preserves_properties"));
 let mut b=app.scene.sheets[0].effective_source_border();b.color="#ff0000".into();b.on=false;app.scene.sheets[0].set_source_border(b).unwrap();app.redo();rows.push(snapshot(&app,"direct_property_preserves_redo"));app.undo();app.undo();rows.push(snapshot(&app,"undo_all_items"));app.redo();app.redo();rows.push(snapshot(&app,"redo_all_items"));println!("{}",serde_json::json!({"rows":rows}));
}
