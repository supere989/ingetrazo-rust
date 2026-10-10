use ingetrazo_gui::native_vulkan::{App,System};
use ingetrazo_core::sheet_shape::SheetShape;
struct Fixture {shapes:Vec<SheetShape>,out:std::path::PathBuf}
impl App for Fixture {
 fn title()->&'static str {"IngeTrazo physical sheet shape acceptance"}
 fn new(_: &mut System,_:Option<std::path::PathBuf>)->Self {let input=std::env::var("INGETRAZO_SHAPE_RECIPES").unwrap();Self {shapes:serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap(),out:std::env::var("INGETRAZO_SHAPE_OUT").unwrap().into()}}
 fn build_ui(&mut self,ctx:&egui::Context){let phase=std::fs::read_to_string(self.out.join("phase")).unwrap_or_else(|_|"0".into()).trim().parse::<usize>().unwrap();let shape=&self.shapes[phase];egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::WHITE)).show(ctx,|ui|{ingetrazo_gui::composer::paint_sheet_shape(ui.painter(),shape,8.,egui::pos2(50.-shape.x_mm as f32*8.,50.-shape.y_mm as f32*8.)).unwrap();});std::fs::write(self.out.join("state.tmp"),serde_json::json!({"phase":phase}).to_string()).unwrap();std::fs::rename(self.out.join("state.tmp"),self.out.join("state.json")).unwrap();ctx.request_repaint_after(std::time::Duration::from_millis(30));}
}
fn main()->Result<(),Box<dyn std::error::Error>> {ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
