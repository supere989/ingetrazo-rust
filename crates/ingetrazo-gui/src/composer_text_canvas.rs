// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct canvas glyph coverage from the shared physical paper text painter.
use std::{collections::{HashMap,VecDeque},sync::Arc};
use egui::{Color32,Pos2,Rect,Vec2,TextureHandle};
use ingetrazo_core::composition::{Sheet,TextItem};
use ingetrazo_formats::{paper_font::{FontKey,CachedFont},paper_text_painter::Painter,sheet_pdf::{Draw,PathOp}};
#[derive(Clone)]struct Layout {text:String,draws:Arc<Vec<Draw>>,bytes:usize,fonts:Vec<(FontKey,usize)>}
#[derive(Clone,Copy,PartialEq,Eq,Hash)]struct GlyphKey {font:FontKey,glyph:u32,scale:i64,phase:[u8;2],italic:bool}
#[derive(Clone)]struct GlyphImage {texture:Option<TextureHandle>,left:i32,top:i32,width:usize,height:usize,bytes:usize}
#[derive(Clone)]enum Ink{Image(TextureHandle,Rect,Rect,Color32),Fill(Rect,Rect,Color32),Line([Pos2;2],Rect,egui::Stroke)}
#[derive(Clone,Default)]pub(super) struct CanvasPainter {
 context:Option<(u64,u64,uuid::Uuid)>,layouts:HashMap<uuid::Uuid,Layout>,layout_bytes:usize,layout_order:VecDeque<uuid::Uuid>,font_refs:HashMap<FontKey,(usize,usize)>,font_bytes:usize,
 glyphs:HashMap<GlyphKey,GlyphImage>,order:VecDeque<GlyphKey>,glyph_bytes:usize,
 ops:Vec<Ink>,frame_ops:usize,frame_textures:Vec<TextureHandle>,pub layout_builds:u64,pub raster_builds:u64,
}
impl std::fmt::Debug for CanvasPainter{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("CanvasPainter").field("stats",&self.stats()).finish()}}
impl CanvasPainter {
 pub fn stats(&self)->[u64;5]{[self.layouts.len()as u64,self.glyphs.len()as u64,self.glyph_bytes as u64,self.layout_builds,self.raster_builds]}
 pub fn begin_frame(&mut self){self.frame_textures.clear();self.ops.clear();self.frame_ops=0;}
 fn prepare(&mut self,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,resources:&mut Painter)->Result<Arc<Vec<Draw>>,String>{
  let context=(model,scene,sheet.id);if self.context!=Some(context){self.layouts.clear();self.layout_bytes=0;self.layout_order.clear();self.font_refs.clear();self.font_bytes=0;self.context=Some(context);}
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

 fn image(&mut self,ctx:&egui::Context,font:&CachedFont,glyph:u32,scale:f64,phase:[u8;2],italic:bool)->Result<GlyphImage,String>{
  let key=GlyphKey{font:font.key(),glyph,scale:(scale*65536.).round()as i64,phase,italic};
  if let Some(image)=self.glyphs.get(&key){return Ok(image.clone());}
  let bitmap=font.rasterize_glyph(glyph,scale,phase,italic)?;let bytes=bitmap.coverage.len().checked_mul(4).ok_or("Canvas glyph texture size overflow")?;
  if bytes>16*1024*1024{return Err("Canvas glyph texture budget exceeded".into());}
  self.glyphs.try_reserve(1).map_err(|_|"Canvas glyph cache allocation failed")?;self.order.try_reserve(1).map_err(|_|"Canvas glyph cache allocation failed")?;
  let texture=if bytes==0{None}else{let mut pixels=Vec::new();pixels.try_reserve_exact(bitmap.coverage.len()).map_err(|_|"Canvas glyph texture allocation failed")?;pixels.extend(bitmap.coverage.iter().map(|&alpha|Color32::from_white_alpha(alpha)));Some(ctx.load_texture("source-paper-glyph",egui::ColorImage{size:[bitmap.width,bitmap.height],pixels},egui::TextureOptions::NEAREST))};
  while self.glyphs.len()>=4096||self.glyph_bytes+bytes>32*1024*1024{let old=self.order.pop_front().ok_or("Canvas glyph cache invariant failed")?;let retired=self.glyphs.remove(&old).ok_or("Canvas glyph cache identity failed")?;self.glyph_bytes-=retired.bytes;}
  let image=GlyphImage{texture,left:bitmap.left,top:bitmap.top,width:bitmap.width,height:bitmap.height,bytes};self.glyphs.insert(key,image.clone());self.order.push_back(key);self.glyph_bytes+=bytes;self.raster_builds+=1;Ok(image)
 }
 pub fn paint(&mut self,painter:&egui::Painter,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,zoom:f32,origin:Pos2,resources:&mut Painter)->Result<Rect,String>{
  self.ops.clear();if !zoom.is_finite()||zoom<=0.||!origin.x.is_finite()||!origin.y.is_finite(){return Err("Invalid canvas text transform".into());}
  let draws=self.prepare(sheet,item,text,model,scene,resources)?;let count=draws.iter().map(|d|d.text.as_ref().map_or(usize::from(!d.path.is_empty()),|t|t.glyphs.len()+t.decorations.len())).sum::<usize>();if self.frame_ops.checked_add(count).is_none_or(|n|n>262144){return Err("Canvas glyph frame budget exceeded".into());}self.ops.try_reserve(count).map_err(|_|"Canvas text operation allocation failed")?;self.frame_textures.try_reserve(count).map_err(|_|"Canvas glyph lifetime allocation failed")?;
  let ppp=painter.ctx().pixels_per_point()as f64;let mut bounds=Rect::NOTHING;
  for draw in draws.iter(){let pos=origin+Vec2::new((draw.origin_mm[0]as f32)*zoom,(draw.origin_mm[1]as f32)*zoom);let rgba=draw.fill.unwrap_or([0.,0.,0.,1.]);let color=Color32::from_rgba_unmultiplied((rgba[0]*255.).round()as u8,(rgba[1]*255.).round()as u8,(rgba[2]*255.).round()as u8,(rgba[3]*255.).round()as u8);
   let mut ink=painter.clone();if let Some(clip)=&draw.clip{let mut rect=Rect::NOTHING;for op in clip{if let PathOp::Move(p)|PathOp::Line(p)=op{rect.extend_with(pos+Vec2::new(p[0]as f32*zoom,p[1]as f32*zoom));}}ink=ink.with_clip_rect(painter.clip_rect().intersect(rect));}
   if let Some(text)=&draw.text{let scale=text.scale_mm*f64::from(zoom)*ppp;
    if text.font.synthetic_bold_offset_px(text.bold)?>0.{return Err("Canvas synthetic-bold glyph raster remains unsupported".into());}
    for glyph in &text.glyphs{let x=f64::from(pos.x)*ppp+glyph.position_px[0]*scale;let y=f64::from(pos.y)*ppp+glyph.position_px[1]*scale;let fixed_x=(x*64.).round()as i64;let fixed_y=(y*64.).round()as i64;let fx=fixed_x.div_euclid(64)as f64;let fy=fixed_y.div_euclid(64)as f64;let phase=[fixed_x.rem_euclid(64)as u8,fixed_y.rem_euclid(64)as u8];let image=self.image(painter.ctx(),&text.font,glyph.glyph,scale,phase,text.italic)?;
     if let Some(texture)=image.texture{let rect=Rect::from_min_size(Pos2::new(((fx+f64::from(image.left))/ppp)as f32,((fy-f64::from(image.top))/ppp)as f32),Vec2::new((image.width as f64/ppp)as f32,(image.height as f64/ppp)as f32));self.ops.push(Ink::Image(texture,rect,ink.clip_rect(),color));bounds=bounds.union(rect.intersect(ink.clip_rect()));}
    }
    for decoration in &text.decorations{let [x0,y0,x1,y1]=decoration.line_px;self.ops.push(Ink::Line([pos+Vec2::new((x0*text.scale_mm)as f32*zoom,(y0*text.scale_mm)as f32*zoom),pos+Vec2::new((x1*text.scale_mm)as f32*zoom,(y1*text.scale_mm)as f32*zoom)],ink.clip_rect(),egui::Stroke::new((decoration.width_px*text.scale_mm)as f32*zoom,color)));}
   }else if !draw.path.is_empty(){let mut rect=Rect::NOTHING;for op in &draw.path{if let PathOp::Move(p)|PathOp::Line(p)=op{rect.extend_with(pos+Vec2::new(p[0]as f32*zoom,p[1]as f32*zoom));}}self.ops.push(Ink::Fill(rect,ink.clip_rect(),color));bounds=bounds.union(rect);}
  }
  // Publish only after the complete item has valid layout, glyphs and resources.
  for op in self.ops.drain(..){match op{Ink::Image(texture,rect,clip,color)=>{painter.with_clip_rect(clip).image(texture.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),color);self.frame_textures.push(texture);},Ink::Fill(rect,clip,color)=>{painter.with_clip_rect(clip).rect_filled(rect,0.,color);},Ink::Line(line,clip,stroke)=>{painter.with_clip_rect(clip).line_segment(line,stroke);}}}self.frame_ops+=count;Ok(bounds)
 }
}
