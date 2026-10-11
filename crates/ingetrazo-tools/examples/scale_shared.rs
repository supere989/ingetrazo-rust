// SPDX-License-Identifier: GPL-3.0-or-later
use glam::DVec3;
use ingetrazo_core::{
    groups::Group,
    image_plane::ScaleMatrix,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_tools::scale_tool::grips;
use serde_json::{json, Value};
#[path = "../src/scale_tool/targets.rs"]
mod targets;
fn mesh(m: &Mesh) -> Value {
    json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>(),"attrs":f.attrs})).collect::<Vec<_>>()})
}
fn snap(s: &Scene, undo: usize, redo: usize) -> Value {
    json!({"mesh":mesh(&s.mesh),"groups":s.groups.iter().map(|g|json!({"mesh":mesh(&g.mesh),"transform":if g.transform==glam::DMat4::IDENTITY{Value::Null}else{json!(g.transform.to_cols_array())},"axes":g.native_document.get("axes").cloned().unwrap_or(Value::Null)})).collect::<Vec<_>>(),"images":[],"undo":undo,"redo":redo})
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let input: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut out = vec![];
    for c in input["cases"].as_array().unwrap() {
        let kind = c["kind"].as_str().unwrap();
        let mut s = Scene::new();
        let mut m = Mesh::new();
        let vs = [[0., 0., 0.], [2., 0., 0.], [2., 3., 0.], [0., 3., 0.]]
            .map(|p| m.add_vertex(DVec3::from_array(p)));
        let id = m.add_face(vs.to_vec(), vec![]).unwrap();
        m.faces[id].attrs = serde_json::from_value(
            json!({"vendor":71,"texture":{"path":"scan","uvw":[0.25,0,0,0.1,0,0.5,0,0.2]}}),
        )
        .unwrap();
        s.groups.push(Group::with_mesh("selected", m.clone()));
        if kind.starts_with("shared") {
            let token = uuid::Uuid::new_v4();
            s.register_mesh_share(token, m.clone()).unwrap();
            s.groups[0].mesh_share = Some(token);
            let mut sibling = Group::with_mesh("sibling", m.clone());
            sibling.mesh_share = Some(token);
            s.groups.push(sibling);
            s.groups.push(Group::with_mesh("independent equal", m));
        }
        s.select(SelectionItem::Group(0));
        if kind == "shared-two" {
            s.selection.insert(SelectionItem::Group(1));
        }
        let original = s.clone();
        let mut t = targets::Targets::capture(&s);
        let mut states = serde_json::Map::new();
        states.insert("before".into(), snap(&s, 0, 0));
        states.insert("grab".into(), snap(&s, 0, 0));
        let matrix = |anchor, f| ScaleMatrix::new(anchor, DVec3::new(f, f, 1.), None);
        t.apply(&mut s, Some(&matrix(DVec3::ZERO, 1.75)), false);
        states.insert("preview".into(), snap(&s, 0, 0));
        let center = DVec3::new(1., 1.5, 0.);
        t.apply(&mut s, Some(&matrix(center, 1.75)), false);
        states.insert("center".into(), snap(&s, 0, 0));
        t.apply(&mut s, None, false);
        t.apply(&mut s, Some(&matrix(center, 2.)), true);
        states.insert("commit".into(), snap(&s, 1, 0));
        t.apply(&mut s, None, true);
        t.apply(&mut s, Some(&matrix(center, -1.)), true);
        states.insert("retype".into(), snap(&s, 1, 0));
        let accepted = s.clone();
        s = original.clone();
        states.insert("undo".into(), snap(&s, 0, 1));
        s = accepted;
        states.insert("redo".into(), snap(&s, 1, 0));
        let mut cancel = targets::Targets::capture(&s);
        let bounds = targets::selection_box(&s).unwrap();
        let center = bounds.to_world((bounds.low + bounds.high) * 0.5);
        cancel.apply(&mut s, Some(&matrix(center, 1.5)), false);
        states.insert("cancel-preview".into(), snap(&s, 1, 0));
        cancel.apply(&mut s, None, false);
        states.insert("cancel".into(), snap(&s, 1, 0));
        let registry_consistent = s.groups[0]
            .mesh_share
            .is_none_or(|token| s.mesh_shares.get(&token) == Some(&s.groups[0].mesh));
        out.push(json!({"kind":kind,"states":states,"registry_consistent":registry_consistent}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":out})).unwrap(),
    )
    .unwrap();
}
