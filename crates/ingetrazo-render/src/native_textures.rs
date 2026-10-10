//! Cold, platform-neutral texture geometry for direct Vulkan/Metal submission.
//! Back geometry follows the front prefix so physical contour extraction does
//! not accidentally double-count a separately painted back face.
use crate::buffer::{MeshRenderBuffer, SourceTextureRun};
use bytemuck::{Pod, Zeroable};
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct TextureVertex { pub position:[f32;3], pub uv:[f32;2] }
#[derive(Clone, Debug)]
pub struct Draw { pub first:u32, pub count:u32, pub source:SourceTextureRun }
#[derive(Clone, Debug, Default)]
pub struct Geometry { pub vertices:Vec<TextureVertex>, pub front_vertices:u32, pub draws:Vec<Draw> }
impl Geometry {
 pub fn prepare(buffer:&MeshRenderBuffer)->Result<Self,String>{
  let count=u32::try_from(buffer.indices.len()).map_err(|_|"Texture vertex count exceeds native draw range")?;
  if buffer.indices.len()%3!=0{return Err("Texture indices must form triangles".into());}
  let mut covered=vec![false;buffer.indices.len()];
  for run in &buffer.source_texture_runs {
   let end=run.first.checked_add(run.count).ok_or("Texture span overflow")?;
   if run.first%3!=0||run.count%3!=0||end>covered.len()||!run.opacity.is_finite()||!run.shade.is_finite(){return Err("Invalid texture paint span".into());}
   for slot in &mut covered[run.first..end]{if *slot{return Err("Overlapping texture paint spans".into());}*slot=true;}
  }
  if covered.iter().any(|v|!*v){return Err("Unclassified texture geometry".into());}
  // Validation completes before allocating/copying the GPU candidate.
  for &index in &buffer.indices {let v=buffer.vertices.get(index as usize).ok_or("Texture index out of bounds")?;if v.position.iter().chain(v.uv.iter()).any(|x|!x.is_finite()){return Err("Nonfinite texture position or UV".into());}}
  let mut result=Self{vertices:Vec::with_capacity(count as usize),..Self::default()};
  for back in [false,true]{
   for source in buffer.source_texture_runs.iter().filter(|r|r.back_side==back){
    let first=u32::try_from(result.vertices.len()).map_err(|_|"Texture draw offset overflow")?;
    for &index in &buffer.indices[source.first..source.first+source.count]{let v=buffer.vertices[index as usize];result.vertices.push(TextureVertex{position:v.position,uv:v.uv});}
    result.draws.push(Draw{first,count:source.count as u32,source:source.clone()});
   }
   if !back{result.front_vertices=result.vertices.len() as u32;}
  }
  Ok(result)
 }
 pub fn upload_bytes(&self)->&[u8]{bytemuck::cast_slice(&self.vertices)}
}
#[cfg(test)]mod tests {
 use super::*;use crate::buffer::Vertex;
 fn fixture()->MeshRenderBuffer {let mut b=MeshRenderBuffer::new();for x in [10.,20.,30.]{let v=Vertex::new([x,0.,0.],[0.;3],[1.;4],[x,0.]);b.push_triangle(v,v,v);}b.source_texture_runs=vec![(true,0),(false,3),(true,6)].into_iter().map(|(back,first)|SourceTextureRun{back_side:back,first,count:3,order:first,shade:0.75,opacity:0.99899999999,front_cull:false}).collect();b}
 #[test]fn exact_twenty_byte_contract_front_prefix_and_back_uv_are_retained(){let g=Geometry::prepare(&fixture()).unwrap();assert_eq!(std::mem::size_of::<TextureVertex>(),20);assert_eq!(g.front_vertices,3);assert_eq!(g.vertices.iter().map(|v|v.position[0]).collect::<Vec<_>>(),vec![20.,20.,20.,10.,10.,10.,30.,30.,30.]);assert_eq!(g.draws.iter().map(|d|d.first).collect::<Vec<_>>(),vec![0,3,6]);assert_eq!(g.draws[0].source.opacity,0.99899999999);assert_eq!(g.upload_bytes().len(),180);assert_eq!(g.vertices[3].uv,[10.,0.]);}
 #[test]fn malformed_spans_indices_and_nonfinite_uv_are_rejected(){let mut b=fixture();b.source_texture_runs[1].first=0;assert!(Geometry::prepare(&b).is_err());let mut b=fixture();b.indices[0]=u32::MAX;assert!(Geometry::prepare(&b).is_err());let mut b=fixture();b.vertices[0].uv[0]=f32::NAN;assert!(Geometry::prepare(&b).is_err());let mut b=fixture();b.source_texture_runs.pop();assert!(Geometry::prepare(&b).is_err());}
}
