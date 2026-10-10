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
    c.assert_script("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\noldloop=held.loop\noldposition=v.position\noldattrs=held.attrs\nshared=[]\nheld.attrs['shared']=shared");
    states.push(c.observe_history("initial"));
    c.assert_script("v.position.setX(2)\nshared.append('live')\nadded=m.add_face([QVector3D(0,0,0),QVector3D(0,0,1),QVector3D(0,1,0)])\nheld.loop[1]=added.loop[1]");
    c.assert_script("held_added_vertex=added.loop[1]\nassert held.loop[1] is held_added_vertex");
    states.push(c.observe_history("mixed"));
    assert!(c.undo().unwrap());
    c.assert_script("assert all(f is not added for f in m.faces)\nassert held.loop[1] is not held_added_vertex\nassert held_added_vertex.position.z()==1");
    states.push(c.observe_history("undone"));
    assert!(c.redo().unwrap());
    c.assert_script("assert added is m.faces[-1]\nassert held.loop[1] is held_added_vertex\nassert added.loop[1] is held_added_vertex");
    states.push(c.observe_history("redone"));
    assert!(c.undo().unwrap());
    states.push(c.observe_history("undone-again"));
    assert!(c.redo().unwrap());
    states.push(c.observe_history("redone-again"));
    assert!(c.undo().unwrap());
    let old = c.observe_history("quota");
    let counts = c.history_counts();
    assert!(c.execute("v.position.setZ(7)\nfailed=m.add_face([QVector3D(3,0,0),QVector3D(4,0,0),QVector3D(3,1,0)])\nraise ValueError('mixed rollback')").is_err());
    assert_eq!(old, c.observe_history("quota"));
    assert_eq!(counts, c.history_counts());
    let charge = c.budget().snapshot().total();
    c.budget().set_limit(Some(charge));
    assert!(c.redo().is_err());
    assert_eq!(old, c.observe_history("quota"));
    assert_eq!(charge, c.budget().snapshot().total());
    c.budget().set_limit(None);
    assert!(c.redo().unwrap());
    assert!(c.undo().unwrap());
    assert!(c.undo().unwrap());
    assert_eq!(c.native_counts("m"), (0, 0, 0));
    assert!(c.redo().unwrap());
    assert!(c.redo().unwrap());
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"states":states,"appended_reference_identity":true,"quota_preserves_command":true,"execution_refusal_preserves_redo":true,"chronological_undo_redo":true,"final_charge":0,"production_enabled":false})
    );
}
