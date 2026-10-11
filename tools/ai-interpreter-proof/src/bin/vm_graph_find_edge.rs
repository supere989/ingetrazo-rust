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
fn main(){let mut c=vm_mesh::Controller::new();c.enable_history();let recipes:Vec<String>=serde_json::from_str(include_str!("../../../../scripts/fixtures/ai-graph-find-edge.json")).unwrap();for recipe in recipes{c.assert_script(&recipe);}let observed=c.observe_graph_order();let budget=c.budget();drop(c);assert_eq!(budget.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));println!("{}",observed);}
