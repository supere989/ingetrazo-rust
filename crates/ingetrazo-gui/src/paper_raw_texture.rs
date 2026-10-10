// SPDX-License-Identifier: GPL-3.0-or-later
//! Cold source RGBA8 uploads bypass Color32 conversion. Native managed textures
//! own descriptor/fence lifetime; this queue retains bytes until upload succeeds.
use std::{collections::HashMap,sync::{Arc,Mutex}};
use egui::{Context,TextureHandle,TextureId};
#[derive(Clone)]pub(crate) struct Image{pub size:[usize;2],pub pixels:Arc<Vec<u8>>}
#[derive(Default)]struct Pending{images:HashMap<TextureId,Image>,bytes:usize,published:u64,published_bytes:u64,acknowledged:u64}
#[derive(Clone,Default)]struct Uploads(Arc<Mutex<Pending>>);
fn key()->egui::Id{egui::Id::new("source-paper-encoded-rgba8")}
/// Backend capability, distinct from runtime queue health.
pub(crate)fn installed(ctx:&Context)->bool{ctx.data(|d|d.get_temp::<Uploads>(key()).is_some())}
pub(crate)fn install(ctx:&Context){ctx.data_mut(|d|d.insert_temp(key(),Uploads::default()));}
fn provider(ctx:&Context)->Result<Uploads,String>{ctx.data(|d|d.get_temp(key())).ok_or_else(||"Native source RGBA8 texture provider unavailable".into())}
pub(crate)fn create(ctx:&Context,size:[usize;2],pixels:Vec<u8>)->Result<TextureHandle,String>{
 let count=size[0].checked_mul(size[1]).and_then(|n|n.checked_mul(4)).filter(|&n|n<=16*1024*1024).ok_or("Source texture dimensions exceeded")?;
 if size.iter().any(|&n|n==0||n>4096)||pixels.len()!=count{return Err("Invalid source texture dimensions".into());}
 let provider=provider(ctx)?;let mut pending=provider.0.lock().map_err(|_|"Source texture queue unavailable")?;
 if pending.images.len()>=4096||pending.bytes.checked_add(count).is_none_or(|n|n>32*1024*1024){return Err("Source texture upload queue budget exceeded".into());}
 pending.images.try_reserve(1).map_err(|_|"Source texture queue allocation failed")?;
 // Four-byte logical registration only: raw dimensions/payload are supplied to
 // the managed uploader at this ID. Caller paints using actual bitmap bounds.
 let texture=ctx.load_texture("source-paper-rgba8",egui::ColorImage::new([1,1],egui::Color32::TRANSPARENT),egui::TextureOptions::NEAREST);
 pending.images.insert(texture.id(),Image{size,pixels:Arc::new(pixels)});pending.bytes+=count;pending.published+=1;pending.published_bytes+=count as u64;Ok(texture)
}
pub(crate)fn peek(ctx:&Context,id:TextureId)->Result<Option<Image>,String>{let provider=provider(ctx)?;let pending=provider.0.lock().map_err(|_|"Source texture queue unavailable")?;Ok(pending.images.get(&id).cloned())}
pub(crate)fn acknowledge(ctx:&Context,id:TextureId)->Result<(),String>{let provider=provider(ctx)?;let mut pending=provider.0.lock().map_err(|_|"Source texture queue unavailable")?;if let Some(image)=pending.images.remove(&id){pending.bytes-=image.pixels.len();pending.acknowledged+=1;}Ok(())}

