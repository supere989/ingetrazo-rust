// Actual production composer selected PDF exports; source helper contexts are independently frozen.
use ingetrazo_core::composition::{
    Orientation, PaperSize, Sheet, SheetItem, TextItem, TitleBlock, ViewFrame,
};
use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let mut app = IngeTrazoApp::new();
    let mut rows = vec![];
    for case in source["pdf_cases"].as_array().unwrap() {
        let r = &case["recipe"];
        let index = case["index"].as_u64().unwrap() as usize;
        let total = case["total"].as_u64().unwrap() as usize;
        app.scene.sheets = (0..total)
            .map(|_| {
                let mut s = Sheet::new("Other", PaperSize::A4, Orientation::Portrait);
                s.items.clear();
                s
            })
            .collect();
        let sheet = &mut app.scene.sheets[index];
        sheet.name = r["sheet"].as_str().unwrap().into();
        if let Some(title) = r.get("title") {
            let mut t = TitleBlock::default();
            t.project_name = title["proyecto"].as_str().unwrap_or("").into();
            t.author = title["autor"].as_str().unwrap_or("").into();
            t.sheet_number = title["lamina"].as_str().unwrap_or("L-01").into();
            sheet
                .native_items
                .insert(t.id, serde_json::json!({"hidden":true}));
            sheet.items.push(SheetItem::TitleBlock(t));
        }
        if let Some(frames) = r["frames"].as_array() {
            for f in frames {
                let mut frame = ViewFrame::default();
                frame.rect.w = f["w_mm"].as_f64().unwrap();
                frame.rect.h = f["h_mm"].as_f64().unwrap();
                frame.scale_n = f["scale_n"].as_f64().unwrap();
                frame.view_key = f["view_key"].as_str().unwrap().into();
                sheet
                    .native_items
                    .insert(frame.id, serde_json::json!({"uid":f["uid"],"hidden":true}));
                sheet.items.push(SheetItem::ViewFrame(frame));
            }
        }
        let item = TextItem {
            id: uuid::Uuid::new_v4(),
            text: case["text"].as_str().unwrap().into(),
            pos: (20., 30.),
            font_size_pt: 14.,
            bold: false,
        };
        sheet
            .native_items
            .insert(item.id, serde_json::json!({"frame_uid":r["frame_uid"]}));
        sheet.items.push(SheetItem::Text(item));
        app.current_file = r["path"].as_str().map(std::path::PathBuf::from);
        let before = app.scene.clone();
        let path = out.join(format!("{}.native.pdf", r["name"].as_str().unwrap()));
        assert_eq!(app.export_composer_pdf(Some(index), &path).unwrap(), 1);
        assert_eq!(before, app.scene);
        let bytes = std::fs::read(&path).unwrap();
        let resources = app.composer_state.paper_text.borrow().stats();
        app.export_composer_pdf(Some(index), &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(resources, app.composer_state.paper_text.borrow().stats());
        assert_eq!(before, app.scene);
        rows.push(serde_json::json!({"case":r["name"],"warm_resources_unchanged":true,"authored_scene_unchanged":true}));
    }
    if let Some(protocol) = args.get(3) {
        let callers: serde_json::Value =
            serde_json::from_slice(&std::fs::read(protocol).unwrap()).unwrap();
        let caller_out = out.join("caller");
        std::fs::create_dir_all(&caller_out).unwrap();
        for case in callers["cases"].as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            app.scene.sheets.clear();
            app.current_file = None;
            for i in 0..2 {
                let mut sheet = Sheet::new("Same", PaperSize::A4, Orientation::Portrait);
                sheet.items.clear();
                sheet.items.push(SheetItem::Text(TextItem {
                    id: uuid::Uuid::new_v4(),
                    pos: (
                        if i == 1 && name == "different_text_position" {
                            21.
                        } else {
                            20.
                        },
                        20.,
                    ),
                    text: "{hoja}/{total}".into(),
                    font_size_pt: 14.,
                    bold: false,
                }));
                if name == "large_metadata_integer" {
                    let id = sheet.items[0].id();
                    sheet
                        .native_items
                        .insert(id, serde_json::json!({"z":9007199254740992u64+i as u64}));
                }
                if name.ends_with("frame_uid") {
                    let frame = ViewFrame::default();
                    sheet.native_items.insert(frame.id,serde_json::json!({"uid":if i==0||name=="same_literal_frame_uid"{"first"}else{"second"},"hidden":true}));
                    sheet.items.push(SheetItem::ViewFrame(frame));
                }
                app.scene.sheets.push(sheet);
            }
            let before = app.scene.clone();
            let path = caller_out.join(format!("{name}.native.pdf"));
            app.export_composer_pdf(Some(1), &path).unwrap();
            assert_eq!(app.scene, before);
            let bytes = std::fs::read(&path).unwrap();
            let stats = app.composer_state.paper_text.borrow().stats();
            app.export_composer_pdf(Some(1), &path).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(stats, app.composer_state.paper_text.borrow().stats());
            assert_eq!(app.scene, before);
            rows.push(serde_json::json!({"case":name,"actual_caller_protocol":case,"warm_resources_unchanged":true,"authored_scene_unchanged":true}));
        }
    }
    std::fs::write(out.join("controller.json"),serde_json::to_vec_pretty(&serde_json::json!({"cases":rows,"scope":"Actual selected composer export controller; date/path/index/total live; not real save-dialog gesture"})).unwrap()).unwrap();
}
