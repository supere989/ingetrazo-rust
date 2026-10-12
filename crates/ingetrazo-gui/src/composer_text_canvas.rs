// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct canvas glyph coverage from the shared physical paper text painter.
use std::{collections::{HashMap,VecDeque},sync::{Arc,Weak}};
use egui::{Color32,Pos2,Rect,Vec2,TextureHandle};
use ingetrazo_core::composition::{Sheet,TextItem};
use ingetrazo_formats::{paper_font::{FontKey,CachedFont},paper_text_painter::Painter,sheet_pdf::{Draw,PathOp}};
#[derive(Clone)]struct Layout {text:String,draws:Arc<Vec<Draw>>,bytes:usize,fonts:Vec<(FontKey,usize)>}
#[derive(Clone,Copy,PartialEq,Eq,Hash)]struct GlyphKey {font:FontKey,glyph:u32,scale:u64,phase:[u8;2],bold:bool,italic:bool,rotation:u64,color:[u8;4]}
#[derive(Clone)]struct GlyphImage {pixels:Arc<Vec<u8>>,left:i32,top:i32,width:usize,height:usize,bytes:usize}
#[path="composer_text_ordered.rs"] mod ordered;
#[derive(Clone)]struct Tile{factory_identity:Option<(u8,usize)>,ordered:Option<ordered::Ordered>,draws:Weak<Vec<Draw>>,density:u64,phase:[u64;2],offset:[i64;2],size:[usize;2],texture:Option<TextureHandle>,bytes:usize}
#[derive(Clone)]struct Sample{x:i64,y:i64,image:GlyphImage,clip:[i64;4]}
/// Explicit cold-export diagnostic; contains no font bytes or glyph coverage.
#[derive(Clone,serde::Serialize)]
struct PhysicalGlyphObservation {
 draw:usize,glyph_index:usize,glyph:u32,font_owner:uuid::Uuid,font_face:u32,
 origin_mm:[f64;2],scale_mm:f64,density:f64,rotation_deg:f64,
 position_px:[f64;2],position_bits:[u64;2],device_px:[f64;2],device_bits:[u64;2],
 device_26_6:[i64;2],phase_count:u8,raster_origin:[f64;2],phase:[u8;2],
}
#[derive(Clone,Default)]pub(crate) struct CanvasPainter {
 context:Option<(u64,u64,uuid::Uuid)>,layouts:HashMap<uuid::Uuid,Layout>,layout_bytes:usize,layout_order:VecDeque<uuid::Uuid>,font_refs:HashMap<FontKey,(usize,usize)>,font_bytes:usize,
 glyphs:HashMap<GlyphKey,GlyphImage>,order:VecDeque<GlyphKey>,glyph_bytes:usize,
 tiles:HashMap<uuid::Uuid,Tile>,tile_order:VecDeque<uuid::Uuid>,tile_bytes:usize,pub tile_builds:u64,
 // Explicit fixture opt-in; no runtime retention or warm-path capture.
 sample_observation:Option<Vec<Sample>>,
 physical_observation:Option<Vec<PhysicalGlyphObservation>>,
 ordered_builds:u64,ordered_items:usize,ordered_parts:usize,ordered_bytes:usize,
 frame_ops:usize,frame_textures:Vec<TextureHandle>,pub layout_builds:u64,pub raster_builds:u64,
}
impl std::fmt::Debug for CanvasPainter{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("CanvasPainter").field("stats",&self.stats()).finish()}}
impl CanvasPainter {
 /// Physical export reuses source glyph coverage and decoration producers;
 /// positions stay double until the existing QFixed/raster cache boundary.
 pub(crate) fn physical_records(&mut self,draws:&[Draw],density:f64)->Result<Vec<(Arc<ingetrazo_render::cad2d::Geometry>,Arc<[ingetrazo_render::cad2d::Batch]>)>,String>{
  if self.physical_observation.is_some(){self.physical_records_impl::<true>(draws,density)}else{self.physical_records_impl::<false>(draws,density)}
 }
 pub(crate) fn begin_physical_observation(&mut self)->Result<(),String>{
  if self.physical_observation.is_some(){return Err("Physical glyph observation already active".into());}
  let mut rows=Vec::new();rows.try_reserve_exact(2048).map_err(|_|"Physical glyph observation allocation refused")?;
  self.physical_observation=Some(rows);Ok(())
 }
 pub(crate) fn take_physical_observation(&mut self)->Result<serde_json::Value,String>{
  let rows=self.physical_observation.take().ok_or("Physical glyph observation was not enabled")?;
  serde_json::to_value(rows).map_err(|e|e.to_string())
 }
 fn physical_records_impl<const OBSERVE:bool>(&mut self,draws:&[Draw],density:f64)->Result<Vec<(Arc<ingetrazo_render::cad2d::Geometry>,Arc<[ingetrazo_render::cad2d::Batch]>)>,String>{
  if !density.is_finite()||density<=0.{return Err("Invalid physical text density".into());}
  let mut samples=Vec::new();let mut extent=None;let mut working=0;
  for (draw_index,draw) in draws.iter().enumerate(){
   let pos=draw.origin_mm.map(|v|v*density);
   let rgba=draw.fill.unwrap_or([0.,0.,0.,1.]);
   let color=Color32::from_rgba_unmultiplied((rgba[0]*255.).round()as u8,(rgba[1]*255.).round()as u8,(rgba[2]*255.).round()as u8,(rgba[3]*255.).round()as u8);
   let mut clip=[i64::MIN/2,i64::MIN/2,i64::MAX/2,i64::MAX/2];
   let mut masks=Vec::new();
   for path in [&draw.clip,&draw.clip_intersection].into_iter().flatten(){
    if let Some(bounds)=crate::sheet_image_paths::axis_rectangle(path){
     let round=|v:f64|if v>=0.{(v+0.5).floor()as i64}else{(v-0.5).ceil()as i64};
     let bounds=[round(pos[0]+bounds[0]*density),round(pos[1]+bounds[1]*density),round(pos[0]+bounds[2]*density),round(pos[1]+bounds[3]*density)];
     clip=[clip[0].max(bounds[0]),clip[1].max(bounds[1]),clip[2].min(bounds[2]),clip[3].min(bounds[3])];continue;
    }
    let mask=ingetrazo_render::qt_gray::rasterize_transformed_paths(&crate::sheet_image_paths::paths(path)?,density,pos,250_000,100_000)?;
    masks.push(mask);
   }

   let Some(text)=&draw.text else{
    let mut rect=[f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY];for op in &draw.path{if let PathOp::Move(p)|PathOp::Line(p)=op{let q=[pos[0]+p[0]*density,pos[1]+p[1]*density];rect[0]=rect[0].min(q[0]);rect[1]=rect[1].min(q[1]);rect[2]=rect[2].max(q[0]);rect[3]=rect[3].max(q[1]);}else if !matches!(op,PathOp::Close){return Err("Physical text decoration lacks exact coverage painter".into());}}
    if let Some(sample)=rectangle_sample(rect,rgba,clip)?{include_sample(&mut samples,&mut extent,&mut working,Self::physical_clip_sample(sample,&masks)?)?;}continue;
   };
   let scale=text.scale_mm*density;let(sin,cos)=text.rotation_deg.to_radians().sin_cos();
   if text.rotation_deg!=0.&&!text.decorations.is_empty(){return Err("Rotated physical text decorations unavailable".into());}
   for (glyph_index,glyph) in text.glyphs.iter().enumerate(){
    let x=pos[0]+(glyph.position_px[0]*cos-glyph.position_px[1]*sin)*scale;let y=pos[1]+(glyph.position_px[0]*sin+glyph.position_px[1]*cos)*scale;
    let count=text.font.raster_phase_count(glyph.glyph,scale,text.bold,text.italic)?;
    let(fx,fy,phase)=device_position_with_count((x*64.)as i64,(y*64.)as i64,count);
    if OBSERVE {
     let rows=self.physical_observation.as_mut().ok_or("Physical observation owner lost")?;
     if rows.len()==2048{return Err("Physical glyph observation exceeds 2048 glyphs".into());}
     let key=text.font.key();
     rows.push(PhysicalGlyphObservation{draw:draw_index,glyph_index,glyph:glyph.glyph,font_owner:key.identity,font_face:key.face_index,
      origin_mm:draw.origin_mm,scale_mm:text.scale_mm,density,rotation_deg:text.rotation_deg,
      position_px:glyph.position_px,position_bits:glyph.position_px.map(f64::to_bits),device_px:[x,y],device_bits:[x.to_bits(),y.to_bits()],
      device_26_6:[(x*64.)as i64,(y*64.)as i64],phase_count:count,raster_origin:[fx,fy],phase});
    }
    let image=self.rotated_image(&text.font,glyph.glyph,scale,phase,text.bold,text.italic,color,text.rotation_deg)?;
    let sample=Sample{x:fx as i64+i64::from(image.left),y:fy as i64-i64::from(image.top),image,clip};
    include_sample(&mut samples,&mut extent,&mut working,Self::physical_clip_glyph(sample,&masks)?)?;
   }
   for decoration in &text.decorations{let[x0,y0,x1,_]=decoration.line_px;let half=decoration.width_px*scale*0.5;let rect=[pos[0]+x0*scale,pos[1]+y0*scale-half,pos[0]+x1*scale,pos[1]+y0*scale+half];let sample=if decoration.width_px*scale<=1.{thin_line_sample([rect[0],pos[1]+y0*scale,rect[2]],decoration.width_px*scale,rgba,clip)?}else{rectangle_sample(rect,rgba,clip)?};if let Some(sample)=sample{include_sample(&mut samples,&mut extent,&mut working,Self::physical_clip_sample(sample,&masks)?)?;}}
  }
  let factory=crate::composer_backend::Factory::MetalProducer(Default::default());
  let ordered=ordered::Ordered::build(&samples,[0;2],[0.;2],1.,&factory,ingetrazo_render::cad2d::Pose{scale:[1.;2],translation:[0.;2]})?;
  Ok(ordered.records())
 }

