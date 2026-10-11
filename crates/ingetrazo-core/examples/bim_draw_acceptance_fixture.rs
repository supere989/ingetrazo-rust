use glam::DVec3;
use ingetrazo_core::{
    drafting::{add_segments, add_segments_tagged},
    mesh::Mesh,
};
use serde_json::{json, Value};
fn p(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn main() {
    let a: Vec<_> = std::env::args().collect();
    let cases: Value = serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mut out = vec![];
    for c in cases.as_array().unwrap() {
        let mut m = Mesh::new();
        for f in c["faces"].as_array().unwrap() {
            let ids = f["vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| m.add_vertex(p(v)))
                .collect();
            let holes = f["holes"]
                .as_array()
                .map(|hs| {
                    hs.iter()
                        .map(|h| {
                            h.as_array()
                                .unwrap()
                                .iter()
                                .map(|v| m.add_vertex(p(v)))
                                .collect()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let i = m.add_face(ids, holes).unwrap();
            m.faces[i].attrs = serde_json::from_value(f["attrs"].clone()).unwrap();
        }
        for e in c["edges"].as_array().unwrap() {
            let a = m.add_vertex(p(&e["points"][0]));
            let b = m.add_vertex(p(&e["points"][1]));
            let i = m.add_edge(a, b).unwrap();
            m.edges[i].curve_id = e["curve"].as_u64().map(|v| v as usize);
        }
        for op in c["ops"].as_array().unwrap() {
            let seg: Vec<_> = op["segments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| (p(&e[0]), p(&e[1])))
                .collect();
            let explicit = op["explicit"]
                .as_array()
                .map(|r| r.iter().map(p).collect::<Vec<_>>());
            let active = c.get("active");
            if explicit.is_none() && op["curve"].is_null() {
                for e in &seg {
                    m = add_segments_tagged(&m, &[*e], None, None, active).unwrap();
                }
            } else {
                m = add_segments_tagged(
                    &m,
                    &seg,
                    explicit.as_deref(),
                    op["curve"].as_u64().map(|n| n as usize),
                    active,
                )
                .unwrap();
            }
        }
        assert!(add_segments(&m, &[(DVec3::splat(f64::NAN), DVec3::X)], None, None).is_err());
        out.push(json!({"faces":m.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":f.attrs})).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|json!({"points":[m.vertices[e.v0].position.to_array(),m.vertices[e.v1].position.to_array()],"curve":e.curve_id})).collect::<Vec<_>>()}));
    }
    std::fs::write(&a[2], serde_json::to_vec(&out).unwrap()).unwrap();
}
