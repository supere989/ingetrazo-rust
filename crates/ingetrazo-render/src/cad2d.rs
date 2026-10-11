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
/// Literal encoded premultiplied bytes already produced by a source painter.
/// No straight-color reconstruction, transfer conversion, or premultiplication.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct PremultipliedRgba8([u8;4]);
impl PremultipliedRgba8 {
 pub fn new(bytes:[u8;4])->Result<Self,String>{if bytes[..3].iter().any(|&v|v>bytes[3]){return Err("CAD2D premultiplied channel exceeds alpha".into());}Ok(Self(bytes))}
 pub fn bytes(self)->[u8;4]{self.0}
 pub fn shader_units(self)->[f32;4]{self.0.map(|v|f32::from(v)/255.)}
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
  Self::validate_rows(g,&g.indices,&rows,zoom,ppp,phase)?;
  Ok(Self{rows:rows.into(),zoom,ppp,phase})
 }
 fn validate_rows(g:&Geometry,indices:&[u32],rows:&[SourceSpan],zoom:f32,ppp:f32,phase:[f64;2])->Result<(),String>{
  let density=zoom*ppp;
  if !zoom.is_finite()||zoom<=0.||!ppp.is_finite()||ppp<=0.||!density.is_finite()||density<=0.||phase.iter().any(|p|!p.is_finite()||p.abs()>1_000_000.)||rows.len()>100_000||indices.len()!=rows.len()*6{return Err("Invalid immutable source span admission".into());}
  for (row,indices) in rows.iter().zip(indices.chunks_exact(6)){
   let right=row.x.checked_add(row.len).ok_or("Source span integer overflow")?;let bottom=row.y.checked_add(1).ok_or("Source span integer overflow")?;
   if row.len<=0||[row.x,row.y,right,bottom].iter().any(|p|p.unsigned_abs()>1_000_000)||indices[0]!=indices[3]||indices[2]!=indices[4]{return Err("Invalid source span quad topology/domain".into());}
   let expected=[[row.x,row.y],[right,row.y],[right,bottom],[row.x,bottom]];
   for (index,pixel) in [indices[0],indices[1],indices[2],indices[5]].into_iter().zip(expected){
    let expected=[((f64::from(pixel[0])-phase[0])/f64::from(density))as f32,((f64::from(pixel[1])-phase[1])/f64::from(density))as f32];let vertex=g.vertices[index as usize];
    if vertex.position!=expected||vertex.coverage!=f32::from(row.coverage)/255.{return Err("Source span metadata differs from immutable geometry".into());}
   }
  }
  Ok(())
 }
}
/// Annotation-only mixed layers. Direct layers may overlap internally only
/// because admission requires every participating batch to write opaque pixels
/// without destination access. Coverage layers retain the ordinary span proof.
#[derive(Debug)]
pub struct AnnotationLayers {span_layers:Arc<[(std::ops::Range<u32>,Arc<[SourceSpan]>)]>,grid:SourceSpans,direct:Arc<[std::ops::Range<u32>]>,union_quads:Arc<[std::ops::Range<u32>]>}
impl AnnotationLayers {
 pub fn ppp(&self)->f32{self.grid.ppp()}
 pub fn translation(&self,pose:Pose,ppp:f32)->Result<[i32;2],String>{self.grid.translation(pose,ppp)}
 pub fn span_layers(&self)->&[(std::ops::Range<u32>,Arc<[SourceSpan]>)]{&self.span_layers}
 pub fn direct_ranges(&self)->&[std::ops::Range<u32>]{&self.direct}
 pub fn overlaps_direct(&self,first:u32,count:u32)->bool{
  if count==0{return false;}
  let index=self.direct.partition_point(|range|range.end<=first);
  self.direct.get(index).is_some_and(|range|range.start<first.saturating_add(count))
 }
}
#[derive(Debug)]
pub enum AnnotationLayer {
 /// One admitted axis-aligned quad: the fragment operator evaluates source coverage once.
 UnionQuad{range:std::ops::Range<u32>},
 Spans {range:std::ops::Range<u32>,rows:Vec<SourceSpan>},
 Opaque {range:std::ops::Range<u32>},
}
/// ID is allocated once for this immutable allocation. Changing palette/pose
/// retains ID; changing geometry constructs a fresh ID, preventing stale cache.
#[derive(Debug)]pub struct Geometry {id:u64,vertices:Arc<[Vertex]>,indices:Arc<[u32]>,ordered_draws:Option<Arc<[std::ops::Range<u32>]>>,bounds:[[f32;2];2],opaque_coverage:bool,premultiplied_palette:Option<Arc<[PremultipliedRgba8]>>,source_spans:Option<Arc<SourceSpans>>,annotation_layers:Option<Arc<AnnotationLayers>>}
impl Geometry {
 pub fn new(vertices:Vec<Vertex>,indices:Vec<u32>)->Result<Arc<Self>,String>{
  if vertices.len()>u32::MAX as usize||indices.len()>u32::MAX as usize{return Err("CAD2D geometry exceeds index domain".into());}
  if indices.len()%3!=0||indices.iter().any(|&i|i as usize>=vertices.len()){return Err("CAD2D triangle index bounds invalid".into());}
  if vertices.iter().any(|v|!v.position.iter().all(|p|p.is_finite())||!v.coverage.is_finite()||!(0. ..=1.).contains(&v.coverage)){return Err("CAD2D vertex position/coverage invalid".into());}
  let mut bounds=[[0.;2];2];if let Some(first)=vertices.first(){bounds=[first.position;2];for v in &vertices{for axis in 0..2{bounds[0][axis]=bounds[0][axis].min(v.position[axis]);bounds[1][axis]=bounds[1][axis].max(v.position[axis]);}}}
  static NEXT:AtomicU64=AtomicU64::new(1);let id=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|v|v.checked_add(1)).map_err(|_|"CAD2D geometry identity exhausted")?;
  let opaque_coverage=vertices.iter().all(|v|v.coverage==1.);
  Ok(Arc::new(Self{id,vertices:vertices.into(),indices:indices.into(),ordered_draws:None,bounds,opaque_coverage,premultiplied_palette:None,source_spans:None,annotation_layers:None}))
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
  if self.ordered_draws.is_some(){for v in self.bounds{for axis in 0..2{let product=v[axis]*pose.scale[axis];let sum=product+pose.translation[axis];let physical=sum*ppp;if !product.is_finite()||!sum.is_finite()||!(sum*2.).is_finite()||!physical.is_finite()||physical.abs()>1_000_000.{return Err("CAD2D pose overflows shader f32 intermediates".into());}let p=(f64::from(v[axis])*f64::from(pose.scale[axis])+f64::from(pose.translation[axis]))*f64::from(ppp);if !p.is_finite()||p.abs()>1_000_000.{return Err("CAD2D ordered pose exceeds bounded physical domain".into());}}}}Ok(())
 }
 pub fn source_spans(&self)->Option<&Arc<SourceSpans>>{self.source_spans.as_ref()}
 pub fn annotation_layers(&self)->Option<&Arc<AnnotationLayers>>{self.annotation_layers.as_ref()}
 pub fn new_with_annotation_layers(vertices:Vec<Vertex>,indices:Vec<u32>,layers:Vec<AnnotationLayer>,ppp:f32)->Result<Arc<Self>,String>{
  if layers.len()>100_000||!ppp.is_finite()||ppp<=0.{return Err("Annotation layer count/density admission exceeded".into());}
  let mut g=Self::new(vertices,indices)?;let mut end=0;let mut direct=Vec::new();let mut ranges=Vec::new();let mut union_quads=Vec::new();let mut span_layers=Vec::new();
  span_layers.try_reserve_exact(layers.len()).map_err(|_|"Annotation span layer allocation failed")?;
  union_quads.try_reserve(layers.len()).map_err(|_|"Annotation union metadata allocation failed")?;
  direct.try_reserve(layers.len()).map_err(|_|"Annotation direct metadata allocation failed")?;
  ranges.try_reserve(layers.len()).map_err(|_|"Annotation ordered metadata allocation failed")?;
  for layer in layers{
   let range=match &layer{AnnotationLayer::Spans{range,..}|AnnotationLayer::Opaque{range}|AnnotationLayer::UnionQuad{range}=>range};
   if range.start!=end||range.end<=range.start||range.end%3!=0||range.end as usize>g.indices.len(){return Err("Annotation layers do not partition geometry".into());}end=range.end;
   match &layer{
    AnnotationLayer::UnionQuad{..}=>{
     verify_single_union_quad(&g,range)?;union_quads.push(range.clone());direct.push(range.clone());
    }
    AnnotationLayer::Opaque{..}=>{
     if g.indices[range.start as usize..range.end as usize].iter().any(|&i|g.vertices[i as usize].coverage!=1.){return Err("Direct annotation coverage must be opaque".into());}
     direct.push(range.clone());
    }
    AnnotationLayer::Spans{rows,..}=>{
     verify_disjoint_span_draws(&g,std::slice::from_ref(range))?;
     SourceSpans::validate_rows(&g,&g.indices[range.start as usize..range.end as usize],rows,1.,ppp,[0.;2])?;
    }
   }
   ranges.push(range.clone());
   if let AnnotationLayer::Spans{range,rows}=layer {span_layers.push((range,rows.into()));}
  }
  if end as usize!=g.indices.len(){return Err("Annotation layers do not cover geometry".into());}
  let grid=SourceSpans{rows:Arc::from([]),zoom:1.,ppp,phase:[0.;2]};
  let inner=Arc::get_mut(&mut g).unwrap();inner.ordered_draws=Some(ranges.into());inner.annotation_layers=Some(Arc::new(AnnotationLayers{span_layers:span_layers.into(),grid,direct:direct.into(),union_quads:union_quads.into()}));Ok(g)
 }
 pub fn new_with_source_spans(vertices:Vec<Vertex>,indices:Vec<u32>,ranges:Vec<std::ops::Range<u32>>,spans:Vec<SourceSpan>,zoom:f32,ppp:f32,phase:[f64;2])->Result<Arc<Self>,String>{
  let mut geometry=Self::new_with_ordered_draws(vertices,indices,ranges)?;
  let source=SourceSpans::validated(&geometry,spans,zoom,ppp,phase)?;
  Arc::get_mut(&mut geometry).unwrap().source_spans=Some(Arc::new(source));Ok(geometry)
 }
 /// Exact palette is immutable geometry policy, with its own geometry identity.
 /// Consumers must include geometry ID in palette-cache identity for this path.
 pub fn new_with_premultiplied_source_spans(vertices:Vec<Vertex>,indices:Vec<u32>,ranges:Vec<std::ops::Range<u32>>,spans:Vec<SourceSpan>,zoom:f32,ppp:f32,phase:[f64;2],colors:Vec<PremultipliedRgba8>,batches:&[Batch])->Result<Arc<Self>,String>{
  if colors.len()!=batches.len(){return Err("CAD2D exact palette batch count mismatch".into());}
  let mut geometry=Self::new_with_source_spans(vertices,indices,ranges,spans,zoom,ppp,phase)?;
  Arc::get_mut(&mut geometry).unwrap().premultiplied_palette=Some(colors.into());
  geometry.validate_batches(batches)?;Ok(geometry)
 }
 pub fn premultiplied_palette(&self)->Option<&Arc<[PremultipliedRgba8]>>{self.premultiplied_palette.as_ref()}
 pub fn palette_cache_identity(&self)->u64{if self.premultiplied_palette.is_some(){self.id}else{0}}
 pub fn resolve_batch_colors(&self,batches:&[Batch])->Result<Vec<[f32;4]>,String>{
  self.validate_batches(batches)?;
  if let Some(colors)=&self.premultiplied_palette{Ok(colors.iter().map(|c|c.shader_units()).collect())}else{batches.iter().map(|b|b.color.qt_premultiplied8()).collect()}
 }
 pub fn ordered_draws(&self)->Option<Arc<[std::ops::Range<u32>]>>{self.ordered_draws.clone()}
 /// Derived from every vertex and palette, never a caller flag.
 pub fn is_opaque(&self,batches:&[Batch])->bool{self.opaque_coverage&&self.premultiplied_palette.as_ref().map_or_else(||batches.iter().all(|b|b.color.0[3]==1.),|colors|colors.len()==batches.len()&&colors.iter().all(|c|c.bytes()[3]==255))}
 pub fn id(&self)->u64{self.id}pub fn vertices(&self)->&[Vertex]{&self.vertices}pub fn indices(&self)->&[u32]{&self.indices}
 pub fn logical_bytes(&self)->u64{self.vertices.len()as u64*std::mem::size_of::<Vertex>()as u64+self.indices.len()as u64*4}
 pub fn validate_batches(&self,batches:&[Batch])->Result<(),String>{if self.premultiplied_palette.as_ref().is_some_and(|colors|colors.len()!=batches.len()){return Err("CAD2D exact palette batch count mismatch".into());}for (batch_index,b) in batches.iter().enumerate(){b.color.validate()?;if b.first%3!=0||b.count%3!=0||b.first.checked_add(b.count).is_none_or(|end|end as usize>self.indices.len()){return Err("CAD2D batch index bounds invalid".into());}
 if self.annotation_layers.as_ref().is_some_and(|layers|layers.overlaps_direct(b.first,b.count)){
  let alpha=self.premultiplied_palette.as_ref().map_or(b.color.0[3]==1.,|p|p[batch_index].bytes()[3]==255);
  if !alpha&&!{let ranges=&self.annotation_layers.as_ref().unwrap().union_quads;let i=ranges.partition_point(|r|r.end<=b.first);ranges.get(i).is_some_and(|r|r.start==b.first&&r.end==b.first+b.count)}{return Err("Direct annotation batch must be opaque or one admitted union quad".into());}
 }
 }Ok(())}
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn alpha_union_quad_rejects_intradraw_overlap(){
  let v=[[0.,0.],[10.,0.],[0.,10.],[10.,10.]].map(|position|Vertex{position,coverage:1.}).to_vec();
  let good=Geometry::new_with_annotation_layers(v.clone(),vec![0,1,2,2,1,3],vec![AnnotationLayer::UnionQuad{range:0..6}],1.).unwrap();
  let b=Batch{first:0,count:6,color:EncodedRgba::from_bytes([37,93,157,128],1.).unwrap()};assert!(good.validate_batches(&[b]).is_ok());
  assert!(Geometry::new_with_annotation_layers(v,vec![0,1,2,0,1,2],vec![AnnotationLayer::UnionQuad{range:0..6}],1.).is_err());
 }
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

