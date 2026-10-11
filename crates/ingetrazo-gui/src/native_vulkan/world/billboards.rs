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
fn face_basis(group:&Group,face:usize,vertices:&[Vec3])->(Vec3,f64){
 let ring=&group.mesh.faces[face].vertices;let mut n=DVec3::ZERO;
 for(&a,&b)in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()){let a=vertices[a].as_dvec3();let b=vertices[b].as_dvec3();n.x+=(a.y-b.y)*(a.z+b.z);n.y+=(a.z-b.z)*(a.x+b.x);n.z+=(a.x-b.x)*(a.y+b.y);}
 let n=n.as_vec3();let scale=n.abs().max_element();let length=if scale==0.{0.}else{let v=n/scale;scale*((v.x*v.x+v.y*v.y)+v.z*v.z).sqrt()};
 (if length<1e-9{Vec3::Z}else{n/length},0.5*length as f64)
}
fn qt_dot(a:Vec3,b:Vec3)->f32{(a.x*b.x+a.y*b.y)+a.z*b.z}
fn uv_axes(tex:&serde_json::Value,normal:Vec3,m:&[f32;16])->(Vec3,f64,Vec3,f64){
 if let Some(uvw)=tex.get("uvw").and_then(|v|v.as_array()).filter(|v|v.len()==8){
  let matrix: [[f32;4];4]=std::array::from_fn(|c|std::array::from_fn(|r|m[c*4+r]));let inverse=crate::navigation_ray::affine_inverse(matrix);
  let axis=|base:usize|{let g:[f64;3]=std::array::from_fn(|i|uvw[base+i].as_f64().unwrap_or(0.));let c=uvw[base+3].as_f64().unwrap_or(0.);if let Some(inv)=inverse{let gp:[f64;3]=std::array::from_fn(|r|(inv[r][0]as f64*g[0]+inv[r][1]as f64*g[1])+inv[r][2]as f64*g[2]);(Vec3::new(gp[0]as f32,gp[1]as f32,gp[2]as f32),c-((gp[0]*m[12]as f64+gp[1]*m[13]as f64)+gp[2]*m[14]as f64))}else{(Vec3::new(g[0]as f32,g[1]as f32,g[2]as f32),c)}};
  let(u,cu)=axis(0);let(v,cv)=axis(4);return(u,cu,v,cv);
 }
 let(u,v)=ingetrazo_render::pipeline::projection_basis(normal.as_dvec3());let(mut u,mut v)=(u.as_vec3(),v.as_vec3());let angle=tex.get("rot").and_then(|v|v.as_f64()).unwrap_or(0.).to_radians();if angle!=0.{let(c,s)=(angle.cos()as f32,angle.sin()as f32);(u,v)=(u*c+v*s,v*c-u*s);}
 let scale=|key:&str|tex.get(key).and_then(|v|v.as_f64()).filter(|&v|v!=0.).unwrap_or(1.)as f32;
 (u/scale("sw"),0.,v/scale("sh"),0.)
}
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
     let mut biggest=None;for(f,_)in g.mesh.faces.iter().enumerate(){let (_,area)=face_basis(g,f,&vertices);if biggest.is_none_or(|(_,best)|area>best){biggest=Some((f,area));}}
     let Some((f,_))=biggest else{return Ok(());};let (base,_)=face_basis(g,f,&vertices);let length=(base.x as f64).hypot(base.y as f64);if length<=1e-9{return Ok(());}
     let nh=[base.x as f64/length,base.y as f64/length];let dl=(direction.x as f64).hypot(direction.y as f64);let d=if dl<1e-9{nh}else{[direction.x as f64/dl,direction.y as f64/dl]};let cos=nh[0]*d[0]+nh[1]*d[1];let sin=nh[0]*d[1]-nh[1]*d[0];
     let mut local_runs:Vec<Run>=Vec::new();
     for(fid,face)in g.mesh.faces.iter().enumerate(){
      let texture=face.attrs.get("texture");let key=texture.and_then(path);let c=face.attrs.get("color").and_then(|v|v.as_array());let color=std::array::from_fn(|i|c.and_then(|v|v.get(i)).and_then(|v|v.as_f64()).unwrap_or([0.96,0.95,0.925][i])as f32);
      let (normal,_)=face_basis(g,fid,&vertices);let axes=texture.map(|t|uv_axes(t,normal,&m));
      let tris=g.mesh.triangulate_face(fid);let mut positions=Vec::new();positions.try_reserve_exact(tris.len().checked_mul(3).ok_or("Billboard triangle count overflow")?).map_err(|_|"Billboard triangles admission")?;
      for(a,b,c)in tris{for local in [a,b,c]{let v=placed(local,&m)?;let x=v.x as f64-anchor.x as f64;let y=v.y as f64-anchor.y as f64;let p=[(anchor.x as f64+cos*x-sin*y)as f32,(anchor.y as f64+sin*x+cos*y)as f32,v.z];let uv=axes.map_or([0.,0.],|(gu,cu,gv,cv)|[(qt_dot(gu,v)as f64+cu)as f32,(qt_dot(gv,v)as f64+cv)as f32]);positions.push((p,uv));}}
      if let Some(run)=local_runs.iter_mut().find(|r|r.path==key&&(key.is_some()||r.color==color)){run.positions.try_reserve(positions.len()).map_err(|_|"Billboard merged run admission")?;run.positions.extend(positions);}else{local_runs.try_reserve(1).map_err(|_|"Billboard run admission")?;local_runs.push(Run{path:key,color,positions});}
     }
     local_runs.sort_by_key(|r|r.path.is_none());runs.try_reserve(local_runs.len()).map_err(|_|"Billboard run admission")?;runs.extend(local_runs);
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

