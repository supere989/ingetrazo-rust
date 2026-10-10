//! Generic egui coverage/sampling contract, not authored CAD alpha parity.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
 use ingetrazo_metal::{NativeMetal, egui_painter::EguiPainter};
 use std::{sync::Arc,time::Duration,path::PathBuf};
 let out=std::env::args().find_map(|a| a.strip_prefix("--output=").map(PathBuf::from)).ok_or("--output=FRESH_DIR required")?;
 std::fs::create_dir_all(&out)?;
 if out.join("contract.rgba").exists() || out.join("result.json").exists(){return Err("stale artifacts refused".into())}
 let metal=NativeMetal::new()?;
 eprintln!("Native Metal egui contract device={}; unified={}",metal.info.name,metal.info.unified_memory);
 let mut painter=EguiPainter::new(&metal)?;
 let mut delta=egui::TexturesDelta::default();
 for (i,coverage) in [0.,0.25,0.5,0.75,1.].into_iter().enumerate(){
  let mut f=egui::FontImage::new([1,1]); f.pixels[0]=coverage;
  delta.set.push((egui::TextureId::Managed(i as u64),egui::epaint::ImageDelta::full(f,egui::TextureOptions::LINEAR)));
 }
 delta.set.push((egui::TextureId::Managed(5),egui::epaint::ImageDelta::full(egui::ColorImage::new([1,1],egui::Color32::WHITE),egui::TextureOptions::NEAREST)));
 for (id,opt) in [(6,egui::TextureOptions::LINEAR),(7,egui::TextureOptions::NEAREST),(8,egui::TextureOptions::NEAREST_REPEAT),(9,egui::TextureOptions::NEAREST)] {
 delta.set.push((egui::TextureId::Managed(id),egui::epaint::ImageDelta::full(egui::ColorImage{size:[2,1],pixels:vec![egui::Color32::BLACK,egui::Color32::WHITE]},opt)));
 }
 painter.update_textures(&metal,&delta)?;
 let scene:Vec<_>=(0..10).map(|i|{
 let x=i as f32*8.; let color=if i==5 {egui::Color32::from_rgba_unmultiplied(0,0,0,128)} else if i<5{egui::Color32::BLACK}else{egui::Color32::WHITE};
 let uv=if i>=8{egui::pos2(1.25,0.5)}else{egui::pos2(0.5,0.5)};
 egui::ClippedPrimitive{clip_rect:egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(80.,8.)),primitive:egui::epaint::Primitive::Mesh(egui::epaint::Mesh{vertices:[egui::pos2(x,0.),egui::pos2(x+8.,0.),egui::pos2(x+8.,8.),egui::pos2(x,8.)].into_iter().map(|pos|egui::epaint::Vertex{pos,uv,color}).collect(),indices:vec![0,1,2,0,2,3],texture_id:egui::TextureId::Managed(i)})}
 }).collect();
 let paint=painter.prepare(&metal,&scene,1.)?;
 let before=painter.stats();
 let mut initial=None;
 for _ in 0..6 {
 let mut submission=painter.submit_diagnostic(&metal,Arc::clone(&paint),80,8,false)?;
 let rgba=submission.wait_readback(Duration::from_secs(5))?;
 if let Some(ref old)=initial{assert_eq!(&rgba,old)}else{std::fs::write(out.join("contract.rgba"),&rgba)?; initial=Some(rgba)}
 }
 let shades=[255,136,81,37,0,127,128,255,0,255];
 let expected:Vec<u8>=(0..8).flat_map(|_|shades.into_iter().flat_map(|v|[v,v,v,255].repeat(8))).collect();
 std::fs::write(out.join("expected.rgba"),&expected)?;
 assert_eq!(initial.unwrap(),expected,"strict fractional font/alpha/filter/wrap pixel contract");
 assert_eq!(painter.stats().geometry_uploads,before.geometry_uploads);
 assert_eq!(painter.stats().texture_uploads,before.texture_uploads);
 drop(paint);drop(painter);assert_eq!(metal.budget.lock().unwrap().total_committed(),0);
 std::fs::write(out.join("result.json"),"{\"passed\":true,\"cases\":10,\"warm_gpu_images\":5,\"physical_mac_claim\":false}")?;
 Ok(())
}
#[cfg(not(target_os="macos"))]
fn main(){eprintln!("native macOS required");std::process::exit(2)}
