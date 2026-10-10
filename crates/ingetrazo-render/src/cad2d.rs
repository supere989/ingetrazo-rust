// SPDX-License-Identifier: GPL-3.0-or-later
//! Precise authored CAD paint. Encoded source RGBA never crosses Color32.
//! Immutable geometry is shared/cached; palette and pose are independent uniforms.
use std::sync::{Arc,atomic::{AtomicU64,Ordering}};
#[derive(Clone,Copy,Debug,PartialEq)]pub struct EncodedRgba(pub [f32;4]);
impl EncodedRgba {
 pub fn from_bytes(rgba:[u8;4],opacity:f32)->Result<Self,String>{let c=Self([rgba[0]as f32/255.,rgba[1]as f32/255.,rgba[2]as f32/255.,rgba[3]as f32/255.*opacity]);c.validate()?;if !opacity.is_finite()||!(0. ..=1.).contains(&opacity){return Err("CAD2D opacity outside0..1".into());}Ok(c)}
 /// Qt6.12 solid QColor brush: QRgba64::premultiplied followed by
 /// rounded div257 to the eight-bit raster target, before coverage.
 /// https://github.com/qt/qtbase/blob/6.12/src/gui/painting/qrgba64.h
 /// Straight authored encoded RGBA remains unchanged. QRgb's div255
 /// shortcut differs at24 byte pairs and is not this brush operation.
 pub fn qt_premultiplied8(self)->Result<[f32;4],String>{
  self.validate()?;
  let rgba=self.0.map(|v|(v*65535.).round()as u64);let a=rgba[3];
  let to8=|v:u64|{let n=v+128;((n-(n>>8))>>8)as f32/255.};
  let mul=|v:u64|{let n=v*a;to8((n+(n>>16)+32768)>>16)};
  Ok([mul(rgba[0]),mul(rgba[1]),mul(rgba[2]),to8(a)])
 }
 pub fn validate(self)->Result<(),String>{if self.0.iter().all(|v|v.is_finite()&&(0. ..=1.).contains(v)){Ok(())}else{Err("CAD2D encoded RGBA must be finite0..1".into())}}
}
#[repr(C)]#[derive(Clone,Copy,Debug,PartialEq,bytemuck::Pod,bytemuck::Zeroable)]pub struct Vertex {pub position:[f32;2],pub coverage:f32}
#[derive(Clone,Copy,Debug,PartialEq)]pub struct Batch {pub first:u32,pub count:u32,pub color:EncodedRgba}
#[derive(Clone,Copy,Debug,PartialEq)]pub struct Pose {pub scale:[f32;2],pub translation:[f32;2]}
impl Default for Pose {fn default()->Self{Self{scale:[1.;2],translation:[0.;2]}}}
impl Pose {pub fn validate(self)->Result<(),String>{if self.scale.iter().chain(&self.translation).all(|v|v.is_finite()){Ok(())}else{Err("CAD2D pose must be finite".into())}}}
/// Original integer Qt coverage rows, before conversion to logical f32 vertices.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct SourceSpan {pub x:i32,pub y:i32,pub len:i32,pub coverage:u8}
#[derive(Debug)]
pub struct SourceSpans {rows:Arc<[SourceSpan]>,zoom:f32,ppp:f32,phase:[f64;2]}
impl SourceSpans {
 pub fn rows(&self)->&[SourceSpan]{&self.rows}
 pub fn zoom(&self)->f32{self.zoom}
 pub fn ppp(&self)->f32{self.ppp}
 pub fn phase(&self)->[f64;2]{self.phase}
 pub fn logical_bytes(&self)->u64{(self.rows.len()*std::mem::size_of::<SourceSpan>())as u64}
 pub fn translation(&self,pose:Pose,ppp:f32)->Result<[i32;2],String>{
  pose.validate()?;
  if pose.scale!=[self.zoom;2]||ppp!=self.ppp{return Err("Source span density/scale changed; producer rerasterization required".into());}
  let mut result=[0;2];for axis in 0..2{let value=f64::from(pose.translation[axis])*f64::from(ppp)-self.phase[axis];if !value.is_finite()||value.fract()!=0.||value.abs()>1_000_000.{return Err("Source span phase changed; producer rerasterization required".into());}result[axis]=value as i32;}Ok(result)
 }
 fn validated(g:&Geometry,rows:Vec<SourceSpan>,zoom:f32,ppp:f32,phase:[f64;2])->Result<Self,String>{
  let density=zoom*ppp;
  if !zoom.is_finite()||zoom<=0.||!ppp.is_finite()||ppp<=0.||!density.is_finite()||density<=0.||phase.iter().any(|p|!p.is_finite()||p.abs()>1_000_000.)||rows.len()>100_000||g.indices.len()!=rows.len()*6{return Err("Invalid immutable source span admission".into());}
  for (row,indices) in rows.iter().zip(g.indices.chunks_exact(6)){
   let right=row.x.checked_add(row.len).ok_or("Source span integer overflow")?;let bottom=row.y.checked_add(1).ok_or("Source span integer overflow")?;
   if row.len<=0||[row.x,row.y,right,bottom].iter().any(|p|p.unsigned_abs()>1_000_000)||indices[0]!=indices[3]||indices[2]!=indices[4]{return Err("Invalid source span quad topology/domain".into());}
   let expected=[[row.x,row.y],[right,row.y],[right,bottom],[row.x,bottom]];
   for (index,pixel) in [indices[0],indices[1],indices[2],indices[5]].into_iter().zip(expected){
    let expected=[((f64::from(pixel[0])-phase[0])/f64::from(density))as f32,((f64::from(pixel[1])-phase[1])/f64::from(density))as f32];let vertex=g.vertices[index as usize];
    if vertex.position!=expected||vertex.coverage!=f32::from(row.coverage)/255.{return Err("Source span metadata differs from immutable geometry".into());}
   }
  }
  Ok(Self{rows:rows.into(),zoom,ppp,phase})
 }
}
/// ID is allocated once for this immutable allocation. Changing palette/pose
/// retains ID; changing geometry constructs a fresh ID, preventing stale cache.
#[derive(Debug)]pub struct Geometry {id:u64,vertices:Arc<[Vertex]>,indices:Arc<[u32]>,ordered_draws:Option<Arc<[std::ops::Range<u32>]>>,bounds:[[f32;2];2],opaque_coverage:bool,source_spans:Option<Arc<SourceSpans>>}
impl Geometry {
 pub fn new(vertices:Vec<Vertex>,indices:Vec<u32>)->Result<Arc<Self>,String>{
  if vertices.len()>u32::MAX as usize||indices.len()>u32::MAX as usize{return Err("CAD2D geometry exceeds index domain".into());}
  if indices.len()%3!=0||indices.iter().any(|&i|i as usize>=vertices.len()){return Err("CAD2D triangle index bounds invalid".into());}
  if vertices.iter().any(|v|!v.position.iter().all(|p|p.is_finite())||!v.coverage.is_finite()||!(0. ..=1.).contains(&v.coverage)){return Err("CAD2D vertex position/coverage invalid".into());}
  let mut bounds=[[0.;2];2];if let Some(first)=vertices.first(){bounds=[first.position;2];for v in &vertices{for axis in 0..2{bounds[0][axis]=bounds[0][axis].min(v.position[axis]);bounds[1][axis]=bounds[1][axis].max(v.position[axis]);}}}
  static NEXT:AtomicU64=AtomicU64::new(1);let id=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|v|v.checked_add(1)).map_err(|_|"CAD2D geometry identity exhausted")?;
  let opaque_coverage=vertices.iter().all(|v|v.coverage==1.);
  Ok(Arc::new(Self{id,vertices:vertices.into(),indices:indices.into(),ordered_draws:None,bounds,opaque_coverage,source_spans:None}))
 }
 /// Pixel-aligned source span producer declares nonoverlapping ordered draw ranges.
 /// Metadata partitions all indices; a caller must not use this for arbitrary
 /// triangles or a later pose that overlaps previously disjoint source pixels.
 pub fn new_with_ordered_draws(vertices:Vec<Vertex>,indices:Vec<u32>,ranges:Vec<std::ops::Range<u32>>)->Result<Arc<Self>,String>{
  if ranges.len()>4096{return Err("CAD2D ordered draws exceed4096 admission".into());}
  let mut end=0;for range in &ranges{if range.start!=end||range.end<=range.start||range.start%3!=0||range.end%3!=0{return Err("Invalid CAD2D ordered draw partition".into());}end=range.end;}
  if end as usize!=indices.len(){return Err("CAD2D ordered draws do not cover indices".into());}
  let mut g=Self::new(vertices,indices)?;verify_disjoint_span_draws(&g,&ranges)?;Arc::get_mut(&mut g).unwrap().ordered_draws=Some(ranges.into());Ok(g)
 }
 /// Diagonal scale/translation preserves disjointness and shared boundaries.
 /// Refuse overflow before allocation/recording; source physical coordinates
 /// are bounded to the same million-pixel domain as the rasterizer.
 pub fn validate_ordered_pose(&self,pose:Pose,ppp:f32)->Result<(),String>{
  pose.validate()?;if !ppp.is_finite()||ppp<=0.{return Err("Invalid CAD2D physical density".into());}
  if self.ordered_draws.is_some(){for v in self.bounds{for axis in 0..2{let product=v[axis]*pose.scale[axis];let sum=product+pose.translation[axis];if !product.is_finite()||!sum.is_finite()||!(sum*2.).is_finite(){return Err("CAD2D pose overflows shader f32 intermediates".into());}let p=(f64::from(v[axis])*f64::from(pose.scale[axis])+f64::from(pose.translation[axis]))*f64::from(ppp);if !p.is_finite()||p.abs()>1_000_000.{return Err("CAD2D ordered pose exceeds bounded physical domain".into());}}}}Ok(())
 }
 pub fn source_spans(&self)->Option<&Arc<SourceSpans>>{self.source_spans.as_ref()}
 pub fn new_with_source_spans(vertices:Vec<Vertex>,indices:Vec<u32>,ranges:Vec<std::ops::Range<u32>>,spans:Vec<SourceSpan>,zoom:f32,ppp:f32,phase:[f64;2])->Result<Arc<Self>,String>{
  let mut geometry=Self::new_with_ordered_draws(vertices,indices,ranges)?;
  let source=SourceSpans::validated(&geometry,spans,zoom,ppp,phase)?;
  Arc::get_mut(&mut geometry).unwrap().source_spans=Some(Arc::new(source));Ok(geometry)
 }
 pub fn ordered_draws(&self)->Option<Arc<[std::ops::Range<u32>]>>{self.ordered_draws.clone()}
 /// Derived from every vertex and palette, never a caller flag.
 pub fn is_opaque(&self,batches:&[Batch])->bool{self.opaque_coverage&&batches.iter().all(|b|b.color.0[3]==1.)}
 pub fn id(&self)->u64{self.id}pub fn vertices(&self)->&[Vertex]{&self.vertices}pub fn indices(&self)->&[u32]{&self.indices}
 pub fn logical_bytes(&self)->u64{self.vertices.len()as u64*std::mem::size_of::<Vertex>()as u64+self.indices.len()as u64*4}
 pub fn validate_batches(&self,batches:&[Batch])->Result<(),String>{for b in batches{b.color.validate()?;if b.first%3!=0||b.count%3!=0||b.first.checked_add(b.count).is_none_or(|end|end as usize>self.indices.len()){return Err("CAD2D batch index bounds invalid".into());}}Ok(())}
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn opaque_admission(){let v=Vertex{position:[0.,0.],coverage:1.};let g=Geometry::new(vec![v;3],vec![0,1,2]).unwrap();let mut b=Batch{first:0,count:3,color:EncodedRgba([0.1,0.2,0.3,1.])};assert!(g.is_opaque(&[b]));b.color.0[3]=0.999999;assert!(!g.is_opaque(&[b]));b.color.0[3]=1.;assert!(!Geometry::new(vec![Vertex{coverage:0.999999,..v};3],vec![0,1,2]).unwrap().is_opaque(&[b]));}
 #[test]fn original_bytes_survive_without_linear_premultiplication_loss(){let rgba=EncodedRgba::from_bytes([37,94,156,128],1.).unwrap();assert_eq!(rgba.0,[37./255.,94./255.,156./255.,128./255.]);assert!(EncodedRgba::from_bytes([0,0,0,128],f32::NAN).is_err());}
 #[test]fn qt_premultiply_matches_exact_div255_for_all_channel_alpha_pairs(){
  for alpha in 0..=255u32{for channel in 0..=255u32{let c=EncodedRgba::from_bytes([channel as u8,channel as u8,channel as u8,alpha as u8],1.).unwrap();let p=c.qt_premultiplied8().unwrap();let expected=((channel*alpha+127)/255)as f32/255.;assert_eq!(p[..3],[expected;3],"channel={channel} alpha={alpha}");assert_eq!(p[3],alpha as f32/255.);}}
  assert_eq!(EncodedRgba::from_bytes([1,7,19,128],1.).unwrap().qt_premultiplied8().unwrap(),[1./255.,4./255.,10./255.,128./255.]);
 }
 #[test]fn immutable_geometry_identity_and_bounds_are_not_palette_keys(){let v=Vertex{position:[1.,2.],coverage:1.};let g=Geometry::new(vec![v;3],vec![0,1,2]).unwrap();let shared=g.clone();assert_eq!(g.id(),shared.id());let distinct=Geometry::new(vec![v;3],vec![0,1,2]).unwrap();assert_ne!(g.id(),distinct.id());let good=Batch{first:0,count:3,color:EncodedRgba::from_bytes([0,0,0,255],1.).unwrap()};g.validate_batches(&[good]).unwrap();assert!(g.validate_batches(&[Batch{first:u32::MAX,..good}]).is_err());assert!(Geometry::new(vec![v],vec![0,1,0]).is_err());assert!(Geometry::new(vec![Vertex{coverage:f32::NAN,..v}],vec![]).is_err());}
}

