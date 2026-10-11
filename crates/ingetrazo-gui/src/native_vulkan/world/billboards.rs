// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera-facing source receiver packets. Ordinary scene geometry is immutable.
use glam::{DMat4,DVec3,Vec3};
use ingetrazo_core::{scene::Scene,groups::Group,camera::OrbitCamera};
use ingetrazo_render::buffer::{SourceTextureRun,TextureBuffers,Vertex};
#[derive(Clone)]
pub(super) struct Run {pub path:Option<String>,pub color:[f32;3],pub positions:Vec<([f32;3],[f32;2])>}
pub(super) struct Packet {pub runs:Vec<Run>}
fn placed(p:DVec3,m:&[f32;16])->Result<Vec3,String>{
 let [x,y,z]=p.as_vec3().to_array();let w=((m[3]*x+m[7]*y)+m[11]*z)+m[15];
 let p: [f32;3]=std::array::from_fn(|r|{let n=((m[r]*x+m[4+r]*y)+m[8+r]*z)+m[12+r];if w==1.{n}else{n/w}});
 if !p.iter().all(|v|v.is_finite()){return Err("Billboard placement arithmetic admission".into());}Ok(Vec3::from_array(p))
}
fn path(t:&serde_json::Value)->Option<String>{t.get("embed").or_else(||t.get("path")).and_then(|v|v.as_str()).filter(|s|!s.is_empty()).map(str::to_owned)}
impl Packet {
 pub(super) fn build(scene:&Scene,camera:&OrbitCamera)->Result<Self,String>{
  let snapshot=crate::navigation_ray::camera_snapshot(camera).ok_or("Billboard camera admission")?;
  let eye=Vec3::from_array(snapshot.eye);let target=camera.target.as_vec3();let mut runs=Vec::new();
  fn visit(g:&Group,parent:DMat4,scene:&Scene,camera:&OrbitCamera,eye:Vec3,target:Vec3,runs:&mut Vec<Run>)->Result<(),String>{
   if g.hidden||!scene.is_layer_visible(g.layer.as_deref()){return Ok(());}
   let world=parent*g.transform;
   if g.billboard {
    let m=world.as_mat4().to_cols_array();let mut vertices=Vec::new();vertices.try_reserve_exact(g.mesh.vertices.len()).map_err(|_|"Billboard vertex admission")?;
    for v in &g.mesh.vertices{vertices.push(placed(v.position,&m)?);}
    if vertices.is_empty(){return Ok(());}
    let mut lo=Vec3::splat(f32::INFINITY);let mut hi=Vec3::splat(f32::NEG_INFINITY);for &p in &vertices{lo=lo.min(p);hi=hi.max(p);}
    let anchor=Vec3::new(((lo.x as f64+hi.x as f64)*0.5)as f32,((lo.y as f64+hi.y as f64)*0.5)as f32,lo.z);
    let direction=if camera.perspective{eye-anchor}else{eye-target};
    let mesh=g.native_document.get("billboard").and_then(|v|v.as_str())==Some("mesh");
    if !mesh {
     let Some(texture)=g.mesh.faces.iter().find_map(|f|f.attrs.get("texture").and_then(path))else{return Ok(());};
     let width=(hi.x as f64-lo.x as f64).hypot(hi.y as f64-lo.y as f64);let height=hi.z as f64-lo.z as f64;
     if width<1e-9||height<1e-9{return Ok(());}
     let mut d=Vec3::new(direction.x,direction.y,0.);if d.length()<1e-6{d=Vec3::X;}d=super::shadows::qt_normalized(d);
     let right=Vec3::new(-d.y,d.x,0.);let c0=anchor-right*(width as f32*0.5);let c1=anchor+right*(width as f32*0.5);let up=Vec3::Z*height as f32;let corners=[c0,c1,c1+up,c0+up];let uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]];
     let mut positions=Vec::new();positions.try_reserve_exact(6).map_err(|_|"Billboard quad admission")?;for i in [0,1,2,0,2,3]{positions.push((corners[i].to_array(),uv[i]));}runs.try_reserve(1).map_err(|_|"Billboard run admission")?;runs.push(Run{path:Some(texture),color:[1.;3],positions});
    } else {
     let mut biggest=None;for(f,_)in g.mesh.faces.iter().enumerate(){let area=g.mesh.face_area(f);if biggest.is_none_or(|(_,best)|area>best){biggest=Some((f,area));}}
     let Some((f,_))=biggest else{return Ok(());};let base=world.transform_vector3(g.mesh.face_normal(f));let length=base.x.hypot(base.y);if length<=1e-9{return Ok(());}
     let nh=[base.x/length,base.y/length];let dl=(direction.x as f64).hypot(direction.y as f64);let d=if dl<1e-9{nh}else{[direction.x as f64/dl,direction.y as f64/dl]};let cos=nh[0]*d[0]+nh[1]*d[1];let sin=nh[0]*d[1]-nh[1]*d[0];
     for(fid,face)in g.mesh.faces.iter().enumerate(){
      let texture=face.attrs.get("texture");let key=texture.and_then(path);let c=face.attrs.get("color").and_then(|v|v.as_array());let color=std::array::from_fn(|i|c.and_then(|v|v.get(i)).and_then(|v|v.as_f64()).unwrap_or([0.96,0.95,0.925][i])as f32);
      let normal=world.transform_vector3(g.mesh.face_normal(fid));let axes=texture.map(|t|ingetrazo_render::pipeline::face_uv_axes(t,normal));
      let tris=g.mesh.triangulate_face(fid);let mut positions=Vec::new();positions.try_reserve_exact(tris.len().checked_mul(3).ok_or("Billboard triangle count overflow")?).map_err(|_|"Billboard triangles admission")?;
      for(a,b,c)in tris{for local in [a,b,c]{let v=placed(local,&m)?;let x=v.x as f64-anchor.x as f64;let y=v.y as f64-anchor.y as f64;let p=[(anchor.x as f64+cos*x-sin*y)as f32,(anchor.y as f64+sin*x+cos*y)as f32,v.z];let uv=axes.map_or([0.,0.],|(gu,cu,gv,cv)|[(gu.dot(v.as_dvec3())+cu)as f32,(gv.dot(v.as_dvec3())+cv)as f32]);positions.push((p,uv));}}
      runs.try_reserve(1).map_err(|_|"Billboard run admission")?;runs.push(Run{path:key,color,positions});
     }
    }
    return Ok(());
   }
   for c in &g.children{visit(c,world,scene,camera,eye,target,runs)?;}Ok(())
  }
  for g in &scene.groups{visit(g,DMat4::IDENTITY,scene,camera,eye,target,&mut runs)?;}
  for r in &runs{if !r.color.iter().all(|v|v.is_finite())||!r.positions.iter().all(|(p,uv)|p.iter().chain(uv).all(|v|v.is_finite())){return Err("Billboard final packet arithmetic admission".into());}}
  Ok(Self{runs})
 }
}

