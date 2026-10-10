// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct canvas glyph coverage from the shared physical paper text painter.
use std::{collections::{HashMap,VecDeque},sync::{Arc,Weak}};
use egui::{Color32,Pos2,Rect,Vec2,TextureHandle};
use ingetrazo_core::composition::{Sheet,TextItem};
use ingetrazo_formats::{paper_font::{FontKey,CachedFont},paper_text_painter::Painter,sheet_pdf::{Draw,PathOp}};
#[derive(Clone)]struct Layout {text:String,draws:Arc<Vec<Draw>>,bytes:usize,fonts:Vec<(FontKey,usize)>}
#[derive(Clone,Copy,PartialEq,Eq,Hash)]struct GlyphKey {font:FontKey,glyph:u32,scale:i64,phase:[u8;2],italic:bool,color:[u8;4]}
#[derive(Clone)]struct GlyphImage {pixels:Arc<Vec<u8>>,left:i32,top:i32,width:usize,height:usize,bytes:usize}
#[derive(Clone)]struct Tile{draws:Weak<Vec<Draw>>,density:u64,phase:[u64;2],offset:[i64;2],size:[usize;2],texture:Option<TextureHandle>,bytes:usize}
#[derive(Clone)]struct Sample{x:i64,y:i64,image:GlyphImage,clip:[i64;4]}
#[derive(Clone)]enum Ink{Image(TextureHandle,Rect,Rect,Color32),Fill(Rect,Rect,Color32),Line([Pos2;2],Rect,egui::Stroke)}
#[derive(Clone,Default)]pub(super) struct CanvasPainter {
 context:Option<(u64,u64,uuid::Uuid)>,layouts:HashMap<uuid::Uuid,Layout>,layout_bytes:usize,layout_order:VecDeque<uuid::Uuid>,font_refs:HashMap<FontKey,(usize,usize)>,font_bytes:usize,
 glyphs:HashMap<GlyphKey,GlyphImage>,order:VecDeque<GlyphKey>,glyph_bytes:usize,
 tiles:HashMap<uuid::Uuid,Tile>,tile_order:VecDeque<uuid::Uuid>,tile_bytes:usize,pub tile_builds:u64,
 ops:Vec<Ink>,frame_ops:usize,frame_textures:Vec<TextureHandle>,pub layout_builds:u64,pub raster_builds:u64,
}
impl std::fmt::Debug for CanvasPainter{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("CanvasPainter").field("stats",&self.stats()).finish()}}
impl CanvasPainter {
 pub fn stats(&self)->[u64;5]{[self.layouts.len()as u64,self.glyphs.len()as u64,self.glyph_bytes as u64,self.layout_builds,self.raster_builds]}
 pub fn begin_frame(&mut self){self.frame_textures.clear();self.ops.clear();self.frame_ops=0;}
 fn prepare(&mut self,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,resources:&mut Painter)->Result<Arc<Vec<Draw>>,String>{
  let context=(model,scene,sheet.id);if self.context!=Some(context){self.layouts.clear();self.layout_bytes=0;self.layout_order.clear();self.font_refs.clear();self.font_bytes=0;self.tiles.clear();self.tile_order.clear();self.tile_bytes=0;self.context=Some(context);}
  if let Some(entry)=self.layouts.get(&item.id){if entry.text==text{return Ok(entry.draws.clone());}}
  let expanded=TextItem{id:item.id,pos:item.pos,text:text.into(),font_size_pt:item.font_size_pt,bold:item.bold};let draws=super::pdf::text_draws(sheet,&expanded,resources,None)?;
  let bytes=draws.iter().try_fold(text.len(),|n,d|n.checked_add(d.path.len()*24+d.clip.as_ref().map_or(0,|p|p.len()*24)+d.text.as_ref().map_or(0,|t|t.glyphs.len()*24+t.decorations.len()*40))).ok_or("Canvas text layout size overflow")?;
  let mut fonts=Vec::new();let mut font_bytes=0usize;for d in &draws{if let Some(t)=&d.text{let key=t.font.key();if !fonts.iter().any(|(k,_)|*k==key){let bytes=t.font.shared_bytes().len();font_bytes=font_bytes.checked_add(bytes).ok_or("Canvas retained font size overflow")?;fonts.try_reserve(1).map_err(|_|"Canvas font identity allocation failed")?;fonts.push((key,bytes));}}}
  if bytes>32*1024*1024||font_bytes>32*1024*1024{return Err("Canvas text layout/font budget exceeded".into());}
  let mut authored=String::new();authored.try_reserve_exact(text.len()).map_err(|_|"Canvas text cache allocation failed")?;authored.push_str(text);
  self.layouts.try_reserve(1).map_err(|_|"Canvas text layout allocation failed")?;self.layout_order.try_reserve(1).map_err(|_|"Canvas text order allocation failed")?;self.font_refs.try_reserve(fonts.len()).map_err(|_|"Canvas font identity allocation failed")?;
  self.retire_layout(item.id);self.layout_order.retain(|&id|id!=item.id);
  loop{let additional=fonts.iter().filter(|(key,_)|!self.font_refs.contains_key(key)).map(|(_,bytes)|*bytes).sum::<usize>();if self.layout_bytes+bytes<=32*1024*1024&&self.font_bytes+additional<=32*1024*1024&&self.layouts.len()<10000{break;}let id=self.layout_order.pop_front().ok_or("Canvas layout cache invariant failed")?;self.retire_layout(id);}
  for &(key,bytes)in &fonts{let entry=self.font_refs.entry(key).or_insert_with(||{self.font_bytes+=bytes;(0,bytes)});entry.0+=1;}
  let draws=Arc::new(draws);self.layouts.insert(item.id,Layout{text:authored,draws:draws.clone(),bytes,fonts});self.layout_order.push_back(item.id);self.layout_bytes+=bytes;self.layout_builds+=1;Ok(draws)
 }
 fn retire_layout(&mut self,id:uuid::Uuid){if let Some(layout)=self.layouts.remove(&id){self.layout_bytes-=layout.bytes;for (key,_)in layout.fonts{if let Some(entry)=self.font_refs.get_mut(&key){entry.0-=1;if entry.0==0{let (_,bytes)=self.font_refs.remove(&key).unwrap();self.font_bytes-=bytes;}}}}}

