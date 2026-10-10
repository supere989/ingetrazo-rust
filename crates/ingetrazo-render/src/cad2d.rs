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
/// ID is allocated once for this immutable allocation. Changing palette/pose
/// retains ID; changing geometry constructs a fresh ID, preventing stale cache.
#[derive(Debug)]pub struct Geometry {id:u64,vertices:Arc<[Vertex]>,indices:Arc<[u32]>}
impl Geometry {
 pub fn new(vertices:Vec<Vertex>,indices:Vec<u32>)->Result<Arc<Self>,String>{
  if vertices.len()>u32::MAX as usize||indices.len()>u32::MAX as usize{return Err("CAD2D geometry exceeds index domain".into());}
  if indices.len()%3!=0||indices.iter().any(|&i|i as usize>=vertices.len()){return Err("CAD2D triangle index bounds invalid".into());}
  if vertices.iter().any(|v|!v.position.iter().all(|p|p.is_finite())||!v.coverage.is_finite()||!(0. ..=1.).contains(&v.coverage)){return Err("CAD2D vertex position/coverage invalid".into());}
  static NEXT:AtomicU64=AtomicU64::new(1);let id=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|v|v.checked_add(1)).map_err(|_|"CAD2D geometry identity exhausted")?;
  Ok(Arc::new(Self{id,vertices:vertices.into(),indices:indices.into()}))
 }
 pub fn id(&self)->u64{self.id}pub fn vertices(&self)->&[Vertex]{&self.vertices}pub fn indices(&self)->&[u32]{&self.indices}
 pub fn logical_bytes(&self)->u64{self.vertices.len()as u64*std::mem::size_of::<Vertex>()as u64+self.indices.len()as u64*4}
 pub fn validate_batches(&self,batches:&[Batch])->Result<(),String>{for b in batches{b.color.validate()?;if b.first%3!=0||b.count%3!=0||b.first.checked_add(b.count).is_none_or(|end|end as usize>self.indices.len()){return Err("CAD2D batch index bounds invalid".into());}}Ok(())}
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn original_bytes_survive_without_linear_premultiplication_loss(){let rgba=EncodedRgba::from_bytes([37,94,156,128],1.).unwrap();assert_eq!(rgba.0,[37./255.,94./255.,156./255.,128./255.]);assert!(EncodedRgba::from_bytes([0,0,0,128],f32::NAN).is_err());}
 #[test]fn qt_premultiply_matches_exact_div255_for_all_channel_alpha_pairs(){
  for alpha in 0..=255u32{for channel in 0..=255u32{let c=EncodedRgba::from_bytes([channel as u8,channel as u8,channel as u8,alpha as u8],1.).unwrap();let p=c.qt_premultiplied8().unwrap();let expected=((channel*alpha+127)/255)as f32/255.;assert_eq!(p[..3],[expected;3],"channel={channel} alpha={alpha}");assert_eq!(p[3],alpha as f32/255.);}}
  assert_eq!(EncodedRgba::from_bytes([1,7,19,128],1.).unwrap().qt_premultiplied8().unwrap(),[1./255.,4./255.,10./255.,128./255.]);
 }
 #[test]fn immutable_geometry_identity_and_bounds_are_not_palette_keys(){let v=Vertex{position:[1.,2.],coverage:1.};let g=Geometry::new(vec![v;3],vec![0,1,2]).unwrap();let shared=g.clone();assert_eq!(g.id(),shared.id());let distinct=Geometry::new(vec![v;3],vec![0,1,2]).unwrap();assert_ne!(g.id(),distinct.id());let good=Batch{first:0,count:3,color:EncodedRgba::from_bytes([0,0,0,255],1.).unwrap()};g.validate_batches(&[good]).unwrap();assert!(g.validate_batches(&[Batch{first:u32::MAX,..good}]).is_err());assert!(Geometry::new(vec![v],vec![0,1,0]).is_err());assert!(Geometry::new(vec![Vertex{coverage:f32::NAN,..v}],vec![]).is_err());}
}
