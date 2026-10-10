use glam::DVec3;
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_core::scene::SelectionItem;
fn main(){
 let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());let mut app=IngeTrazoApp::new();app.scene=ingetrazo_formats::igz::load_igz(&dir.join("source.igz")).unwrap();
 let edge=app.scene.mesh.edges.iter().position(|e|e.curve_id==Some(29)).unwrap();app.scene.select(SelectionItem::Face(0));app.scene.select(SelectionItem::Edge(edge));app.copy_selection();let before=app.scene.clone();let depth=app.undo_stack.len();
 app.paste_selection();app.update_paste_preview(DVec3::new(20.,30.,40.));assert_eq!(app.scene,before);assert_eq!(app.undo_stack.len(),depth);app.cancel_paste_preview();assert_eq!(app.scene,before);
 app.paste_selection();app.update_paste_preview(DVec3::new(20.,30.,40.));assert!(app.commit_paste_preview());assert_eq!(app.undo_stack.len(),depth+1);ingetrazo_formats::igz::save_igz(&app.scene,&dir.join("pasted.igz")).unwrap();
 app.undo();assert_eq!(app.scene,before);app.redo();assert_eq!(app.scene.mesh.faces.len(),2);
}
