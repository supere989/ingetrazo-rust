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
    c.assert_script("m=mesh\nvertices=m.vertices\nfaces=m.faces\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nloop=held.loop\nheldattrs=held.attrs\nheldposition=held.loop[0].position\nshared=[]\nheld.attrs['shared']=shared");
    let initial = c.observe("initial");
    c.assert_script("heldposition.setX(2)\nheld.loop.reverse()\nshared.append('live')");
    let edited = c.observe("edited");
    assert_eq!(
        edited["mesh"]["vertices"][0],
        serde_json::json!([2., 0., 0.])
    );
    assert_eq!(
        edited["mesh"]["faces"][0]["vertices"],
        serde_json::json!([2, 1, 0])
    );
    c.assert_script("assert heldposition.x()==2 and heldposition is m.vertices[0].position\nassert held.loop is loop and held.attrs is heldattrs\nassert held.attrs['shared'] is shared and shared==['live']");
    assert!(c.execute("held.loop.append(QVector3D(9,9,9))").is_err());
    let refused = c.observe("refused");
    assert_eq!(refused["mesh"], edited["mesh"]);
    c.assert_script("assert len(held.loop)==3 and held.loop is not loop\nassert m.vertices[0].position is not heldposition");
    // Existing source weld keys survive raw QVector edits. A later face at old
    // coordinates reuses the moved actual Vertex; the new coordinate creates one.
    c.assert_script("old=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nassert old.loop[0] is m.vertices[0]\nnew=m.add_face([QVector3D(2,0,0),QVector3D(3,0,0),QVector3D(2,1,0)])\nassert new.loop[0] is not m.vertices[0]\nassert new.loop[0].position==m.vertices[0].position");
    let counts = c.native_counts("m");
    assert_eq!(counts, (6, 6, 3));
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"states":[initial,edited,refused],"raw_weld_key_identity":true,"new_key_distinct_coincident_vertex":true,"invalid_loop_refusal_restores_vm_and_native":true,"final_charge":0,"unchanged_membership_only":true,"controller_history":false,"production_enabled":false})
    );
}