/// Retire unsubmitted raw bytes and queued sets together when egui releases an ID.
/// Submitted GPU textures remain on the host's existing post-fence free path.
pub(crate)fn cancel(ctx:&Context,ids:&[TextureId],sets:&mut std::collections::VecDeque<(TextureId,egui::epaint::ImageDelta)>)->Result<(),String>{
 if ids.is_empty(){return Ok(());}
 let mut released=std::collections::HashSet::new();released.try_reserve(ids.len()).map_err(|_|"Source texture release identity admission failed")?;released.extend(ids.iter().copied());
 let provider=provider(ctx)?;let mut pending=provider.0.lock().map_err(|_|"Source texture queue unavailable")?;
 for id in &released{if let Some(image)=pending.images.remove(id){pending.bytes-=image.pixels.len();}}
 sets.retain(|(id,_)|!released.contains(id));Ok(())
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn same_frame_set_paint_free_remains_available_until_retirement(){
  let ctx=Context::default();install(&ctx);ctx.begin_pass(egui::RawInput::default());let handle=create(&ctx,[1,1],vec![0,0,0,255]).unwrap();let id=handle.id();ctx.layer_painter(egui::LayerId::background()).image(id,egui::Rect::from_min_size(egui::Pos2::new(1.,1.),egui::Vec2::splat(10.)),egui::Rect::from_min_max(egui::Pos2::ZERO,egui::Pos2::new(1.,1.)),egui::Color32::WHITE);drop(handle);let frame=ctx.end_pass();assert!(frame.textures_delta.set.iter().any(|(key,_)|*key==id));assert!(frame.textures_delta.free.contains(&id));assert!(!frame.shapes.is_empty());assert!(peek(&ctx,id).unwrap().is_some());let mut sets=std::collections::VecDeque::from(frame.textures_delta.set);cancel(&ctx,&frame.textures_delta.free,&mut sets).unwrap();assert!(!sets.iter().any(|(key,_)|*key==id));assert!(peek(&ctx,id).unwrap().is_none());
 }
 #[test]fn failed_set_then_drop_retires_bytes_and_retry_identity(){
  let ctx=Context::default();install(&ctx);let handle=create(&ctx,[1,1],vec![0;4]).unwrap();let id=handle.id();let frame=ctx.tex_manager().write().take_delta();let mut sets=std::collections::VecDeque::from(frame.set);assert!(peek(&ctx,id).unwrap().is_some());drop(handle);let freed=ctx.tex_manager().write().take_delta().free;assert_eq!(freed,vec![id]);cancel(&ctx,&freed,&mut sets).unwrap();assert!(!sets.iter().any(|(key,_)|*key==id));assert!(peek(&ctx,id).unwrap().is_none());assert_eq!(provider(&ctx).unwrap().0.lock().unwrap().bytes,0);
 }
 #[test]fn unavailable_queue_is_explicit_and_preserves_payload(){
  let ctx=Context::default();assert!(peek(&ctx,TextureId::Managed(1)).is_err());install(&ctx);let handle=create(&ctx,[1,1],vec![0;4]).unwrap();let queue=provider(&ctx).unwrap();let poisoned=queue.clone();let _=std::thread::spawn(move||{let _guard=poisoned.0.lock().unwrap();panic!("test queue refusal");}).join();assert!(peek(&ctx,handle.id()).is_err());assert!(acknowledge(&ctx,handle.id()).is_err());let pending=queue.0.lock().err().unwrap().into_inner();assert_eq!(pending.bytes,4);assert_eq!(pending.images.len(),1);
 }
 #[test]fn bytes_shared_until_success_and_refusal_does_not_register_texture(){
  let ctx=Context::default();assert!(create(&ctx,[1,1],vec![0,0,0,0]).is_err());install(&ctx);
  let allocated=ctx.tex_manager().read().num_allocated();let before=provider(&ctx).unwrap();
  assert!(create(&ctx,[2,2],vec![0;4]).is_err());assert_eq!(ctx.tex_manager().read().num_allocated(),allocated);assert!(before.0.lock().unwrap().images.is_empty());
  let handle=create(&ctx,[1,1],vec![8,9,12,68]).unwrap();let first=peek(&ctx,handle.id()).unwrap().unwrap();let second=peek(&ctx,handle.id()).unwrap().unwrap();assert!(Arc::ptr_eq(&first.pixels,&second.pixels));assert_eq!(&**first.pixels,&[8,9,12,68]);assert_eq!(before.0.lock().unwrap().bytes,4);
  acknowledge(&ctx,handle.id()).unwrap();assert!(peek(&ctx,handle.id()).unwrap().is_none());assert_eq!(before.0.lock().unwrap().bytes,0);assert_eq!(&**first.pixels,&[8,9,12,68]);
 }
}

pub(crate)fn stats(ctx:&Context)->Option<[u64;5]>{let provider=provider(ctx).ok()?;let p=provider.0.lock().ok()?;Some([p.images.len()as u64,p.bytes as u64,p.published,p.published_bytes,p.acknowledged])}