use ash::vk;
use std::sync::Arc;
use crate::native_vulkan::{textures::{Textures,Candidate as TextureCandidate,Publication,VertexRefresh},resources::Resources,budget::VulkanBudget,VulkanContext};
type Key=([u64;2],[u32;7]);
pub(super) enum Candidate {Cold(TextureCandidate,Publication,Key),Pose(VertexRefresh,Key),Warm}
pub(super) struct Owner {textures:Textures,key:Option<Key>}
impl Packet{
 fn textures(self)->Result<TextureBuffers,String>{
  let mut out=TextureBuffers::default();out.try_reserve(self.runs.len()).map_err(|_|"Billboard material identity admission")?;
  for(index,run)in self.runs.into_iter().enumerate(){
   let solid=run.path.is_none();let key=run.path.unwrap_or_else(||format!("@billboard-color-{index}"));let order=out.take_order();let buffer=out.entry(key).or_default();
   if buffer.source_billboard_color.is_some()&&buffer.source_billboard_color!=Some(run.color){return Err("Billboard solid color identity admission".into());}
   if solid{buffer.source_billboard_color=Some(run.color);}
   let first=buffer.indices.len();buffer.vertices.try_reserve(run.positions.len()).map_err(|_|"Billboard vertex admission")?;buffer.indices.try_reserve(run.positions.len()).map_err(|_|"Billboard index admission")?;buffer.source_texture_runs.try_reserve(1).map_err(|_|"Billboard run admission")?;
   for(position,uv)in run.positions{let id:u32=buffer.vertices.len().try_into().map_err(|_|"Billboard index overflow")?;buffer.vertices.push(Vertex{position,uv,..Default::default()});buffer.indices.push(id);}
   buffer.source_texture_runs.push(SourceTextureRun{first,count:buffer.indices.len()-first,order,back_side:false,shade:1.,opacity:1.,front_cull:false});
  }Ok(out)
 }
}
impl Owner{
 pub(super) fn new(context:&VulkanContext,render_pass:vk::RenderPass,samples:vk::SampleCountFlags,budget:Arc<VulkanBudget>)->Result<Self,String>{Ok(Self{textures:Textures::new_billboard(context,render_pass,samples,budget)?,key:None})}
 pub(super) fn prepare(&mut self,context:&VulkanContext,resources:&mut Resources,scene:&Scene,camera:&OrbitCamera,epoch:u64,fence:vk::Fence,completed:u64)->Result<Candidate,String>{
  let snapshot=crate::navigation_ray::camera_snapshot(camera).ok_or("Billboard camera admission")?;
  let target=camera.target.as_vec3().to_array();let key=([scene.version,epoch],[u32::from(camera.perspective),snapshot.eye[0].to_bits(),snapshot.eye[1].to_bits(),snapshot.eye[2].to_bits(),target[0].to_bits(),target[1].to_bits(),target[2].to_bits()]);
  if self.key==Some(key){return Ok(Candidate::Warm);}
  let buffers=Packet::build(scene,camera)?.textures()?;
  if self.key.is_some_and(|old|old.0==key.0){return Ok(Candidate::Pose(self.textures.prepare_vertex_refresh(context,resources,&buffers,fence,completed,false)?,key));}
  let candidate=self.textures.prepare_mode(context,resources,&buffers,&scene.embedded_textures,scene,fence,completed,u64::MAX,false)?;
  let proof=match self.textures.preflight_publication(context,resources,&candidate,fence,completed,&[],&[]){Ok(proof)=>proof,Err(error)=>{self.textures.abort(resources,candidate,fence,completed)?;return Err(error);}};
  Ok(Candidate::Cold(candidate,proof,key))
 }
 pub(super) fn publish(&mut self,resources:&mut Resources,scene:&Scene,completed:u64,candidate:Candidate){match candidate{Candidate::Warm=>{},Candidate::Cold(c,p,key)=>{self.textures.publish_admitted(resources,c,p,completed,scene);self.key=Some(key);},Candidate::Pose(c,key)=>{self.textures.publish_vertex_refresh(resources,c);self.key=Some(key);}}}
 pub(super) fn abort(&mut self,resources:&mut Resources,candidate:Candidate,fence:vk::Fence,completed:u64)->Result<(),String>{match candidate{Candidate::Warm=>Ok(()),Candidate::Cold(c,_,_)=>self.textures.abort(resources,c,fence,completed),Candidate::Pose(c,_)=>self.textures.abort_vertex_refresh(resources,c,fence,completed)}}
 pub(super) fn record(&mut self,resources:&mut Resources,command:vk::CommandBuffer,serial:u64,mvp:&[f32;16],plane:&[f32;4],clip:u32,height:u32)->Result<(),String>{self.textures.record(resources,command,serial,crate::native_vulkan::textures::Phase::Opaque,mvp,plane,clip,height,false)}
}
