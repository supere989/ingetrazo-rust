// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera-facing source receiver packets. Ordinary scene geometry is immutable.
use glam::{DMat4,DVec3,Vec3};
use ingetrazo_core::{scene::Scene,groups::Group,camera::OrbitCamera};
use crate::buffer::{SourceTextureRun,TextureBuffers,Vertex};
#[derive(Clone,Copy)]
enum Facing{Quad{anchor:Vec3,width:f64,height:f64},Mesh{anchor:DVec3,normal:[f64;2]}}
pub struct Run {facing:Facing,pub path:Option<String>,pub color:[f32;3],pub positions:Vec<([f32;3],[f32;2])>}
pub struct Packet {pub runs:Vec<Run>}
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
  let matrix: [[f32;4];4]=std::array::from_fn(|c|std::array::from_fn(|r|m[c*4+r]));let inverse=affine_inverse(matrix);
  let axis=|base:usize|{let g:[f64;3]=std::array::from_fn(|i|uvw[base+i].as_f64().unwrap_or(0.));let c=uvw[base+3].as_f64().unwrap_or(0.);if let Some(inv)=inverse{let gp:[f64;3]=std::array::from_fn(|r|(inv[r][0]as f64*g[0]+inv[r][1]as f64*g[1])+inv[r][2]as f64*g[2]);(Vec3::new(gp[0]as f32,gp[1]as f32,gp[2]as f32),c-((gp[0]*m[12]as f64+gp[1]*m[13]as f64)+gp[2]*m[14]as f64))}else{(Vec3::new(g[0]as f32,g[1]as f32,g[2]as f32),c)}};
  let(u,cu)=axis(0);let(v,cv)=axis(4);return(u,cu,v,cv);
 }
 let(u,v)=crate::pipeline::projection_basis(normal.as_dvec3());let(mut u,mut v)=(u.as_vec3(),v.as_vec3());let angle=tex.get("rot").and_then(|v|v.as_f64()).unwrap_or(0.).to_radians();if angle!=0.{let(c,s)=(angle.cos()as f32,angle.sin()as f32);(u,v)=(u*c+v*s,v*c-u*s);}
 let scale=|key:&str|tex.get(key).and_then(|v|v.as_f64()).filter(|&v|v!=0.).unwrap_or(1.)as f32;
 (u/scale("sw"),0.,v/scale("sh"),0.)
}
impl Packet {
 pub fn retained_bytes(&self)->usize{std::mem::size_of::<Self>()+self.runs.capacity()*std::mem::size_of::<Run>()+self.runs.iter().map(|r|r.positions.capacity()*std::mem::size_of::<([f32;3],[f32;2])>()+r.path.as_ref().map_or(0,String::capacity)).sum::<usize>()}
 pub fn build(scene:&Scene)->Result<Self,String>{
  let mut runs=Vec::new();
  fn visit(g:&Group,parent:DMat4,scene:&Scene,runs:&mut Vec<Run>)->Result<(),String>{
   if g.hidden||!scene.is_layer_visible(g.layer.as_deref()){return Ok(());}
   let world=parent*g.transform;
   if g.billboard { (|| -> Result<(),String> {
    let m=world.as_mat4().to_cols_array();let mut vertices=Vec::new();vertices.try_reserve_exact(g.mesh.vertices.len()).map_err(|_|"Billboard vertex admission")?;
    for v in &g.mesh.vertices{vertices.push(placed(v.position,&m)?);}
    if vertices.is_empty(){return Ok(());}
    let mut lo=Vec3::splat(f32::INFINITY);let mut hi=Vec3::splat(f32::NEG_INFINITY);for &p in &vertices{lo=lo.min(p);hi=hi.max(p);}
    let anchor=Vec3::new(((lo.x as f64+hi.x as f64)*0.5)as f32,((lo.y as f64+hi.y as f64)*0.5)as f32,lo.z);
    let mesh=g.native_document.get("billboard").and_then(|v|v.as_str())==Some("mesh");
    if !mesh {
     let Some(texture)=g.mesh.faces.iter().find_map(|f|f.attrs.get("texture").and_then(path))else{return Ok(());};
     let width=(hi.x as f64-lo.x as f64).hypot(hi.y as f64-lo.y as f64);let height=hi.z as f64-lo.z as f64;
     if width<1e-9||height<1e-9{return Ok(());}
     runs.try_reserve(1).map_err(|_|"Billboard run admission")?;runs.push(Run{facing:Facing::Quad{anchor,width,height},path:Some(texture),color:[1.;3],positions:Vec::new()});
    } else {
     let mut biggest=None;for(f,_)in g.mesh.faces.iter().enumerate(){let (_,area)=face_basis(g,f,&vertices);if biggest.is_none_or(|(_,best)|area>best){biggest=Some((f,area));}}
     let Some((f,_))=biggest else{return Ok(());};let (base,_)=face_basis(g,f,&vertices);let length=(base.x as f64).hypot(base.y as f64);if length<=1e-9{return Ok(());}
     let nh=[base.x as f64/length,base.y as f64/length];let mesh_anchor=DVec3::new((lo.x as f64+hi.x as f64)*0.5,(lo.y as f64+hi.y as f64)*0.5,lo.z as f64);
     let mut local_runs:Vec<Run>=Vec::new();
     for(fid,face)in g.mesh.faces.iter().enumerate(){
      let texture=face.attrs.get("texture");let key=texture.and_then(path);let c=face.attrs.get("color").and_then(|v|v.as_array());let color=std::array::from_fn(|i|c.and_then(|v|v.get(i)).and_then(|v|v.as_f64()).unwrap_or([0.96,0.95,0.925][i])as f32);
      let (normal,_)=face_basis(g,fid,&vertices);let axes=texture.map(|t|uv_axes(t,normal,&m));
      let tris=g.mesh.triangulate_face(fid);let mut positions=Vec::new();positions.try_reserve_exact(tris.len().checked_mul(3).ok_or("Billboard triangle count overflow")?).map_err(|_|"Billboard triangles admission")?;
      for(a,b,c)in tris{for local in [a,b,c]{let v=placed(local,&m)?;let p=v.to_array();let uv=axes.map_or([0.,0.],|(gu,cu,gv,cv)|[(qt_dot(gu,v)as f64+cu)as f32,(qt_dot(gv,v)as f64+cv)as f32]);positions.push((p,uv));}}
      if let Some(run)=local_runs.iter_mut().find(|r|r.path==key&&(key.is_some()||r.color==color)){run.positions.try_reserve(positions.len()).map_err(|_|"Billboard merged run admission")?;run.positions.extend(positions);}else{local_runs.try_reserve(1).map_err(|_|"Billboard run admission")?;local_runs.push(Run{facing:Facing::Mesh{anchor:mesh_anchor,normal:nh},path:key,color,positions});}
     }
     local_runs.sort_by_key(|r|r.path.is_none());runs.try_reserve(local_runs.len()).map_err(|_|"Billboard run admission")?;runs.extend(local_runs);
    }
    Ok(())
   })()?; }
   for c in &g.children{visit(c,world,scene,runs)?;}Ok(())
  }
  for g in &scene.groups{visit(g,DMat4::IDENTITY,scene,&mut runs)?;}
  for r in &runs{if !r.color.iter().all(|v|v.is_finite())||!r.positions.iter().all(|(p,uv)|p.iter().chain(uv).all(|v|v.is_finite())){return Err("Billboard final packet arithmetic admission".into());}}
  Ok(Self{runs})
 }
}

