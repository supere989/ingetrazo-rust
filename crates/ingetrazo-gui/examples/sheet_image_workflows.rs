//! Actual App request preparation and native GPU physical sheet image output.
use ingetrazo_core::composition::{
    Orientation, PaperSize, ShapeItem, Sheet, SheetItem, SourcePaperBorder, TextItem,
};
use ingetrazo_formats::sheet_image::Format;
use std::path::Path;
fn sheet(row: &serde_json::Value) -> Sheet {
    let paper = if row["paper"] == "A3" {
        PaperSize::A3
    } else {
        PaperSize::A4
    };
    let orientation = if row["landscape"].as_bool().unwrap() {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    };
    let mut sheet = Sheet::new("Image", paper, orientation);
    sheet.items.clear();
    sheet.margin_mm = 10.;
    sheet.source_border = Some(SourcePaperBorder {
        on: false,
        ..Default::default()
    });
    if row["content"] == "shapes" {
        let base = ingetrazo_core::sheet_shape::SheetShape::default();
        let shapes = [
            ingetrazo_core::sheet_shape::SheetShape {
                kind: "rect".into(),
                x_mm: 20.,
                y_mm: 25.,
                w_mm: 45.,
                h_mm: 30.,
                fill: true,
                fill_color: "#80dd6432".into(),
                radius_mm: 5.,
                z: 2.,
                ..base.clone()
            },
            ingetrazo_core::sheet_shape::SheetShape {
                kind: "elipse".into(),
                x_mm: 40.,
                y_mm: 30.,
                w_mm: 60.,
                h_mm: 45.,
                fill: true,
                fill_color: "#40a0d060".into(),
                z: 1.,
                ..base.clone()
            },
            ingetrazo_core::sheet_shape::SheetShape {
                kind: "linea".into(),
                x_mm: 10.,
                y_mm: 70.,
                w_mm: 80.,
                h_mm: 25.,
                color: "#255d9d".into(),
                stroke_mm: 0.6,
                z: 3.,
                ..base.clone()
            },
            ingetrazo_core::sheet_shape::SheetShape {
                x_mm: 1.,
                y_mm: 1.,
                w_mm: 200.,
                h_mm: 200.,
                fill: true,
                fill_color: "#ff0000".into(),
                hidden: true,
                z: 10.,
                ..base
            },
        ];
        sheet.items.extend(shapes.into_iter().map(|shape| {
            SheetItem::Shape(ShapeItem {
                id: uuid::Uuid::new_v4(),
                shape,
            })
        }));
        sheet.source_border = Some(SourcePaperBorder {
            on: true,
            mm: 0.5,
            color: "#80255d9d".into(),
            radius_mm: 4.,
            style: "double".into(),
        });
    }
    if row["content"] == "text" {
        let id = uuid::Uuid::new_v4();
        sheet.items.push(SheetItem::Text(TextItem {
            id,
            pos: (20., 30.),
            text: "IngeTrazo\nPhysical sheet 1:100".into(),
            font_size_pt: 14.,
            bold: false,
        }));
        sheet.native_items.insert(
            id,
            serde_json::json!({"w_mm":100.,"family":"Sans Serif","color":"#1e242c","align":"left"}),
        );
    }
    sheet
}
#[cfg(any(target_os = "linux", target_os = "windows"))]
struct Renderer {
    exporter: Option<ingetrazo_gui::native_vulkan::sheet_image::Exporter>,
    budget: std::sync::Arc<ingetrazo_gui::native_vulkan::budget::VulkanBudget>,
    context: ingetrazo_gui::native_vulkan::VulkanContext,
}
#[cfg(any(target_os = "linux", target_os = "windows"))]
impl Renderer {
    fn new(software: bool) -> Result<Self, Box<dyn std::error::Error>> {
        use ingetrazo_gui::native_vulkan::{
            budget::VulkanBudget, headless::DeviceRequest, sheet_image::Exporter, VulkanContext,
        };
        let request = if software {
            DeviceRequest::software_fixture()
        } else {
            DeviceRequest::nvidia_hardware()
        };
        let context = VulkanContext::new_headless(&request)?;
        let budget = std::sync::Arc::new(VulkanBudget::new(
            &context.instance,
            context.physical_device,
        )?);
        let exporter = Some(Exporter::new(&context, budget.clone())?);
        Ok(Self {
            context,
            exporter,
            budget,
        })
    }
    fn save(&mut self, plan: &ingetrazo_gui::sheet_image::Plan, path: &Path) -> Result<(), String> {
        self.exporter
            .as_mut()
            .unwrap()
            .save(&self.context, plan, path)
    }
    fn cleanup(&mut self) -> Result<(), String> {
        self.exporter.take();
        if self.budget.ledger.lock().unwrap().total_committed() != 0 {
            return Err("Paper GPU ledger did not drain".into());
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
struct Renderer(ingetrazo_gui::sheet_image::MetalExporter);
#[cfg(target_os = "macos")]
impl Renderer {
    fn new(_: bool) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self(ingetrazo_gui::sheet_image::MetalExporter::new(
            std::sync::Arc::new(ingetrazo_metal::NativeMetal::new()?),
        )?))
    }
    fn save(&mut self, plan: &ingetrazo_gui::sheet_image::Plan, path: &Path) -> Result<(), String> {
        self.0.save(plan, path)
    }
    fn cleanup(&mut self) -> Result<(), String> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    // macOS geometry/codec diagnostic: Linux Sans Serif is not a Mac font oracle.
    // Default acceptance still requires every image, including text.
    let skip_text = args.iter().any(|v| v == "--skip-text");
    let source = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(source.join("source.json"))?)?;
    let mut inputs = vec![(source.to_path_buf(), original)];
    if let Some(i) = args.iter().position(|v| v == "--views") {
        let views = std::path::PathBuf::from(args.get(i + 1).ok_or("Missing views directory")?);
        let records = serde_json::from_slice(&std::fs::read(views.join("source.json"))?)?;
        inputs.push((views, records));
    }
    let mut renderer = Renderer::new(args.iter().any(|v| v == "--software"))?;
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |_| {});
    let mut app = ingetrazo_gui::IngeTrazoApp::new();
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &out.join("session"),
    ));
    if let Some(i) = args.iter().position(|v| v == "--worker") {
        app.composer_compute = ingetrazo_gui::composer_compute::ComposerCompute::with_worker(
            Path::new(args.get(i + 1).ok_or("Missing worker")?),
        );
    }
    let mut results = Vec::new();
    for (input, original) in &inputs {
        for row in original["images"].as_array().ok_or("images")? {
            if skip_text && row["case"] == "text" { continue; }
            if let Some(scene) = row["scene"].as_str() {
                app.mark_document_clean();
                app.open_document(&input.join(scene));
                assert!(!app.document_session.confirm_unsaved,"Fixture Open was deferred");
                assert!(app.scene.sheets.first().is_some_and(|s|s.items.iter().any(|i|matches!(i,SheetItem::ViewFrame(_)))));
                app.camera = serde_json::from_value(row["camera"].clone())?;
                app.scene.camera = app.camera.clone();
            } else {
                app.scene.sheets = vec![sheet(row)];
            }
            let file = row["image"]["file"].as_str().ok_or("file")?;
            let path = out.join(file);
            let dpi = row["dpi"].as_u64().ok_or("dpi")? as u16;
            app.begin_sheet_image_export(0);
            app.choose_sheet_image_path(0, Some(&path), Format::from_path(&path).unwrap());
            app.choose_sheet_image_resolution(0, path.clone(), Some(dpi))?;
            let (request, plan) = app
                .prepare_sheet_image_request(&ctx)
                .ok_or("Missing App export request")??;
            renderer.save(&plan, &request.path)?;
            let decoded = image::open(&path)?.to_rgba8();
            std::fs::write(
                out.join(format!("{}.rgba", row["case"].as_str().unwrap())),
                decoded.as_raw(),
            )?;
            results.push(serde_json::json!({"case":row["case"],"file":file,"metrics":plan.metrics,"pixels":[decoded.width(),decoded.height()],"dpi_remembered":app.document_session.preferences.extra["composer/image_dpi"]}));
        }
    }
    app.document_session
        .preferences
        .extra
        .remove("composer/image_dpi");
    app.persist_preferences();
    let mut controls = Vec::new();
    for (name, path, format, dpi, accept) in [
        ("path-cancel", "", Format::Png, 200, true),
        ("resolution-cancel", "cancel.png", Format::Png, 100, false),
        ("png-extension", "chosen", Format::Png, 90, true),
        ("jpg-extension", "chosen-jpeg", Format::Jpeg, 110, true),
        ("uppercase-extension", "upper.JPG", Format::Png, 120, true),
        ("failure", "missing/file.png", Format::Png, 130, true),
    ] {
        app.scene.sheets = vec![sheet(
            &serde_json::json!({"paper":"A4","landscape":true,"content":"blank"}),
        )];
        let before = app
            .document_session
            .preferences
            .extra
            .get("composer/image_dpi")
            .cloned();
        let refreshes = app.sheet_image.chooser_refreshes;
        app.begin_sheet_image_export(0);
        let mut args = Vec::new();
        let mut error = None;
        let mut images = Vec::new();
        if path.is_empty() {
            app.choose_sheet_image_path(0, None, format);
        } else {
            let raw = out.join(path);
            let (resolved, _) = ingetrazo_formats::sheet_image::resolve_path(&raw, format);
            app.choose_sheet_image_path(0, Some(&raw), format);
            args.push([
                app.sheet_image_resolution_default()
                    .ok_or("Missing DPI chooser")? as u32,
                50,
                1200,
                10,
            ]);
            app.choose_sheet_image_resolution(0, resolved.clone(), accept.then_some(dpi))?;
            if let Some(request) = app.prepare_sheet_image_request(&ctx) {
                let result =
                    request.and_then(|(request, plan)| renderer.save(&plan, &request.path));
                if result.is_err() {
                    error = Some("OSError");
                } else {
                    images.push(
                        serde_json::json!({"file":resolved.file_name().unwrap().to_string_lossy()}),
                    );
                }
            }
        }
        controls.push(serde_json::json!({"case":name,"refreshes":app.sheet_image.chooser_refreshes-refreshes,"dialog_args":args,"settings_before":before,"settings_after":app.document_session.preferences.extra.get("composer/image_dpi"),"error":error,"images":images}));
    }
    // A lazy JPEG provider failure after the first MCU band must never publish.
    let sentinel = out.join("lazy-failure.jpg");
    std::fs::write(&sentinel, b"prior destination")?;
    let metrics = ingetrazo_formats::sheet_image::Metrics::new([20., 20.], 50)?;
    let refused = ingetrazo_formats::sheet_image::save_rows(&sentinel, metrics, |y, rows| {
        if y >= 8 {
            Err("retained readback failure".into())
        } else {
            Ok(vec![255; metrics.pixels()[0] as usize * rows as usize * 4])
        }
    })
    .is_err();
    assert!(refused);
    assert_eq!(std::fs::read(&sentinel)?, b"prior destination");
    // Unsupported visible painter refuses before publication; hidden does not.
    let mut unsupported = Sheet::new("Refusal", PaperSize::A4, Orientation::Portrait);
    unsupported.items.clear();
    unsupported.native_document=serde_json::json!({"leyendas":[{"hidden":false,"title":"Unsupported legend"}]});
    app.scene.sheets = vec![unsupported];
    app.begin_sheet_image_export(0);
    app.choose_sheet_image_path(0, Some(&sentinel), Format::Jpeg);
    app.choose_sheet_image_resolution(0, sentinel.clone(), Some(100))?;
    assert!(app.prepare_sheet_image_request(&ctx).unwrap().is_err());
    assert_eq!(std::fs::read(&sentinel)?, b"prior destination");
    // A queued request is bound to the cold chooser target, not a reused index.
    app.scene.sheets = vec![sheet(
        &serde_json::json!({"paper":"A4","landscape":false,"content":"blank"}),
    )];
    app.begin_sheet_image_export(0);
    app.choose_sheet_image_path(0, Some(&out.join("stale.png")), Format::Png);
    let remembered = app.document_session.preferences.extra["composer/image_dpi"].clone();
    app.scene.sheets[0].id = uuid::Uuid::new_v4();
    assert!(app
        .choose_sheet_image_resolution(0, out.join("stale.png"), Some(50))
        .is_err());
    assert_eq!(
        app.document_session.preferences.extra["composer/image_dpi"],
        remembered
    );
    // Actual egui resolution-modal Escape must retain an underlying draft and
    // selection, and Ctrl+Z in the same packet must not leak to document history.
    let id = uuid::Uuid::new_v4();
    app.scene.sheets[0].items.push(SheetItem::Shape(ShapeItem {
        id,
        shape: Default::default(),
    }));
    app.scene.sheets[0].selected_item = Some(0);
    app.scene.active_sheet_index = Some(0);
    app.enter_composer_sheet(0);
    app.composer_state.shape_tool = Some("rect".into());
    app.composer_state.shape_start = Some(glam::DVec2::new(2., 3.));
    app.begin_sheet_image_export(0);
    app.choose_sheet_image_path(0, Some(&out.join("modal.png")), Format::Png);
    let scene = app.scene.clone();
    let undo = app.undo_stack.len();
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000., 800.),
            )),
            events: vec![
                egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                },
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        },
        |ctx| app.update_ui(ctx),
    );
    assert_eq!(app.scene.sheets, scene.sheets);
    assert_eq!(app.undo_stack.len(), undo);
    assert_eq!(
        app.composer_state.shape_start,
        Some(glam::DVec2::new(2., 3.))
    );
    assert!(app.prepare_sheet_image_request(&ctx).is_none());
    renderer.cleanup()?;
    let no_temps = std::fs::read_dir(out)?.all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".ingetrazo-image-")
    });
    assert!(no_temps);
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"text_scope":if skip_text {"excluded-platform-font-diagnostic"} else {"included"},"images":results,"controls":controls,"lazy_readback_refused":refused,"unsupported_preserved":true,"stale_target_refused":true,"modal_input_owned":true,"no_temps":no_temps,"scope":"Actual App path/DPI preparation and cached native physical CAD GPU render/readback; no desktop chooser or full painter claim"}),
        )?,
    )?;
    Ok(())
}
