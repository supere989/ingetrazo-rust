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

fn main(){let cases:Vec<serde_json::Value>=serde_json::from_str(include_str!("../../../../scripts/fixtures/ai-vertex-membership.json")).unwrap();let mut states=Vec::new();for case in cases{let name=case["name"].as_str().unwrap();let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nV=list(m.vertices)\nv=V[0]");assert!(c.execute(case["recipe"].as_str().unwrap()).unwrap());states.push(c.observe_vertex_membership(&format!("{name}/edited")));assert!(c.undo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/undone")));assert!(c.redo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/redone")));if name=="detached-pose-in-removal"{assert!(c.undo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/undone-again")));assert!(c.redo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/redone-again")));}if name=="registry-alias"{c.assert_script("g=m.add_face([QVector3D(9,0,0),QVector3D(10,0,0),QVector3D(9,1,0)])\nV.extend(m.vertices[3:])\nassert g.loop[0] is v");states.push(c.observe_vertex_membership(&format!("{name}/appended")));assert!(c.undo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/append-undone")));assert!(c.redo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/append-redone")));}let b=c.budget();drop(c);assert_eq!(b.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));}let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nV=list(m.vertices)\nv=V[0]");c.assert_script("m.vertices.remove(v)");assert!(c.undo().unwrap());let before=c.observe_vertex_membership("refused");let charge=c.budget().snapshot().total();c.pressure_after_script(true);assert!(c.execute("m.vertices.clear()").is_err());c.pressure_after_script(false);assert_eq!(c.observe_vertex_membership("refused"),before);assert_eq!(c.budget().snapshot().total(),charge);assert!(c.redo().unwrap());c.assert_script("meta=[37]\nm._registry['vendor']=meta");assert!(c.undo().unwrap());c.assert_script("assert 'vendor' not in m._registry");assert!(c.redo().unwrap());c.assert_script("assert m._registry['vendor'] is meta\nassert meta==[37]");let b=c.budget();drop(c);assert_eq!(b.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));println!("{}",serde_json::json!({"states":states,"final_charge":0,"quota_refusal_preserves_graph_redo_and_charge":true,"opaque_registry_payload_identity_history":true,"production_enabled":false}));}
