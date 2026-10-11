use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_core::scene::SelectionItem;
use std::path::PathBuf;
fn main(){let root=PathBuf::from(std::env::args().nth(1).unwrap());let mut rows=Vec::new();
 for fixture in ["retained","nested-leaves"] {
  let mut app=IngeTrazoApp::new();app.system_clipboard=None;app.open_document(&root.join(format!("{fixture}.igz")));assert_eq!(app.scene.groups.len(),2);assert!(!app.document_dirty());
  app.scene.select(SelectionItem::Group(1));app.delete_selection();assert_eq!(app.scene.groups.len(),1);assert!(app.document_dirty());assert_eq!(app.undo_stack.len(),1);
  for phase in ["edited","undo","redo"] {
   if phase=="undo"{app.undo();assert_eq!(app.scene.groups.len(),2);}if phase=="redo"{app.redo();assert_eq!(app.scene.groups.len(),1);}
   let path=root.join(format!("{fixture}-{phase}.native.igz"));app.save_to_path(&path);assert!(path.is_file());assert!(!app.document_dirty(),"{}",app.status_message);
   rows.push(serde_json::json!({"fixture":fixture,"phase":phase,"groups":app.scene.groups.len(),"saved":true,"clean":true}));
  }
 }
 println!("{}",serde_json::json!({"rows":rows,"scope":"actual app Open/Delete/Undo/Redo/Save entrypoints, no desktop input claim"}));
}
