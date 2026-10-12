// SPDX-License-Identifier: GPL-3.0-or-later
//! Cached world-space hard-edge geometry. No camera projection or frame sorting.
use glam::DMat4;
use ingetrazo_core::{mesh::Mesh,groups::Group,scene::Scene};
#[derive(Default,Debug)]pub struct HardEdges {pub positions:Vec<[f32;3]>}
pub fn build_hard_edges(scene:&Scene)->HardEdges {
 let mut output=HardEdges::default();append(&scene.mesh,DMat4::IDENTITY,scene,&mut output);
 for group in &scene.groups{walk(group,DMat4::IDENTITY,scene,&mut output);}output
}
/// Active loose mesh exclusions preserve ordinary nested placement ordering.
pub fn build_hard_edges_suppressed(scene:&Scene,suppressed:&[usize])->HardEdges{
 let mut output=HardEdges::default();
 for (id,edge) in scene.mesh.edges.iter().enumerate(){if suppressed.binary_search(&id).is_ok()||edge.soft||edge.hidden||!scene.is_layer_visible(edge.layer.as_deref().or_else(||edge.attrs.get("layer").and_then(|v|v.as_str()))){continue;}
  if let(Some(a),Some(b))=(scene.mesh.vertices.get(edge.v0),scene.mesh.vertices.get(edge.v1)){output.positions.extend([a.position.as_vec3().to_array(),b.position.as_vec3().to_array()]);}}
 for group in &scene.groups{walk(group,DMat4::IDENTITY,scene,&mut output);}output
}
pub fn append_borrowed_mesh(mesh:&Mesh,scene:&Scene,out:&mut HardEdges){append(mesh,DMat4::IDENTITY,scene,out);}
pub fn append_borrowed_group(group:&Group,scene:&Scene,out:&mut HardEdges){walk(group,DMat4::IDENTITY,scene,out);}
fn walk(group:&Group,parent:DMat4,scene:&Scene,out:&mut HardEdges){
 if group.hidden||!scene.is_layer_visible(group.layer.as_deref()){return;}
 let world=parent*group.transform;append(&group.mesh,world,scene,out);for child in &group.children{walk(child,world,scene,out);}
}
fn append(mesh:&Mesh,world:DMat4,scene:&Scene,out:&mut HardEdges){
 for edge in &mesh.edges {
  if edge.soft||edge.hidden||!scene.is_layer_visible(edge.layer.as_deref().or_else(||edge.attrs.get("layer").and_then(|v|v.as_str()))){continue;}
  if let(Some(a),Some(b))=(mesh.vertices.get(edge.v0),mesh.vertices.get(edge.v1)){for v in [a,b]{out.positions.push(world.transform_point3(v.position).as_vec3().to_array());}}
 }
}

#[cfg(test)]mod tests {
 use super::*;use glam::DVec3;
 fn wire()->Mesh {let mut m=Mesh::new();let a=m.add_vertex(DVec3::new(1.,2.,3.));let b=m.add_vertex(DVec3::new(4.,5.,6.));m.add_edge(a,b).unwrap();m}
 #[test]fn nested_reflection_transform_and_visibility_filter_endpoints(){
  let mut scene=Scene::new();let mut child=Group::with_mesh("child",wire());child.transform=DMat4::from_scale(DVec3::new(-2.,3.,4.));let mut parent=Group::new("parent");parent.transform=DMat4::from_translation(DVec3::new(10.,20.,30.));parent.children.push(child);scene.groups.push(parent);
  assert_eq!(build_hard_edges(&scene).positions,vec![[8.,26.,42.],[2.,35.,54.]]);
  scene.groups[0].hidden=true;assert!(build_hard_edges(&scene).positions.is_empty());scene.groups[0].hidden=false;
  let mut layer=ingetrazo_core::layers::Layer::new("Hidden");layer.visible=false;scene.layers.push(layer);scene.groups[0].children[0].layer=Some("Hidden".into());assert!(build_hard_edges(&scene).positions.is_empty());
 }
 #[test]fn soft_hidden_layer_and_invalid_edges_do_not_emit_gpu_addresses(){
  let mut scene=Scene::new();scene.mesh=wire();let mut hidden=ingetrazo_core::layers::Layer::new("Hidden");hidden.visible=false;scene.layers.push(hidden);
  for attr in ["soft","hidden","layer","invalid"]{let e=&mut scene.mesh.edges[0];e.soft=false;e.hidden=false;e.attrs.clear();e.v1=1;match attr{"soft"=>e.soft=true,"hidden"=>e.hidden=true,"layer"=>{e.attrs.insert("layer".into(),serde_json::json!("Hidden"));},_=>e.v1=999};assert!(build_hard_edges(&scene).positions.is_empty(),"{attr}");}
 }
}
