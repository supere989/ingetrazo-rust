use ingetrazo_core::{camera::OrbitCamera,composition::{ViewFrame,SheetItem}};
use ingetrazo_gui::{app::IngeTrazoApp,composer_compute::ComposerCompute};
fn main(){
 let root=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());let worker=std::path::PathBuf::from(std::env::args_os().nth(2).unwrap());let mut rows=vec![];
 for entry in std::fs::read_dir(&root).unwrap(){let path=entry.unwrap().path();if !path.to_string_lossy().ends_with(".reference.json"){continue;}
 let reference:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();let name=reference["name"].as_str().unwrap();
 let mut app=IngeTrazoApp::new();app.composer_compute=ComposerCompute::with_worker(&worker);app.scene=ingetrazo_formats::load_igz(&root.join(format!("{name}.igz"))).unwrap();app.scene.camera=serde_json::from_value::<OrbitCamera>(reference["camera"].clone()).unwrap();app.camera=app.scene.camera.clone();app.add_saved_view("Export".into());app.scene.sheets.push(Default::default());app.scene.sheets[0].items.clear();
 let mut frame=ViewFrame::default();frame.view_key="scene:Export".into();frame.title_text="Custom title must not name DXF layers".into();frame.scale_n=100.;frame.rect.w=100.;frame.rect.h=100.;frame.profiles=true;
 for hidden in [false,true]{frame.hidden_lines=hidden;app.scene.sheets[0].items=vec![SheetItem::ViewFrame(frame.clone())];app.scene.sheets[0].selected_item=Some(0);let before=app.scene.clone();let dest=root.join(format!("{name}.{hidden}.native.dxf"));let count=app.export_composer_view_dxf(0,&dest).unwrap();assert_eq!(before,app.scene);let pids=app.composer_compute.worker_ids();assert_eq!(pids.len(),2);let jobs=app.composer_compute.completed_jobs;assert_eq!(count,app.export_composer_view_dxf(0,&dest).unwrap());assert_eq!(jobs,app.composer_compute.completed_jobs);assert_eq!(pids,app.composer_compute.worker_ids());
 app.scene.sheets[0].selected_item=None;let bytes=std::fs::read(&dest).unwrap();assert!(app.export_composer_view_dxf(0,&dest).is_err());assert_eq!(bytes,std::fs::read(&dest).unwrap());app.scene.sheets[0].selected_item=Some(0);
 let directory=root.join("invalid-directory-target");std::fs::create_dir_all(&directory).unwrap();assert!(app.export_composer_view_dxf(0,&directory).is_err());assert!(directory.is_dir());assert_eq!(before,app.scene);
 rows.push(serde_json::json!({"name":name,"hidden":hidden,"count":count,"worker_pids":pids,"warm_cache_no_jobs":true,"scene_unchanged":true,"invalid_selection_preserves_destination":true,"directory_io_failure_preserves_document":true}));
 }
 }
 assert_eq!(rows.len(),26);std::fs::write(root.join("native-export-controls.json"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}
