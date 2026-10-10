use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_core::composition::{Sheet,PaperSize,Orientation};
#[test]
fn current_page_export_and_atlas_refusal_preserve_existing_destination(){
 let mut app=IngeTrazoApp::new();
 let mut supported=Sheet::new("empty",PaperSize::A4,Orientation::Portrait);supported.items.clear();
 app.scene.sheets=vec![supported,Sheet::new("unsupported frame",PaperSize::A3,Orientation::Landscape)];
 let path=std::env::temp_dir().join(format!("composer-pdf-{}.pdf",uuid::Uuid::new_v4()));
 assert_eq!(app.export_composer_pdf(Some(0),&path).unwrap(),1);
 let old=std::fs::read(&path).unwrap();assert!(old.starts_with(b"%PDF-"));
 assert!(app.export_composer_pdf(None,&path).is_err());assert_eq!(std::fs::read(&path).unwrap(),old);
 assert!(app.export_composer_pdf(Some(999),&path).is_err());assert_eq!(std::fs::read(&path).unwrap(),old);
 std::fs::remove_file(path).unwrap();
}