impl Packet{
 pub(super) fn textures(self)->Result<TextureBuffers,String>{
  let mut out=TextureBuffers::default();out.try_reserve(self.runs.len()).map_err(|_|"Billboard material identity admission")?;
  for(index,run)in self.runs.into_iter().enumerate(){
   let solid=run.path.is_none();let key=run.path.unwrap_or_else(||format!("@billboard-color-{index}"));let order=out.take_order();let buffer=out.entry(key).or_default();
   if buffer.source_billboard_color.is_some()&&buffer.source_billboard_color!=Some(run.color){return Err("Billboard solid color identity admission".into());}
   if solid{buffer.source_billboard_color=Some(run.color);}
   buffer.source_billboard_receiver=true;let first=buffer.indices.len();buffer.vertices.try_reserve(run.positions.len()).map_err(|_|"Billboard vertex admission")?;buffer.indices.try_reserve(run.positions.len()).map_err(|_|"Billboard index admission")?;buffer.source_texture_runs.try_reserve(1).map_err(|_|"Billboard run admission")?;
   for(position,uv)in run.positions{let id:u32=buffer.vertices.len().try_into().map_err(|_|"Billboard index overflow")?;buffer.vertices.push(Vertex{position,uv,..Default::default()});buffer.indices.push(id);}
   buffer.source_texture_runs.push(SourceTextureRun{first,count:buffer.indices.len()-first,order,back_side:false,shade:1.,opacity:1.,front_cull:false});
  }Ok(out)
 }
}
pub(super) fn key(scene:&Scene,camera:&OrbitCamera,epoch:u64)->Result<([u64;2],[u32;7]),String>{
 let snapshot=crate::navigation_ray::camera_snapshot(camera).ok_or("Billboard camera admission")?;let target=camera.target.as_vec3().to_array();Ok(([scene.version,epoch],[u32::from(camera.perspective),snapshot.eye[0].to_bits(),snapshot.eye[1].to_bits(),snapshot.eye[2].to_bits(),target[0].to_bits(),target[1].to_bits(),target[2].to_bits()]))
}
