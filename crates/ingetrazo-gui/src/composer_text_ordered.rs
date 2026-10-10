// SPDX-License-Identifier: GPL-3.0-or-later
//! Cached exact premultiplied glyph runs, kept in operative source paint order.
use super::Sample;
use std::sync::Arc;
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch,EncodedRgba,PremultipliedRgba8,SourceSpan,Pose};
#[derive(Clone)]pub(super) struct Ordered {parts:Vec<Part>,pub bytes:usize}
#[derive(Clone)]struct Part {geometry:Arc<Geometry>,batches:Arc<[Batch]>,handle:crate::composer_backend::Handle}
const CHUNK_RUNS:usize=4096;
const ITEM_BYTES:usize=32*1024*1024;
// Requested chunk storage, independently of allocator metadata. Arc conversion
// may temporarily coexist with these Vecs: admit two copies for the cold peak.
const CHUNK_ARRAY_BYTES:usize=CHUNK_RUNS*(4*std::mem::size_of::<Vertex>()+6*4+8+std::mem::size_of::<SourceSpan>()+4+std::mem::size_of::<Batch>());
const CHUNK_PEAK_BYTES:usize=CHUNK_ARRAY_BYTES*2+CHUNK_RUNS*16;
#[derive(Default)]struct Builder {vertices:Vec<Vertex>,indices:Vec<u32>,ranges:Vec<std::ops::Range<u32>>,spans:Vec<SourceSpan>,colors:Vec<PremultipliedRgba8>,batches:Vec<Batch>,range_start:u32}
impl Builder {
 // Reserve a single fixed-size cold chunk. Charge actual Rust-visible capacity,
 // including the backend resolved float palette; allocator headers and GPU
 // residency belong to their independent allocator/renderer budgets.
 fn reserve_chunk(&mut self)->Result<(),String>{
  if self.vertices.capacity()!=0{return Ok(());}
  self.vertices.try_reserve_exact(CHUNK_RUNS*4).map_err(|_|"Ordered vertices allocation failed")?;
  self.indices.try_reserve_exact(CHUNK_RUNS*6).map_err(|_|"Ordered indices allocation failed")?;
  self.ranges.try_reserve_exact(CHUNK_RUNS).map_err(|_|"Ordered ranges allocation failed")?;
  self.spans.try_reserve_exact(CHUNK_RUNS).map_err(|_|"Ordered spans allocation failed")?;
  self.colors.try_reserve_exact(CHUNK_RUNS).map_err(|_|"Ordered colors allocation failed")?;
  self.batches.try_reserve_exact(CHUNK_RUNS).map_err(|_|"Ordered batches allocation failed")?;
  Ok(())
 }
 fn capacity_charge(&self)->Result<usize,String>{
  let arrays=[self.vertices.capacity()*std::mem::size_of::<Vertex>(),self.indices.capacity()*4,self.ranges.capacity()*8,self.spans.capacity()*std::mem::size_of::<SourceSpan>(),self.colors.capacity()*4,self.batches.capacity()*std::mem::size_of::<Batch>()];
  arrays.into_iter().try_fold(0usize,|n,v|n.checked_add(v).ok_or("Ordered capacity overflow".into())).and_then(|n|n.checked_add(self.batches.capacity()*16).ok_or("Ordered owner charge overflow".into()))
 }

