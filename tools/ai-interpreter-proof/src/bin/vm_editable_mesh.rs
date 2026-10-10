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
    let mut states = vec![];
    c.assert_script("m=mesh\nvertices=m.vertices\nfaces=m.faces\np=QVector3D(0,0,0)\nf=m.add_face([p,QVector3D(1,0,0),QVector3D(0,1,0)])\nheld=f\nloop=f.loop\nheldattrs=f.attrs\nheldposition=f.loop[0].position\nshared=[]\nf.attrs['shared']=shared\nassert isinstance(f,Face) and isinstance(f.loop[0],Vertex)\nassert f is m.faces[0] and f.loop is loop\nassert f.loop[0].position is not p\nassert f.loop[0].position==p");
    assert_eq!(c.native_counts("m"), (3, 3, 1));
    states.push(c.observe("initial"));
    c.assert_script("g=m.add_face([QVector3D(0,0,0),QVector3D(0,1,0),QVector3D(0,0,1)])\nassert g.loop[0] is held.loop[0] and g.loop[1] is held.loop[2]\nassert m.vertices is vertices and m.faces is faces\nassert held.attrs['shared'] is shared");
    assert_eq!(c.native_counts("m"), (4, 5, 2));
    states.push(c.observe("shared"));
    assert!(c.execute("detached=m.add_face([QVector3D(4,0,0),QVector3D(5,0,0),QVector3D(4,1,0)])\nraise ValueError('rollback')").is_err());
    assert_eq!(c.native_counts("m"), (4, 5, 2));
    states.push(c.observe("abort"));
    c.assert_script("assert len(m.faces)==2 and len(m.vertices)==4\nassert detached is not held and detached not in m.faces\nassert detached.loop[0].position==QVector3D(4,0,0)\nassert m.faces is faces and m.vertices is vertices");
    c.assert_script("n=Mesh()");
    c.adopt("n");
    c.assert_script("a=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nb=n.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])");
    assert_eq!(c.native_counts("m"), (4, 5, 3));
    assert_eq!(c.native_counts("n"), (3, 3, 1));
    c.assert_script("assert held.normal()==QVector3D(0,0,1)\nassert held.centroid()==QVector3D(1/3,1/3,0)\nassert held.vertices is not held.vertices\nassert held.vertices[0] is held.loop[0].position");
    c.pressure_after(Some(1));
    assert!(c.execute("failed_a=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nfailed_b=n.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])").is_err());
    assert_eq!(c.last_prepared(), 1);
    assert_eq!(c.native_counts("m"), (4, 5, 3));
    assert_eq!(c.native_counts("n"), (3, 3, 1));
    c.pressure_after(None);
    c.assert_script("assert len(m.faces)==3 and len(n.faces)==1\nassert failed_a not in m.faces and failed_b not in n.faces\nassert len(failed_a.loop)==3 and len(failed_b.loop)==3");
    assert!(c.execute("private=Mesh()\nprivate.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nraise ValueError('private survives')").is_err());
    assert_eq!(c.native_counts("private"), (3, 3, 1));
    c.assert_script("held.attrs['cycle']=m");
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"states":states,"persistent_mesh_lists":true,"real_face_vertex_identity":true,"source_welded_vertex_reuse":true,"detached_face_valid_after_abort":true,"native_projection":true,"face_live_geometry_reads":true,"two_mesh_publication":true,"second_owner_pressure_retains_both":true,"private_scope_edit_survives_error":true,"final_zero_with_cycle":true,"general_existing_object_rollback":false,"undo_redo_controller":false,"production_enabled":false})
    );
}
