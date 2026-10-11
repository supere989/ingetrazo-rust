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
fn main(){
 let args=std::env::args().collect::<Vec<_>>();let input:Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();let mut rows=vec![];
 for case in input["cases"].as_array().unwrap(){
  let original=load(&case["source"]);let id=case["face"].as_u64().unwrap()as usize;
  let tri=original.triangulate_face(id)[0];let anchor=(tri.0+tri.1+tri.2)/3.;let authored=original.face_normal(id);
  let ctx=ToolContext::new(anchor,Ray::new(anchor+authored*0.01,-authored));
  let mut app=IngeTrazoApp::new();app.system_clipboard=None;app.scene.mesh=original.clone();app.set_tool(ToolType::PushPull);
  let revision=app.model_revision;app.dispatch_tool_click(&ctx);
  let acquired=app.scene.mesh.clone();let normal=app.active_tool.face_sweep_preview().unwrap().normal;
  let acquisition_undo=app.undo_stack.len();let invalidated=app.model_revision!=revision;
  app.dispatch_tool_cancel();let cancel=app.scene.mesh.clone();
  app.dispatch_tool_click(&ctx);let mut moved=ctx.clone();let requested=case["requested"].as_f64().unwrap();moved.world+=normal*requested/normal.length_squared();app.dispatch_tool_move(&moved);
  let preview=app.active_tool.face_sweep_preview().unwrap();
  let outcome=app.dispatch_tool_vcb(&requested.to_string());let error=outcome.err();let result=app.scene.mesh.clone();let commit_undo=app.undo_stack.len();
  let mut undo=result.clone();let mut redo=result.clone();if commit_undo>0{app.undo();undo=app.scene.mesh.clone();app.redo();redo=app.scene.mesh.clone();}
  rows.push(json!({"name":case["name"],"normal":normal.to_array(),"acquired":snap(&acquired),"cancel":snap(&cancel),"result":snap(&result),"undo":snap(&undo),"redo":snap(&redo),"error":error,"acquisition_undo":acquisition_undo,"commit_undo":commit_undo,"acquisition_invalidated":invalidated,"offset":preview.offset.to_array()}));
 }
 std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap()).unwrap();
}
