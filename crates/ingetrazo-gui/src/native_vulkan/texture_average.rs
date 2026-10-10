//! Cold immutable source means. Candidate entries become visible only on publication.
use super::texture_cache::AssetId;
use ingetrazo_render::{source_texture_average::source_average_pixels,source_texture_decode::{decode,Limits}};
use std::collections::HashMap;
#[derive(Default,Clone,Copy,Debug)]
pub struct Stats { pub decodes:u64,pub hits:u64,pub retained_entries:usize }
pub struct Cache { entries:HashMap<AssetId,[f32;3]>,limits:Limits,max_entries:usize,pub stats:Stats }
#[derive(Default)]
pub struct Candidate { entries:HashMap<AssetId,[f32;3]> }
impl Cache {
 pub fn new(limits:Limits,max_entries:usize)->Self { Self{entries:HashMap::new(),limits,max_entries,stats:Stats::default()} }
 pub fn limits(&self)->Limits{self.limits}
 pub fn configure(&mut self,limits:Limits,max_entries:usize)->Result<(),String>{
  if max_entries<self.entries.len(){return Err("Average cache policy below retained entries".into());}
  self.limits=limits;self.max_entries=max_entries;Ok(())
 }
 pub fn reserve_publication(&mut self,candidate:&Candidate)->Result<(),String>{
  self.entries.try_reserve(candidate.entries.len()).map_err(|_|"Average cache publication admission".into())
 }
 pub fn prepare(&mut self,candidate:&mut Candidate,id:AssetId,bytes:&[u8])->Result<[f32;3],String>{
  if let Some(value)=candidate.entries.get(&id).or_else(||self.entries.get(&id)){self.stats.hits+=1;return Ok(*value);}
  if self.entries.len().checked_add(candidate.entries.len()).ok_or("Average cache entry overflow")? >= self.max_entries {return Err("Average cache retained admission".into());}
  candidate.entries.try_reserve(1).map_err(|_|"Average cache allocation admission")?;
  let image=decode(bytes,self.limits)?;
  let pixels=source_average_pixels(&image.rgba,image.width,image.height,image.alpha)?;
  let mut sums=[0u32;3];for pixel in pixels {for channel in 0..3{sums[channel]+=pixel[channel] as u32;}}
  // Source _set_color narrows the mean to a uniform float separately from shade.
  let mean=sums.map(|sum|(sum as f64/(255.0*16.0)) as f32);
  candidate.entries.insert(id,mean);self.stats.decodes+=1;Ok(mean)
 }
 pub fn publish(&mut self,candidate:Candidate,active:&std::collections::HashSet<AssetId>){

  self.entries.extend(candidate.entries);self.entries.retain(|id,_|active.contains(id));self.stats.retained_entries=self.entries.len();
 }
}

#[cfg(test)]
mod tests{
 use super::*;
 fn png()->Vec<u8>{
  let image=image::RgbaImage::from_pixel(4,4,image::Rgba([37,93,157,255]));
  let mut bytes=std::io::Cursor::new(Vec::new());
  image::DynamicImage::ImageRgba8(image).write_to(&mut bytes,image::ImageFormat::Png).unwrap();bytes.into_inner()
 }
 #[test]fn immutable_candidate_deduplicates_and_abandon_preserves_old(){
  let bytes=png();let mut ids=super::super::texture_cache::TextureCache::default();let id=ids.identify(&bytes).id();
  let mut cache=Cache::new(Limits::default(),2);let mut candidate=Candidate::default();
  let mean=cache.prepare(&mut candidate,id,&bytes).unwrap();assert_eq!(mean,[37.,93.,157.].map(|v|v/255.));
  assert_eq!(cache.prepare(&mut candidate,id,&bytes).unwrap(),mean);assert_eq!(cache.stats.decodes,1);
  drop(candidate);assert!(cache.entries.is_empty());
  let mut candidate=Candidate::default();cache.prepare(&mut candidate,id,&bytes).unwrap();
  cache.reserve_publication(&candidate).unwrap();cache.publish(candidate,&[id].into_iter().collect());
  let mut candidate=Candidate::default();assert_eq!(cache.prepare(&mut candidate,id,&bytes).unwrap(),mean);assert_eq!(cache.stats.decodes,2);
  assert!(candidate.entries.is_empty());assert_eq!(cache.entries.len(),1);
 }
 #[test]fn cold_budget_refusal_never_publishes(){
  let bytes=png();let mut ids=super::super::texture_cache::TextureCache::default();let id=ids.identify(&bytes).id();
  let mut cache=Cache::new(Limits{encoded_bytes:1,..Limits::default()},2);let mut candidate=Candidate::default();
  assert!(cache.prepare(&mut candidate,id,&bytes).is_err());assert!(candidate.entries.is_empty());assert!(cache.entries.is_empty());
 }
}
