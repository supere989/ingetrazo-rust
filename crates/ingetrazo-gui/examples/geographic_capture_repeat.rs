use ingetrazo_gui::IngeTrazoApp;
fn main(){let mut a=IngeTrazoApp::new();let files:Vec<_>=std::env::args().skip(1).collect();a.import_geographic_paths(std::path::Path::new(&files[0])).unwrap();let first=a.scene.tile_layer.clone();a.undo();a.import_geographic_paths(std::path::Path::new(&files[1])).unwrap();println!("{}",serde_json::json!({"first":first,"second":a.scene.tile_layer}));}
