use ingetrazo_gui::{app::IngeTrazoApp,native_vulkan::{App,System}};
use ingetrazo_core::{composition::{Sheet,SheetItem,ShapeItem},sheet_shape::SheetShape};
struct Fixture{app:IngeTrazoApp,dir:std::path::PathBuf,current:usize,all:usize}
impl App for Fixture {
 fn title()->&'static str{"IngeTrazo actual PDF button acceptance"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{
 let mut app=IngeTrazoApp::new();app.set_language("en");app.native_composer.factory=Some(system.enable_cad2d().unwrap().factory().into());
 let shapes:Vec<SheetShape>=serde_json::from_slice(&std::fs::read(std::env::var("INGETRAZO_PDF_UI_RECIPES").unwrap()).unwrap()).unwrap();
 app.scene.sheets=shapes.into_iter().map(|shape|{let mut s=Sheet::default();s.items.clear();s.items.push(SheetItem::Shape(ShapeItem{id:uuid::Uuid::new_v4(),shape}));s}).collect();if std::env::var_os("INGETRAZO_PDF_UI_UNSUPPORTED").is_some(){app.scene.sheets=vec![Sheet::default()];}if let Ok(path)=std::env::var("INGETRAZO_PDF_UI_TEXT_OBSERVATIONS") {
 let source:serde_json::Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
 app.scene.sheets=source["cases"].as_array().unwrap().iter().map(|case|{
 let mut sheet=Sheet::new(case["name"].as_str().unwrap(),ingetrazo_core::composition::PaperSize::A4,ingetrazo_core::composition::Orientation::Portrait);sheet.items.clear();
 let call=&case["source_draw_calls"][0];let item=ingetrazo_core::composition::TextItem{id:uuid::Uuid::new_v4(),pos:(20.,30.),text:call["text"].as_str().or_else(||case["authored_text"].as_str()).unwrap().into(),font_size_pt:14.,bold:call["font"]["bold"].as_bool().or_else(||case["properties"]["bold"].as_bool()).unwrap_or(false)};
 sheet.native_items.insert(item.id,case["properties"].clone());sheet.items.push(SheetItem::Text(item));sheet}).collect();
 }
 app.scene.active_sheet_index=Some(0);
 Self{app,dir:std::env::var("INGETRAZO_PDF_UI_OUT").unwrap().into(),current:0,all:0}}
 fn build_ui(&mut self,ctx:&egui::Context){
 egui::CentralPanel::default().show(ctx,|ui|ingetrazo_gui::composer::render_sheet_composer(&mut self.app,ui,0));
 let observation=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Rect,bool,bool)>(egui::Id::new("pdf-ui-observation")));
 if let Some((a,b,current,all))=observation{self.current+=usize::from(current);self.all+=usize::from(all);let a=a.center();let b=b.center();let value=serde_json::json!({"current_button":[a.x,a.y],"all_button":[b.x,b.y],"current_clicks":self.current,"all_clicks":self.all,"status":self.app.status_message,"error_dialog":self.app.composer_state.pdf_export_error,"error_ok":ctx.data(|d|d.get_temp::<egui::Rect>(egui::Id::new("pdf-ui-error-ok"))).map(|r|[r.center().x,r.center().y]),"export_result":ctx.data(|d|d.get_temp::<String>(egui::Id::new("pdf-ui-result"))),"sheets":self.app.scene.sheets.len()});std::fs::write(self.dir.join("state.tmp"),value.to_string()).unwrap();std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();}
 ctx.request_repaint_after(std::time::Duration::from_millis(30));}
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