 fn image(&mut self,font:&CachedFont,glyph:u32,scale:f64,phase:[u8;2],italic:bool,color:Color32)->Result<GlyphImage,String>{
  let key=GlyphKey{font:font.key(),glyph,scale:(scale*65536.)as i64,phase,italic,color:color.to_array()};
  if let Some(image)=self.glyphs.get(&key){return Ok(image.clone());}
  let bitmap=font.rasterize_glyph(glyph,scale,phase,italic)?;let bytes=bitmap.coverage.len().checked_mul(4).ok_or("Canvas glyph texture size overflow")?;
  if bytes>16*1024*1024{return Err("Canvas glyph texture budget exceeded".into());}
  self.glyphs.try_reserve(1).map_err(|_|"Canvas glyph cache allocation failed")?;self.order.try_reserve(1).map_err(|_|"Canvas glyph cache allocation failed")?;
  let mut pixels=Vec::new();pixels.try_reserve_exact(bytes).map_err(|_|"Canvas glyph coverage allocation failed")?;
  let [r,g,b,a]=color.to_srgba_unmultiplied();
  for &coverage in &bitmap.coverage{let alpha=byte_mul(coverage,a);pixels.extend_from_slice(&[byte_mul(r,alpha),byte_mul(g,alpha),byte_mul(b,alpha),alpha]);}
  let pixels=Arc::new(pixels);
  while self.glyphs.len()>=4096||self.glyph_bytes+bytes>32*1024*1024{let old=self.order.pop_front().ok_or("Canvas glyph cache invariant failed")?;let retired=self.glyphs.remove(&old).ok_or("Canvas glyph cache identity failed")?;self.glyph_bytes-=retired.bytes;}
  let image=GlyphImage{pixels,left:bitmap.left,top:bitmap.top,width:bitmap.width,height:bitmap.height,bytes};self.glyphs.insert(key,image.clone());self.order.push_back(key);self.glyph_bytes+=bytes;self.raster_builds+=1;Ok(image)
 }
 fn tile(&mut self,painter:&egui::Painter,draws:&Arc<Vec<Draw>>,item:&TextItem,zoom:f32,origin:Pos2)->Result<Option<(TextureHandle,Rect)>,String>{
  let ppp=painter.ctx().pixels_per_point()as f64;let density=f64::from(zoom)*ppp;
  let base=origin+Vec2::new(item.pos.0 as f32*zoom,item.pos.1 as f32*zoom);let physical=[f64::from(base.x)*ppp,f64::from(base.y)*ppp];let anchor=[physical[0].floor()as i64,physical[1].floor()as i64];let phase=[physical[0].fract().to_bits(),physical[1].fract().to_bits()];
  if let Some(tile)=self.tiles.get(&item.id){if tile.density==density.to_bits()&&tile.phase==phase&&tile.draws.upgrade().is_some_and(|old|Arc::ptr_eq(&old,draws)){return Ok(tile_image(tile,anchor,ppp));}}
  let mut samples=Vec::new();let count=draws.iter().map(|d|d.text.as_ref().map_or(0,|t|t.glyphs.len())).sum();samples.try_reserve(count).map_err(|_|"Canvas glyph sample allocation failed")?;let mut working_bytes=0usize;let mut extent:Option<[i64;4]>=None;
  for draw in draws.iter(){let Some(text)=&draw.text else{continue;};let pos=origin+Vec2::new(draw.origin_mm[0]as f32*zoom,draw.origin_mm[1]as f32*zoom);let scale=text.scale_mm*density;let rgba=draw.fill.unwrap_or([0.,0.,0.,1.]);let color=Color32::from_rgba_unmultiplied((rgba[0]*255.).round()as u8,(rgba[1]*255.).round()as u8,(rgba[2]*255.).round()as u8,(rgba[3]*255.).round()as u8);
   let mut clip=[i64::MIN/2,i64::MIN/2,i64::MAX/2,i64::MAX/2];if let Some(path)=&draw.clip{let mut min=[f64::INFINITY;2];let mut max=[f64::NEG_INFINITY;2];for op in path{if let PathOp::Move(p)|PathOp::Line(p)=op{for axis in 0..2{let value=if axis==0{f64::from(pos.x)}else{f64::from(pos.y)}*ppp+p[axis]*density;min[axis]=min[axis].min(value);max[axis]=max[axis].max(value);}}}clip=[min[0].floor()as i64,min[1].floor()as i64,max[0].ceil()as i64,max[1].ceil()as i64];}
   for glyph in &text.glyphs{let x=f64::from(pos.x)*ppp+glyph.position_px[0]*scale;let y=f64::from(pos.y)*ppp+glyph.position_px[1]*scale;let(fx,fy,phase)=device_position((x*64.)as i64,(y*64.)as i64);let image=self.image(&text.font,glyph.glyph,scale,phase,text.italic,color)?;
    working_bytes=working_bytes.checked_add(image.bytes).filter(|&n|n<=32*1024*1024).ok_or("Canvas glyph working budget exceeded")?;
    let x=fx as i64+i64::from(image.left);let y=fy as i64-i64::from(image.top);let bounds=[x.max(clip[0]),y.max(clip[1]),(x+image.width as i64).min(clip[2]),(y+image.height as i64).min(clip[3])];if bounds[0]>=bounds[2]||bounds[1]>=bounds[3]{continue;}
    extent=Some(if let Some(old)=extent{[old[0].min(bounds[0]),old[1].min(bounds[1]),old[2].max(bounds[2]),old[3].max(bounds[3])]}else{bounds});samples.push(Sample{x,y,image,clip});
   }
  }
  let (size,offset,pixels)=if let Some(extent)=extent{let width=usize::try_from(extent[2]-extent[0]).map_err(|_|"Canvas tile dimension invalid")?;let height=usize::try_from(extent[3]-extent[1]).map_err(|_|"Canvas tile dimension invalid")?;let bytes=width.checked_mul(height).and_then(|n|n.checked_mul(4)).filter(|&n|n<=16*1024*1024).ok_or("Canvas tile coverage budget exceeded")?;if width>4096||height>4096{return Err("Canvas tile dimensions exceeded".into());}let mut pixels=Vec::new();pixels.try_reserve_exact(bytes).map_err(|_|"Canvas tile allocation failed")?;pixels.resize(bytes,0);
   for sample in samples{for row in 0..sample.image.height{let y=sample.y+row as i64;if y<sample.clip[1]||y>=sample.clip[3]{continue;}for col in 0..sample.image.width{let x=sample.x+col as i64;if x<sample.clip[0]||x>=sample.clip[2]{continue;}let src=&sample.image.pixels[(row*sample.image.width+col)*4..][..4];let dst=&mut pixels[((y-extent[1])as usize*width+(x-extent[0])as usize)*4..][..4];source_over(dst,src);}}}
   ([width,height],[extent[0]-anchor[0],extent[1]-anchor[1]],pixels)
  }else{([0,0],[0,0],Vec::new())};
  self.tiles.try_reserve(1).map_err(|_|"Canvas tile cache allocation failed")?;self.tile_order.try_reserve(1).map_err(|_|"Canvas tile order allocation failed")?;
  let bytes=pixels.len();let texture=if bytes==0{None}else{Some(crate::paper_raw_texture::create(painter.ctx(),size,pixels)?)};
  if let Some(old)=self.tiles.remove(&item.id){self.tile_bytes-=old.bytes;}self.tile_order.retain(|&id|id!=item.id);while self.tiles.len()>=10000||self.tile_bytes+bytes>32*1024*1024{let id=self.tile_order.pop_front().ok_or("Canvas tile cache invariant failed")?;if let Some(old)=self.tiles.remove(&id){self.tile_bytes-=old.bytes;}}
  let tile=Tile{draws:Arc::downgrade(draws),density:density.to_bits(),phase,offset,size,texture,bytes};let result=tile_image(&tile,anchor,ppp);self.tiles.insert(item.id,tile);self.tile_order.push_back(item.id);self.tile_bytes+=bytes;self.tile_builds+=1;Ok(result)
 }
 pub fn paint(&mut self,painter:&egui::Painter,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,zoom:f32,origin:Pos2,resources:&mut Painter)->Result<Rect,String>{
  self.ops.clear();if !zoom.is_finite()||zoom<=0.||!origin.x.is_finite()||!origin.y.is_finite(){return Err("Invalid canvas text transform".into());}
  let draws=self.prepare(sheet,item,text,model,scene,resources)?;let count=draws.iter().map(|d|d.text.as_ref().map_or(usize::from(!d.path.is_empty()),|t|t.glyphs.len()+t.decorations.len())).sum::<usize>();if self.frame_ops.checked_add(count).is_none_or(|n|n>262144){return Err("Canvas glyph frame budget exceeded".into());}self.ops.try_reserve(count).map_err(|_|"Canvas text operation allocation failed")?;self.frame_textures.try_reserve(count).map_err(|_|"Canvas glyph lifetime allocation failed")?;
  let mut bounds=Rect::NOTHING;
  for draw in draws.iter(){let pos=origin+Vec2::new((draw.origin_mm[0]as f32)*zoom,(draw.origin_mm[1]as f32)*zoom);let rgba=draw.fill.unwrap_or([0.,0.,0.,1.]);let color=Color32::from_rgba_unmultiplied((rgba[0]*255.).round()as u8,(rgba[1]*255.).round()as u8,(rgba[2]*255.).round()as u8,(rgba[3]*255.).round()as u8);
   let mut ink=painter.clone();if let Some(clip)=&draw.clip{let mut rect=Rect::NOTHING;for op in clip{if let PathOp::Move(p)|PathOp::Line(p)=op{rect.extend_with(pos+Vec2::new(p[0]as f32*zoom,p[1]as f32*zoom));}}ink=ink.with_clip_rect(painter.clip_rect().intersect(rect));}
   if let Some(text)=&draw.text{
    if text.font.synthetic_bold_offset_px(text.bold)?>0.{return Err("Canvas synthetic-bold glyph raster remains unsupported".into());}
    for decoration in &text.decorations{let [x0,y0,x1,y1]=decoration.line_px;self.ops.push(Ink::Line([pos+Vec2::new((x0*text.scale_mm)as f32*zoom,(y0*text.scale_mm)as f32*zoom),pos+Vec2::new((x1*text.scale_mm)as f32*zoom,(y1*text.scale_mm)as f32*zoom)],ink.clip_rect(),egui::Stroke::new((decoration.width_px*text.scale_mm)as f32*zoom,color)));}
   }else if !draw.path.is_empty(){let mut rect=Rect::NOTHING;for op in &draw.path{if let PathOp::Move(p)|PathOp::Line(p)=op{rect.extend_with(pos+Vec2::new(p[0]as f32*zoom,p[1]as f32*zoom));}}self.ops.push(Ink::Fill(rect,ink.clip_rect(),color));bounds=bounds.union(rect);}
  }
  let tile=self.tile(painter,&draws,item,zoom,origin)?;if let Some((texture,rect))=tile{let index=self.ops.iter().position(|op|matches!(op,Ink::Line(..))).unwrap_or(self.ops.len());self.ops.insert(index,Ink::Image(texture,rect,painter.clip_rect(),Color32::WHITE));bounds=bounds.union(rect);}
  // Publish only after the complete item has valid layout, glyphs and resources.
  for op in self.ops.drain(..){match op{Ink::Image(texture,rect,clip,color)=>{painter.with_clip_rect(clip).image(texture.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),color);self.frame_textures.push(texture);},Ink::Fill(rect,clip,color)=>{painter.with_clip_rect(clip).rect_filled(rect,0.,color);},Ink::Line(line,clip,stroke)=>{painter.with_clip_rect(clip).line_segment(line,stroke);}}}self.frame_ops+=count;Ok(bounds)
 }
}

