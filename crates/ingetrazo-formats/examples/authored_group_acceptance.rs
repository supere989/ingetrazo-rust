use glam::DMat4;
use ingetrazo_core::{groups::Group,scene::Scene};
fn observe(scene:&Scene)->serde_json::Value{
 let mut rows=vec![];
 fn visit(g:&Group,parent:DMat4,path:Vec<usize>,rows:&mut Vec<serde_json::Value>){
  let world=parent*g.transform;let mut points=vec![];
  for f in &g.mesh.faces{for &id in &f.vertices{points.push(world.transform_point3(g.mesh.vertices[id].position).to_array());}}
  points.sort_by(|a,b|a.partial_cmp(b).unwrap());
  rows.push(serde_json::json!({"path":path,"uid":g.native_uid(),"is_component":g.component_definition.is_some(),"component_flag":g.native_document.get("component"),"native_xform":g.native_document.get("xform"),"transform":g.transform.to_cols_array(),"world_points":points,"ext":g.native_document.get("ext")}));
  for(i,child)in g.children.iter().enumerate(){let mut p=path.clone();p.push(i);visit(child,world,p,rows);}
 }
 for(i,g)in scene.groups.iter().enumerate(){visit(g,DMat4::IDENTITY,vec![i],&mut rows);}
 serde_json::json!(rows)
}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().collect::<Vec<_>>();let root=std::path::Path::new(&args[1]);let out=std::path::Path::new(&args[2]);std::fs::create_dir_all(out)?;
 let source:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("original-alpha-results.json"))?)?;let mut rows=vec![];
 for r in source["cases"].as_array().unwrap(){let name=r["case"].as_str().unwrap();let scene=ingetrazo_formats::igz::load_igz(&root.join(format!("{name}.igz")))?;let before=observe(&scene);let path=out.join(format!("{name}.rust-roundtrip.igz"));ingetrazo_formats::igz::save_igz(&scene,&path)?;let reopened=ingetrazo_formats::igz::load_igz(&path)?;assert_eq!(before,observe(&reopened));rows.push(serde_json::json!({"case":name,"groups":before}));}
 std::fs::write(out.join("native-group-observations.json"),serde_json::to_vec_pretty(&serde_json::json!({"cases":rows}))?)?;Ok(())
}
