// SPDX-License-Identifier: GPL-3.0-or-later
//! Cold source RGBA8 uploads bypass Color32 conversion. Native managed textures
//! own descriptor/fence lifetime; this queue retains bytes until upload succeeds.
use std::{collections::HashMap,sync::{Arc,Mutex}};
use egui::{Context,TextureHandle,TextureId};
#[derive(Clone)]pub(crate) struct Image{pub size:[usize;2],pub pixels:Arc<Vec<u8>>}
#[derive(Default)]struct Pending{images:HashMap<TextureId,Image>,bytes:usize,published:u64,published_bytes:u64,acknowledged:u64}
#[derive(Clone,Default)]struct Uploads(Arc<Mutex<Pending>>);
fn key()->egui::Id{egui::Id::new("source-paper-encoded-rgba8")}
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
pub(crate)fn peek(ctx:&Context,id:TextureId)->Option<Image>{provider(ctx).ok()?.0.lock().ok()?.images.get(&id).cloned()}
pub(crate)fn acknowledge(ctx:&Context,id:TextureId){if let Ok(provider)=provider(ctx){if let Ok(mut pending)=provider.0.lock(){if let Some(image)=pending.images.remove(&id){pending.bytes-=image.pixels.len();pending.acknowledged+=1;}}}}

#[cfg(test)]mod tests{use super::*;
 #[test]fn bytes_shared_until_success_and_refusal_does_not_register_texture(){
  let ctx=Context::default();assert!(create(&ctx,[1,1],vec![0,0,0,0]).is_err());install(&ctx);
  let allocated=ctx.tex_manager().read().num_allocated();let before=provider(&ctx).unwrap();
  assert!(create(&ctx,[2,2],vec![0;4]).is_err());assert_eq!(ctx.tex_manager().read().num_allocated(),allocated);assert!(before.0.lock().unwrap().images.is_empty());
  let handle=create(&ctx,[1,1],vec![8,9,12,68]).unwrap();let first=peek(&ctx,handle.id()).unwrap();let second=peek(&ctx,handle.id()).unwrap();assert!(Arc::ptr_eq(&first.pixels,&second.pixels));assert_eq!(&**first.pixels,&[8,9,12,68]);assert_eq!(before.0.lock().unwrap().bytes,4);
  acknowledge(&ctx,handle.id());assert!(peek(&ctx,handle.id()).is_none());assert_eq!(before.0.lock().unwrap().bytes,0);assert_eq!(&**first.pixels,&[8,9,12,68]);
 }
}

pub(crate)fn stats(ctx:&Context)->Option<[u64;5]>{let provider=provider(ctx).ok()?;let p=provider.0.lock().ok()?;Some([p.images.len()as u64,p.bytes as u64,p.published,p.published_bytes,p.acknowledged])}
