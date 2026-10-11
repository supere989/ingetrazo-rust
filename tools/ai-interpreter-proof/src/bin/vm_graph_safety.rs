#[path = "../face_owner.rs"]
mod face_owner;
#[path = "../journal.rs"]
mod journal;
#[path = "../owner.rs"]
mod owner;
#[path = "../owner_budget.rs"]
mod owner_budget;
#[path = "../owner_registry.rs"]
mod owner_registry;
#[path = "../session.rs"]
mod session;
#[path = "../source_holes.rs"]
mod source_holes;
#[path = "../vm_groups.rs"]
mod vm_groups;
#[path = "../vm_mesh.rs"]
mod vm_mesh;
fn main() {
 let mut c=vm_mesh::Controller::new(); c.enable_history();
 c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=m.vertices[0]\ne=m.edges[0]\noldvertices=m.vertices\noldedges=m.edges\noldfaces=m.faces\noldvedges=v.edges\noldefaces=e.faces\nV=list(m.vertices)\nE=list(m.edges)\nF=list(m.faces)\ntag=[]");
 assert!(c.undo().unwrap()); assert!(c.redo().unwrap()); assert!(c.undo().unwrap());
 // Preserve an actual nonempty Redo stack for every refused publication.
 let baseline=c.observe_graph("unchanged"); let charge=c.budget().snapshot().total();
 let mut controls=Vec::new();
 for (name,script) in [
  ("created-edge-order", "m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nm.edges.reverse()"),
  ("created-edge-replacement", "m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nm.edges[0]=Edge(m.vertices[0],m.vertices[1])"),
  ("created-edge-endpoint", "m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nm.edges[0].v0=m.vertices[2]")
 ] {
  assert!(c.execute(script).is_err(),"{name}");
  assert_eq!(c.observe_graph("unchanged"),baseline); assert_eq!(c.budget().snapshot().total(),charge);
  controls.push(name);
 }
 for kind in 1..=2 {
  c.graph_reentry_probe(kind);
  assert!(c.execute("m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])").is_err());
  assert_eq!(c.observe_graph("unchanged"),baseline); assert_eq!(c.budget().snapshot().total(),charge);
  // A fresh execution recovers only after the poisoned stage has been discarded.
  c.assert_script("assert len(m.edges)==0");
 }
 assert!(c.redo().unwrap());
 let budget=c.budget();drop(c);assert_eq!(budget.snapshot().total(),0);
 assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));
 println!("{}",serde_json::json!({"identity_refusals":controls,"real_caught_reentry_controls":2,"redo_preserved":true,"final_charge":0,"production_enabled":false,"source_parity_claim":false}));
}
