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

fn main(){let cases:Vec<serde_json::Value>=serde_json::from_str(include_str!("../../../../scripts/fixtures/ai-detached-append.json")).unwrap();let mut states=Vec::new();for case in cases{let name=case["name"].as_str().unwrap();let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nV=list(m.vertices)\nv=V[0]");c.assert_script(case["before"].as_str().unwrap());c.assert_script(case["append"].as_str().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/appended")));let b=c.budget();let charge=b.snapshot().total();let before=c.observe_vertex_membership("refusal");b.set_limit(Some(charge));assert!(c.undo().is_err());b.set_limit(None);assert_eq!(c.observe_vertex_membership("refusal"),before);assert_eq!(b.snapshot().total(),charge);assert!(c.undo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/undone")));let charge=b.snapshot().total();let before=c.observe_vertex_membership("refusal");b.set_limit(Some(charge));assert!(c.redo().is_err());b.set_limit(None);assert_eq!(c.observe_vertex_membership("refusal"),before);assert_eq!(b.snapshot().total(),charge);assert!(c.redo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/redone")));assert!(c.undo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/undone-again")));assert!(c.redo().unwrap());states.push(c.observe_vertex_membership(&format!("{name}/redone-again")));let b=c.budget();drop(c);assert_eq!(b.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));}println!("{}",serde_json::json!({"states":states,"final_charge":0,"undo_redo_refusals_preserve_graph_history_charge":true,"production_enabled":false}));}