 fn barrier(&mut self)->Result<(),String>{let end=self.indices.len()as u32;if end>self.range_start{self.ranges.try_reserve(1).map_err(|_|"Ordered glyph barriers allocation failed")?;self.ranges.push(self.range_start..end);self.range_start=end;}Ok(())}
 fn finish(&mut self,factory:&crate::composer_backend::Factory,pose:Pose,phase:[f64;2],ppp:f32)->Result<Option<(Part,usize)>,String>{
  if self.indices.is_empty(){return Ok(None);}self.barrier()?;
  let capacity_charge=self.capacity_charge()?;let mut b=std::mem::take(self);let palette_bytes=b.colors.len()*4;let batch_bytes=b.batches.len()*std::mem::size_of::<Batch>();let range_bytes=b.ranges.len()*8;
  let geometry=Geometry::new_with_premultiplied_source_spans(b.vertices,b.indices,b.ranges,b.spans,1.,ppp,phase,b.colors,&b.batches)?;
  let bytes=usize::try_from(geometry.logical_bytes()).map_err(|_|"Ordered glyph byte overflow")?.checked_add(geometry.source_spans().unwrap().logical_bytes()as usize).and_then(|n|n.checked_add(palette_bytes+batch_bytes+range_bytes)).ok_or("Ordered glyph byte overflow")?;
  geometry.validate_ordered_pose(pose,ppp)?;geometry.source_spans().unwrap().translation(pose,ppp)?;
  let batches:Arc<[Batch]>=std::mem::take(&mut b.batches).into();let handle=factory.handle(geometry.clone(),batches.clone(),pose)?;
  let owned=handle.retained_cpu_bytes()?.checked_add(std::mem::size_of::<Geometry>()+std::mem::size_of::<ingetrazo_render::cad2d::SourceSpans>()).ok_or("Ordered owner byte overflow")?;
  // Retained cache charge deliberately includes cold array capacity and a
  // actual backend-owned records, not merely geometry logical_bytes.
  Ok(Some((Part{geometry,batches,handle},bytes.max(capacity_charge).checked_add(owned).ok_or("Ordered owner byte overflow")?)))
 }
 fn run(&mut self,x:i32,y:i32,len:i32,color:[u8;4],phase:[f64;2],ppp:f32)->Result<(),String>{
  let rgba=PremultipliedRgba8::new(color)?;self.reserve_chunk()?;let first=self.indices.len()as u32;let base=self.vertices.len()as u32;
  let right=x.checked_add(len).ok_or("Ordered glyph right overflow")?;let bottom=y.checked_add(1).ok_or("Ordered glyph bottom overflow")?;if len<=0||[x,y,right,bottom].iter().any(|v|v.unsigned_abs()>1_000_000){return Err("Ordered glyph span domain exceeded".into());}let positions=[[x,y],[right,y],[right,bottom],[x,bottom]];
  self.vertices.try_reserve(4).map_err(|_|"Ordered glyph vertices allocation failed")?;self.indices.try_reserve(6).map_err(|_|"Ordered glyph indices allocation failed")?;self.spans.try_reserve(1).map_err(|_|"Ordered glyph spans allocation failed")?;self.colors.try_reserve(1).map_err(|_|"Ordered glyph palette allocation failed")?;self.batches.try_reserve(1).map_err(|_|"Ordered glyph batches allocation failed")?;
  self.vertices.extend(positions.map(|p|Vertex{position:[((f64::from(p[0])-phase[0])/f64::from(ppp))as f32,((f64::from(p[1])-phase[1])/f64::from(ppp))as f32],coverage:1.}));self.indices.extend([base,base+1,base+2,base,base+2,base+3]);self.spans.push(SourceSpan{x,y,len,coverage:255});self.colors.push(rgba);self.batches.push(Batch{first,count:6,color:EncodedRgba([0.;4])});Ok(())
 }
}
impl Ordered {
 pub fn build(samples:&[Sample],anchor:[i64;2],phase:[f64;2],ppp:f32,factory:&crate::composer_backend::Factory,pose:Pose)->Result<Self,String>{
  if !ppp.is_finite()||ppp<=0.{return Err("Invalid ordered glyph density".into());}pose.validate()?;
  let owner_bound=factory.retained_cpu_bytes_bound(CHUNK_RUNS)?.checked_add(std::mem::size_of::<Geometry>()+std::mem::size_of::<ingetrazo_render::cad2d::SourceSpans>()+std::mem::size_of::<Part>()).ok_or("Ordered owner bound overflow")?;
  let cold_bound=CHUNK_PEAK_BYTES.checked_add(owner_bound).ok_or("Ordered cold bound overflow")?;
  let mut parts=Vec::new();parts.try_reserve_exact(64).map_err(|_|"Ordered parts allocation failed")?;
  let mut builder=Builder::default();let mut bytes=parts.capacity()*std::mem::size_of::<Part>();let mut runs=0usize;
  for sample in samples {
   for row in 0..sample.image.height{let y=sample.y+row as i64;if y<sample.clip[1]||y>=sample.clip[3]{continue;}let mut col=0usize;
    while col<sample.image.width{
     let x=sample.x+col as i64;let src=&sample.image.pixels[(row*sample.image.width+col)*4..][..4];let color:[u8;4]=src.try_into().unwrap();let mut end=col+1;
     while end<sample.image.width&&sample.image.pixels[(row*sample.image.width+end)*4..][..4]==color{end+=1;}
     let left=x.max(sample.clip[0]);let right=(sample.x+end as i64).min(sample.clip[2]);
     if color[3]!=0&&left<right{
      runs=runs.checked_add(1).filter(|&n|n<=262144).ok_or("Ordered glyph run budget exceeded")?;
      // Count is a separate admission cap; actual capacity is charged per chunk.
      if runs>262144{return Err("Ordered glyph run budget exceeded".into());}
      if builder.batches.len()==4096{if let Some((part,n))=builder.finish(factory,pose,phase,ppp)?{bytes=bytes.checked_add(n).ok_or("Ordered glyph byte overflow")?;parts.try_reserve(1).map_err(|_|"Ordered glyph parts allocation failed")?;parts.push(part);}}
      let x=i32::try_from(left-anchor[0]).map_err(|_|"Ordered glyph x outside signed domain")?;let y=i32::try_from(y-anchor[1]).map_err(|_|"Ordered glyph y outside signed domain")?;let len=i32::try_from(right-left).map_err(|_|"Ordered glyph width outside signed domain")?;if builder.vertices.capacity()==0&&bytes.checked_add(cold_bound).is_none_or(|n|n>ITEM_BYTES){return Err("Ordered glyph cold chunk admission exceeded".into());}builder.reserve_chunk()?;if bytes.checked_add(builder.capacity_charge()?).is_none_or(|n|n>ITEM_BYTES){return Err("Ordered glyph item capacity budget exceeded".into());}builder.run(x,y,len,color,phase,ppp)?;
     }col=end;
    }
   }builder.barrier()?;
  }
  if let Some((part,n))=builder.finish(factory,pose,phase,ppp)?{bytes=bytes.checked_add(n).ok_or("Ordered glyph byte overflow")?;parts.try_reserve(1).map_err(|_|"Ordered glyph parts allocation failed")?;parts.push(part);}
  if bytes>ITEM_BYTES{return Err("Ordered glyph item byte budget exceeded".into());}Ok(Self{parts,bytes})
 }
 /// Engineering byte observation of the actual immutable GPU records.
 /// Does not execute GPU work or substitute for native presentation acceptance.
 pub fn observe(&self,size:[usize;2],pose:Pose,ppp:f32)->Result<Vec<u8>,String>{
  let bytes=size[0].checked_mul(size[1]).and_then(|n|n.checked_mul(4)).filter(|&n|n<=16*1024*1024).ok_or("Ordered observation byte budget exceeded")?;let mut frame=Vec::new();frame.try_reserve_exact(bytes).map_err(|_|"Ordered observation allocation failed")?;frame.resize(bytes,255);
  for part in &self.parts{let source=part.geometry.source_spans().unwrap();let shift=source.translation(pose,ppp)?;let colors=part.geometry.premultiplied_palette().unwrap();
   for (batch,color)in part.batches.iter().zip(colors.iter()){let color=color.bytes();for row in &source.rows()[batch.first as usize/6..(batch.first+batch.count)as usize/6]{let y=i64::from(row.y)+i64::from(shift[1]);if y<0||y>=size[1]as i64{continue;}for x in i64::from(row.x)+i64::from(shift[0])..i64::from(row.x+row.len)+i64::from(shift[0]){if x>=0&&x<size[0]as i64{let index=(y as usize*size[0]+x as usize)*4;super::source_over(&mut frame[index..index+4],&color);}}}}
  }Ok(frame)
 }
 pub fn part_count(&self)->usize{self.parts.len()}
 pub fn paint(&self,painter:&egui::Painter,pose:Pose)->Result<(),String>{
  for part in &self.parts{part.geometry.validate_ordered_pose(pose,painter.ctx().pixels_per_point())?;}
  for part in &self.parts{part.handle.update(part.geometry.clone(),part.batches.clone(),pose)?;}
  for part in &self.parts{painter.add(part.handle.shape(painter.clip_rect()));}Ok(())
 }
}

