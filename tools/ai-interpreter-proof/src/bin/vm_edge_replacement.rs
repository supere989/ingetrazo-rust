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
fn main(){let cases:Vec<serde_json::Value>=serde_json::from_str(include_str!("../../../../scripts/fixtures/ai-edge-replacement.json")).unwrap();let mut states=Vec::new();for case in cases{let name=case["name"].as_str().unwrap();let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nv0=m.vertices[0]\nv1=m.vertices[1]\nv2=m.vertices[2]\ne=m.edges[0]\noldedges=list(m.edges)");c.assert_script(case["recipe"].as_str().unwrap());states.push(c.observe_edge_replacement(&format!("{name}/edited")));assert!(c.undo().unwrap());c.assert_script("assert m.edges[0] is e");states.push(c.observe_edge_replacement(&format!("{name}/undone")));assert!(c.redo().unwrap());states.push(c.observe_edge_replacement(&format!("{name}/redone")));let budget=c.budget();drop(c);assert_eq!(budget.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));}let mut c=vm_mesh::Controller::new();c.enable_history();c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\ne=m.edges[0]");c.assert_script("e.soft=True");assert!(c.undo().unwrap());let before=c.observe_edge_replacement("refused");let charge=c.budget().snapshot().total();c.pressure_after_script(true);assert!(c.execute("m.remove_edge(e)").is_err());c.pressure_after_script(false);assert_eq!(c.observe_edge_replacement("refused"),before);assert_eq!(c.budget().snapshot().total(),charge);assert!(c.redo().unwrap());assert!(c.undo().unwrap());assert!(c.execute("m.edges[0]=Edge(Vertex(QVector3D(8,8,8)),m.vertices[1])").is_err());assert_eq!(c.observe_edge_replacement("refused"),before);assert_eq!(c.budget().snapshot().total(),charge);assert!(c.redo().unwrap());let budget=c.budget();drop(c);assert_eq!(budget.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));println!("{}",serde_json::json!({"states":states,"quota_refusal_preserves_graph_history_and_charge":true,"detached_endpoint_refusal_preserves_graph_history_and_charge":true,"final_charge":0,"production_enabled":false}));}