#[cfg(test)]mod ordering_admission{use super::*;
 #[test]fn partition_is_contiguous_bounded_and_complete(){let v=Vertex{position:[0.,0.],coverage:1.};let vertices=vec![v;3];let indices=vec![0,1,2,0,1,2];let g=Geometry::new_with_ordered_draws(vertices.clone(),indices.clone(),vec![0..3,3..6]).unwrap();assert_eq!(&*g.ordered_draws().unwrap(),&[0..3,3..6]);for ranges in [vec![0..3],vec![0..6,3..6],vec![3..6],vec![0..4,4..6],vec![0..0,0..6]]{assert!(Geometry::new_with_ordered_draws(vertices.clone(),indices.clone(),ranges).is_err());}}
}

fn verify_disjoint_span_draws(g:&Geometry,ranges:&[std::ops::Range<u32>])->Result<(),String>{
 use std::collections::BTreeMap;
 if g.indices.len()>600_000{return Err("CAD2D span proof exceeds100000 quad admission".into());}
 let key=|v:f32|{let bits=if v==0.{0}else{v.to_bits()};if bits>>31!=0{!bits}else{bits^(1<<31)}};
 for range in ranges{
  let indices=&g.indices[range.start as usize..range.end as usize];
  if indices.len()==3{continue;} // A single triangle cannot overlap itself.
  if indices.len()%6!=0{return Err("CAD2D draw requires canonical source span quads".into());}
  let mut rows=BTreeMap::<u32,(f32,BTreeMap<u32,f32>)>::new();
  for q in indices.chunks_exact(6){
   if q[0]!=q[3]||q[2]!=q[4]{return Err("CAD2D span triangulation is not canonical".into());}
   let [a,b,c,d]=[q[0],q[1],q[2],q[5]].map(|i|g.vertices[i as usize]);
   let [x0,y0]=a.position;let [x1,y1]=c.position;
   if !(x0<x1&&y0<y1)||b.position!=[x1,y0]||d.position!=[x0,y1]||[b,c,d].iter().any(|v|v.coverage!=a.coverage){return Err("CAD2D draw is not constant-coverage axis-aligned spans".into());}
   let ykey=key(y0);
   if !rows.contains_key(&ykey){
    if rows.range(..ykey).next_back().is_some_and(|(_,row)|row.0>y0)||rows.range(ykey..).next().is_some_and(|(next,_)|*next<key(y1)){return Err("CAD2D ordered draw has overlapping rows".into());}
    rows.insert(ykey,(y1,BTreeMap::new()));
   }
   let row=rows.get_mut(&ykey).unwrap();if row.0!=y1{return Err("CAD2D source row heights disagree".into());}
   let xkey=key(x0);if row.1.range(..=xkey).next_back().is_some_and(|(_,end)|*end>x0)||row.1.range(xkey..).next().is_some_and(|(start,_)|*start<key(x1)){return Err("CAD2D ordered draw contains overlapping spans".into());}
   row.1.insert(xkey,x1);
  }
 }Ok(())
}
#[cfg(test)]mod immutable_order_proof{use super::*;
 fn quad(x:f32)->[Vertex;4]{[[x,0.],[x+2.,0.],[x+2.,1.],[x,1.]].map(|position|Vertex{position,coverage:0.5})}
 #[test]fn rejects_same_draw_overlap_but_allows_source_ordered_barriers(){let vertices=quad(0.).into_iter().chain(quad(1.)).collect::<Vec<_>>();let indices=vec![0,1,2,0,2,3,4,5,6,4,6,7];assert!(Geometry::new_with_ordered_draws(vertices.clone(),indices.clone(),vec![0..12]).unwrap_err().contains("overlapping spans"));let g=Geometry::new_with_ordered_draws(vertices,indices,vec![0..6,6..12]).unwrap();assert!(g.validate_ordered_pose(Pose{scale:[-2.,0.75],translation:[0.25,0.3]},1.25).is_ok());assert!(g.validate_ordered_pose(Pose{scale:[f32::MAX;2],..Default::default()},2.).is_err());}
 #[test]fn canonical_shared_boundaries_are_safe(){let vertices=quad(0.).into_iter().chain(quad(2.)).collect::<Vec<_>>();let indices=vec![0,1,2,0,2,3,4,5,6,4,6,7];assert!(Geometry::new_with_ordered_draws(vertices,indices,vec![0..12]).is_ok());let v=Vertex{position:[0.,0.],coverage:1.};assert!(Geometry::new_with_ordered_draws(vec![v;3],vec![0,1,2,0,1,2],vec![0..6]).is_err());}
}