 // Qt 6.12 solid A8 alphamapBlit clips glyphs by span presence, not
 // fractional clip coverage (qdrawhelper.cpp qt_alphamapblit_argb32).
 // Physical glyphs here are grayscale coverage, solid color, SourceOver,
 // full painter opacity. Decorations and other samples retain AA clipping.
 fn physical_clip_glyph(sample:Sample,masks:&[Vec<ingetrazo_render::qt_gray::Span>])->Result<Sample,String>{
  if masks.len()<=1{return Self::physical_clip_sample_mode::<true>(sample,masks);}
  // Qt intersects clip coverages before alphamapBlit checks span presence.
  let mut combined=masks[0].clone();
  for mask in &masks[1..]{combined=ingetrazo_render::qt_gray::intersect_spans(&combined,mask,100_000)?;}
  Self::physical_clip_sample_mode::<true>(sample,std::slice::from_ref(&combined))
 }
 fn physical_clip_sample(sample:Sample,masks:&[Vec<ingetrazo_render::qt_gray::Span>])->Result<Sample,String>{Self::physical_clip_sample_mode::<false>(sample,masks)}
 fn physical_clip_sample_mode<const GLYPH:bool>(mut sample:Sample,masks:&[Vec<ingetrazo_render::qt_gray::Span>])->Result<Sample,String>{
  if masks.is_empty(){return Ok(sample);}
  let mut pixels=Vec::new();pixels.try_reserve_exact(sample.image.bytes).map_err(|_|"Physical glyph clipping allocation refused")?;pixels.extend_from_slice(&sample.image.pixels);
  for mask in masks{
   for y in 0..sample.image.height {let gy=sample.y+y as i64;let start=mask.partition_point(|s|i64::from(s.y)<gy);let end=mask.partition_point(|s|i64::from(s.y)<=gy);let row=&mask[start..end];
    for x in 0..sample.image.width {let gx=sample.x+x as i64;let i=row.partition_point(|s|i64::from(s.x)+i64::from(s.len)<=gx);let coverage=row.get(i).filter(|s|i64::from(s.x)<=gx&&gx<i64::from(s.x)+i64::from(s.len)).map_or(0,|s|s.coverage);
     let at=(y*sample.image.width+x)*4;if GLYPH{if coverage==0{pixels[at..at+4].fill(0);}}else{for v in &mut pixels[at..at+4]{let n=u32::from(*v)*u32::from(coverage);*v=((n+(n>>8)+128)>>8)as u8;}}
    }
   }
  }
  sample.image.pixels=Arc::new(pixels);Ok(sample)
 }
 pub(crate) fn physical_rectangle(&mut self,draw:&Draw,density:f64)->Result<Vec<(Arc<ingetrazo_render::cad2d::Geometry>,Arc<[ingetrazo_render::cad2d::Batch]>)>,String>{
  self.physical_records(std::slice::from_ref(draw),density)
 }
 pub fn stats(&self)->[u64;5]{[self.layouts.len()as u64,self.glyphs.len()as u64,self.glyph_bytes as u64,self.layout_builds,self.raster_builds]}
 pub fn ordered_stats(&self)->[u64;4]{[self.ordered_builds,self.ordered_items as u64,self.ordered_parts as u64,self.ordered_bytes as u64]}
 fn retire_tile(&mut self,id:uuid::Uuid){if let Some(old)=self.tiles.remove(&id){self.tile_bytes-=old.bytes;if let Some(ordered)=old.ordered{self.ordered_items-=1;self.ordered_parts-=ordered.part_count();self.ordered_bytes-=ordered.bytes;}}}
 pub fn begin_frame(&mut self){self.frame_textures.clear();self.frame_ops=0;}
 fn prepare(&mut self,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,resources:&mut Painter)->Result<Arc<Vec<Draw>>,String>{
  let context=(model,scene,sheet.id);if self.context!=Some(context){self.layouts.clear();self.layout_bytes=0;self.layout_order.clear();self.font_refs.clear();self.font_bytes=0;self.tiles.clear();self.tile_order.clear();self.tile_bytes=0;self.ordered_items=0;self.ordered_parts=0;self.ordered_bytes=0;self.context=Some(context);}
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

 fn image(&mut self,font:&CachedFont,glyph:u32,scale:f64,phase:[u8;2],bold:bool,italic:bool,color:Color32)->Result<GlyphImage,String>{self.rotated_image(font,glyph,scale,phase,bold,italic,color,0.)}
 fn rotated_image(&mut self,font:&CachedFont,glyph:u32,scale:f64,phase:[u8;2],bold:bool,italic:bool,color:Color32,angle:f64)->Result<GlyphImage,String>{
  let key=GlyphKey{font:font.key(),glyph,scale:scale.to_bits(),phase,bold,italic,rotation:angle.to_bits(),color:color.to_array()};
  if let Some(image)=self.glyphs.get(&key){return Ok(image.clone());}
  let bitmap=font.rasterize_rotated_glyph(glyph,scale,phase,bold,italic,angle)?;let bytes=bitmap.coverage.len().checked_mul(4).ok_or("Canvas glyph texture size overflow")?;
  if bytes>16*1024*1024{return Err("Canvas glyph texture budget exceeded".into());}
  self.glyphs.try_reserve(1).map_err(|_|"Canvas glyph cache allocation failed")?;self.order.try_reserve(1).map_err(|_|"Canvas glyph cache allocation failed")?;
  let mut pixels=Vec::new();pixels.try_reserve_exact(bytes).map_err(|_|"Canvas glyph coverage allocation failed")?;
  let [r,g,b,a]=color.to_srgba_unmultiplied();
  for &coverage in &bitmap.coverage{let alpha=byte_mul(coverage,a);pixels.extend_from_slice(&[byte_mul(r,alpha),byte_mul(g,alpha),byte_mul(b,alpha),alpha]);}
  let pixels=Arc::new(pixels);
  while self.glyphs.len()>=4096||self.glyph_bytes+bytes>32*1024*1024{let old=self.order.pop_front().ok_or("Canvas glyph cache invariant failed")?;let retired=self.glyphs.remove(&old).ok_or("Canvas glyph cache identity failed")?;self.glyph_bytes-=retired.bytes;}
  let image=GlyphImage{pixels,left:bitmap.left,top:bitmap.top,width:bitmap.width,height:bitmap.height,bytes};self.glyphs.insert(key,image.clone());self.order.push_back(key);self.glyph_bytes+=bytes;self.raster_builds+=1;Ok(image)
 }
 fn tile(&mut self,painter:&egui::Painter,draws:&Arc<Vec<Draw>>,item:&TextItem,zoom:f32,origin:Pos2,factory:Option<&crate::composer_backend::Factory>)->Result<Option<(TextureHandle,Rect)>,String>{
  let ppp=painter.ctx().pixels_per_point()as f64;let density=f64::from(zoom)*ppp;
  let base=origin+Vec2::new(item.pos.0 as f32*zoom,item.pos.1 as f32*zoom);let physical=[f64::from(base.x)*ppp,f64::from(base.y)*ppp];let anchor=[physical[0].floor()as i64,physical[1].floor()as i64];let phase=[physical[0].fract().to_bits(),physical[1].fract().to_bits()];
  if let Some(tile)=self.tiles.get(&item.id){if tile.factory_identity==factory.map(|f|f.cache_identity())&&tile.density==density.to_bits()&&tile.phase==phase&&tile.draws.upgrade().is_some_and(|old|Arc::ptr_eq(&old,draws)){return Ok(tile_image(tile,anchor,ppp));}}
  let mut samples=Vec::new();let count=draws.iter().map(|d|d.text.as_ref().map_or(0,|t|t.glyphs.len())).sum();samples.try_reserve(count).map_err(|_|"Canvas glyph sample allocation failed")?;let mut working_bytes=0usize;let mut extent:Option<[i64;4]>=None;
  for draw in draws.iter(){let pos=origin+Vec2::new(draw.origin_mm[0]as f32*zoom,draw.origin_mm[1]as f32*zoom);let scale=draw.text.as_ref().map_or(1.,|t|t.scale_mm)*density;let rgba=draw.fill.unwrap_or([0.,0.,0.,1.]);let color=Color32::from_rgba_unmultiplied((rgba[0]*255.).round()as u8,(rgba[1]*255.).round()as u8,(rgba[2]*255.).round()as u8,(rgba[3]*255.).round()as u8);
   let mut clip=[i64::MIN/2,i64::MIN/2,i64::MAX/2,i64::MAX/2];if let Some(path)=&draw.clip{let mut min=[f64::INFINITY;2];let mut max=[f64::NEG_INFINITY;2];for op in path{if let PathOp::Move(p)|PathOp::Line(p)=op{for axis in 0..2{let value=if axis==0{f64::from(pos.x)}else{f64::from(pos.y)}*ppp+p[axis]*density;min[axis]=min[axis].min(value);max[axis]=max[axis].max(value);}}}clip=[min[0].floor()as i64,min[1].floor()as i64,max[0].ceil()as i64,max[1].ceil()as i64];}
   let Some(text)=&draw.text else{let mut rect=[f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY];for op in &draw.path{if let PathOp::Move(p)|PathOp::Line(p)=op{let point=[f64::from(pos.x)*ppp+p[0]*density,f64::from(pos.y)*ppp+p[1]*density];rect[0]=rect[0].min(point[0]);rect[1]=rect[1].min(point[1]);rect[2]=rect[2].max(point[0]);rect[3]=rect[3].max(point[1]);}}if let Some(sample)=rectangle_sample(rect,rgba,clip)?{include_sample(&mut samples,&mut extent,&mut working_bytes,sample)?;}continue;};
   for glyph in &text.glyphs{let x=f64::from(pos.x)*ppp+glyph.position_px[0]*scale;let y=f64::from(pos.y)*ppp+glyph.position_px[1]*scale;let count=text.font.raster_phase_count(glyph.glyph,scale,text.bold,text.italic)?;let(fx,fy,phase)=device_position_with_count((x*64.)as i64,(y*64.)as i64,count);let image=self.image(&text.font,glyph.glyph,scale,phase,text.bold,text.italic,color)?;
    working_bytes=working_bytes.checked_add(image.bytes).filter(|&n|n<=32*1024*1024).ok_or("Canvas glyph working budget exceeded")?;
    let x=fx as i64+i64::from(image.left);let y=fy as i64-i64::from(image.top);let bounds=[x.max(clip[0]),y.max(clip[1]),(x+image.width as i64).min(clip[2]),(y+image.height as i64).min(clip[3])];if bounds[0]>=bounds[2]||bounds[1]>=bounds[3]{continue;}
    extent=Some(if let Some(old)=extent{[old[0].min(bounds[0]),old[1].min(bounds[1]),old[2].max(bounds[2]),old[3].max(bounds[3])]}else{bounds});samples.push(Sample{x,y,image,clip});
   }
   for decoration in &text.decorations{let [x0,y0,x1,_]=decoration.line_px;let half=decoration.width_px*scale*0.5;let rect=[f64::from(pos.x)*ppp+x0*scale,f64::from(pos.y)*ppp+y0*scale-half,f64::from(pos.x)*ppp+x1*scale,f64::from(pos.y)*ppp+y0*scale+half];let sample=if decoration.width_px*scale<=1.{thin_line_sample([rect[0],f64::from(pos.y)*ppp+y0*scale,rect[2]],decoration.width_px*scale,rgba,clip)?}else{rectangle_sample(rect,rgba,clip)?};if let Some(sample)=sample{include_sample(&mut samples,&mut extent,&mut working_bytes,sample)?;}}
  }
  if let Some(observation)=&mut self.sample_observation{observation.clear();observation.try_reserve(samples.len()).map_err(|_|"Canvas sample observation allocation failed")?;observation.extend(samples.iter().cloned());}
  if let Some(extent)=extent{let w=usize::try_from(extent[2]-extent[0]).map_err(|_|"Canvas tile dimension invalid")?;let h=usize::try_from(extent[3]-extent[1]).map_err(|_|"Canvas tile dimension invalid")?;if w>4096||h>4096||w.checked_mul(h).and_then(|n|n.checked_mul(4)).is_none_or(|n|n>16*1024*1024){return Err("Canvas tile coverage budget exceeded".into());}}
  let pose=ingetrazo_render::cad2d::Pose{scale:[1.;2],translation:[base.x,base.y]};
  let ordered=if let Some(factory)=factory{Some(ordered::Ordered::build(&samples,anchor,[physical[0]-anchor[0]as f64,physical[1]-anchor[1]as f64],ppp as f32,factory,pose)?)}else{None};
  let (size,offset,pixels)=if let Some(extent)=extent{let width=usize::try_from(extent[2]-extent[0]).map_err(|_|"Canvas tile dimension invalid")?;let height=usize::try_from(extent[3]-extent[1]).map_err(|_|"Canvas tile dimension invalid")?;let bytes=width.checked_mul(height).and_then(|n|n.checked_mul(4)).filter(|&n|n<=16*1024*1024).ok_or("Canvas tile coverage budget exceeded")?;if width>4096||height>4096{return Err("Canvas tile dimensions exceeded".into());}let mut pixels=Vec::new();if ordered.is_none(){pixels.try_reserve_exact(bytes).map_err(|_|"Canvas tile allocation failed")?;pixels.resize(bytes,0);}
   if ordered.is_none(){for sample in samples{for row in 0..sample.image.height{let y=sample.y+row as i64;if y<sample.clip[1]||y>=sample.clip[3]{continue;}for col in 0..sample.image.width{let x=sample.x+col as i64;if x<sample.clip[0]||x>=sample.clip[2]{continue;}let src=&sample.image.pixels[(row*sample.image.width+col)*4..][..4];let dst=&mut pixels[((y-extent[1])as usize*width+(x-extent[0])as usize)*4..][..4];source_over(dst,src);}}}}
   ([width,height],[extent[0]-anchor[0],extent[1]-anchor[1]],pixels)
  }else{([0,0],[0,0],Vec::new())};
  self.tiles.try_reserve(1).map_err(|_|"Canvas tile cache allocation failed")?;self.tile_order.try_reserve(1).map_err(|_|"Canvas tile order allocation failed")?;
  let bytes=ordered.as_ref().map_or(pixels.len(),|p|p.bytes);let texture=if ordered.is_some()||pixels.is_empty(){None}else{Some(crate::paper_raw_texture::create(painter.ctx(),size,pixels)?)};
  self.retire_tile(item.id);self.tile_order.retain(|&id|id!=item.id);while self.tiles.len()>=10000||self.tile_bytes+bytes>32*1024*1024{let id=self.tile_order.pop_front().ok_or("Canvas tile cache invariant failed")?;self.retire_tile(id);}
  if let Some(ordered)=&ordered{self.ordered_builds+=1;self.ordered_items+=1;self.ordered_parts+=ordered.part_count();self.ordered_bytes+=ordered.bytes;}let tile=Tile{factory_identity:factory.map(|f|f.cache_identity()),ordered,draws:Arc::downgrade(draws),density:density.to_bits(),phase,offset,size,texture,bytes};let result=tile_image(&tile,anchor,ppp);self.tiles.insert(item.id,tile);self.tile_order.push_back(item.id);self.tile_bytes+=bytes;self.tile_builds+=1;Ok(result)
 }
 pub fn paint(&mut self,painter:&egui::Painter,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,zoom:f32,origin:Pos2,resources:&mut Painter)->Result<Rect,String>{self.paint_with_factory(painter,sheet,item,text,model,scene,zoom,origin,resources,None)}
 pub fn paint_with_factory(&mut self,painter:&egui::Painter,sheet:&Sheet,item:&TextItem,text:&str,model:u64,scene:u64,zoom:f32,origin:Pos2,resources:&mut Painter,factory:Option<&crate::composer_backend::Factory>)->Result<Rect,String>{
  if !zoom.is_finite()||zoom<=0.||!origin.x.is_finite()||!origin.y.is_finite(){return Err("Invalid canvas text transform".into());}
  let draws=self.prepare(sheet,item,text,model,scene,resources)?;self.paint_prepared(painter,&draws,item,zoom,origin,factory)
 }
 pub(super) fn paint_prepared(&mut self,painter:&egui::Painter,draws:&Arc<Vec<Draw>>,item:&TextItem,zoom:f32,origin:Pos2,factory:Option<&crate::composer_backend::Factory>)->Result<Rect,String>{
  if !zoom.is_finite()||zoom<=0.||!origin.x.is_finite()||!origin.y.is_finite(){return Err("Invalid canvas text transform".into());}
  let count=draws.iter().map(|d|d.text.as_ref().map_or(usize::from(!d.path.is_empty()),|t|t.glyphs.len()+t.decorations.len())).sum::<usize>();if self.frame_ops.checked_add(count).is_none_or(|n|n>262144){return Err("Canvas glyph frame budget exceeded".into());}self.frame_textures.try_reserve(1).map_err(|_|"Canvas glyph lifetime allocation failed")?;
  let mut bounds=Rect::NOTHING;
  for draw in draws.iter(){if let Some(text)=&draw.text{if text.font.synthetic_bold_offset_px(text.bold)?>0. && !text.font.supports_synthetic_bold_rasterization(){return Err("Canvas synthetic-bold glyph raster remains unsupported".into());}}}
  // Publish one complete cached item only after all source paints/resources validate.
  if let Some((texture,rect))=self.tile(painter,&draws,item,zoom,origin,factory)?{painter.image(texture.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);self.frame_textures.push(texture);bounds=bounds.union(rect);}if let Some(tile)=self.tiles.get(&item.id){if let Some(ordered)=&tile.ordered{
   let base=origin+Vec2::new(item.pos.0 as f32*zoom,item.pos.1 as f32*zoom);ordered.paint(painter,ingetrazo_render::cad2d::Pose{scale:[1.;2],translation:[base.x,base.y]})?;
   let ppp=painter.ctx().pixels_per_point()as f64;let anchor=[(f64::from(base.x)*ppp).floor()as i64,(f64::from(base.y)*ppp).floor()as i64];
   if tile.size[0]>0&&tile.size[1]>0{let min=Pos2::new(((anchor[0]+tile.offset[0])as f64/ppp)as f32,((anchor[1]+tile.offset[1])as f64/ppp)as f32);bounds=bounds.union(Rect::from_min_size(min,Vec2::new(tile.size[0]as f32/ppp as f32,tile.size[1]as f32/ppp as f32)));}
  }}self.frame_ops+=count;Ok(bounds)
 }
}

// Qt 6.12 raster painter: horizontal four-phase FT cache; without the
// VerticalSubpixelPositioning hint the baseline is rounded and y phase is zero.
#[cfg(test)]
fn device_position(x:i64,y:i64)->(f64,f64,[u8;2]){device_position_with_count(x,y,4)}
fn device_position_with_count(x:i64,y:i64,count:u8)->(f64,f64,[u8;2]){
 // QFontEngine divides by QFixed(count), whose division rounds nearest.
 // Integer division here incorrectly maps phase2/3 to42 instead of43.
 let phase=if count<=1{0}else{let count=i64::from(count);let bucket=(x.rem_euclid(64)+1)*count/64;(bucket*128+count)/(count*2)};
 (x.div_euclid(64)as f64+(phase/64)as f64,(y+32).div_euclid(64)as f64,[(phase%64)as u8,0])
}
#[cfg(test)]mod device_position_tests{use super::*;
 #[test]fn native_dynamic_phase_count_preserves_qfixed_fraction_and_negative_floor(){
  assert_eq!(device_position_with_count(21,31,3),(0.,0.,[21,0]));
  assert_eq!(device_position_with_count(59,31,3),(0.,0.,[43,0]));
  assert_eq!(device_position_with_count(14,32,12),(0.,1.,[11,0]));
  assert_eq!(device_position_with_count(-1,-33,3),(0.,-1.,[0,0]));
  assert_eq!(device_position_with_count(63,0,3),(1.,0.,[0,0]));
  assert_eq!(device_position_with_count(21,31,1),(0.,0.,[0,0]));
  assert_ne!(device_position_with_count(21,31,3),device_position(21,31));
 }
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
  let text=first.iter().find_map(|d|d.text.as_ref()).unwrap();let glyph=text.glyphs[0].glyph;let a=cache.image(&text.font,glyph,1.,[0,0],false,false,Color32::BLACK).unwrap();let b=cache.image(&text.font,glyph,1.+1e-7,[0,0],false,false,Color32::BLACK).unwrap();assert!(!Arc::ptr_eq(&a.pixels,&b.pixels));assert_eq!(cache.glyphs.len(),2);
  cache.retire_layout(item.id);assert_eq!((cache.layout_bytes,cache.font_bytes,cache.font_refs.len()),(0,0,0));
 }
}

