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
 for case in input["cases"].as_array().unwrap(){
  let original=load(&case["source"]);let id=case["face"].as_u64().unwrap()as usize;
  let mut app=IngeTrazoApp::new();app.system_clipboard=None;
  let nested=case["nested"].as_bool().unwrap_or(false);
  if nested {
   use ingetrazo_core::{groups::Group,scene::SelectionItem};
   let mut child=Group::with_mesh("shared-stack",original.clone());
   child.transform=glam::DMat4::from_translation(DVec3::new(2.,3.,0.));
   let uid=child.id;app.scene.groups.push(child);app.scene.make_component(uid).unwrap();
   let sibling=app.scene.groups[0].duplicate_instance();let child=app.scene.groups.remove(0);
   let mut parent=Group::new("parent");parent.transform=glam::DMat4::from_translation(DVec3::new(4.,5.,6.));parent.children.push(child);
   app.scene.groups.extend([parent,sibling]);
   app.scene.select(SelectionItem::Group(0));app.begin_component_edit();
   app.scene.select(SelectionItem::Group(0));app.begin_component_edit();
   assert_eq!(app.component_edit_stack.len(),2);
  } else {app.scene.mesh=original.clone();}
  let tri=app.scene.mesh.triangulate_face(id)[0];let anchor=(tri.0+tri.1+tri.2)/3.;let authored=app.scene.mesh.face_normal(id);
  let mut ctx=ToolContext::new(anchor,Ray::new(anchor+authored*0.01,-authored));ctx.ctrl_held=case["ctrl"].as_bool().unwrap_or(false);
  app.set_tool(ToolType::PushPull);
  let revision=app.model_revision;app.dispatch_tool_click(&ctx);
  let acquired=app.scene.mesh.clone();let normal=app.active_tool.face_sweep_preview().unwrap().normal;
  let acquisition_undo=app.undo_stack.len();let invalidated=app.model_revision!=revision;
  app.dispatch_tool_cancel();let cancel=app.scene.mesh.clone();
  app.dispatch_tool_click(&ctx);let mut moved=ctx.clone();let requested=case["distance"].as_f64().unwrap();moved.world+=normal*requested/normal.length_squared();app.dispatch_tool_move(&moved);
  let preview=app.active_tool.face_sweep_preview().unwrap();
  let outcome=app.dispatch_tool_vcb(&requested.abs().to_string());let error=outcome.err();let result=app.scene.mesh.clone();let commit_undo=app.undo_stack.len();let commit_status=app.status_message.clone();
  assert!(error.is_none(), "{}: {:?}",case["name"],error);
  assert_eq!(commit_undo,1);assert_eq!(cancel,acquired);assert_eq!(identities(&cancel),identities(&acquired));assert_eq!(acquisition_undo,0);
  let mut undo=result.clone();let mut redo=result.clone();if commit_undo>0{app.undo();undo=app.scene.mesh.clone();assert_eq!(undo,acquired);assert_eq!(identities(&undo),identities(&acquired));app.redo();redo=app.scene.mesh.clone();assert_eq!(redo,result);assert_eq!(identities(&redo),identities(&result));}
  let mut linked_finish=false;
  if nested {
   app.finish_component_edit();app.finish_component_edit();assert!(app.component_edit_stack.is_empty());
   assert_eq!(app.scene.groups[0].children[0].mesh,app.scene.groups[1].mesh);
   let accepted=app.scene.groups[1].mesh.clone();app.undo();assert_eq!(app.scene.groups[1].mesh,original);
   app.redo();assert_eq!(app.scene.groups[1].mesh,accepted);linked_finish=true;
  }
  rows.push(json!({"name":case["name"],"nested":nested,"linked_finish":linked_finish,"cancel_exact":cancel==acquired,"undo_exact":undo==acquired,"redo_exact":redo==result,"normal":normal.to_array(),"acquired":snap(&acquired),"cancel":snap(&cancel),"result":snap(&result),"undo":snap(&undo),"redo":snap(&redo),"error":error,"acquisition_undo":acquisition_undo,"commit_undo":commit_undo,"acquisition_invalidated":invalidated,"offset":preview.offset.to_array(),"vcb_positive_magnitude":requested.abs(),"status":commit_status}));
 }
 std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap()).unwrap();
}
