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
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (0, 0));
    let mut c = vm_mesh::Controller::new();
    c.proof_automatic_gc(false);
    c.assert_script("a=Vertex(QVector3D(0,0,0))\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)\nheld=a\ndel a,b,e");
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (2, 1));
    c.collect_proof_gc();
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (2, 1));
    c.assert_script(
        "assert len(held.edges)==1\nassert next(iter(held.edges)).v0 is held\ndel held",
    );
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (2, 1));
    eprintln!("before-final-collection {}", c.proof_gc_graph_inventory());
    c.collect_proof_gc();
    eprintln!("after-final-collection {}", c.proof_gc_graph_inventory());
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (0, 0));
    let mut slot_cases = Vec::new();
    for field in ["faces", "layer", "curve", "soft", "hidden"] {
        c.assert_script(&format!("a=Vertex(QVector3D())\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\ne.{field}={{e}}\ndel a,b,e"));
        assert_eq!(vm_mesh::graph_payload_lifetimes(), (2, 1));
        c.collect_proof_gc();
        assert_eq!(vm_mesh::graph_payload_lifetimes(), (0, 0));
        slot_cases.push(field);
    }
    c.assert_script("private=Mesh()\nf=private.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\na=f.loop[0]\nb=f.loop[1]\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)\ne.faces.append(f)\ndel private,f,a,b,e");
    assert_eq!(vm_mesh::graph_payload_lifetimes(),(3,4));
    c.collect_proof_gc(); assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));
    let mut other = vm_mesh::Controller::new();
    other.proof_automatic_gc(false);
    other.assert_script("a=Vertex(QVector3D())\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)");
    c.assert_script("a=Vertex(QVector3D())\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)");
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (4, 2));
    let budget = c.budget();
    drop(c);
    assert_eq!(budget.snapshot().total(), 0);
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (2, 1));
    other.assert_script("assert e.v0 is a and e in a.edges and e.v1 is b and e in b.edges");
    let budget = other.budget();
    drop(other);
    assert_eq!(budget.snapshot().total(), 0);
    assert_eq!(vm_mesh::graph_payload_lifetimes(), (0, 0));
    println!(
        "{}",
        serde_json::json!({"generated_face_vertex_edge_cycle_collected":true,"opaque_slot_cycles_collected":slot_cases,"external_alias_retains_cycle":true,"actual_cycle_survives_without_roots_before_gc":true,"actual_payloads_destroyed_after_gc":true,"controller_drop_preserves_other_scope":true,"actual_final_live_payloads":[0,0],"production_enabled":false,"total_heap_budget_claimed":false})
    );
}