fn verify_single_union_quad(g:&Geometry,range:&std::ops::Range<u32>)->Result<(),String>{
 let indices=&g.indices[range.start as usize..range.end as usize];
 if indices.len()!=6||indices[2]!=indices[3]||indices[1]!=indices[4]{return Err("Union quad topology invalid".into());}
 let v=[indices[0],indices[1],indices[2],indices[5]].map(|i|g.vertices[i as usize]);
 let p=v.map(|v|v.position);
 if v.iter().any(|v|v.coverage!=1.)||p[0][0]!=p[2][0]||p[1][0]!=p[3][0]||p[0][1]!=p[1][1]||p[2][1]!=p[3][1]||p[0][0]>=p[1][0]||p[0][1]>=p[2][1]{return Err("Union quad must be one nonoverlapping rectangle".into());}Ok(())
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

#[cfg(test)]mod premultiplied_tests {
 use super::*;
 fn exact(bytes:[u8;4])->(Arc<Geometry>,Vec<Batch>){
  let vertices=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]].map(|position|Vertex{position,coverage:1.}).to_vec();
  let batches=vec![Batch{first:0,count:6,color:EncodedRgba([0.;4])}];
  let g=Geometry::new_with_premultiplied_source_spans(vertices,vec![0,1,2,0,2,3],vec![0..6],vec![SourceSpan{x:0,y:0,len:1,coverage:255}],1.,1.,[0.;2],vec![PremultipliedRgba8::new(bytes).unwrap()],&batches).unwrap();(g,batches)
 }
 #[test]fn literal_bytes_survive_every_valid_channel_alpha_pair(){
  for a in 0..=255u8{for c in 0..=a{let bytes=[c,c,c,a];let p=PremultipliedRgba8::new(bytes).unwrap();assert_eq!(p.shader_units().map(|v|(v*255.).round()as u8),bytes);}}
  for bytes in [[1,0,0,0],[128,0,0,127],[0,255,0,128]]{assert!(PremultipliedRgba8::new(bytes).is_err());}
 }
 #[test]fn exact_palette_is_geometry_identity_and_never_re_premultiplied(){
  let(g,b)=exact([17,31,63,100]);let(other,_)=exact([1,2,3,4]);
  assert_eq!(g.resolve_batch_colors(&b).unwrap(),vec![[17./255.,31./255.,63./255.,100./255.]]);
  assert_ne!(g.palette_cache_identity(),other.palette_cache_identity());assert!(!g.is_opaque(&b));
  assert!(g.resolve_batch_colors(&[]).unwrap_err().contains("count mismatch"));
  let plain=Geometry::new(vec![Vertex{position:[0.;2],coverage:1.};3],vec![0,1,2]).unwrap();let batches=[Batch{first:0,count:3,color:EncodedRgba::from_bytes([17,31,63,100],1.).unwrap()}];
  assert_eq!(plain.palette_cache_identity(),0);assert_eq!(plain.resolve_batch_colors(&batches).unwrap(),vec![batches[0].color.qt_premultiplied8().unwrap()]);
 }
}

