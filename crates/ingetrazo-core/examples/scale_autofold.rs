// SPDX-License-Identifier: GPL-3.0-or-later
use glam::DVec3;
use ingetrazo_core::{autofold, mesh::Mesh};
use serde_json::{json, Value};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let input: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for c in input["cases"].as_array().unwrap() {
        let mut m = Mesh::new();
        for f in c["source"].as_array().unwrap() {
            let mut lp = |v: &Value| {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        let q = p.as_array().unwrap();
                        m.add_vertex(DVec3::new(
                            q[0].as_f64().unwrap(),
                            q[1].as_f64().unwrap(),
                            q[2].as_f64().unwrap(),
                        ))
                    })
                    .collect()
            };
            let outer = lp(&f["vertices"]);
            let holes = f["holes"].as_array().unwrap().iter().map(lp).collect();
            let id = m.add_face(outer, holes).unwrap();
            m.faces[id].attrs = serde_json::from_value(f["attrs"].clone()).unwrap();
        }
        let old = m.clone();
        let need = autofold::needs_fold(&m);
        let report = autofold::fold_nonplanar_faces(&mut m).unwrap();
        let result=m.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":f.attrs,"interior":f.interior})).collect::<Vec<_>>();
        rows.push(json!({"name":c["name"],"needed":need,"folded":report.folded_history_ids.len(),"result":result,"untouched_identity":m.faces.iter().any(|f|f.history_id==old.faces[1].history_id),"original_edges_survive":old.edges.iter().all(|e|m.edges.iter().any(|v|v.history_id==e.history_id))}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