#[cfg(test)]mod shader_pose_bounds{use super::*;
 #[test]fn physical_f64_bounds_do_not_hide_shader_intermediate_overflow(){let v=Vertex{position:[f32::MAX,0.],coverage:1.};let g=Geometry::new_with_ordered_draws(vec![v;3],vec![0,1,2],vec![0..3]).unwrap();assert!(g.validate_ordered_pose(Pose{scale:[2.,1.],translation:[-f32::MAX,0.]},1e-34).unwrap_err().contains("f32 intermediates"));}
}

#[cfg(test)] mod original_span_admission {
 use super::*;
 fn make(zoom:f32,ppp:f32,phase:[f64;2],coverage:u8)->(Vec<Vertex>,Vec<u32>,Vec<SourceSpan>){
  let row=SourceSpan{x:2,y:3,len:4,coverage};let density=zoom*ppp;
  let vertices=[[2,3],[6,3],[6,4],[2,4]].map(|p|Vertex{position:[((f64::from(p[0])-phase[0])/f64::from(density))as f32,((f64::from(p[1])-phase[1])/f64::from(density))as f32],coverage:f32::from(coverage)/255.}).to_vec();
  (vertices,vec![0,1,2,0,2,3],vec![row])
 }
 #[test] fn exact_density_phase_and_original_bytes_are_immutable(){
  let (v,i,s)=make(1.5,1.25,[0.125,0.375],127);
  let g=Geometry::new_with_source_spans(v,i,vec![0..6],s,1.5,1.25,[0.125,0.375]).unwrap();let m=g.source_spans().unwrap();
  assert_eq!(m.rows()[0].coverage,127);
  let pose=Pose{scale:[1.5;2],translation:[8.1,16.3]};
  // f32 translations carry their true phase, not decimal display rounding.
  assert!(m.translation(pose,1.25).is_err());
  let pose=Pose{scale:[1.5;2],translation:[0.1,0.3]};assert!(m.translation(pose,1.25).is_err());
  let (v,i,s)=make(2.,1.5,[0.5,0.25],133);
  let g=Geometry::new_with_source_spans(v,i,vec![0..6],s,2.,1.5,[0.5,0.25]).unwrap();let m=g.source_spans().unwrap();
  assert_eq!(m.translation(Pose{scale:[2.;2],translation:[3.,1.5]},1.5).unwrap(),[4,2]);
  assert!(m.translation(Pose{scale:[2.;2],translation:[3.000001,1.5]},1.5).is_err());
  assert!(m.translation(Pose{scale:[2.;2],translation:[3.,1.5]},1.25).is_err());
 }
 #[test] fn altered_metadata_is_rejected_without_rounding(){
  let (v,i,mut s)=make(1.5,1.25,[20.125,30.375],127);s[0].coverage=128;
  assert!(Geometry::new_with_source_spans(v.clone(),i.clone(),vec![0..6],s,1.5,1.25,[20.125,30.375]).is_err());
  let (_,_,s)=make(1.5,1.25,[20.125,30.375],127);let mut wrong=v.clone();wrong[0].position[0]=f32::from_bits(wrong[0].position[0].to_bits()+1);
  assert!(Geometry::new_with_source_spans(wrong,i.clone(),vec![0..6],s.clone(),1.5,1.25,[20.125,30.375]).is_err());
  assert!(Geometry::new_with_source_spans(v,i,vec![0..6],s,1.5,1.25,[20.125,30.375]).is_ok());
 }
}