impl Packet{
 pub fn textures(&self,camera:&OrbitCamera,eye:Vec3)->Result<TextureBuffers,String>{
  if !eye.is_finite()||!camera.target.is_finite(){return Err("Billboard camera admission".into());}let target=camera.target.as_vec3();
  let mut out=TextureBuffers::default();out.try_reserve(self.runs.len()).map_err(|_|"Billboard material identity admission")?;
  for(index,run)in self.runs.iter().enumerate(){
   let solid=run.path.is_none();let key=run.path.clone().unwrap_or_else(||format!("@billboard-color-{index}"));let order=out.take_order();let buffer=out.entry(key).or_default();
   if (buffer.source_billboard_receiver&&buffer.source_billboard_color.is_some()!=solid)||(buffer.source_billboard_color.is_some()&&buffer.source_billboard_color!=Some(run.color)){return Err("Billboard solid color identity admission".into());}
   if solid{buffer.source_billboard_color=Some(run.color);}
   let mut quad=[([0.;3],[0.;2]);6];let mesh_positions;
   let positions=match run.facing{
    Facing::Quad{anchor,width,height}=>{let direction=if camera.perspective{eye-anchor}else{eye-target};let mut d=Vec3::new(direction.x,direction.y,0.);if d.length()<1e-6{d=Vec3::X;}d=qt_normalized(d);let right=Vec3::new(-d.y,d.x,0.);let a=anchor-right*(width as f32*0.5);let b=anchor+right*(width as f32*0.5);let up=Vec3::Z*height as f32;let corners=[a,b,b+up,a+up];let uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]];for(slot,index)in quad.iter_mut().zip([0,1,2,0,2,3]){*slot=(corners[index].to_array(),uv[index]);}&quad[..]},
    Facing::Mesh{anchor,normal}=>{let direction=if camera.perspective{eye-anchor.as_vec3()}else{eye-target};let length=(direction.x as f64).hypot(direction.y as f64);let d=if length<1e-9{normal}else{[direction.x as f64/length,direction.y as f64/length]};let cos=normal[0]*d[0]+normal[1]*d[1];let sin=normal[0]*d[1]-normal[1]*d[0];let mut next=Vec::new();next.try_reserve_exact(run.positions.len()).map_err(|_|"Billboard rotated vertex admission")?;for&(p,uv)in &run.positions{let x=p[0]as f64-anchor.x;let y=p[1]as f64-anchor.y;next.push(([(anchor.x+cos*x-sin*y)as f32,(anchor.y+sin*x+cos*y)as f32,p[2]],uv));}mesh_positions=next;&mesh_positions[..]}
   };
   if !positions.iter().all(|(p,uv)|p.iter().chain(uv).all(|v|v.is_finite())){return Err("Billboard facing arithmetic admission".into());}
   buffer.source_billboard_receiver=true;let first=buffer.indices.len();buffer.vertices.try_reserve(positions.len()).map_err(|_|"Billboard vertex admission")?;buffer.indices.try_reserve(positions.len()).map_err(|_|"Billboard index admission")?;buffer.source_texture_runs.try_reserve(1).map_err(|_|"Billboard run admission")?;
   for&(position,uv)in positions{let id:u32=buffer.vertices.len().try_into().map_err(|_|"Billboard index overflow")?;buffer.vertices.push(Vertex{position,uv,..Default::default()});buffer.indices.push(id);}
   buffer.source_texture_runs.push(SourceTextureRun{first,count:buffer.indices.len()-first,order,back_side:false,shade:1.,opacity:1.,front_cull:false});
  }Ok(out)
 }
}
fn det2(m: &[[f64; 4]; 4], c: [usize; 2], r: [usize; 2]) -> f64 {
    m[c[0]][r[0]] * m[c[1]][r[1]] - m[c[0]][r[1]] * m[c[1]][r[0]]
}
fn det3(m: &[[f64; 4]; 4], c: [usize; 3], r: [usize; 3]) -> f64 {
    m[c[0]][r[0]] * det2(m, [c[1], c[2]], [r[1], r[2]])
        - m[c[1]][r[0]] * det2(m, [c[0], c[2]], [r[1], r[2]])
        + m[c[2]][r[0]] * det2(m, [c[0], c[1]], [r[1], r[2]])
}
fn without3(excluded: usize) -> [usize; 2] {
    let mut values = [0; 2];
    let mut n = 0;
    for i in 0..3 {
        if i != excluded {
            values[n] = i;
            n += 1;
        }
    }
    values
}

