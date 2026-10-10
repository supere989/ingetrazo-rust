//! Two real devices; foreign-owner calls are rejected before Vulkan handle use.
#[cfg(not(target_os="macos"))]
fn main()->Result<(),Box<dyn std::error::Error>> {
 use ash::vk;
 use ingetrazo_gui::native_vulkan::{VulkanContext,budget::VulkanBudget,headless::DeviceRequest,resources::{Resources,TextureImage}};
 use std::sync::Arc;
 let request=if std::env::args().any(|s|s=="--software-fixture"){DeviceRequest::software_fixture()}else{DeviceRequest::nvidia_hardware()};
 let a=VulkanContext::new_headless(&request)?;
 let b=VulkanContext::new_headless(&request)?;
 let ba=Arc::new(VulkanBudget::new(&a.instance,a.physical_device)?);
 let bb=Arc::new(VulkanBudget::new(&b.instance,b.physical_device)?);
 let mut ra=Resources::new(&a,ba.clone());let mut rb=Resources::new(&b,bb.clone());
 let pixels=[19u8,73,151,255,201,13,7,255];let extent=vk::Extent2D{width:2,height:1};
 let ta=TextureImage::rgba8(&a,&mut ra,extent,&pixels,false,true)?;
 let tb=TextureImage::rgba8(&b,&mut rb,extent,&pixels,false,true)?;
 let data=[37u8;16];let buffer=ra.upload(&a,&data,vk::BufferUsageFlags::TRANSFER_SRC,true)?;
 assert_eq!(ra.readback_buffer(&a,buffer)?,data);
 assert_eq!(ra.readback_texture_mip(&a,&ta,0)?,pixels);
 let before=serde_json::to_value(&ra.stats)?;
 let charged=ba.ledger.lock().unwrap().total_committed();
 // These execute only the corrected admission path, never the old foreign handles.
 assert_eq!(TextureImage::rgba8(&b,&mut ra,extent,&pixels,false,true).err().as_deref(),Some("Upload context differs from resource owner"));
 assert_eq!(ra.readback_buffer(&b,buffer).err().as_deref(),Some("Upload context differs from resource owner"));
 assert_eq!(ra.readback_texture_mip(&b,&ta,0).err().as_deref(),Some("Upload context differs from resource owner"));
 assert_eq!(ra.readback_texture_mip(&a,&tb,0).err().as_deref(),Some("Texture differs from resource owner"));
 assert_eq!(charged,ba.ledger.lock().unwrap().total_committed());
 assert_eq!(before,serde_json::to_value(&ra.stats)?,"Rejected calls allocated or transferred resources");
 assert_eq!(ra.readback_buffer(&a,buffer)?,data);
 assert_eq!(ra.readback_texture_mip(&a,&ta,0)?,pixels);
 assert_eq!(rb.readback_texture_mip(&b,&tb,0)?,pixels);
 drop(ta);drop(tb);drop(ra);drop(rb);
 assert_eq!(ba.ledger.lock().unwrap().total_committed(),0);
 assert_eq!(bb.ledger.lock().unwrap().total_committed(),0);
 println!("ownership PASS: two contexts, four early refusals, valid bytes preserved, final ledgers zero");Ok(())
}
#[cfg(target_os="macos")]
fn main(){eprintln!("Vulkan ownership diagnostic is Linux/Windows only");}
