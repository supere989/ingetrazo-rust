use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_core::{scene::SelectionItem,groups::Group};
fn main(){let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());let mut app=IngeTrazoApp::new();app.scene=ingetrazo_formats::igz::load_igz(&dir.join("component-source.igz")).unwrap();
 let mut parent=Group::new("parent");parent.children.push(app.scene.groups[0].duplicate_instance());let parentid=parent.id;app.scene.groups.push(parent);app.scene.make_component(parentid).unwrap();let copy=app.scene.groups[3].duplicate_instance();app.scene.groups.push(copy);
 app.scene.clear_selection();app.scene.select(SelectionItem::Group(3));let before=app.scene.clone();
 app.begin_component_edit();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();app.scene.mesh.vertices[0].position.z=5.;app.finish_component_edit();app.finish_component_edit();
 assert_eq!(app.undo_stack.len(),1);let committed=app.scene.clone();app.save_to_path(&dir.join("editor-committed.igz"));
 app.undo();assert_eq!(app.scene,before);app.save_to_path(&dir.join("editor-undone.igz"));app.redo();assert_eq!(app.scene,committed);
 app.scene.clear_selection();app.scene.select(SelectionItem::Group(0));let cancelbefore=app.scene.clone();app.begin_component_edit();app.scene.mesh.vertices[0].position.z=9.;let blocked=dir.join("fragment.igz");app.save_to_path(&blocked);assert!(!blocked.exists());app.cancel_component_edit();assert_eq!(app.scene,cancelbefore);
 app.make_component_unique();app.begin_component_edit();app.scene.mesh.vertices[0].position.z=7.;app.finish_component_edit();app.save_to_path(&dir.join("editor-unique.igz"));
 app.scene.clear_selection();app.scene.select(SelectionItem::Group(0));app.copy_selection();app.paste_in_place();app.save_to_path(&dir.join("editor-pasted.igz"));
}