// Qt 6.12 raster painter: horizontal four-phase FT cache; without the
// VerticalSubpixelPositioning hint the baseline is rounded and y phase is zero.
fn device_position(x:i64,y:i64)->(f64,f64,[u8;2]){
 let phase=((x.rem_euclid(64)+1)/16)*16;
 (x.div_euclid(64)as f64+(phase/64)as f64,(y+32).div_euclid(64)as f64,[(phase%64)as u8,0])
}
#[cfg(test)]mod device_position_tests{use super::*;
 #[test]fn source_quarter_phases_and_rounded_baselines(){
  assert_eq!(device_position(14,31),(0.,0.,[0,0]));assert_eq!(device_position(15,32),(0.,1.,[16,0]));
  assert_eq!(device_position(63,63),(1.,1.,[0,0]));assert_eq!(device_position(-1,-33),(0.,-1.,[0,0]));
  assert_eq!(device_position(-64,-32),(-1.,0.,[0,0]));
 }
}

#[cfg(all(test,target_os="linux"))]mod layout_admission_tests{use super::*;
 #[test]fn warm_layout_and_refusal_keep_retained_font_identity(){
  let sheet=Sheet::default();let item=TextItem{id:uuid::Uuid::new_v4(),pos:(20.,30.),text:"hello".into(),font_size_pt:14.,bold:false};
  let mut resources=Painter::default();let mut cache=CanvasPainter::default();let first=cache.prepare(&sheet,&item,&item.text,1,1,&mut resources).unwrap();
  let counts=(cache.layout_builds,cache.font_bytes,cache.layout_bytes,cache.font_refs.len());
  let warm=cache.prepare(&sheet,&item,&item.text,1,1,&mut resources).unwrap();assert!(Arc::ptr_eq(&first,&warm));assert_eq!(counts,(cache.layout_builds,cache.font_bytes,cache.layout_bytes,cache.font_refs.len()));
  let huge="X".repeat(20*1024);assert!(cache.prepare(&sheet,&item,&huge,1,1,&mut resources).is_err());assert_eq!(counts,(cache.layout_builds,cache.font_bytes,cache.layout_bytes,cache.font_refs.len()));assert_eq!(cache.layout_order.iter().filter(|&&id|id==item.id).count(),1);
  let restored=cache.prepare(&sheet,&item,&item.text,1,1,&mut resources).unwrap();assert!(Arc::ptr_eq(&first,&restored));
  cache.retire_layout(item.id);assert_eq!((cache.layout_bytes,cache.font_bytes,cache.font_refs.len()),(0,0,0));
 }
}

fn byte_mul(color:u8,alpha:u8)->u8{let product=u16::from(color)*u16::from(alpha)+128;((product+(product>>8))>>8)as u8}

fn tile_image(tile:&Tile,anchor:[i64;2],ppp:f64)->Option<(TextureHandle,Rect)>{tile.texture.clone().map(|texture|(texture,Rect::from_min_size(Pos2::new(((anchor[0]+tile.offset[0])as f64/ppp)as f32,((anchor[1]+tile.offset[1])as f64/ppp)as f32),Vec2::new((tile.size[0]as f64/ppp)as f32,(tile.size[1]as f64/ppp)as f32))))}
fn source_over(dst:&mut [u8],src:&[u8]){let inverse=255-src[3];for channel in 0..4{dst[channel]=src[channel].saturating_add(byte_mul(dst[channel],inverse));}}