fn affine_inverse(m:[[f32;4];4])->Option<[[f32;4];4]>{
 let d=m.map(|c|c.map(f64::from));let mut inv=[[0.;4];4];
        let det = det3(&d, [0, 1, 2], [0, 1, 2]);
        if det == 0. {
            return None;
        }
        let reciprocal = 1. / det;
        for c in 0..3 {
            for r in 0..3 {
                let cs = without3(r);
                let rs = without3(c);
                let sign = if (c + r) % 2 == 0 { 1. } else { -1. };
                inv[c][r] = (sign * det2(&d, [cs[0], cs[1]], [rs[0], rs[1]]) * reciprocal) as f32;
            }
        }
        for r in 0..3 {
            inv[3][r] = -inv[0][r] * m[3][0] - inv[1][r] * m[3][1] - inv[2][r] * m[3][2];
        }
        inv[3][3] = 1.;

Some(inv)
}
fn qt_normalized(v: glam::Vec3) -> glam::Vec3 {
        let m = v.abs().max_element();
        let length = if m == 0. {
            0.
        } else {
            let s = v / m;
            m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
        };
        if (length - 1.).abs() <= 1e-5 {
            v
        } else if length.abs() <= 1e-5 {
            glam::Vec3::ZERO
        } else {
            v / length
        }
    }
