//! Actual IGZ adapters; helpers only select files and report results.
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let root=std::path::Path::new(&args[1]);
    let input: serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("cases.json")).unwrap()).unwrap();
    let rows:Vec<_>=input["cases"].as_array().unwrap().iter().enumerate().map(|(i,_)|{
        let path=root.join(format!("case-{i}.igz"));
        match ingetrazo_formats::igz::load_igz(&path) {
            Ok(scene)=>{
                ingetrazo_formats::igz::save_igz(&scene,&root.join(format!("native-{i}.igz"))).unwrap();
                serde_json::json!({"case":i,"style":scene.display_style,"shadows":scene.shadows,"error":null})
            }
            Err(ingetrazo_formats::igz::IgzError::InvalidDocument(e))=>serde_json::json!({"case":i,"error":e}),
            Err(e)=>panic!("Unexpected format refusal: {e}"),
        }
    }).collect();
    println!("{}",serde_json::json!({"cases":rows}));
}
