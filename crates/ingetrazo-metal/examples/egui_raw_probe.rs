//! Direct encoded RGBA acceptance through the real native Metal texture owner.
#[cfg(target_os="macos")]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ingetrazo_metal::{NativeMetal,egui_painter::EguiPainter};
 use std::{sync::Arc,time::Duration,path::PathBuf};
 let out=std::env::args().find_map(|a|a.strip_prefix("--output=").map(PathBuf::from)).ok_or("--output=FRESH_DIR required")?;
 std::fs::create_dir_all(&out)?;if out.join("result.json").exists()||out.join("actual.rgba").exists(){return Err("stale artifacts refused".into());}
 let metal=NativeMetal::new()?;eprintln!("Raw Metal device={}",metal.info.name);let mut painter=EguiPainter::new(&metal)?;
 let id=egui::TextureId::Managed(901);
 let delta=egui::TexturesDelta{set:vec![(id,egui::epaint::ImageDelta::full(egui::ColorImage::new([1,1],egui::Color32::TRANSPARENT),egui::TextureOptions::NEAREST))],free:vec![]};
 let bytes=[8,9,12,68,1,2,3,255,0,0,0,0,127,0,0,127];
 let baseline=metal.budget.lock().unwrap().total_committed();
 for invalid in [vec![(id,[4,1],&bytes[..15])],vec![(id,[4,1],&bytes[..]),(id,[4,1],&bytes[..])],vec![(egui::TextureId::Managed(902),[4,1],&bytes[..])]]{
  assert!(painter.update_textures_encoded(&metal,&delta,&invalid).is_err());assert_eq!(metal.budget.lock().unwrap().total_committed(),baseline);
 }
 painter.update_textures_encoded(&metal,&delta,&[(id,[4,1],&bytes)])?;
 let scene:Vec<_>=(0..4).map(|i|{let x=i as f32*8.;let uv=egui::pos2((i as f32+0.5)/4.,0.5);egui::ClippedPrimitive{clip_rect:egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(32.,8.)),primitive:egui::epaint::Primitive::Mesh(egui::epaint::Mesh{vertices:[egui::pos2(x,0.),egui::pos2(x+8.,0.),egui::pos2(x+8.,8.),egui::pos2(x,8.)].into_iter().map(|pos|egui::epaint::Vertex{pos,uv,color:egui::Color32::WHITE}).collect(),indices:vec![0,1,2,0,2,3],texture_id:id})}}).collect();
 let paint=painter.prepare(&metal,&scene,1.)?;
 let render=|p:&EguiPainter|->Result<Vec<u8>,String>{p.submit_diagnostic(&metal,Arc::clone(&paint),32,8,false)?.wait_readback(Duration::from_secs(5))};
 let actual=render(&painter)?;let expected:Vec<u8>=(0..8).flat_map(|_|[[195,196,199,255],[1,2,3,255],[255,255,255,255],[255,128,128,255]].into_iter().flat_map(|c|c.repeat(8))).collect();
 std::fs::write(out.join("actual.rgba"),&actual)?;std::fs::write(out.join("expected.rgba"),&expected)?;assert_eq!(actual,expected,"raw bytes must bypass Color32 roundtrip");
 let committed=metal.budget.lock().unwrap().total_committed();let limit={let mut ledger=metal.budget.lock().unwrap();let old=ledger.total_limit;ledger.total_limit=Some(committed);old};
 let denied=painter.update_textures_encoded(&metal,&delta,&[(id,[4,1],&bytes)]);metal.budget.lock().unwrap().total_limit=limit;assert!(denied.is_err());assert_eq!(metal.budget.lock().unwrap().total_committed(),committed);assert_eq!(render(&painter)?,actual);
 let before=painter.stats();for _ in 0..3{assert_eq!(render(&painter)?,actual);}assert_eq!(painter.stats().texture_uploads,before.texture_uploads);
 painter.free_textures(&[id]);assert_eq!(render(&painter)?,actual,"prepared owner survives cache removal");drop(paint);drop(painter);assert_eq!(metal.budget.lock().unwrap().total_committed(),0);
 std::fs::write(out.join("result.json"),"{\"passed\":true,\"pixels\":256,\"physical_mac_claim\":false}")?;Ok(())
}
#[cfg(not(target_os="macos"))]fn main(){eprintln!("native macOS required");std::process::exit(2)}
