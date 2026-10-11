use ingetrazo_core::{mesh::Mesh,math::DVec3};
use serde_json::{Value,json};
fn point(v:&Value)->DVec3{DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
fn snapshot(m:&Mesh)->Value{json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|json!({"v":[e.v0,e.v1],"faces":e.faces,"soft":e.soft,"hidden":e.hidden,"curve":e.curve_id,"layer":e.layer})).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|json!({"loop":f.vertices,"holes":f.holes,"attrs":f.attrs})).collect::<Vec<_>>()})}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().collect::<Vec<_>>();let cases:Value=serde_json::from_slice(&std::fs::read(&args[1])?)?;let mut result=Vec::new();
 for case in cases.as_array().unwrap(){let mut mesh=Mesh::new();let mut states=Vec::new();let name=case["name"].as_str().unwrap();
  for op in case["operations"].as_array().unwrap(){match op["kind"].as_str().unwrap(){
   "vertex"=>{mesh.add_vertex(point(&op["p"]));},
   "face"=>{let outer=op["outer"].as_array().unwrap().iter().map(|v|mesh.add_vertex(point(v))).collect();let holes=op["holes"].as_array().unwrap().iter().map(|h|h.as_array().unwrap().iter().map(|v|mesh.add_vertex(point(v))).collect()).collect();let id=mesh.add_face(outer,holes).unwrap();mesh.faces[id].attrs=serde_json::from_value(op["attrs"].clone())?;},
   "edge"=>{let a=mesh.add_vertex(point(&op["a"]));let b=mesh.add_vertex(point(&op["b"]));if let Some(id)=mesh.add_edge(a,b){let e=&mut mesh.edges[id];if let Some(flags)=op["flags"].as_object(){e.attrs.insert("native_opaque".into(),json!({"keep":[1,2,3]}));if let Some(v)=flags.get("soft"){e.soft=v.as_bool().unwrap();}if let Some(v)=flags.get("hidden"){e.hidden=v.as_bool().unwrap();}if let Some(v)=flags.get("curve"){e.curve_id=v.as_u64().map(|v|v as usize);}if let Some(v)=flags.get("layer"){e.layer=v.as_str().map(str::to_owned);}}}},
   "place"=>{let id=op["vertex"].as_u64().unwrap()as usize;let uid=mesh.vertices[id].uid.as_u128();mesh.place_vertices(std::iter::once((id,uid,point(&op["p"]))));},
   "split"=>{let a=mesh.vertex_at_position(point(&op["a"])).unwrap();let b=mesh.vertex_at_position(point(&op["b"])).unwrap();let e=mesh.find_edge(a,b).unwrap();let vertices=mesh.vertices.iter().map(|v|v.uid).collect::<Vec<_>>();let faces=mesh.faces.iter().map(|f|f.history_id).collect::<Vec<_>>();let removed=mesh.edges[e].history_id;let oldedges=mesh.edges.iter().map(|e|(e.history_id,e.attrs.clone())).collect::<Vec<_>>();let no_op=mesh.vertex_at_position(point(&op["p"])).is_some_and(|v|v==a||v==b);let pair=mesh.split_edge(e,point(&op["p"]))?;for id in [pair.0,pair.1]{if !oldedges.iter().any(|(old,_)|*old==mesh.edges[id].history_id){assert!(mesh.edges[id].attrs.is_empty());}}assert_eq!(mesh.vertices[..vertices.len()].iter().map(|v|v.uid).collect::<Vec<_>>(),vertices);assert_eq!(mesh.faces.iter().map(|f|f.history_id).collect::<Vec<_>>(),faces);if no_op{assert_eq!(pair,(e,e));}else{assert!(!mesh.edges.iter().any(|e|e.history_id==removed));}for(id,attrs)in oldedges{if let Some(e)=mesh.edges.iter().find(|e|e.history_id==id){assert_eq!(e.attrs,attrs);}}},
   _=>panic!("unknown operation")}
   if !name.starts_with("bulk-")&&!name.starts_with("large-"){states.push(snapshot(&mesh));}
  }
  if name.starts_with("bulk-")||name.starts_with("large-"){
   let before=snapshot(&mesh);
   let self_edges=mesh.edges.iter().enumerate().filter(|(_,e)|e.v0==e.v1).map(|(id,e)|(id,e.v0)).collect::<Vec<_>>();
   assert_eq!(self_edges.len(),1);
   for (id,v) in self_edges {
    assert_eq!(mesh.find_edge(v,v),Some(id));
    assert_eq!(mesh.vertices[v].edges.iter().filter(|&&e|e==id).count(),1);
    assert_eq!(mesh.add_edge(v,v),None);
    assert!(mesh.split_edge(id,mesh.vertices[v].position+DVec3::Z).is_err());
   }
   assert_eq!(snapshot(&mesh),before);
   mesh.rebuild_incidence();
   assert_eq!(snapshot(&mesh),before);
   for vertex in &mesh.vertices {
    let unique=vertex.edges.iter().copied().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(),vertex.edges.len());
   }
   states.push(snapshot(&mesh));
  }
  result.push(json!({"name":name,"states":states}));
 }
 // Native f64 controls: no Qt narrowing at large coordinate or near its weld cell.
 let mut m=Mesh::new();let p=DVec3::new(100000000.000049,0.,0.);let q=DVec3::new(100000000.01,0.,0.);let a=m.add_vertex(p);let b=m.add_vertex(q);assert_ne!(a,b);assert_eq!(m.vertices[a].position,p);assert_eq!(m.vertices[b].position,q);
 let c=m.add_vertex(DVec3::new(100000000.000051,0.,0.));assert_eq!(a,c);assert_eq!(m.vertices[a].position,p);
 let before=snapshot(&m);assert!(m.split_edge(usize::MAX,DVec3::ZERO).is_err());assert!(m.split_edge(0,DVec3::splat(f64::NAN)).is_err());assert_eq!(snapshot(&m),before);assert_eq!(m.find_edge(a,usize::MAX),None);
 let mut invalid=Mesh::new();let a=invalid.add_vertex(DVec3::ZERO);let b=invalid.add_vertex(DVec3::X);let e=invalid.add_edge(a,b).unwrap();invalid.edges[e].faces.push(usize::MAX);let old=snapshot(&invalid);assert!(invalid.split_edge(e,DVec3::X*0.5).is_err());assert_eq!(snapshot(&invalid),old);
 println!("{}",serde_json::to_string(&result)?);Ok(())
}