#[cfg(test)]mod tests {
 use super::*;
 fn sample(color:[u8;4])->Sample{Sample{x:0,y:0,image:super::super::GlyphImage{pixels:Arc::new(color.to_vec()),left:0,top:0,width:1,height:1,bytes:4},clip:[-100,-100,100,100]}}
 fn ink(coverage:u8)->[u8;4]{[super::super::byte_mul(30,coverage),super::super::byte_mul(36,coverage),super::super::byte_mul(44,coverage),coverage]}
 fn factory()->crate::composer_backend::Factory{crate::composer_backend::Factory::MetalProducer(Default::default())}
 fn pixel(ordered:&Ordered)->[u8;4]{let mut result=[255;4];for part in &ordered.parts{let colors=part.geometry.resolve_batch_colors(&part.batches).unwrap();for (batch,color)in part.batches.iter().zip(colors){let bytes=color.map(|v|(v*255.).round()as u8);for _ in &part.geometry.source_spans().unwrap().rows()[batch.first as usize/6..(batch.first+batch.count)as usize/6]{super::super::source_over(&mut result,&bytes);}}}result}
 #[test]fn exact_qt_ordered_composition_and_aggregate_negative(){
  let reference:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/coretext-native-eighth/qt-compositing-controls.json")).unwrap();let mut negatives=0;
  for case in reference["cases"].as_array().unwrap(){let a=case["coverage"][0].as_u64().unwrap()as u8;let b=case["coverage"][1].as_u64().unwrap()as u8;let samples=[sample(ink(a)),sample(ink(b))];let ordered=Ordered::build(&samples,[0;2],[0.;2],1.,&factory(),Pose::default()).unwrap();let expected=std::array::from_fn(|i|case["source_direct"][i].as_u64().unwrap()as u8);assert_eq!(pixel(&ordered),expected);
   let mut aggregate=[0;4];for sample in &samples{super::super::source_over(&mut aggregate,&sample.image.pixels);}let mut old=[255;4];super::super::source_over(&mut old,&aggregate);if old!=expected{negatives+=1;}assert_eq!(old,std::array::from_fn(|i|case["source_grouped"][i].as_u64().unwrap()as u8));
  }assert_eq!(negatives,14);
 }
 #[test]fn phase_clip_and_warm_callback_identity(){
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});let native=ingetrazo_render::cad_callback::Factory::default();let factory=crate::composer_backend::Factory::MetalProducer(native.clone());let base=[10.37f32,20.23f32];let ppp=1.25f32;let physical=base.map(|v|f64::from(v)*f64::from(ppp));let anchor=physical.map(|v|v.floor()as i64);let phase=std::array::from_fn(|i|physical[i]-anchor[i]as f64);let mut source=sample([10,20,30,100]);source.x=anchor[0];source.y=anchor[1];source.clip=[source.x,source.y,source.x+1,source.y+1];let pose=Pose{scale:[1.;2],translation:base};let ordered=Ordered::build(&[source],anchor,phase,ppp,&factory,pose).unwrap();let id=ordered.parts[0].geometry.id();assert_eq!(ordered.parts[0].geometry.source_spans().unwrap().translation(pose,ppp).unwrap(),anchor.map(|v|v as i32));
  ctx.set_pixels_per_point(ppp);for _ in 0..3{let _=ctx.run(Default::default(),|ctx|ordered.paint(&ctx.layer_painter(egui::LayerId::background()),pose).unwrap());}assert_eq!(ordered.parts[0].geometry.id(),id);assert_eq!(native.metrics().handles,1);assert_eq!(native.metrics().updates,0);assert_eq!(native.metrics().unchanged,3);
 }
 #[test]fn chunk_barriers_and_invalid_bytes(){
  assert!(Ordered::build(&[sample([101,0,0,100])],[0;2],[0.;2],1.,&factory(),Pose::default()).is_err());let mut s=sample([0;4]);s.image.width=4097;s.image.pixels=Arc::new((0..4097).flat_map(|i|[i as u8%2,0,0,255]).collect());s.image.bytes=s.image.pixels.len();s.clip=[0,0,5000,1];let ordered=Ordered::build(&[s],[0;2],[0.;2],1.,&factory(),Pose::default()).unwrap();assert_eq!(ordered.parts.len(),2);assert_eq!(ordered.parts[0].batches.len(),4096);assert_eq!(ordered.parts[1].batches.len(),1);assert_eq!(ordered.parts[0].geometry.ordered_draws().unwrap().len(),1);
  assert!(ordered.bytes>=CHUNK_ARRAY_BYTES*2);
  assert!(ordered.bytes<=ITEM_BYTES);
  assert_eq!(ordered.parts[0].geometry.premultiplied_palette().unwrap().len(),4096);
  assert!(ordered.parts[0].batches.windows(2).all(|b|b[0].first+b[0].count==b[1].first));
 }
}

