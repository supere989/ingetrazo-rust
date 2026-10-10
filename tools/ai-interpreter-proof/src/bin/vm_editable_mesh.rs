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
    c.assert_script("held.attrs[(1,2)]=('kept',shared)\nheld.attrs['vendor']={'opaque':[1,2]}\nbefore_loop=held.loop\nbefore_attrs=held.attrs\nbefore_position=held.loop[0].position");
    assert!(c.execute("held.attrs[(1,2)]=('wrong',)\nheld.attrs['added']=7\nheld.loop.reverse()\nheld.loop[0].position.setX(12)\nshared.append('nested')\nm.faces.clear()\nm.vertices.clear()\nraise ValueError('field rollback')").is_err());
    c.assert_script("assert held.attrs[(1,2)][0]=='kept' and held.attrs[(1,2)][1] is shared\nassert held.attrs['vendor']=={'opaque':[1,2]}\nassert 'added' not in held.attrs\nassert shared==['nested']\nassert held.loop is not before_loop and held.attrs is not before_attrs\nassert held.loop[0].position is not before_position\nassert held.normal()==QVector3D(0,0,1)\nassert len(m.faces)==2 and len(m.vertices)==4\nassert m.faces is faces and m.vertices is vertices");
    states.push(c.observe("field-abort"));
    c.assert_script("finalizer_events=[]\nclass Reenter:\n def __del__(self):\n  finalizer_events.append('entered')\n  try:\n   m.add_face([QVector3D(20,0,0),QVector3D(21,0,0),QVector3D(20,1,0)])\n  except BaseException:\n   finalizer_events.append('caught')");
    assert!(
        c.execute("held.attrs['reentry']=Reenter()\nraise ValueError('finalizer discard')")
            .is_err()
    );
    c.assert_script("assert finalizer_events==[]\nassert 'reentry' not in held.attrs");
    assert_eq!(c.native_counts("m"), (4, 5, 2));
    let limit = c.budget().snapshot().total();
    c.budget().set_limit(Some(limit));
    assert!(c.execute("never_ran=True").is_err());
    c.budget().set_limit(None);
    c.assert_script("assert 'never_ran' not in globals()");
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
    c.panic_after_execution(true);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _=c.execute("held.loop.reverse()\nheld.attrs['panic']=1\nheld.loop[0].position.setX(55)\nm.add_face([QVector3D(9,0,0),QVector3D(10,0,0),QVector3D(9,1,0)])");
    }));
    assert!(unwind.is_err());
    assert!(c.tainted());
    assert!(c.execute("42").is_err());
    assert_eq!(c.native_counts("m"), (4, 5, 3));
    let restored = c.observe("unwind");
    assert_eq!(restored["face_loop_preserved"], false);
    assert_eq!(restored["attrs_dict_preserved"], false);
    assert_eq!(restored["vertex_position_preserved"], false);
    let mut neighbor = vm_mesh::Controller::new();
    neighbor.assert_script("assert len(mesh.faces)==0");
    drop(neighbor);
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"states":states,"persistent_mesh_lists":true,"real_face_vertex_identity":true,"source_welded_vertex_reuse":true,"detached_face_valid_after_abort":true,"native_projection":true,"face_live_geometry_reads":true,"two_mesh_publication":true,"second_owner_pressure_retains_both":true,"private_scope_edit_survives_error":true,"final_zero_with_cycle":true,"shallow_existing_field_rollback":true,"discard_finalizer_bytecode_refused":true,"preparation_refusal_session_recovers":true,"unwind_restores_fields_and_retires_session":true,"general_existing_object_rollback":false,"undo_redo_controller":false,"production_enabled":false})
    );
}
