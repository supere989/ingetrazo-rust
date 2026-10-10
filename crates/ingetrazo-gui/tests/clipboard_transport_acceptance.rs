use glam::DVec3;use ingetrazo_gui::IngeTrazoApp;use ingetrazo_core::scene::SelectionItem;
#[test]
#[ignore="requires disposable X11 clipboard server"]
fn native_clipboard_transport_rejects_foreign_text_and_malformed_payload_without_stale_cache(){
 let mut owner=IngeTrazoApp::new();owner.system_clipboard=Some(arboard::Clipboard::new().unwrap());let a=owner.scene.mesh.add_vertex(DVec3::new(2.,3.,4.));let b=owner.scene.mesh.add_vertex(DVec3::new(5.,6.,7.));owner.scene.mesh.add_edge(a,b);owner.scene.select(SelectionItem::Edge(0));owner.copy_selection();
 let mut receiver=IngeTrazoApp::new();receiver.system_clipboard=Some(arboard::Clipboard::new().unwrap());receiver.paste_in_place();assert_eq!(receiver.scene.mesh.edges.len(),1);assert_eq!(receiver.scene.mesh.vertices[0].position,DVec3::new(2.,3.,4.));let before=receiver.scene.clone();let depth=receiver.undo_stack.len();
 let mut external=arboard::Clipboard::new().unwrap();
 for invalid in ["unrelated text".to_owned(),"IngeTrazo-Rust-geometry-v1\n{broken".to_owned()] {
  external.set_text(invalid).unwrap();receiver.paste_in_place();assert_eq!(receiver.scene,before);assert_eq!(receiver.undo_stack.len(),depth);assert!(receiver.clipboard.is_none(),"foreign payload must clear cached geometry");receiver.paste_selection();assert!(receiver.paste_preview.is_none());
 }
 let mut malformed=owner.clipboard.clone().unwrap();malformed.mesh.edges[0].v1=999;external.set_text(format!("IngeTrazo-Rust-geometry-v1\n{}",serde_json::to_string(&malformed).unwrap())).unwrap();receiver.paste_in_place();assert_eq!(receiver.scene,before);assert_eq!(receiver.undo_stack.len(),depth);assert!(receiver.clipboard.is_none());
 owner.copy_selection();receiver.paste_selection();assert!(receiver.paste_preview.is_some());external.set_text("IngeTrazo-Rust-geometry-v1\n{broken").unwrap();receiver.paste_selection();assert!(receiver.paste_preview.is_none(),"new malformed request must retire previous valid preview");
}
