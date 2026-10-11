use std::io::{self, Read};
fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&s).unwrap();
    let mut result = Vec::new();
    for row in rows["cases"].as_array().unwrap() {
        let p: ingetrazo_formats::text3d::Params =
            serde_json::from_value(row["params"].clone()).unwrap();
        match ingetrazo_formats::text3d::make_text_group(&p) {
            Ok(g) => {
                let meshes:Vec<_>=g.children.iter().map(|c|serde_json::json!({"char":c.name,"vertices":c.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"faces":c.mesh.faces.iter().map(|f|serde_json::json!({"outer":f.vertices,"holes":f.holes})).collect::<Vec<_>>(),"edges":c.mesh.edges.iter().map(|e|serde_json::json!({"vertices":[e.v0,e.v1],"soft":e.soft})).collect::<Vec<_>>() })).collect();
                result.push(serde_json::json!({"name":row["name"],"meshes":meshes,"state":g.native_document}));
            }
            Err(e) => result.push(serde_json::json!({"name":row["name"],"error":e})),
        }
    }
    println!("{}", serde_json::to_string(&result).unwrap());
}
