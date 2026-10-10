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
fn main() {
 let rows:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(std::env::args().nth(1).expect("recipes")).unwrap()).unwrap();
 let mut c=vm_groups::Controller::new();
 let mut names=vec![];
 for row in rows.as_array().unwrap(){match c.execute(row["code"].as_str().unwrap(),100000){Ok(changed)=>assert!(!changed),Err(e)=>{eprintln!("case {}",row["name"]);c.print_failure(&e);panic!("vector control rejected")}}names.push(row["name"].clone());}
 println!("{}",serde_json::json!({"passed":names,"production_enabled":false}));
}
