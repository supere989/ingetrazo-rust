// Production composer export controller. Actual XTEST dialog acceptance is a separate required stage.
use ingetrazo_core::composition::{Orientation, PaperSize, Sheet, SheetItem, TextItem};
use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let mut app = IngeTrazoApp::new();
    let mut rows = vec![];
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let call = &case["source_draw_calls"][0];
        let props = &case["properties"];
        let mut sheet = Sheet::new(name, PaperSize::A4, Orientation::Portrait);
        sheet.items.clear();
        let item = TextItem {
            id: uuid::Uuid::new_v4(),
            pos: (20., 30.),
            text: call["text"].as_str().unwrap().into(),
            font_size_pt: 14.,
            bold: call["font"]["bold"].as_bool().unwrap(),
        };
        sheet.native_items.insert(item.id, props.clone());
        sheet.items.push(SheetItem::Text(item));
        app.scene.sheets = vec![sheet];
        let before = app.scene.clone();
        let path = out.join(format!("{name}.native.pdf"));
        assert_eq!(app.export_composer_pdf(Some(0), &path).unwrap(), 1);
        assert_eq!(app.scene, before);
        let cache = app.composer_state.paper_text.borrow().stats();
        let bytes = std::fs::read(&path).unwrap();
        app.export_composer_pdf(Some(0), &path).unwrap();
        assert_eq!(bytes, std::fs::read(&path).unwrap());
        assert_eq!(cache, app.composer_state.paper_text.borrow().stats());
        rows.push(serde_json::json!({"case":name,"scene_unchanged":true,"warm_bytes_exact":true,"warm_resources_unchanged":true}));
    }
    let path = out.join("preserved.pdf");
    std::fs::write(&path, b"preserved").unwrap();
    if let SheetItem::Text(item) = &mut app.scene.sheets[0].items[0] {
        item.pos.0 = f64::NAN;
    }
    assert!(app.export_composer_pdf(Some(0), &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"preserved");
    std::fs::write(out.join("controller.json"),serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"cases":rows,"invalid_style_preserves_destination":true,"scope":"Production composer export controller, not OS dialog or rendered pointer proof"})).unwrap()).unwrap();
}