#[cfg(test)]mod annotation_mixed_admission {
 use super::*;
 #[test]fn direct_overlap_requires_opaque_palette_and_grid_identity(){
  let vertices=vec![Vertex{position:[0.,0.],coverage:1.},Vertex{position:[2.,0.],coverage:1.},Vertex{position:[0.,2.],coverage:1.}];
  let g=Geometry::new_with_annotation_layers(vertices.clone(),vec![0,1,2,0,1,2],vec![AnnotationLayer::Opaque{range:0..6}],1.).unwrap();
  let b=Batch{first:0,count:6,color:EncodedRgba::from_bytes([10,20,30,255],1.).unwrap()};
  g.validate_batches(&[b]).unwrap();
  assert!(g.validate_batches(&[Batch{color:EncodedRgba::from_bytes([10,20,30,254],1.).unwrap(),..b}]).is_err());
  let layout=g.annotation_layers().unwrap();assert_eq!(layout.translation(Pose::default(),1.).unwrap(),[0;2]);
  assert!(layout.translation(Pose{translation:[0.25,0.],..Default::default()},1.).is_err());
  assert!(layout.translation(Pose::default(),1.25).is_err());
  let mut partial=vertices;partial[0].coverage=0.5;
  assert!(Geometry::new_with_annotation_layers(partial,vec![0,1,2],vec![AnnotationLayer::Opaque{range:0..3}],1.).is_err());
 }
 #[test]fn coverage_layers_still_require_literal_source_span_geometry(){
  let v=[[0.,0.],[2.,0.],[2.,1.],[0.,1.]].map(|position|Vertex{position,coverage:0.5}).to_vec();
  let i=vec![0,1,2,0,2,3];let row=SourceSpan{x:0,y:0,len:2,coverage:128};
  assert!(Geometry::new_with_annotation_layers(v,i,vec![AnnotationLayer::Spans{range:0..6,rows:vec![row]}],1.).is_err());
 }
}

