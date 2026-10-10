//! Actual inline production precise CAD2D with UI primitives before and after.
use std::sync::Arc;
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch,EncodedRgba,Pose};
use ingetrazo_gui::native_vulkan::{App,System,cad2d::Handle};
struct FailOnce(std::sync::atomic::AtomicBool);
impl egui_ash_renderer::VulkanPaint for FailOnce {
 fn as_any(&self)->&dyn std::any::Any{self}
 fn paint(&self,_:&egui::PaintCallbackInfo,_:&ash::Device,_:ash::vk::CommandBuffer)->egui_ash_renderer::RendererResult<()>{if !self.0.swap(true,std::sync::atomic::Ordering::Relaxed){Err(egui_ash_renderer::RendererError::Init("engineering callback failure after prior commands".into()))}else{Ok(())}}
}
struct Fixture{stage:usize,geometry:Arc<Geometry>,uniform_updated:bool,paint:Handle,texture:egui::TextureHandle,error:Option<Arc<egui_ash_renderer::VulkanPaintCallback>>}
impl App for Fixture {
 fn title()->&'static str{"IngeTrazo precise CAD2D fixture"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{
  if std::env::var_os("INGETRAZO_CAD2D_PRESSURE").is_some(){system.verify_cad2d_rollback_for_probe().unwrap();}
  let geometry=Geometry::new_with_ordered_draws(vec![Vertex{position:[0.,0.],coverage:1.},Vertex{position:[80.,0.],coverage:1.},Vertex{position:[80.,80.],coverage:1.},Vertex{position:[0.,80.],coverage:1.}],vec![0,1,2,0,2,3],vec![0..6]).unwrap();
  let rgba=std::env::var("INGETRAZO_CAD2D_RGBA").unwrap_or("37,93,157,128".into()).split(',').map(|v|v.parse::<u8>().unwrap()).collect::<Vec<_>>();let rgba:[u8;4]=rgba.try_into().unwrap();
  let batches:Arc<[Batch]>=vec![Batch{first:0,count:6,color:EncodedRgba::from_bytes(rgba,1.).unwrap()}].into();
  let paint=system.enable_cad2d().unwrap().handle(geometry.clone(),batches,Pose{translation:[20.,20.],..Default::default()}).unwrap();
  let texture=system.egui_ctx.load_texture("callback-restoration",egui::ColorImage{size:[2,2],pixels:vec![egui::Color32::RED,egui::Color32::GREEN,egui::Color32::BLUE,egui::Color32::WHITE]},egui::TextureOptions::NEAREST);Self{stage:0,geometry,uniform_updated:false,paint,texture,error:std::env::var_os("INGETRAZO_CAD2D_CALLBACK_ERROR").map(|_|Arc::new(egui_ash_renderer::VulkanPaintCallback(Arc::new(FailOnce(Default::default())))))}
 }
 fn prepare_gpu(&mut self,system:&mut System,before_ui:bool){
  use ingetrazo_gui::native_vulkan::resize::Stage::*;
  const STAGES:[ingetrazo_gui::native_vulkan::resize::Stage;8]=[BeforeCreate,ImageView(1),RenderPass,Framebuffer(1),EguiPipeline,CadPipeline,Sync,BeforeCommit];
  if before_ui&&system.submitted_frames_for_probe()>=3&&self.stage<STAGES.len(){
   let stage=STAGES[self.stage];let audit=system.verify_resize_cancellation_for_probe(stage).unwrap();
   eprintln!("RESIZE_CANCELLATION {:?} published_owners_preserved=true retired_retry=true audit={}",stage,serde_json::to_string(&audit).unwrap());
   self.stage+=1;
   if self.stage==STAGES.len(){eprintln!("RESIZE_TRANSACTION_ALL_8_PASS");}
  }
 }
 fn build_ui(&mut self,ctx:&egui::Context){
  if self.stage<8{ctx.request_repaint_after(std::time::Duration::from_millis(30));}
  if std::env::var_os("INGETRAZO_CAD2D_UNIFORM_UPDATE").is_some()&&!self.uniform_updated{
   if ctx.input(|i|i.time)>0.5{let batches:Arc<[Batch]>=vec![Batch{first:0,count:6,color:EncodedRgba::from_bytes([255,0,0,255],1.).unwrap()}].into();self.paint.update(self.geometry.clone(),batches,Pose{translation:[60.,20.],..Default::default()}).unwrap();self.uniform_updated=true;}else{ctx.request_repaint_after(std::time::Duration::from_millis(100));}
  }
  if std::env::var_os("INGETRAZO_CAD2D_CALLBACK_ONLY").is_some(){ctx.layer_painter(egui::LayerId::background()).add(self.paint.shape(egui::Rect::from_min_size(egui::pos2(20.,20.),egui::vec2(80.,80.))));}
  else {egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::WHITE)).show(ctx,|ui|{
  let painter=ui.painter();painter.rect_filled(egui::Rect::from_min_size(egui::pos2(10.,10.),egui::vec2(100.,100.)),0.,egui::Color32::WHITE);
  painter.add(self.paint.shape(egui::Rect::from_min_size(egui::pos2(20.,20.),egui::vec2(80.,80.))));
  painter.image(self.texture.id(),egui::Rect::from_min_size(egui::pos2(120.,20.),egui::vec2(80.,80.)),egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),egui::Color32::WHITE);
  painter.rect_filled(egui::Rect::from_min_size(egui::pos2(30.,30.),egui::vec2(10.,10.)),0.,egui::Color32::BLACK);
 });}
 if let Some(error)=&self.error{ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground,egui::Id::new("injectedfailure"))).add(egui::PaintCallback{rect:ctx.screen_rect(),callback:error.clone()});}
 if ctx.input(|i|i.viewport().close_requested()){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
 fn gpu_error(&mut self,error:&str){if self.error.is_some(){eprintln!("EXPECTED callback error: {error}");}else{panic!("Precise CAD2D failed: {error}");}}
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