#[cfg(target_os="macos")]
pub(super) struct MetalObservation {
 metal:ingetrazo_metal::NativeMetal,
 painter:ingetrazo_metal::egui_painter::EguiPainter,
 pool:ingetrazo_metal::compute_canvas::Pool,
}
#[cfg(target_os="macos")]
impl MetalObservation {
 pub fn new()->Result<Self,String>{let metal=ingetrazo_metal::NativeMetal::new()?;let painter=ingetrazo_metal::egui_painter::EguiPainter::new_compute(&metal)?;Ok(Self{metal,painter,pool:Default::default()})}
 pub fn capture(&mut self,ordered:&Ordered,size:[usize;2],ppp:f32)->Result<(Vec<u8>,serde_json::Value),String>{
  let mut primitives=Vec::new();
  for part in &ordered.parts {
   let egui::Shape::Callback(callback)=part.handle.shape(egui::Rect::EVERYTHING) else{return Err("Ordered callback shape absent".into());};
   primitives.push(egui::ClippedPrimitive{clip_rect:egui::Rect::EVERYTHING,primitive:egui::epaint::Primitive::Callback(callback)});
  }
  let render=|owner:&mut Self|->Result<Vec<u8>,String>{let prepared=owner.painter.prepare(&owner.metal,&primitives,ppp)?;let mut submitted=owner.painter.submit_diagnostic_with_pool(&owner.metal,prepared,size[0],size[1],false,&mut owner.pool)?;submitted.wait_readback(std::time::Duration::from_secs(5))};
  let pixels=render(self)?;let before=self.painter.compute_stats().ok_or("Compute counters absent")?;let allocations=self.pool.stats.allocations;
  for _ in 0..3{if render(self)?!=pixels{return Err("Ordered native Metal warm pixels changed".into());}}
  let after=self.painter.compute_stats().ok_or("Compute counters absent")?;
  if before.deep_admissions!=after.deep_admissions||before.plan_uploads!=after.plan_uploads||before.palette_uploads!=after.palette_uploads||allocations!=self.pool.stats.allocations{return Err("Ordered native Metal warm upload/allocation changed".into());}
  Ok((pixels,serde_json::json!({"backend":"production native Metal compute callback consumer","warm_frames":3,"deep_admissions":after.deep_admissions,"warm_deep_admissions":after.deep_admissions-before.deep_admissions,"plan_uploads":after.plan_uploads,"palette_uploads":after.palette_uploads,"canvas_allocations":self.pool.stats.allocations,"warm_unchanged":true,"presentation_claim":false,"physical_mac_claim":false})))
 }
}
