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
fn main(){let cases:Vec<serde_json::Value>=serde_json::from_str(include_str!("../../../../scripts/fixtures/ai-edge-permutation.json")).unwrap();let mut states=Vec::new();for case in cases {let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script("m=mesh\nf0=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nf1=m.add_face([QVector3D(1,0,0),QVector3D(),QVector3D(0,0,1)])");let name=case["name"].as_str().unwrap();c.assert_script(case["recipe"].as_str().unwrap());states.push(c.observe_packed_graph(&format!("{name}/edited")));assert!(c.undo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/undone")));assert!(c.redo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/redone")));assert!(c.undo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/repeat-undone")));let before=c.observe_packed_graph("refused");let charge=c.budget().snapshot().total();c.pressure_after_script(true);assert!(c.execute("m.edges.reverse()").is_err());c.pressure_after_script(false);assert_eq!(c.observe_packed_graph("refused"),before);assert_eq!(c.budget().snapshot().total(),charge);assert!(c.redo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/repeat-redone")));
 c.assert_script("f2=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(2,2,2)])");
 states.push(c.observe_packed_graph(&format!("{name}/append-shared")));
 assert!(c.undo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/append-undone")));
 assert!(c.undo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/chronological-order-undone")));
 assert!(c.redo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/chronological-order-redone")));
 assert!(c.redo().unwrap());states.push(c.observe_packed_graph(&format!("{name}/append-redone")));
let budget=c.budget();drop(c);assert_eq!(budget.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));}println!("{}",serde_json::json!({"states":states,"production_enabled":false}));}
