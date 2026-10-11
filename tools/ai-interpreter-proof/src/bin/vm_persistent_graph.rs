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
    c.assert_script("m=mesh\nf0=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nf1=m.add_face([QVector3D(1,0,0),QVector3D(),QVector3D(0,0,1)])\nv=m.vertices[0]\ne=m.edges[0]\noldvertices=m.vertices\noldedges=m.edges\noldfaces=m.faces\noldvedges=v.edges\noldefaces=e.faces\nV=list(m.vertices)\nE=list(m.edges)\nF=list(m.faces)\ntag=['opaque',7]");
    let mut states = vec![c.observe_graph("initial")];
    c.assert_script("e.faces.reverse()\nv.edges.remove(e)\ne.soft=tag\ne.layer=tag\ne.curve=tag");
    states.push(c.observe_graph("fields-edited"));
    assert!(c.undo().unwrap());
    states.push(c.observe_graph("fields-undone"));
    assert!(c.redo().unwrap());
    states.push(c.observe_graph("fields-redone"));
    assert!(c
        .execute("e.faces.clear()\nv.edges.clear()\ne.layer=None\nraise ValueError('reject')")
        .is_err());
    states.push(c.observe_graph("error-restored"));
    assert!(c.undo().unwrap());
    states.push(c.observe_graph("repeat-undone"));
    assert!(c.redo().unwrap());
    states.push(c.observe_graph("repeat-redone"));
    c.assert_script("f2=m.add_face([QVector3D(0,1,0),QVector3D(1,0,0),QVector3D(1,1,1)])\nV=list(m.vertices)\nE=list(m.edges)\nF=list(m.faces)");
    states.push(c.observe_graph("append-shared"));
    assert!(c.undo().unwrap());
    states.push(c.observe_graph("append-undone-detached-held"));
    assert!(c.redo().unwrap());
    states.push(c.observe_graph("append-redone"));
    assert!(c
        .execute("e.v0=m.vertices[2]\nraise ValueError('endpoint-retained')")
        .is_err());
    states.push(c.observe_graph("endpoint-error-retained"));
    assert!(c.undo().unwrap());
    states.push(c.observe_graph("endpoint-undo-retained"));
    assert!(c.redo().unwrap());
    states.push(c.observe_graph("endpoint-redo-retained"));
    assert!(c.undo().unwrap());
    let before = c.observe_graph("quota-refused");
    let charge = c.budget().snapshot().total();
    c.pressure_after_script(true);
    assert!(c
        .execute("failed_face=m.add_face([QVector3D(0,1,0),QVector3D(1,0,0),QVector3D(9,9,9)])")
        .is_err());
    c.pressure_after_script(false);
    assert_eq!(c.observe_graph("quota-refused"), before);
    assert_eq!(c.budget().snapshot().total(), charge);
    assert!(c.redo().unwrap());
    c.assert_script("hash_calls=[]\nclass HashPayload:\n def __hash__(self):\n  hash_calls.append('hash')\n  m.edges[:]=list(m.edges)\n  return 17\nhash_payload=HashPayload()\nv.edges.add(hash_payload)");
    c.assert_script("assert hash_calls==['hash']\nassert hash_payload in v.edges");
    c.assert_script("assert hash_calls==['hash','hash']\nassert len(m.edges)==7");
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (0, 0));
    println!(
        "{}",
        serde_json::json!({"states":states,"production_enabled":false})
    );
}
