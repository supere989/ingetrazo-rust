//! Borrowed Push/Pull preview geometry; the authoritative mesh is never mutated.
use glam::DVec3;
use ingetrazo_core::{mesh::Mesh,scene::Scene};
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Input {pub face:usize,pub face_identity:u64,pub mesh_revision:u64,pub anchor:DVec3,pub normal:DVec3,pub offset:DVec3,pub attached:bool,pub prism_cap:bool,pub keep_base:bool}


/// Immutable cache identity. Camera changes matter only when base membership changes.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Key {pub input:Input,pub publication:u64,pub suppress_base:bool}
impl Input {pub fn key(self,publication:u64,eye:DVec3)->Key{Key{input:self,publication,suppress_base:self.attached&&!self.keep_base&&self.offset.as_vec3().dot(self.anchor.as_vec3()-eye.as_vec3())>0.}}}
const MAX_PACKET_CPU_BYTES:usize=64*1024*1024;
fn value_bytes(v:&serde_json::Value)->Option<usize>{use serde_json::Value;let base=std::mem::size_of::<Value>();match v{Value::String(s)=>base.checked_add(s.len()),Value::Array(a)=>a.iter().try_fold(base,|n,v|n.checked_add(value_bytes(v)?)),Value::Object(o)=>o.iter().try_fold(base,|n,(k,v)|n.checked_add(k.len())?.checked_add(value_bytes(v)?)),_=>Some(base)}}
fn face_bytes(source:&Mesh,id:usize)->Result<usize,String>{let f=source.faces.get(id).ok_or("Sweep incident face missing")?;let n=f.holes.iter().try_fold(f.vertices.len(),|n,h|n.checked_add(h.len())).ok_or("Sweep point count overflow")?;
 // Logical payload admission plus conservative connectivity/spatial-index allowance.
 // This is not a hard bound on host allocator overhead; GPU residency is separately ledgered.
 let bytes=n.checked_mul(512).ok_or("Sweep CPU admission overflow")?;f.attrs.iter().try_fold(bytes,|n,(k,v)|n.checked_add(k.len())?.checked_add(value_bytes(v)?)).ok_or_else(||"Sweep attribute admission overflow".into())}
