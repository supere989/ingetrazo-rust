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
    c.assert_script("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\noldedges=v.edges");
    let initial_history = c.history_counts();
    assert!(!c.execute("replacement=set()\nv.edges=replacement").unwrap());
    assert_eq!(c.history_counts(), initial_history);
    c.assert_script("assert v.edges is replacement\noldedges=v.edges");
    let counts = c.native_counts("m");
    let history = c.history_counts();
    assert!(c
        .execute("e=Edge(v,held.loop[1])\nv.edges.add(e)\nv.position.setX(9)")
        .is_err());
    assert_eq!(c.native_counts("m"), counts);
    assert_eq!(c.history_counts(), history);
    c.assert_script("assert not v.edges and isinstance(v.edges,set)\nassert v.edges is not oldedges\nassert e in oldedges\nassert v.position.x()==0");
    assert!(c
        .execute("v.edges=['unjournaled']\nv.position.setY(7)")
        .is_err());
    c.assert_script("assert not v.edges and isinstance(v.edges,set)\nassert v.position.y()==0");
    assert_eq!(c.history_counts(), history);
    assert!(c.execute("new=m.add_face([QVector3D(3,0,0),QVector3D(4,0,0),QVector3D(3,1,0)])\nnew.loop[0].edges.add(Edge(new.loop[0],new.loop[1]))").is_err());
    assert_eq!(c.native_counts("m"), counts);
    assert_eq!(c.history_counts(), history);
    assert!(c.undo().unwrap());
    assert_eq!(c.native_counts("m"), (0, 0, 0));
    let empty = c.history_counts();
    // Detached external objects are outside this managed-root safety prerequisite.
    assert!(c.redo().unwrap());
    assert_eq!(c.native_counts("m"), counts);
    c.assert_script("assert not v.edges and isinstance(v.edges,set)");
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"empty_replacement_source_control":{"source_accepts_empty_replacement_without_history":true,"actual_replacement_identity_retained":true,"history_before":initial_history,"history_after":initial_history},"managed_root_refusal_restores":true,"staged_incidence_refused":true,"history_preserved":true,"undo_redo_preserves_empty_incidence":true,"empty_history_counts":empty,"final_owner_charge":0,"production_enabled":false,"source_incidence_parity":false})
    );
}