fn byte_mul(color:u8,alpha:u8)->u8{let product=u32::from(color)*u32::from(alpha);((product+(product>>8)+128)>>8)as u8}

fn tile_image(tile:&Tile,anchor:[i64;2],ppp:f64)->Option<(TextureHandle,Rect)>{tile.texture.clone().map(|texture|(texture,Rect::from_min_size(Pos2::new(((anchor[0]+tile.offset[0])as f64/ppp)as f32,((anchor[1]+tile.offset[1])as f64/ppp)as f32),Vec2::new((tile.size[0]as f64/ppp)as f32,(tile.size[1]as f64/ppp)as f32))))}
fn source_over(dst:&mut [u8],src:&[u8]){let inverse=255-src[3];for channel in 0..4{dst[channel]=src[channel].saturating_add(byte_mul(dst[channel],inverse));}}

// Qt 6.12 QRasterizer::rasterizeLine axis-aligned antialiased branch uses
// signed16.16 bounds and truncates the two-dimensional coverage product.
fn rectangle_sample(rect:[f64;4],color:[f32;4],clip:[i64;4])->Result<Option<Sample>,String>{
 if rect.iter().any(|v|!v.is_finite()||v.abs()>10000000.){return Err("Canvas rectangle coordinate admission exceeded".into());}
 if rect[2]<=rect[0]||rect[3]<=rect[1]{return Ok(None);}
 let x=(rect[0].floor()as i64).max(clip[0]);let y=(rect[1].floor()as i64).max(clip[1]);let right=(rect[2].ceil()as i64).min(clip[2]);let bottom=(rect[3].ceil()as i64).min(clip[3]);if x>=right||y>=bottom{return Ok(None);}
 let width=usize::try_from(right-x).map_err(|_|"Canvas rectangle dimensions invalid")?;let height=usize::try_from(bottom-y).map_err(|_|"Canvas rectangle dimensions invalid")?;let bytes=width.checked_mul(height).and_then(|n|n.checked_mul(4)).filter(|&n|n<=16*1024*1024).ok_or("Canvas rectangle budget exceeded")?;if width>4096||height>4096{return Err("Canvas rectangle dimensions exceeded".into());}
 let fixed=rect.map(|v|(v*65536.)as i64);let premul=brush_premultiplied(color);let mut pixels=Vec::new();pixels.try_reserve_exact(bytes).map_err(|_|"Canvas rectangle allocation failed")?;
 for row in 0..height{let py=(y+row as i64)*65536;let h=(py+65536).min(fixed[3])-py.max(fixed[1]);for col in 0..width{let px=(x+col as i64)*65536;let w=(px+65536).min(fixed[2])-px.max(fixed[0]);let coverage=((h.max(0)*w.max(0)*255)>>32)as u8;pixels.extend_from_slice(&premul.map(|value|byte_mul(value,coverage)));}}
 Ok(Some(Sample{x,y,image:GlyphImage{pixels:Arc::new(pixels),left:0,top:0,width,height,bytes},clip}))
}
fn include_sample(samples:&mut Vec<Sample>,extent:&mut Option<[i64;4]>,bytes:&mut usize,sample:Sample)->Result<(),String>{
 *bytes=bytes.checked_add(sample.image.bytes).filter(|&n|n<=32*1024*1024).ok_or("Canvas item working budget exceeded")?;samples.try_reserve(1).map_err(|_|"Canvas item sample allocation failed")?;
 let bounds=[sample.x,sample.y,sample.x+sample.image.width as i64,sample.y+sample.image.height as i64];*extent=Some(if let Some(old)=*extent{[old[0].min(bounds[0]),old[1].min(bounds[1]),old[2].max(bounds[2]),old[3].max(bounds[3])]}else{bounds});samples.push(sample);Ok(())
}

