use glam::DVec3;
use ingetrazo_core::{mesh::Mesh, triangulate::{triangulate, Tri3}};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::PathBuf;
fn points(v: &Value) -> Vec<DVec3> {
    v.as_array().unwrap().iter().map(|p| DVec3::new(p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap()).as_vec3().as_dvec3()).collect()
}
fn rings(c: &Value) -> (Vec<DVec3>, Vec<Vec<DVec3>>) {
    (points(&c["outer"]), c["holes"].as_array().unwrap().iter().map(points).collect())
}
fn append(m: &mut Mesh, outer: &[DVec3], holes: &[Vec<DVec3>]) -> usize {
    let outer = outer.iter().map(|&p| m.add_vertex(p)).collect();
    let holes = holes.iter().map(|h| h.iter().map(|&p|m.add_vertex(p)).collect()).collect();
    m.add_face(outer, holes).unwrap()
}
fn topology(m: &Mesh) -> Value {
    json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),
        "adjacency":m.vertices.iter().map(|v|{let mut e=v.edges.clone();e.sort_unstable();e}).collect::<Vec<_>>(),
        "edges":m.edges.iter().map(|e|json!({"vertices":[e.v0,e.v1],"faces":e.faces})).collect::<Vec<_>>(),
        "faces":m.faces.iter().map(|f|json!({"outer":f.vertices,"holes":f.holes})).collect::<Vec<_>>()})
}
fn record(a: &IngeTrazoApp, name: &str) -> Value {
    let mut v=topology(&a.scene.mesh);v["name"]=json!(name);v["undo"]=json!(a.undo_stack.len());v["redo"]=json!(a.redo_stack.len());v
}
fn tris(t: Vec<Tri3>) -> Value {json!({"ok":true,"triangles":t.into_iter().map(|(a,b,c)|[a.to_array(),b.to_array(),c.to_array()]).collect::<Vec<_>>()})}
fn main() {
    let dir=PathBuf::from(std::env::args().nth(1).expect("proof directory"));
    let source:Value=serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let mut saves=vec![];let mut triangles=vec![];let mut history=vec![];let mut reader_checks=vec![];
    for c in source["triangulation"].as_array().unwrap() {
        let name=c["name"].as_str().unwrap();let (outer,holes)=rings(&c["raw_input"]);
        let mut a=IngeTrazoApp::new();a.system_clipboard=None;
        a.save_undo_state();let id=append(&mut a.scene.mesh,&outer,&holes);let m=&a.scene.mesh;
        let f=&m.faces[id];
        let face_input=json!({"outer":f.vertices.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()});
        triangles.push(json!({"name":name,"raw_input":c["raw_input"],"mesh_face_input":face_input,"face_normal":m.face_normal(id).to_array(),"face":tris(m.triangulate_face(id)),"direct":tris(triangulate(&outer,&holes,None))}));
        if source["save_load"].as_array().unwrap().iter().any(|s|s["name"]==name) {
            let before=topology(m);let count=m.edges.len()+m.faces.len()*4;
            let path=dir.join(format!("{name}.native.igz"));
            let save=match ingetrazo_formats::igz::save_igz(&a.scene,&path){Ok(())=>json!({"ok":true}),Err(e)=>json!({"ok":false,"error":e.to_string()})};
            let load=match ingetrazo_formats::igz::load_igz(&path){Ok(s)=>json!({"ok":true,"topology":topology(&s.mesh)}),Err(e)=>json!({"ok":false,"error":e.to_string()})};
            let original=ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}.original.igz"))).unwrap();
            reader_checks.push(json!({"name":name,"topology":topology(&original.mesh)}));
            saves.push(json!({"name":name,"constructed_topology":before,"reader_estimated_corners":count,"reader_route":if count<1024{"plain"}else{"bulk"},"save":save,"load":load}));
            history.push(record(&a,name));a.undo();history.push(record(&a,&format!("{name}-undo")));a.redo();history.push(record(&a,&format!("{name}-redo")));
        }
    }
    // Preservation must not turn a later ordinary Push/Pull acquisition into
    // an unchecked empty-ring index. Its refusal owns no model/history change.
    for empty_first in [false,true] {
        use ingetrazo_tools::base::{ToolContext,ToolType};
        let mut a=IngeTrazoApp::new();a.system_clipboard=None;
        if empty_first {append(&mut a.scene.mesh,&[],&[]);}
        append(&mut a.scene.mesh,&[DVec3::ZERO,DVec3::X,DVec3::Y],&[]);
        if !empty_first {append(&mut a.scene.mesh,&[],&[]);}
        a.set_tool(ToolType::PushPull);let before=a.scene.clone();
        let point=DVec3::new(0.2,0.2,0.);
        a.dispatch_tool_click(&ToolContext::new(point,ingetrazo_core::math::Ray::new(point+DVec3::Z,-DVec3::Z)));
        assert!(a.status_message.contains("empty face"),"{}",a.status_message);
        assert_eq!(a.scene,before);assert!(a.undo_stack.is_empty());assert!(a.redo_stack.is_empty());
    }
    let mut invalid=Mesh::new();let v=invalid.add_vertex(DVec3::ZERO);let before=invalid.clone();
    assert!(invalid.add_face(vec![v,999,v],vec![]).is_none());assert_eq!(invalid,before);
    assert!(invalid.add_face(vec![],vec![vec![999]]).is_none());assert_eq!(invalid,before);
    for (name,value) in [("native.json",json!({"schema":1,"save_load":saves,"triangulation":triangles})),("history-native.json",json!(history)),("reader-native.json",json!(reader_checks))] {
        std::fs::write(dir.join(name),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}
