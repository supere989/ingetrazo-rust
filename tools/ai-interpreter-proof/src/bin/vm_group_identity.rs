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
    let mut c = vm_groups::Controller::new();
    let mut states = vec![];
    assert!(c.execute("g=extrude([(0,0),(1,0),(1,1),(0,1)],0,1,name='Authored',color=(0.1,0.2,0.3,0.4))\nheld_mesh=g.mesh\nheld_face=g.mesh.faces[0]\nheld_faces=g.mesh.faces\nassert held_faces is g.mesh.faces\nassert isinstance(g,Group) and isinstance(g.mesh,Mesh) and isinstance(held_face,Face)\nassert g is native_groups.groups()[0]",100000).unwrap());
    states.push(c.observe("finish-group"));
    c.undo().unwrap();
    states.push(c.observe("finish-undo"));
    assert!(!c.execute("held_face.attrs['held']=['while-undone']\nassert g.mesh is held_mesh and held_mesh.faces[0] is held_face",30000).unwrap());
    c.redo().unwrap();
    states.push(c.observe("finish-redo"));
    assert!(!c.execute("assert g is native_groups.groups()[0]\nassert g.mesh is held_mesh\nassert g.mesh.faces is held_faces\nassert g.mesh.faces[0] is held_face\nassert g.mesh.faces[0].attrs['held']==['while-undone']",30000).unwrap());
    c.undo().unwrap();
    let before = c.counts();
    let before_bytes = c.budget().snapshot().total();
    c.budget().set_limit(Some(before_bytes));
    let budget_refusal = c.execute(
        "try:\n    extrude([(0,0),(1,0),(0,1)],0,1)\nexcept ValueError:\n    pass",
        100000,
    );
    assert!(matches!(
        budget_refusal,
        Err(vm_groups::Failure::Preparation(_))
    ));
    assert_eq!(c.counts(), before);
    assert_eq!(c.budget().snapshot().total(), before_bytes);
    c.budget().set_limit(None);
    c.set_limit(Some(1));
    let r=c.execute("detached=extrude([(0,0),(1,0),(0,1)],0,1)\ntry:\n    extrude([(0,0),(1,0),(0,1)],0,1)\nexcept ValueError:\n    pass",100000);
    assert!(matches!(r, Err(vm_groups::Failure::Preparation(_))));
    assert_eq!(c.counts(), before);
    c.set_limit(None);
    assert!(c.execute("replacement=extrude([(0,0),(1,0),(0,1)],0,1)\nassert detached is not replacement and detached.mesh is not replacement.mesh\nassert detached.mesh.faces[0] is not replacement.mesh.faces[0]",100000).unwrap());
    assert_eq!(c.counts(), (1, 1, 0));
    let r = c.execute(
        "try:\n    extrude(iter([(0,0),(1,0),(0,1)]),0,1)\nexcept IndexError:\n    pass",
        100000,
    );
    assert!(!r.unwrap());
    assert_eq!(c.counts(), (1, 1, 0));
    assert!(c.execute("color=[1,2,3]\nl=extrude([(0,0),(1,0),(0,1)],0,1,color=color)\nassert all(type(f.attrs['color']) is tuple for f in l.mesh.faces)\nassert l.mesh.faces[0].attrs['color'] is not l.mesh.faces[1].attrs['color']\nt=(1,2,3)\np=extrude([(0,0),(1,0),(0,1)],0,1,color=t)\nassert all(f.attrs['color'] is t for f in p.mesh.faces)\ni=iter([1,2,3])\nq=extrude([(0,0),(1,0),(0,1)],0,1,color=i)\nassert q.mesh.faces[0].attrs['color']==(1,2,3)\nassert all(f.attrs['color']==() for f in q.mesh.faces[1:])\nname=['arbitrary-name']\nr=extrude([(0,0),(1,0),(0,1)],0,1,name=name)\nassert r.name is name",200000).unwrap());
    assert!(
        !c.execute("g.mesh.faces[0].attrs['owner']=g", 30000)
            .unwrap()
    );
    let held = c.hold_group("g");
    let expected = held.charged_bytes();
    let budget = c.budget();
    drop(c);
    assert_eq!(held.counts(), (8, 12, 6));
    assert_eq!(budget.snapshot().total(), expected);
    drop(held);
    assert_eq!(budget.snapshot().total(), 0);
    let mut c = vm_groups::Controller::new();
    let mut geometry = vec![];
    for (name, recipe) in [
        (
            "ccw",
            "extrude([(0,0),(1,0),(1,1),(0,1)],0,1,name='explicit')",
        ),
        (
            "cw",
            "extrude([(0,1),(1,1),(1,0),(0,0)],0,1,name='explicit')",
        ),
        (
            "negative-height",
            "extrude([(0,0),(1,0),(1,1),(0,1)],2,-1,name='explicit')",
        ),
        (
            "flat",
            "extrude([(0,0),(1,0),(1,1),(0,1)],0,0,name='explicit')",
        ),
        ("empty", "extrude([],0,1,name='explicit')"),
    ] {
        assert!(c.execute(&format!("g={recipe}"), 100000).unwrap());
        geometry.push(serde_json::json!({"name":name,"mesh":c.observe_geometry("g")}));
    }
    assert!(!c.execute("cycle=[]\ng.name.append(g) if type(g.name) is list else None\ng.mesh.faces[0].attrs['owner']=g",30000).unwrap());
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    let mut caught = vm_groups::Controller::new();
    assert!(caught.execute("earlier=extrude([(0,0),(1,0),(0,1)],0,1,name='earlier')\ntry:\n    extrude(iter([(0,0),(1,0),(0,1)]),0,1)\nexcept IndexError:\n    pass",100000).unwrap());
    assert_eq!(caught.counts(), (1, 1, 0));
    let caught_budget = caught.budget();
    drop(caught);
    assert_eq!(caught_budget.snapshot().total(), 0);
    let mut shadow = vm_groups::Controller::new();
    assert!(shadow.execute("tuple=lambda x: 'wrong'\nlist=lambda x: 'wrong'\nlen=lambda x: 0\nrange=lambda n: []\ng=extrude([(0,0),(1,0),(0,1)],0,1,name='shadow',color=[1,2,3])\nassert g.mesh.faces[0].attrs['color']==(1,2,3)",100000).unwrap());
    let shadow_held = shadow.hold_group("g");
    assert_eq!(shadow_held.counts(), (6, 9, 5));
    drop(shadow);
    drop(shadow_held);
    let mut first = vm_groups::Controller::new();
    assert!(
        first
            .execute("g=extrude([],0,1)\nfirst_name=g.name", 100000)
            .unwrap()
    );
    let first_held = first.hold_group("g");
    let first_n: u64 = first_held
        .literal_name()
        .strip_prefix("Group ")
        .unwrap()
        .parse()
        .unwrap();
    let first_budget = first.budget();
    let mut second = vm_groups::Controller::new();
    assert!(
        second
            .execute(
                "g=extrude([],0,1)\nmarker=object()\ng.mesh.faces[0].attrs['marker']=marker",
                100000
            )
            .unwrap()
    );
    let second_held = second.hold_group("g");
    let second_n: u64 = second_held
        .literal_name()
        .strip_prefix("Group ")
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(second_n, first_n + 1);
    drop(first);
    assert_eq!(first_budget.snapshot().total(), first_held.charged_bytes());
    assert!(
        !second
            .execute("assert g.mesh.faces[0].attrs['marker'] is marker", 30000)
            .unwrap()
    );
    drop(first_held);
    assert_eq!(first_budget.snapshot().total(), 0);
    assert!(
        !second
            .execute("assert g.mesh.faces[0].attrs['marker'] is marker", 30000)
            .unwrap()
    );
    drop(second);
    drop(second_held);
    println!(
        "{}",
        serde_json::json!({"group_states":states,"geometry":geometry,"held_mutation_visible_after_redo":true,"caught_admission_discards":true,"refusal_preserves_redo":true,"shared_budget_refusal_preserves_history_and_charges":true,"detached_group_mesh_face_identity":true,"outline_double_iteration":true,"caught_python_helper_error_preserves_prior_group":true,"per_face_color_conversion":true,"helper_globals_not_user_shadowed":true,"arbitrary_truthy_name_identity":true,"external_handle_retains_mesh_and_charge":true,"cross_session_name_counter":true,"other_controller_scope_survives_teardown":true,"cyclic_payload_ledger_released":true,"ledger_released":true,"full_mesh_api":false,"all_helper_orientation":false,"existing_object_rollback":false,"production_enabled":false})
    );
}
