//! One inactive original/suppressed draw inventory, sharing World resources.
//! No model clone or duplicate pipelines; switching requires the existing fence.
use super::*;
#[derive(Clone,PartialEq)]pub(super) struct Key{
 pub revision:(u64,u64,u64),pub texture:bool,pub average:bool,pub shadow:bool,
 pub shadow_extra:u64,pub cap:Option<([[f32;3];4],[f32;4])>,pub billboard:Option<[u32;7]>,
}
pub(super) struct Snapshot{
 pub key:Key,pub membership:(Vec<usize>,Vec<usize>),
 vertices:Option<BufferId>,indices:Option<BufferId>,count:u32,
 transparent_vertices:Option<BufferId>,opacity_runs:Vec<OpacityRun>,ordinary_runs:Vec<(u32,u32,bool)>,default_back_runs:Vec<(u32,u32)>,consolidated_opaque_count:usize,
 edge_vertices:Option<BufferId>,edge_count:u32,ghost_buffers:[Option<BufferId>;3],ghost_counts:[u32;3],contour_batches:Vec<ContourBatch>,contours:PreparedContours,
 pub textures:super::super::textures::DrawState,
}
impl Snapshot{
 pub fn take(world:&mut World)->Self{
  Self{key:world.base_key.take().expect("admitted active base key"),membership:std::mem::take(&mut world.base_membership),
   vertices:world.vertices.take(),indices:world.indices.take(),count:world.count,
   transparent_vertices:world.transparent_vertices.take(),opacity_runs:std::mem::take(&mut world.opacity_runs),ordinary_runs:std::mem::take(&mut world.ordinary_runs),default_back_runs:std::mem::take(&mut world.default_back_runs),consolidated_opaque_count:world.consolidated_opaque_count,
   edge_vertices:world.edge_vertices.take(),edge_count:world.edge_count,ghost_buffers:std::mem::take(&mut world.ghost_buffers),ghost_counts:world.ghost_counts,contour_batches:std::mem::take(&mut world.contour_batches),contours:world.contours.as_mut().unwrap().take_state(),textures:world.textures.as_mut().unwrap().take_draw_state()}
 }
 pub fn swap(&mut self,world:&mut World){
  let active=world.base_key.replace(self.key.clone()).expect("admitted active base key");self.key=active;
  std::mem::swap(&mut self.membership,&mut world.base_membership);
  macro_rules! swap{($($field:ident),*)=>{$(std::mem::swap(&mut self.$field,&mut world.$field);)*}}
  swap!(vertices,indices,count,transparent_vertices,opacity_runs,ordinary_runs,default_back_runs,consolidated_opaque_count,edge_vertices,edge_count,ghost_buffers,ghost_counts,contour_batches);
  world.contours.as_mut().unwrap().swap_state(&mut self.contours);world.textures.as_mut().unwrap().swap_draw_state(&mut self.textures);
 }
 pub fn append_buffers(&self,ids:&mut Vec<BufferId>)->Result<(),String>{
  ids.try_reserve(self.contour_batches.len()+self.textures.buffers().len()+7).map_err(|_|"Inactive base retirement admission")?;
  ids.extend(self.contour_batches.iter().filter(|b|b.owned).map(|b|b.vertices));
  ids.extend([self.vertices,self.indices,self.transparent_vertices,self.edge_vertices].into_iter().flatten());ids.extend(self.ghost_buffers.into_iter().flatten());ids.extend_from_slice(self.textures.buffers());Ok(())
 }
}
