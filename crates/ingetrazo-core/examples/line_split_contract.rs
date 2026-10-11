use glam::DVec3;
use ingetrazo_core::{drafting, mesh::Mesh};
use serde_json::{json, Value};
fn point(p: &Value) -> DVec3 {
    DVec3::new(
        p[0].as_f64().unwrap(),
        p[1].as_f64().unwrap(),
        p[2].as_f64().unwrap(),
    )
}
fn state(m: &Mesh, old: &[u64]) -> Value {
    json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|json!({"vertices":[e.v0,e.v1],"soft":e.soft,"hidden":e.hidden,"layer":e.layer,"curve":e.curve_id,"old_identity":old.iter().position(|&id|id==e.history_id)})).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|&f.vertices).collect::<Vec<_>>()})
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for case in source["cases"].as_array().unwrap() {
        let mut m = Mesh::new();
        for p in case["before"]["vertices"].as_array().unwrap() {
            m.add_vertex(point(p));
        }
        for e in case["before"]["edges"].as_array().unwrap() {
            let id = m
                .add_edge(
                    e["vertices"][0].as_u64().unwrap() as usize,
                    e["vertices"][1].as_u64().unwrap() as usize,
                )
                .unwrap();
            m.edges[id].soft = e["soft"].as_bool().unwrap();
            m.edges[id].hidden = e["hidden"].as_bool().unwrap();
            m.edges[id].layer = e["layer"].as_str().map(str::to_owned);
            m.edges[id].curve_id = e["curve"].as_u64().map(|x| x as usize);
        }
        let old = m.edges.iter().map(|e| e.history_id).collect::<Vec<_>>();
        let before = state(&m, &old);
        let after =
            drafting::add_segments(&m, &[(point(&case["a"]), point(&case["b"]))], None, None)
                .unwrap();
        rows.push(json!({"case":case["case"],"before":before,"after":state(&after,&old)}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
