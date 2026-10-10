//! Cold engineering controls: actual opaque batch order and unordered refusal.
use std::sync::Arc;
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch,EncodedRgba,Pose};
use ingetrazo_gui::native_vulkan::{App,System,cad2d::Handle};
struct Fixture{paint:Handle}
impl App for Fixture{
 fn title()->&'static str{"IngeTrazo opaque CAD ordering fixture"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{
  let p=unsafe{system.vulkan_context.instance.get_physical_device_properties(system.vulkan_context.physical_device)};
  let name=unsafe{std::ffi::CStr::from_ptr(p.device_name.as_ptr())}.to_string_lossy();
  eprintln!("Renderer device: {}",serde_json::json!({"name":name,"vendor_id":p.vendor_id,"device_id":p.device_id,"device_type":format!("{:?}",p.device_type)}));
  let case=std::env::var("INGETRAZO_ALPHA_CASE").unwrap_or("red-blue".into());
  let mut vertices=Vec::new();for [x,y]in [[20.,20.],[60.,60.]]{for position in [[x,y],[x+80.,y],[x+80.,y+80.],[x,y+80.]]{vertices.push(Vertex{position,coverage:1.});}}
  if case=="fractional-coverage"{vertices[0].coverage=0.5;}
  let geometry=Geometry::new(vertices,vec![0,1,2,0,2,3,4,5,6,4,6,7]).unwrap();
  let mut colors=if case=="blue-red"{[[0,0,255,255],[255,0,0,255]]}else{[[255,0,0,255],[0,0,255,255]]};
  if case=="fractional-alpha"{colors[0][3]=128;}
  let batches:Arc<[Batch]>=colors.into_iter().enumerate().map(|(i,c)|Batch{first:i as u32*6,count:6,color:EncodedRgba::from_bytes(c,1.).unwrap()}).collect::<Vec<_>>().into();
  Self{paint:system.enable_cad2d().unwrap().handle(geometry,batches,Pose::default()).unwrap()}
 }
 fn build_ui(&mut self,ctx:&egui::Context){egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::WHITE)).show(ctx,|ui|{ui.painter().add(self.paint.shape(ui.max_rect()));});if ctx.input(|i|i.viewport().close_requested()){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
 fn gpu_error(&mut self,error:&str){panic!("OPAQUE_ADMISSION_REFUSAL: {error}");}
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
