//! CPU diagnostic: compare production surface vertices with captured source GPU inputs.
use serde_json::{json, Value};
fn main() -> Result<(), Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();
 let input=std::path::Path::new(args.get(1).ok_or("input directory")?);
 let reference:Value=serde_json::from_slice(&std::fs::read(args.get(2).ok_or("reference JSON")?)?)?;
 let protocol:Value=serde_json::from_slice(&std::fs::read(input.join("protocol.json"))?)?;
 let case=protocol["cases"].as_array().ok_or("cases")?.iter().find(|c|c["name"]=="gondola_v20-iso").ok_or("gondola case")?;
 let scene=ingetrazo_formats::igz::load_igz(&input.join("gondola_v20.igz"))?;
 let c=&case["camera"];
 let n=|k:&str|c[k].as_f64().ok_or("camera number");
 let mut camera=ingetrazo_core::camera::OrbitCamera::default();
 camera.target=glam::DVec3::new(c["target"][0].as_f64().ok_or("target")?,c["target"][1].as_f64().ok_or("target")?,c["target"][2].as_f64().ok_or("target")?);
 camera.distance=n("distance")?;camera.yaw=n("yaw")?;camera.pitch=n("pitch")?;camera.fov_deg=n("fov_deg")?;camera.perspective=false;camera.aspect=800./600.;
 let mut style=ingetrazo_render::style::StyleConfig::default();
 style.background_color=[1.;4];style.show_sky=false;style.show_edges=true;style.show_profiles=false;style.show_textures=false;
 style.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;style.front_color=[0.96,0.95,0.925,1.];
 let data=ingetrazo_render::pipeline::build_world_surfaces(&scene,&camera,&style);
 let mesh=&data.opaque_mesh;let mut results=Vec::new();
 for target in reference["selected_triangles"].as_array().ok_or("selected triangles")? {
  let rows=target["position_rgb"].as_array().ok_or("rows")?;
  if rows.len()!=3{return Err("triangle requires three vertices".into());}
  let mut expected=Vec::new();
  for row in rows {let mut values=[0f32;6];for(i,v)in values.iter_mut().enumerate(){*v=row[i].as_f64().ok_or("vertex component")? as f32;}expected.push(values);}
  let mut matches=Vec::new();
  for (primitive,ids) in mesh.indices.chunks_exact(3).enumerate(){
   let vertices=ids.iter().map(|&i|&mesh.vertices[i as usize]).collect::<Vec<_>>();
   let permutations=[[0,1,2],[1,2,0],[2,0,1],[0,2,1],[2,1,0],[1,0,2]];
   for order in permutations {
    if (0..3).all(|i|(0..3).all(|j|vertices[order[i]].position[j].to_bits()==expected[i][j].to_bits())) {
     let colors_match=(0..3).all(|i|(0..3).all(|j|vertices[order[i]].color[j].to_bits()==expected[i][j+3].to_bits()));
     matches.push(json!({"primitive":primitive,"order":order,"colors_exact":colors_match,"vertices":vertices.iter().map(|v|json!({"position":v.position,"color":v.color})).collect::<Vec<_>>()}));break;
    }
   }
  }
  results.push(json!({"source_primitive":target["primitive"],"exact_position_matches":matches}));
 }
 println!("{}",serde_json::to_string_pretty(&json!({"vertex_count":mesh.vertices.len(),"index_count":mesh.indices.len(),"comparisons":results,"scope":"CPU production surfaces only; no rendered parity claim"}))?);Ok(())
}