#[cfg(test)]mod source_rectangle_tests{use super::*;
 #[test]fn actual_qt_background_coverage_controls(){let data:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/paper-canvas-background-underline/qt-rectangle-controls.json")).unwrap();for case in data["cases"].as_array().unwrap(){let rect=std::array::from_fn(|i|case["rect"][i].as_f64().unwrap());let color:[u8;4]=std::array::from_fn(|i|case["color"][i].as_u64().unwrap()as u8);let sample=rectangle_sample(rect,color.map(|v|f32::from(v)/255.),[0,0,8,8]).unwrap().unwrap();let mut pixels=vec![0u8;8*8*4];for y in 0..sample.image.height{for x in 0..sample.image.width{let dst=((sample.y as usize+y)*8+sample.x as usize+x)*4;let src=(y*sample.image.width+x)*4;pixels[dst..dst+4].copy_from_slice(&sample.image.pixels[src..src+4]);}}let expected=case["pixels"].as_array().unwrap().iter().map(|v|v.as_u64().unwrap()as u8).collect::<Vec<_>>();assert_eq!(pixels,expected,"rect {:?} color {:?}",rect,color);}}
}

// QColor retains16-bit opacity until the raster brush is converted to ARGB32.
fn brush_premultiplied(color:[f32;4])->[u8;4]{let channels=color.map(|v|(f64::from(v.clamp(0.,1.))*65535.).round()as u64);let alpha=channels[3];std::array::from_fn(|i|{let v=if i==3{alpha}else{(channels[i]*alpha+32767)/65535};((v+128)/257)as u8})}
// Qt6.12 QCosmeticStroker::drawLineAA horizontal, flat-cap branch.
fn thin_line_sample(line:[f64;3],width:f64,color:[f32;4],clip:[i64;4])->Result<Option<Sample>,String>{
 if line.iter().any(|v|!v.is_finite()||v.abs()>10000000.)||!width.is_finite()||width<=0.||width>1.{return Err("Canvas thin line admission failed".into());}
 let x1=(line[0]*64.)as i64;let x2=(line[2]*64.)as i64;if x2<=x1{return Ok(None);}let yfixed=((line[1]*64.)as i64-32)*1024;let y=yfixed>>16;let first=x1>>6;let last=x2>>6;let count=usize::try_from(last-first+1).map_err(|_|"Canvas thin line width exceeded")?;if count>4096{return Err("Canvas thin line width exceeded".into());}
 if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){eprintln!("Paper thin line observation: line={line:?} width={width:?} clip={clip:?}");}
 let opacity=(width*256.)as u64;let rgba=color.map(|v|(f64::from(v.clamp(0.,1.))*65535.).round()as u64);let alpha=rgba[3];let base:[u8;4]=std::array::from_fn(|i|{let v=if i==3{alpha}else{(rgba[i]*alpha+32767)/65535};let faded=(v*opacity)>>8;((faded+128)/257)as u8});let fraction=((yfixed>>8)&255)as u8;
 let mut pixels=Vec::new();pixels.try_reserve_exact(count*8).map_err(|_|"Canvas thin line allocation failed")?;pixels.resize(count*8,0);
 for row in 0..2{for col in 0..count{let px=first+col as i64;let py=y+row as i64;if px<clip[0]||px>=clip[2]||py<clip[1]||py>=clip[3]{continue;}let amount=if first==last{x2-x1}else if col==0{64-(x1&63)}else if col==count-1{x2&63}else{64};let coverage=((i64::from(if row==0{255-fraction}else{fraction})*amount)>>6)as u8;let pixel=if clip[0]==i64::MIN/2{let coverage=(u64::from(coverage)*opacity>>8)as u8;brush_premultiplied(color).map(|v|byte_mul(v,coverage))}else{base.map(|v|byte_mul(v,coverage))};pixels[(row*count+col)*4..][..4].copy_from_slice(&pixel);}}
 Ok(Some(Sample{x:first,y,image:GlyphImage{pixels:Arc::new(pixels),left:0,top:0,width:count,height:2,bytes:count*8},clip}))
}

