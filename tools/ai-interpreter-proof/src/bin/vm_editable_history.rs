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
    c.enable_history();
    let mut states = vec![];
    let empty_charge = c.budget().snapshot();
    c.budget().set_limit(Some(empty_charge.total()));
    assert!(!c.undo().unwrap());
    assert!(!c.redo().unwrap());
    assert_eq!(c.budget().snapshot().total(), empty_charge.total());
    assert_eq!(c.history_counts(), (0, 0));
    c.budget().set_limit(None);

    c.assert_script("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\noldloop=held.loop\noldposition=v.position\noldattrs=held.attrs\nshared=[]\nheld.attrs['shared']=shared");
    states.push(c.observe_history("initial"));
    c.assert_script("v.position.setX(2)\nheld.loop.reverse()\nshared.append('live')");
    states.push(c.observe_history("edited"));
    assert!(c.undo().unwrap());
    states.push(c.observe_history("undone"));
    assert!(c.redo().unwrap());
    states.push(c.observe_history("redone"));
    assert!(c.undo().unwrap());
    states.push(c.observe_history("undone-again"));
    assert!(c.redo().unwrap());
    states.push(c.observe_history("redone-again"));
    // Refusal while Redo exists must not discard either history or published state.
    assert!(c.undo().unwrap());
    let prior = c.observe_history("refused");
    let counts = c.history_counts();
    let limit = c.budget().snapshot().total();
    c.budget().set_limit(Some(limit));
    assert!(c.redo().is_err());
    assert_eq!(c.history_counts(), counts);
    assert_eq!(c.observe_history("refused"), prior);
    c.budget().set_limit(None);
    assert!(c
        .execute("v.position.setZ(7)\nraise ValueError('restore history')")
        .is_err());
    assert_eq!(c.history_counts(), counts);
    assert_eq!(c.observe_history("refused"), prior);
    assert!(!c.execute("noop=42").unwrap());
    assert_eq!(c.history_counts(), counts);
    assert!(c.redo().unwrap());
    // Chronological raw-field Undo restores append token; initial append can then
    // Undo/Redo its true native ownership while held Face remains valid.
    assert!(c.undo().unwrap());
    assert!(c.undo().unwrap());
    assert_eq!(c.native_counts("m"), (0, 0, 0));
    c.assert_script("assert held is not None and len(held.loop)==3");
    assert!(c.redo().unwrap());
    assert!(c.redo().unwrap());
    assert_eq!(c.native_counts("m"), (3, 3, 1));
    assert!(c.undo().unwrap());
    c.assert_script("v.position.setY(5)");
    assert_eq!(c.history_counts(), (2, 0));
    assert!(!c.redo().unwrap());
    assert!(c.undo().unwrap());
    assert_eq!(c.native_counts("m"), (3, 3, 1));
    assert!(c.redo().unwrap());
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"states":states,"redo_refusal_retains_history":true,"execution_refusal_retains_redo":true,"no_op_retains_redo":true,"chronological_append_field_history":true,"branch_truncation_retains_new_command":true,"empty_history_no_admission":true,"final_charge":0,"full_mutation_history":false,"production_enabled":false})
    );
}
