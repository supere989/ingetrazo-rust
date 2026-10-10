//! Actual native cached border callback; frozen recipe constructor + read-only observer.
#[cfg(any(target_os="linux",target_os="windows"))]
mod native{
 use ingetrazo_gui::native_vulkan::{App,System};
 use ingetrazo_core::composition::{Sheet,PaperSize,Orientation,SourcePaperBorder};
 struct Fixture{sheet:Sheet,cache:ingetrazo_gui::composer_border::CanvasBorder,factory:ingetrazo_gui::composer_backend::Factory,cases:Vec<serde_json::Value>,index:usize,out:std::path::PathBuf,stats:serde_json::Value,serial:u64}
 impl Fixture{fn set_case(&mut self){let c=&self.cases[self.index];self.sheet.margin_mm=c["margin"].as_f64().unwrap();self.sheet.set_source_border(SourcePaperBorder{on:true,mm:c["width"].as_f64().unwrap(),radius_mm:c["radius"].as_f64().unwrap(),style:c["style"].as_str().unwrap().into(),color:"#000000".into()}).unwrap();}}
 impl App for Fixture{
  fn title()->&'static str{"IngeTrazo source outer-border native acceptance"}
  fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{let mut cases:Vec<serde_json::Value>=serde_json::from_str(&std::fs::read_to_string(std::env::var("INGETRAZO_BORDER_RECIPES").unwrap()).unwrap()).unwrap();if let Ok(name)=std::env::var("INGETRAZO_BORDER_CASE_ONLY"){cases.retain(|c|c["name"].as_str()==Some(&name));assert_eq!(cases.len(),1);}let mut s=Self{sheet:Sheet::new("Border",PaperSize::A4,Orientation::Landscape),cache:Default::default(),factory:system.enable_cad2d().unwrap().factory().into(),cases,index:0,out:std::env::var("INGETRAZO_BORDER_OUT").unwrap().into(),stats:serde_json::Value::Null,serial:0};s.set_case();s}
  fn prepare_gpu(&mut self,system:&mut System,_:bool){let s=system.enable_cad2d().unwrap().stats();self.stats=serde_json::json!({"uploads":s.uploads,"cache_hits":s.cache_hits,"cached_bytes":s.cached_bytes,"draws":s.draws});self.serial=system.submitted_frames_for_probe();}
  fn build_ui(&mut self,ctx:&egui::Context){let mut next=egui::Pos2::ZERO;let mut close=next;egui::TopBottomPanel::top("controls").show(ctx,|ui|{ui.horizontal(|ui|{let b=ui.button("Next frozen border");next=b.rect.center();if b.clicked(){self.index=(self.index+1)%self.cases.len();self.set_case();}let b=ui.button("Finish");close=b.rect.center();if b.clicked(){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}ui.label(self.cases[self.index]["name"].as_str().unwrap());});});
   let zoom=self.cases[self.index]["density"].as_f64().unwrap()as f32;let origin=egui::pos2(20.37,60.23);egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{
    let(w,h)=self.sheet.dimensions_mm();let painter=ui.painter();painter.rect_filled(egui::Rect::from_min_size(origin,egui::vec2(w as f32*zoom,h as f32*zoom)),0.,egui::Color32::RED);
    assert!(self.cache.paint(painter,Some(&self.factory),&self.sheet,zoom,origin).unwrap());
   });
   let state=serde_json::json!({"index":self.index,"name":self.cases[self.index]["name"],"page":self.sheet.dimensions_mm(),"margin":self.sheet.margin_mm,"border":self.sheet.effective_source_border(),"origin":[f64::from(origin.x),f64::from(origin.y)],"zoom":f64::from(zoom),"ppp":f64::from(ctx.pixels_per_point()),"next":[next.x,next.y],"close":[close.x,close.y],"cold_builds":self.cache.cold_builds,"gpu":self.stats,"serial":self.serial});std::fs::write(self.out.join("state.tmp"),state.to_string()).unwrap();std::fs::rename(self.out.join("state.tmp"),self.out.join("state.json")).unwrap();ctx.request_repaint_after(std::time::Duration::from_millis(30));
  }
  fn gpu_error(&mut self,error:&str){panic!("Native border frame failure: {error}");}
 }
 pub fn run()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
}
#[cfg(any(target_os="linux",target_os="windows"))]fn main()->Result<(),Box<dyn std::error::Error>>{native::run()}
#[cfg(not(any(target_os="linux",target_os="windows")))]fn main(){eprintln!("Actual border Vulkan acceptance requires native Linux/Windows backend");}
