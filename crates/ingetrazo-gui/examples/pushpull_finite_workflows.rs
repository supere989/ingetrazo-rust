use glam::DVec3;
use ingetrazo_core::{
    math::Ray,
    mesh::Mesh,
};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::{
    base::{ToolContext, ToolType},
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
fn identities(m:&Mesh)->(Vec<u64>,Vec<u64>,Vec<uuid::Uuid>){
 (m.faces.iter().map(|f|f.history_id).collect(),m.edges.iter().map(|e|e.history_id).collect(),m.vertices.iter().map(|v|v.uid).collect())
}
fn main(){
 let args=std::env::args().collect::<Vec<_>>();let input:Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();let mut rows=vec![];
 let mut app=IngeTrazoApp::new();app.system_clipboard=None;let mut workflow=String::new();
 for case in input["cases"].as_array().unwrap(){
  let source=load(&case["source"]);let source_id=case["face"].as_u64().unwrap()as usize;
  let current=case["workflow"].as_str().unwrap();
  if workflow!=current {app=IngeTrazoApp::new();app.system_clipboard=None;app.scene.mesh=source.clone();workflow=current.to_owned();}
  let selected=&source.faces[source_id];
  let id=app.scene.mesh.faces.iter().position(|f|f.vertices.len()==selected.vertices.len()&&f.vertices.iter().all(|&v|selected.vertices.iter().any(|&sv|app.scene.mesh.vertices[v].position==source.vertices[sv].position))).expect("persistent selected face exists");
  let tri=app.scene.mesh.triangulate_face(id)[0];let anchor=(tri.0+tri.1+tri.2)/3.;let authored=app.scene.mesh.face_normal(id);
  let ctx=ToolContext::new(anchor,Ray::new(anchor+authored*0.01,-authored));app.set_tool(ToolType::PushPull);
  let before=app.undo_stack.len();app.dispatch_tool_click(&ctx);
  let acquired=app.scene.mesh.clone();let normal=app.active_tool.face_sweep_preview().expect("actual acquisition").normal;
  let acquisition_undo=app.undo_stack.len()-before;
  app.dispatch_tool_cancel();let cancel=app.scene.mesh.clone();
  app.dispatch_tool_click(&ctx);let mut moved=ctx.clone();let requested=case["distance"].as_f64().unwrap();moved.world+=normal*requested/normal.length_squared();app.dispatch_tool_move(&moved);
  let error=app.dispatch_tool_vcb(&requested.abs().to_string()).err();let result=app.scene.mesh.clone();let commit_undo=app.undo_stack.len()-before;
  assert!(error.is_none(),"{}: {:?}",case["name"],error);assert_eq!(commit_undo,1);assert_eq!(acquisition_undo,0);
  assert_eq!(cancel,acquired);assert_eq!(identities(&cancel),identities(&acquired));
  app.undo();let undo=app.scene.mesh.clone();assert_eq!(undo,acquired);assert_eq!(identities(&undo),identities(&acquired));
  app.redo();let redo=app.scene.mesh.clone();assert_eq!(redo,result);assert_eq!(identities(&redo),identities(&result));
  rows.push(json!({"name":case["name"],"workflow":workflow,"error":error,"result":snap(&result),"acquired":snap(&acquired),"undo":snap(&undo),"redo":snap(&redo),"cancel":snap(&cancel),"commit_undo":commit_undo,"acquisition_undo":acquisition_undo,"cancel_exact":true,"undo_exact":true,"redo_exact":true,"persistent_undo_count":app.undo_stack.len()}));
 }
 std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap()).unwrap();
}
