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
fn main(){let cases:Vec<serde_json::Value>=serde_json::from_str(include_str!("../../../../scripts/fixtures/ai-graph-radial.json")).unwrap();let mut states=Vec::new();for case in cases {let name=case["name"].as_str().unwrap();let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script(case["recipe"].as_str().unwrap());states.push(c.observe_mesh_incidence(&format!("{name}/edited")));assert!(c.undo().unwrap());states.push(c.observe_mesh_incidence(&format!("{name}/undone")));assert!(c.redo().unwrap());states.push(c.observe_mesh_incidence(&format!("{name}/redone")));let budget=c.budget();drop(c);assert_eq!(budget.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));}println!("{}",serde_json::json!({"states":states,"production_enabled":false}));}
