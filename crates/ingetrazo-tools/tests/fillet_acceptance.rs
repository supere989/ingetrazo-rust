use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::fillet::fillet_edge;
fn scene()->Scene{let mut s=Scene::new();for p in [[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.],[0.,0.,1.],[2.,0.,1.],[2.,1.,1.],[0.,1.,1.]]{s.mesh.add_vertex(DVec3::from_array(p));}for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]{s.mesh.add_face(f.to_vec(),vec![]).unwrap();}s}
#[test]
fn invalid_radii_are_refused_atomically(){for radius in [0.,-0.1,f64::NAN,f64::INFINITY,2.] {let mut s=scene();let edge=s.mesh.edges.iter().position(|e|s.mesh.vertices[e.v0].position.z==1.&&s.mesh.vertices[e.v1].position.z==1.&&s.mesh.vertices[e.v0].position.y==0.&&s.mesh.vertices[e.v1].position.y==0.).unwrap();let before=serde_json::to_value(&s).unwrap();assert!(fillet_edge(&mut s,edge,radius,8).is_err());assert_eq!(serde_json::to_value(&s).unwrap(),before);}}
#[test]
fn radius_and_segments_create_tangent_cylinder_geometry(){for segments in [3,8,16]{let mut s=scene();let edge=s.mesh.edges.iter().position(|e|s.mesh.vertices[e.v0].position.z==1.&&s.mesh.vertices[e.v1].position.z==1.&&s.mesh.vertices[e.v0].position.y==0.&&s.mesh.vertices[e.v1].position.y==0.).unwrap();fillet_edge(&mut s,edge,0.2,segments).unwrap();let curved=s.mesh.faces.iter().filter(|f|f.vertices.iter().all(|&v|{let p=s.mesh.vertices[v].position;((p.y-0.2).hypot(p.z-0.8)-0.2).abs()<1e-8})).count();assert_eq!(curved,segments);assert!(s.mesh.edges.iter().any(|e|e.soft));}}
#[test]
fn rim_and_spherical_joins_form_closed_oriented_boundaries(){
    for all in [false,true]{let mut s=scene();let edges=(0..s.mesh.edges.len()).filter(|&i|all||(s.mesh.vertices[s.mesh.edges[i].v0].position.z==1.&&s.mesh.vertices[s.mesh.edges[i].v1].position.z==1.)).collect::<Vec<_>>();ingetrazo_tools::fillet::fillet_edges(&mut s,&edges,0.2,8).unwrap();let mut counts=std::collections::HashMap::new();let mut volume=0.;for face in &s.mesh.faces{let a=s.mesh.vertices[face.vertices[0]].position;for i in 1..face.vertices.len()-1{volume+=a.dot(s.mesh.vertices[face.vertices[i]].position.cross(s.mesh.vertices[face.vertices[i+1]].position))/6.;}for i in 0..face.vertices.len(){let a=face.vertices[i];let b=face.vertices[(i+1)%face.vertices.len()];let e=counts.entry((a.min(b),a.max(b))).or_insert((0,0));e.0+=1;e.1+=if a<b{1}else{-1};}}assert!(counts.values().all(|&(count,orientation)|count==2&&orientation==0));assert!(volume>1.8&&volume<2.);if all{assert!(volume<1.9);}}
}

#[test]
fn original_cursor_sizing_and_typed_segment_workflow(){
 use ingetrazo_tools::{fillet::FilletTool,Tool,ToolContext};use ingetrazo_core::{math::Ray,scene::SelectionItem};
 let mut s=scene();let original=s.mesh.clone();let mut tool=FilletTool::default();
 let pick=ToolContext::new(DVec3::new(1.,0.,1.),Ray::new(DVec3::new(1.,-2.,1.),DVec3::Y));
 tool.on_move(&mut s,&pick);assert!(tool.preview_mesh().is_some());assert_eq!(s.mesh,original);
 tool.on_click(&mut s,&pick);let size=ToolContext::new(DVec3::new(1.,0.25,1.),Ray::new(DVec3::new(1.,0.25,3.),-DVec3::Z));tool.on_move(&mut s,&size);
 assert!((tool.radius-0.25).abs()<1e-9);assert_eq!(s.mesh,original);tool.on_cancel(&mut s);assert_eq!(s.mesh,original);
 let mut s=scene();for i in 0..s.mesh.edges.len(){let e=&s.mesh.edges[i];if s.mesh.vertices[e.v0].position.z==1.&&s.mesh.vertices[e.v1].position.z==1.{s.select(SelectionItem::Edge(i));}}
 let mut tool=FilletTool::default();tool.on_activate(&mut s);tool.on_vcb_input(&mut s,"6s").unwrap();assert_eq!(tool.segments,6);let before=s.mesh.clone();assert!(tool.on_vcb_input(&mut s,"-1").is_err());assert_eq!(s.mesh,before);tool.on_vcb_input(&mut s,"0.15").unwrap();
 let tangent=s.mesh.vertices.iter().any(|v|(v.position.y-0.15).abs()<1e-8&&(v.position.z-1.).abs()<1e-8);assert!(tangent);assert!(s.selection.is_empty());
}