#[cfg(test)]mod source_thin_line_tests{use super::*;
 #[test]fn actual_qt_thin_line_clip_branch_controls(){let data:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/paper-canvas-background-underline/qt-thin-line-controls.json")).unwrap();for case in data["cases"].as_array().unwrap(){let line=std::array::from_fn(|i|case["line"][i].as_f64().unwrap());let clip=if case["clipped"].as_bool().unwrap(){[0,0,8,8]}else{[i64::MIN/2,i64::MIN/2,i64::MAX/2,i64::MAX/2]};let sample=thin_line_sample(line,case["width"].as_f64().unwrap(),[30./255.,36./255.,44./255.,1.],clip).unwrap().unwrap();let mut pixels=vec![0u8;8*8*4];for y in 0..sample.image.height{for x in 0..sample.image.width{let dst=((sample.y as usize+y)*8+sample.x as usize+x)*4;let src=(y*sample.image.width+x)*4;pixels[dst..dst+4].copy_from_slice(&sample.image.pixels[src..src+4]);}}let expected=case["pixels"].as_array().unwrap().iter().map(|v|v.as_u64().unwrap()as u8).collect::<Vec<_>>();assert_eq!(pixels,expected,"line {:?} width {} clipped {}",line,case["width"],case["clipped"]);}}
}