fn admit(bytes:usize)->Result<(),String>{if bytes>MAX_PACKET_CPU_BYTES{Err("Sweep CPU packet budget exceeded".into())}else{Ok(())}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind{Overlay,Prism}
pub struct Packet {pub kind:Kind,pub mesh:Mesh,pub suppressed:Vec<usize>,pub suppressed_edges:Vec<usize>,pub lines:Vec<[f32;3]>}
fn moved(p:DVec3,offset:DVec3)->Result<DVec3,String>{let p=(p.as_vec3()+offset.as_vec3()).as_dvec3();if !p.is_finite(){return Err("Face sweep projection exceeds finite source coordinates".into());}Ok(p)}
fn copy_face(source:&Mesh,id:usize,moving:&std::collections::HashSet<usize>,offset:DVec3,out:&mut Mesh,preserve_interior:bool)->Result<(),String>{
 let face=&source.faces[id];let mut ring=|ids:&[usize]|->Result<Vec<usize>,String>{ids.iter().map(|&id|{let p=source.vertices.get(id).ok_or("Face sweep vertex missing")?.position;Ok(out.add_vertex(if moving.contains(&id){moved(p,offset)?}else{p}))}).collect()};
 let outer=ring(&face.vertices)?;let holes=face.holes.iter().map(|h|ring(h)).collect::<Result<Vec<_>,_>>()?;
 let next=out.add_face(outer,holes).ok_or("Face sweep face is degenerate")?;out.faces[next].attrs=face.attrs.clone();out.faces[next].layer=face.layer.clone();out.faces[next].interior=preserve_interior&&face.interior;Ok(())
}
impl Packet {
 pub fn build(scene:&Scene,sweep:Input,eye:DVec3)->Result<Self,String>{
  let face=scene.mesh.faces.get(sweep.face).ok_or("Face sweep acquired face disappeared")?;
  if scene.version!=sweep.mesh_revision||face.history_id!=sweep.face_identity{return Err("Face sweep acquired identity/revision changed".into());}
  if !sweep.offset.is_finite()||!sweep.normal.is_finite()||!sweep.anchor.is_finite()||!eye.is_finite(){return Err("Face sweep parameters nonfinite".into());}
  let count=face.vertices.len().checked_add(face.holes.iter().try_fold(0usize,|n,h|n.checked_add(h.len())).ok_or("Face sweep hole count overflow")?).ok_or("Face sweep point count overflow")?;
  let paint_bytes=face.attrs.iter().filter(|(k,_)|matches!(k.as_str(),"color"|"mat"|"texture"|"opacity")).try_fold(0usize,|n,(k,v)|n.checked_add(k.len())?.checked_add(value_bytes(v)?)).ok_or("Sweep paint byte admission overflow")?;
  admit(count.checked_mul(3072).and_then(|n|n.checked_add(face_bytes(&scene.mesh,sweep.face).ok()?)).and_then(|n|n.checked_add(paint_bytes.checked_mul(count)?.checked_mul(2)?)).ok_or("Sweep CPU admission overflow")?)?;
  let mut packet=Self{kind:if sweep.prism_cap&&!sweep.keep_base{Kind::Prism}else{Kind::Overlay},mesh:Mesh::new(),suppressed:Vec::new(),suppressed_edges:Vec::new(),lines:Vec::new()};
  if sweep.offset.length()<2e-4{return Ok(packet);}
  let moving=face.vertices.iter().chain(face.holes.iter().flatten()).copied().collect::<std::collections::HashSet<_>>();
  if sweep.prism_cap&&!sweep.keep_base {
   // Only faces incident to acquired cap identities need a changed packet.
   let incidence= moving.iter().try_fold(0usize,|n,&v|scene.mesh.vertices[v].edges.iter().try_fold(n,|n,&e|n.checked_add(scene.mesh.edges[e].faces.len()))).ok_or("Sweep incidence admission overflow")?;admit(incidence.checked_mul(256).ok_or("Sweep incidence bytes overflow")?)?;
   let mut affected=std::collections::BTreeSet::new();for &v in &moving{for &edge in &scene.mesh.vertices[v].edges{let e=&scene.mesh.edges[edge];affected.extend(e.faces.iter().copied());packet.suppressed_edges.push(edge);}}
   packet.suppressed_edges.sort_unstable();packet.suppressed_edges.dedup();
   let bytes=affected.iter().try_fold(0usize,|n,&id|n.checked_add(face_bytes(&scene.mesh,id).ok()?)).ok_or("Sweep incident CPU admission overflow")?;admit(bytes)?;
   for id in affected{copy_face(&scene.mesh,id,&moving,sweep.offset,&mut packet.mesh,true)?;packet.suppressed.push(id);}
   for &id in &packet.suppressed_edges{let e=&scene.mesh.edges[id];if e.soft||e.hidden||!scene.is_layer_visible(e.layer.as_deref().or_else(||e.attrs.get("layer").and_then(|v|v.as_str()))){continue;}for v in [e.v0,e.v1]{let p=scene.mesh.vertices[v].position;packet.lines.push(if moving.contains(&v){moved(p,sweep.offset)?}else{p}.as_vec3().to_array());}}
  }else{
   copy_face(&scene.mesh,sweep.face,&moving,sweep.offset,&mut packet.mesh,false)?;
   let mut paint=face.attrs.iter().filter(|(k,_)|matches!(k.as_str(),"color"|"mat"|"texture"|"opacity")).map(|(k,v)|(k.clone(),v.clone())).collect::<std::collections::HashMap<_,_>>();
   if let Some(serde_json::Value::Object(texture))=paint.get_mut("texture"){texture.remove("uvw");texture.insert("planar".into(),true.into());}
   for ring in std::iter::once(&face.vertices).chain(face.holes.iter()){
    if ring.len()<3{return Err("Face sweep ring is malformed".into());}
    let mut normals=Vec::with_capacity(ring.len());
    for i in 0..ring.len(){let a=scene.mesh.vertices[ring[i]].position;let b=scene.mesh.vertices[ring[(i+1)%ring.len()]].position;let cross=(b.as_vec3()-a.as_vec3()).cross(sweep.offset.as_vec3());normals.push(cross.try_normalize());
     let points=[a,b,moved(b,sweep.offset)?,moved(a,sweep.offset)?];let ids=points.map(|p|packet.mesh.add_vertex(p));let id=packet.mesh.add_face(ids.to_vec(),vec![]).ok_or("Face sweep wall is degenerate")?;packet.mesh.faces[id].attrs=paint.clone();}
    for i in 0..ring.len(){let a=scene.mesh.vertices[ring[i]].position;let b=scene.mesh.vertices[ring[(i+1)%ring.len()]].position;packet.lines.extend([moved(a,sweep.offset)?.as_vec3().to_array(),moved(b,sweep.offset)?.as_vec3().to_array()]);
     let soft=normals[(i+ring.len()-1)%ring.len()].zip(normals[i]).is_some_and(|(a,b)|{let d=a.dot(b);d>0.85&&d<0.99995});if !soft{packet.lines.extend([a.as_vec3().to_array(),moved(a,sweep.offset)?.as_vec3().to_array()]);}}
   }
   if sweep.key(0,eye).suppress_base{packet.suppressed.push(sweep.face);}
  }
  Ok(packet)
 }
}

#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->(Scene,Input){let mut s=Scene::new();let v=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].map(|p|s.mesh.add_vertex(p));let f=s.mesh.add_face(v.to_vec(),vec![]).unwrap();let i=Input{face:f,face_identity:s.mesh.faces[f].history_id,mesh_revision:s.version,anchor:DVec3::new(0.5,0.5,0.),normal:DVec3::Z,offset:DVec3::new(0.,0.,-0.25),attached:true,prism_cap:false,keep_base:false};(s,i)}
 #[test]fn immutable_sweep_cap_walls_and_identity_admission(){let(s,i)=fixture();let before=serde_json::to_vec(&s.mesh).unwrap();let p=Packet::build(&s,i,DVec3::new(0.,0.,5.)).unwrap();assert_eq!(p.mesh.faces.len(),5);assert_eq!(p.lines.len(),16);assert_eq!(p.suppressed,vec![0]);assert_eq!(before,serde_json::to_vec(&s.mesh).unwrap());let mut bad=i;bad.face_identity+=1;assert!(Packet::build(&s,bad,DVec3::Z).is_err());bad=i;bad.mesh_revision+=1;assert!(Packet::build(&s,bad,DVec3::Z).is_err());bad=i;bad.offset=DVec3::splat(f64::MAX);assert!(Packet::build(&s,bad,DVec3::Z).is_err());assert!(admit(MAX_PACKET_CPU_BYTES+1).is_err());}
 #[test]fn camera_membership_and_copy_controls(){let(s,i)=fixture();let front=i.key(1,DVec3::Z);let back=i.key(1,-DVec3::Z);assert_ne!(front,back);assert_eq!(front,i.key(1,DVec3::new(5.,5.,5.)));assert_ne!(front,i.key(2,DVec3::Z));let mut copy=i;copy.keep_base=true;assert!(Packet::build(&s,copy,DVec3::Z).unwrap().suppressed.is_empty());copy.offset=DVec3::ZERO;let p=Packet::build(&s,copy,DVec3::Z).unwrap();assert!(p.mesh.faces.is_empty()&&p.lines.is_empty());}
 #[test]fn prism_lines_keep_ordinary_edge_layer_visibility(){
  let(mut scene,mut input)=fixture();input.prism_cap=true;
  let visible=Packet::build(&scene,input,DVec3::Z).unwrap();assert!(!visible.lines.is_empty());
  let mut layer=ingetrazo_core::layers::Layer::new("Hidden");layer.visible=false;scene.layers.push(layer);
  for edge in &mut scene.mesh.edges {edge.attrs.insert("layer".into(),serde_json::json!("Hidden"));}
  assert!(Packet::build(&scene,input,DVec3::Z).unwrap().lines.is_empty());
  input.prism_cap=false;assert!(!Packet::build(&scene,input,DVec3::Z).unwrap().lines.is_empty(),"Overlay rubber-band ignores ordinary edge layers");
 }
 #[test]fn prism_partition_flags_and_repeated_paint_admission(){let(mut scene,mut input)=fixture();scene.mesh.faces[0].interior=true;input.prism_cap=true;let p=Packet::build(&scene,input,DVec3::Z).unwrap();assert!(p.mesh.faces[0].interior);input.prism_cap=false;let p=Packet::build(&scene,input,DVec3::Z).unwrap();assert!(!p.mesh.faces[0].interior);scene.mesh.faces[0].attrs.insert("texture".into(),serde_json::json!({"path":"x".repeat(9*1024*1024)}));assert!(Packet::build(&scene,input,DVec3::Z).err().unwrap().contains("budget"));}

}
