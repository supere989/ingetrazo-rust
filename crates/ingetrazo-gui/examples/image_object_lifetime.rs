use ingetrazo_core::image_plane::ImagePlane;
use ingetrazo_gui::IngeTrazoApp;
use glam::DVec3;
fn state(a:&IngeTrazoApp,id:uuid::Uuid)->serde_json::Value{serde_json::json!({"old_alive":a.scene.image_objects.0.lock().unwrap().contains_key(&id),"live":a.scene.image_planes.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})}
fn main(){let mut a=IngeTrazoApp::new();a.system_clipboard=None;let p=ImagePlane::new("old",DVec3::ZERO,DVec3::X,DVec3::Y);let id=p.id;a.add_image_plane(p);let mut rows=vec![state(&a,id)];a.undo();rows.push(state(&a,id));a.add_image_plane(ImagePlane::new("new",DVec3::ZERO,DVec3::X,DVec3::Y));rows.push(state(&a,id));println!("{}",serde_json::to_string_pretty(&rows).unwrap());}
