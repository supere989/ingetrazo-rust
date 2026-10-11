//! Separate source Hidden Objects/Geometry packet; ordinary surfaces stay hidden.
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
pub struct Vertex {pub position:[f32;3],pub color:[f32;3]}
impl Vertex {fn new(position:DVec3,color:[f32;3])->Self{Self{position:position.as_vec3().to_array(),color}}}
use ingetrazo_core::{scene::Scene,groups::Group,mesh::Mesh};
use glam::{DMat4,DVec3};
#[derive(Default)]pub struct Packet {pub faces:Vec<Vertex>,pub textured:Vec<Vertex>,pub edges:Vec<Vertex>}
fn truth(v:&serde_json::Value)->bool{match v{serde_json::Value::Null=>false,serde_json::Value::Bool(b)=>*b,serde_json::Value::Number(n)=>n.as_f64()!=Some(0.),serde_json::Value::String(s)=>!s.is_empty(),serde_json::Value::Array(a)=>!a.is_empty(),serde_json::Value::Object(o)=>!o.is_empty()}}
fn point(transform:DMat4,p:DVec3)->Result<DVec3,String>{let p=transform.as_mat4().transform_point3(p.as_vec3());if !p.is_finite(){return Err("Hidden geometry transform overflow".into());}Ok(p.as_dvec3())}
impl Packet {
 pub fn build(scene:&Scene)->Result<Self,String>{
  let mut out=Self::default();
  if scene.show_hidden_geometry {out.append_mesh(scene,&scene.mesh,DMat4::IDENTITY,None,false)?;}
  if scene.show_hidden_objects {for group in &scene.groups{out.visit(scene,group,DMat4::IDENTITY,None,false)?;}}
  Ok(out)
 }
 fn visit(&mut self,scene:&Scene,group:&Group,parent:DMat4,layer:Option<&str>,hidden:bool)->Result<(),String>{
  let layer=group.layer.as_deref().or(layer);if !scene.is_layer_visible(layer){return Ok(());}
  let transform=(parent.as_mat4()*group.transform.as_mat4()).as_dmat4();let hidden=hidden||group.hidden;
  if hidden&&!group.billboard{self.append_mesh(scene,&group.mesh,transform,group.native_document.get("material"),true)?;}
  for child in &group.children{self.visit(scene,child,transform,layer,hidden)?;}Ok(())
 }
 fn append_mesh(&mut self,scene:&Scene,mesh:&Mesh,transform:DMat4,paint:Option<&serde_json::Value>,object:bool)->Result<(),String>{
  for (id,face) in mesh.faces.iter().enumerate(){
   let hidden=face.attrs.get("hidden").is_some_and(truth);
   if object&&hidden||!object&&!hidden{continue;}
   if !object&&!scene.is_layer_visible(face.layer.as_deref().or_else(||face.attrs.get("layer").and_then(|v|v.as_str()))){continue;}
   let effective=crate::pipeline::source_effective_face_attrs(&face.attrs,paint);
   let attr=|key:&str|effective.get(key);
   // The original object ghost reads only opaque chunk vcol/by_texture runs.
   if object&&attr("opacity").and_then(|v|v.as_f64()).unwrap_or(1.)<0.999{continue;}
   let texture=object&&attr("texture").and_then(|v|v.get("path")).and_then(|v|v.as_str()).is_some_and(|p|!p.is_empty());
   let translucent_back=attr("back").and_then(|v|v.as_object()).is_some_and(|back| {
    back.get("opacity").and_then(|v|v.as_f64()).unwrap_or(1.)<0.999
     && (back.get("color").is_some_and(|v|!v.is_null())
      || back.get("texture").and_then(|v|v.get("path")).and_then(|v|v.as_str()).is_some_and(|p|!p.is_empty()))
   });
   if object&&!texture&&translucent_back{continue;}
   let shade=crate::pipeline::shade_factor(mesh.face_normal(id)) as f64;
   let base=if object{attr("color")}else{face.attrs.get("color")}.and_then(|v|v.as_array());
   let default=[0.96,0.95,0.925];let rgb=std::array::from_fn::<_,3,_>(|i|((base.and_then(|b|b.get(i)).and_then(|v|v.as_f64()).unwrap_or(default[i])*shade).min(1.)*64.).round_ties_even() as f32/64.);
   let color=if texture{[0.62,0.62,0.62]}else{rgb};
   let target=if texture{&mut self.textured}else{&mut self.faces};
   for(a,b,c)in mesh.triangulate_face(id){target.try_reserve(3).map_err(|_|"Hidden face packet admission")?;for p in [a,b,c]{target.push(Vertex::new(point(transform,p)?,color));}}
  }
  for edge in &mesh.edges{
   if object&&(edge.hidden||edge.soft)||!object&&(!edge.hidden||!scene.is_layer_visible(edge.layer.as_deref())){continue;}
   self.edges.try_reserve(2).map_err(|_|"Hidden edge packet admission")?;
   for id in [edge.v0,edge.v1]{self.edges.push(Vertex::new(point(transform,mesh.vertices[id].position)?,[1.;3]));}
  }Ok(())
 }
}

#[cfg(test)]mod layout_tests{#[test]fn packed_position_rgb_is_twenty_four_bytes(){assert_eq!(std::mem::size_of::<super::Vertex>(),24);assert_eq!(std::mem::align_of::<super::Vertex>(),4);}}
