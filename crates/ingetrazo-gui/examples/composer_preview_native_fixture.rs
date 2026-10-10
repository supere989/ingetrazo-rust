#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
use ingetrazo_gui::{app::IngeTrazoApp,native_vulkan::{App,System}};
use ingetrazo_core::composition::Sheet;
struct Fixture{app:IngeTrazoApp,dir:std::path::PathBuf,stats:serde_json::Value}
impl App for Fixture{
 fn title()->&'static str{"Independent composer direct preview"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{let mut app=IngeTrazoApp::new();app.native_composer.factory=Some(system.enable_cad2d().unwrap().factory().into());let mut sheet=Sheet::default();sheet.items.clear();app.scene.sheets=vec![sheet];app.scene.active_sheet_index=Some(0);app.composer_state.zoom=2.;Self{app,dir:std::env::var("INGETRAZO_PREVIEW_OUT").unwrap().into(),stats:serde_json::Value::Null}}
 fn prepare_gpu(&mut self,system:&mut System,_:bool){let s=system.enable_cad2d().unwrap().stats();self.stats=serde_json::json!({"uploads":s.uploads,"cache_hits":s.cache_hits,"draws":s.draws,"cached_bytes":s.cached_bytes});}
 fn build_ui(&mut self,ctx:&egui::Context){
 if ctx.input(|i|i.modifiers.ctrl&&i.key_pressed(egui::Key::Z)){self.app.undo();}if ctx.input(|i|i.modifiers.ctrl&&i.key_pressed(egui::Key::Y)){self.app.redo();}
 let mut button=egui::Pos2::ZERO;egui::TopBottomPanel::top("fixture-actions").show(ctx,|ui|{let r=ui.button("Place Rectangle");button=r.rect.center();if r.clicked(){self.app.composer_state.shape_tool=Some("rect".into());self.app.composer_state.shape_start=None;}});
 let mut paper=egui::Rect::NOTHING;egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{let(w,h)=self.app.scene.sheets[0].dimensions_mm();let size=egui::vec2(w as f32*2.,h as f32*2.);paper=egui::Rect::from_center_size(ui.max_rect().center(),size);ingetrazo_gui::composer::render_sheet_composer(&mut self.app,ui,0);});
 let history=self.app.composer_state.sheet_history.get(&self.app.scene.sheets[0].id);let value=serde_json::json!({"paper_mm":self.app.scene.sheets[0].dimensions_mm(),"tool":self.app.composer_state.shape_tool,"start":self.app.composer_state.shape_start.map(|p|[p.x,p.y]),"items":self.app.scene.sheets[0].items,"undo":history.map(|h|h.0.len()).unwrap_or(0),"redo":history.map(|h|h.1.len()).unwrap_or(0),"metrics":self.stats,"button":[button.x,button.y],"paper":[paper.left(),paper.top(),paper.right(),paper.bottom()]});std::fs::write(self.dir.join("state.tmp"),value.to_string()).unwrap();std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();ctx.request_repaint_after(std::time::Duration::from_millis(30));
 }
}
pub fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
