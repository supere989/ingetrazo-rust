#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,resources::{Resources,TextureImage},texture_cache::decode_png_jpeg};
 let args:Vec<_>=std::env::args().collect();let input=std::path::Path::new(&args[1]);let original:serde_json::Value=serde_json::from_slice(&std::fs::read(input.join("original.json"))?)?;let output=std::path::Path::new(&args[2]);std::fs::create_dir_all(output)?;
 let context=VulkanContext::new_headless(&DeviceRequest::nvidia_hardware())?;let budget=std::sync::Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);let mut resources=Resources::new(&context,budget.clone());let mut rows=Vec::new();
 for case in original["cases"].as_array().ok_or("cases")?{
  let name=case["asset"].as_str().ok_or("asset")?;let encoded=std::fs::read(input.join(name))?;let mut decoded=decode_png_jpeg(&encoded,256*1024*1024)?;let stride=decoded.extent.width as usize*4;for y in 0..decoded.extent.height as usize/2{let(top,bottom)=decoded.rgba.split_at_mut((decoded.extent.height as usize-1-y)*stride);top[y*stride..(y+1)*stride].swap_with_slice(&mut bottom[..stride]);}
  let image=TextureImage::rgba8(&context,&mut resources,decoded.extent,&decoded.rgba,true,true)?;assert_eq!(image.mip_levels as usize,case["levels"].as_array().unwrap().len());let mut levels=Vec::new();
  for level in case["levels"].as_array().unwrap(){let index=level["level"].as_u64().unwrap()as u32;let file=level["file"].as_str().unwrap();let actual=resources.readback_texture_mip(&context,&image,index)?;let expected=std::fs::read(input.join(file))?;assert_eq!(actual.len(),expected.len());std::fs::write(output.join(file),&actual)?;levels.push(serde_json::json!({"level":index,"different_pixels":actual.chunks_exact(4).zip(expected.chunks_exact(4)).filter(|(a,b)|a!=b).count(),"max_channel_error":actual.iter().zip(&expected).map(|(a,b)|a.abs_diff(*b)).max().unwrap_or(0),"bytes":actual.len()}));}
  rows.push(serde_json::json!({"asset":name,"levels":levels}));
 }
 let stats=resources.stats;resources.release_image_staging();drop(resources);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);std::fs::write(output.join("native.json"),serde_json::to_vec_pretty(&serde_json::json!({"source":option_env!("INGETRAZO_BUILD_SOURCE"),"resources":stats,"cases":rows,"ledger_final":0,"scope":"production GPU mip upload/generation readback compared with actual pinned original; no screen pixel claim"}))?)?;Ok(())
}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("Native Vulkan diagnostic; macOS uses Metal.");std::process::exit(2);}
