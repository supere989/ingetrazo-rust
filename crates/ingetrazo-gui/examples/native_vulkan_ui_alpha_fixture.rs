use ingetrazo_gui::native_vulkan::{App,System,VulkanContext};
use serde_json::Value;
struct Fixture { cases:Vec<Value>, textures:Vec<Option<egui::TextureHandle>>, partial:bool }
fn rgba(v:&Value)->egui::Color32 { let a=v.as_array().unwrap(); egui::Color32::from_rgba_unmultiplied(a[0].as_u64().unwrap() as u8,a[1].as_u64().unwrap() as u8,a[2].as_u64().unwrap() as u8,a[3].as_u64().unwrap() as u8) }
impl App for Fixture {
fn title()->&'static str { "IngeTrazo UI alpha acceptance" }
fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self {
 let input:Value=serde_json::from_slice(&std::fs::read(std::env::var("INGETRAZO_UI_ALPHA_INPUT").unwrap()).unwrap()).unwrap();
 let cases=input["cases"].as_array().unwrap().clone(); assert_eq!(cases.len(),12);
 let textures=cases.iter().enumerate().map(|(i,c)|c.get("pixels").map(|pixels|{let bytes:Vec<u8>=pixels.as_array().unwrap().iter().flat_map(|p|p.as_array().unwrap().iter().map(|n|n.as_u64().unwrap() as u8)).collect(); system.egui_ctx.load_texture(format!("case{i}"),egui::ColorImage::from_rgba_unmultiplied([2,2],&bytes),egui::TextureOptions::NEAREST)})).collect();
 Self{cases,textures,partial:false}
}
fn build_ui(&mut self,ctx:&egui::Context){
 ctx.set_pixels_per_point(1.0);
 let want=std::env::var("INGETRAZO_UI_ALPHA_COMMAND").ok().is_some_and(|p|std::fs::read_to_string(p).unwrap_or_default().trim()=="partial");
 if want&&!self.partial {for (c,t) in self.cases.iter().zip(&mut self.textures){if let Some(p)=c.get("partial"){t.as_mut().unwrap().set_partial([0,0],egui::ColorImage::new([1,1],rgba(p)),egui::TextureOptions::NEAREST);}} self.partial=true;}
 let painter=ctx.layer_painter(egui::LayerId::new(egui::Order::Background,egui::Id::new("alpha")));
 painter.rect_filled(ctx.screen_rect(),0.,egui::Color32::from_gray(180));
 for(c,t)in self.cases.iter().zip(&self.textures){let r=c["rect"].as_array().unwrap();let rect=egui::Rect::from_min_size(egui::pos2(r[0].as_f64().unwrap()as f32,r[1].as_f64().unwrap()as f32),egui::vec2(150.,150.));let b=c["background"].as_array().unwrap();painter.rect_filled(rect,0.,egui::Color32::from_rgb(b[0].as_u64().unwrap()as u8,b[1].as_u64().unwrap()as u8,b[2].as_u64().unwrap()as u8));
 if let Some(t)=t {let alpha=c.get("tint_alpha").and_then(Value::as_u64).unwrap_or(255)as u8;painter.image(t.id(),rect,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),egui::Color32::from_rgba_unmultiplied(255,255,255,alpha));}else{painter.rect_filled(rect,0.,rgba(&c["color"]));}}
 if let Ok(p)=std::env::var("INGETRAZO_UI_ALPHA_STATE"){std::fs::write(p,serde_json::to_vec(&serde_json::json!({"phase":if self.partial{"partial"}else{"baseline"},"red128_color32":egui::Color32::from_rgba_unmultiplied(255,0,0,128).to_array()})).unwrap()).unwrap();} ctx.request_repaint();
}
fn clean(&mut self,_:&VulkanContext){}
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