#[cfg(test)]mod tests{
 use super::*;
 fn scene()->Scene{let mut s=Scene::new();let mut mesh=ingetrazo_core::mesh::Mesh::new();let ids=[DVec3::new(-1.,0.,0.),DVec3::new(1.,0.,0.),DVec3::new(1.,0.,3.),DVec3::new(-1.,0.,3.)].map(|p|mesh.add_vertex(p));let face=mesh.add_face(ids.to_vec(),vec![]).unwrap();mesh.faces[face].attrs.insert("texture".into(),serde_json::json!({"embed":"sprite.png","sw":2.,"sh":2.}));let mut group=Group::with_mesh("sprite",mesh);group.billboard=true;s.groups.push(group);s}
 #[test]fn cached_ortho_packet_uses_view_direction_not_anchor_direction(){let scene=scene();let packet=Packet::build(&scene).unwrap();let mut a=OrbitCamera::default();a.distance=8.;a.yaw=0.;a.pitch=0.;a.perspective=false;a.target=DVec3::ZERO;let first=packet.textures(&a,a.eye().as_vec3()).unwrap();let mut b=a.clone();b.target=DVec3::new(32.,16.,0.);let second=packet.textures(&b,b.eye().as_vec3()).unwrap();assert_eq!(first["sprite.png"].vertices,second["sprite.png"].vertices);a.perspective=true;b.perspective=true;assert_ne!(packet.textures(&a,a.eye().as_vec3()).unwrap()["sprite.png"].vertices,packet.textures(&b,b.eye().as_vec3()).unwrap()["sprite.png"].vertices);}
 #[test]fn empty_billboard_parent_keeps_nested_receiver(){let mut s=scene();let child=s.groups.remove(0);let mut parent=Group::with_mesh("empty parent",ingetrazo_core::mesh::Mesh::new());parent.billboard=true;parent.transform=DMat4::from_translation(DVec3::new(2.,3.,0.));parent.children.push(child);s.groups.push(parent);let packet=Packet::build(&s).unwrap();assert_eq!(packet.runs.len(),1);match packet.runs[0].facing{Facing::Quad{anchor,..}=>assert_eq!(anchor,Vec3::new(2.,3.,0.)),_=>panic!("bitmap child")};}
 #[test]fn shared_image_storage_keeps_placement_texture_color_order(){
  let facing=Facing::Mesh{anchor:DVec3::ZERO,normal:[0.,-1.]};
  let positions=vec![([0.,0.,0.],[0.,0.]),([1.,0.,0.],[1.,0.]),([0.,0.,1.],[0.,1.])];
  let packet=Packet{runs:vec![
   Run{facing,path:Some("same.png".into()),color:[1.;3],positions:positions.clone()},
   Run{facing,path:None,color:[0.1,0.3,0.8],positions:positions.clone()},
   Run{facing,path:Some("same.png".into()),color:[1.;3],positions:positions.clone()},
   Run{facing,path:None,color:[0.8,0.1,0.1],positions} ]};
  let buffers=packet.textures(&OrbitCamera::default(),OrbitCamera::default().eye().as_vec3()).unwrap();
  assert_eq!(buffers.len(),3,"one image storage with two distinct solid colors");
  let image=&buffers["same.png"];assert_eq!(image.source_texture_runs.len(),2);
  assert_eq!(image.source_texture_runs.iter().map(|r|(r.order,r.first,r.count)).collect::<Vec<_>>(),vec![(0,0,3),(2,3,3)]);
  assert_eq!(buffers["@billboard-color-1"].source_texture_runs[0].order,1);
  assert_eq!(buffers["@billboard-color-3"].source_texture_runs[0].order,3);
 }
 #[test]fn malformed_placement_and_camera_refuse_before_any_gpu_publication(){let mut scene=scene();scene.groups[0].transform=DMat4::from_translation(DVec3::splat(f64::MAX));assert!(Packet::build(&scene).is_err());let packet=Packet::build(&self::scene()).unwrap();let mut camera=OrbitCamera::default();camera.yaw=f64::NAN;assert!(packet.textures(&camera,camera.eye().as_vec3()).is_err());}
}