#[cfg(target_os="macos")]
#[path="composer_text_canvas_probe.rs"] mod native_font_probe;
#[cfg(target_os="macos")]
pub(super) fn run_native_font_tiles(root: &std::path::Path) -> Result<(), String> {
    native_font_probe::run(root)
}

#[cfg(test)] mod physical_glyph_clip_tests {
 use super::*;
 #[test] fn glyph_presence_clip_intersects_masks_before_testing_presence() {
  let pixel=vec![30,36,44,128];
  let sample=||Sample{x:0,y:0,image:GlyphImage{pixels:Arc::new(pixel.clone()),left:0,top:0,width:1,height:1,bytes:4},clip:[0,0,1,1]};
  let mask=vec![ingetrazo_render::qt_gray::Span{x:0,y:0,len:1,coverage:1}];
  let single=CanvasPainter::physical_clip_glyph(sample(),std::slice::from_ref(&mask)).unwrap();
  assert_eq!(&*single.image.pixels,&pixel,"nonzero clip must retain glyph bytes");
  let combined=CanvasPainter::physical_clip_glyph(sample(),&[mask.clone(),mask.clone()]).unwrap();
  assert_eq!(&*combined.image.pixels,&vec![0;4],"rounded clip intersection is empty");
  let decoration=CanvasPainter::physical_clip_sample(sample(),&[mask]).unwrap();
  assert_eq!(&*decoration.image.pixels,&vec![0,0,0,1],"nonglyph samples retain fractional coverage");
 }
}
