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
    let mut rows = Vec::new();
    for mode in [
        "false",
        "true",
        "raise",
        "mutating-same",
        "mutating-field",
        "geometry-short-circuit",
    ] {
        let mut c = vm_mesh::Controller::new();
        c.enable_history();
        c.assert_script(include_str!(
            "../../../../scripts/fixtures/ai-graph-equality-setup.py"
        ));
        c.assert_script("m.add_face([QVector3D(2,0,0),QVector3D(3,0,0),QVector3D(2,1,0)])");
        assert!(c.undo().unwrap());
        c.assert_script(&format!("eq_calls.clear()\nmode={mode:?}"));
        let result = c.execute(if mode == "geometry-short-circuit" {
            "newp=EqualPayload()\ne.soft=newp\nm.vertices[0].position=QVector3D(0,0,2)"
        } else {
            "newp=EqualPayload()\ne.soft=newp"
        });
        let state = c.observe_eq_outcome();
        rows.push(serde_json::json!({"mode":mode,"result":result,"state":state}));
        let budget = c.budget();
        drop(c);
        assert_eq!(budget.snapshot().total(), 0);
        assert_eq!(vm_mesh::graph_payload_lifetimes(), (0, 0));
    }
    println!(
        "{}",
        serde_json::json!({"cases":rows,"production_enabled":false})
    );
}
