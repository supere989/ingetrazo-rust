use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group,
    math::Ray,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::{
    base::{Tool, ToolContext, ToolType},
    pushpull::PushPullTool,
};
use serde_json::{json, Value};
fn point(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn load(fs: &Value) -> Mesh {
    let mut m = Mesh::new();
    for f in fs.as_array().unwrap() {
        let mut ring = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|p| m.add_vertex(point(p)))
                .collect::<Vec<_>>()
        };
        let outer = ring(&f["vertices"]);
        let holes = f["holes"]
            .as_array()
            .unwrap()
            .iter()
            .map(&mut ring)
            .collect();
        let id = m.add_face(outer, holes).unwrap();
        m.faces[id].attrs = serde_json::from_value(f["attrs"].clone()).unwrap();
        m.faces[id].interior = f["interior"].as_bool().unwrap();
    }
    m
}
fn snap(m: &Mesh) -> Value {
    json!(m.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":f.attrs,"interior":f.interior})).collect::<Vec<_>>())
}
fn main(){
 let a=std::env::args().collect::<Vec<_>>();let input:Value=serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();let mut rows=vec![];
 for case in input["cases"].as_array().unwrap(){let mesh=load(&case["source"]);let id=case["face"].as_u64().unwrap()as usize;let normal=PushPullTool::acquisition_normal(&mesh,id);let anchor=mesh.faces[id].vertices.iter().map(|&v|mesh.vertices[v].position).sum::<DVec3>()/mesh.faces[id].vertices.len()as f64;let ctx=ToolContext::new(anchor,Ray::new(anchor+mesh.face_normal(id)*0.1,-mesh.face_normal(id)));let mut moved=ctx.clone();moved.world+=normal*case["distance"].as_f64().unwrap();let mut scene=Scene::new();scene.mesh=mesh.clone();let mut tool=PushPullTool::new();tool.on_click(&mut scene,&ctx);assert!(tool.is_dragging);tool.on_move(&mut scene,&moved);let d=tool.extrusion_distance;let sweep=tool.face_sweep_preview().unwrap();let preview=std::iter::once(&mesh.faces[id].vertices).chain(&mesh.faces[id].holes).enumerate().flat_map(|(index,ring)|{let mut faces=vec![];if index==0{faces.push(ring.iter().map(|&v|(mesh.vertices[v].position.as_vec3()+sweep.offset.as_vec3()).as_dvec3().to_array()).collect::<Vec<_>>());}for i in 0..ring.len(){let a=mesh.vertices[ring[i]].position;let b=mesh.vertices[ring[(i+1)%ring.len()]].position;faces.push(vec![a.to_array(),b.to_array(),(b.as_vec3()+sweep.offset.as_vec3()).as_dvec3().to_array(),(a.as_vec3()+sweep.offset.as_vec3()).as_dvec3().to_array()]);}faces}).collect::<Vec<_>>();tool.on_cancel(&mut scene);assert_eq!(scene.mesh,mesh);
 let mut app=IngeTrazoApp::new();app.system_clipboard=None;app.scene.mesh=mesh.clone();app.set_tool(ToolType::PushPull);app.dispatch_tool_click(&ctx);app.dispatch_tool_move(&moved);assert_eq!(app.scene.mesh,mesh);assert!(app.undo_stack.is_empty());app.dispatch_tool_cancel();assert_eq!(app.scene.mesh,mesh);app.dispatch_tool_click(&ctx);app.dispatch_tool_move(&moved);app.dispatch_tool_click(&moved);let result=app.scene.mesh.clone();let changed=result!=mesh;let history=if changed{assert_eq!(app.undo_stack.len(),1);app.undo();assert_eq!(app.scene.mesh,mesh);app.redo();assert_eq!(app.scene.mesh,result);true}else{false};rows.push(json!({"name":case["name"],"normal":normal.to_array(),"distance":d,"preview":preview,"attached":sweep.attached,"prism_cap":sweep.prism_cap,"preview_cancel_exact":true,"history_exact":history,"status":app.status_message,"result":snap(&result)}));
 }std::fs::write(&a[2],serde_json::to_vec_pretty(&json!({"cases":rows,"scope":"Actual tool/app handlers; native visible sweep suppression not yet consumed"})).unwrap()).unwrap();
}