#[cfg(test)]mod annotation_source_owner_tests{
 use super::*;
 #[test]fn mixed_layers_retain_literal_rows_and_partition_identity(){
  let row=SourceSpan{x:7,y:11,len:3,coverage:173};let ppp=1.25;
  let mut vertices=Vec::new();for [x,y]in [[7.,11.],[10.,11.],[10.,12.],[7.,12.]]{vertices.push(Vertex{position:[x/ppp,y/ppp],coverage:173./255.});}
  vertices.extend([[20.,20.],[21.,20.],[20.,21.]].map(|position|Vertex{position,coverage:1.}));
  let g=Geometry::new_with_annotation_layers(vertices,vec![0,1,2,0,2,3,4,5,6],vec![AnnotationLayer::Spans{range:0..6,rows:vec![row]},AnnotationLayer::Opaque{range:6..9}],ppp).unwrap();
  let layers=g.annotation_layers().unwrap();assert_eq!(layers.span_layers().len(),1);assert_eq!(layers.span_layers()[0].0,0..6);let actual=layers.span_layers()[0].1[0];assert_eq!((actual.x,actual.y,actual.len,actual.coverage),(7,11,3,173));assert_eq!(layers.direct_ranges(),&[6..9]);assert!(layers.translation(Pose::default(),ppp).is_ok());assert!(layers.translation(Pose::default(),1.).is_err());
 }
}
