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
    let mut c = vm_mesh::Controller::new();
    let recipe = std::env::args().nth(1).map(|p| std::fs::read_to_string(p).unwrap()).unwrap_or_else(|| include_str!("../../../../scripts/fixtures/ai-edge-objects.py").to_owned());
    c.assert_script(&recipe);
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"assertions_passed":true,"production_enabled":false,"scope":"standalone-constructor-and-incidence-objects","final_owner_charge":0})
    );
}
