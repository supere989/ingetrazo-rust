//! Actual App six-family annotation export, frozen-plan ownership and save consumers.
use ingetrazo_formats::sheet_image::Format;
use std::path::Path;
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
fn prepare(
    app: &mut ingetrazo_gui::IngeTrazoApp,
    context: &egui::Context,
    sheet: usize,
    path: &Path,
    dpi: u16,
) -> Result<ingetrazo_gui::sheet_image::Plan, String> {
    app.begin_sheet_image_export(sheet);
    app.choose_sheet_image_path(sheet, Some(path), Format::Png);
    app.choose_sheet_image_resolution(sheet, path.to_path_buf(), Some(dpi))?;
    app.prepare_sheet_image_request(context)
        .ok_or("Missing annotation export request")?
        .map(|(_, plan)| plan)
}
const FAMILIES: [&str; 6] = [
    "cotas",
    "cotas_ang",
    "cotas_rad",
    "etiquetas",
    "niveles",
    "llamadas",
];
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("Usage: sheet_annotation_workflows SOURCE NATIVE [--software]".into());
    }
    let source = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let reference: serde_json::Value =
        serde_json::from_slice(&std::fs::read(source.join("source.json"))?)?;
    let mut renderer = Renderer::new(args.iter().any(|a| a == "--software"))?;
    let context = egui::Context::default();
    let _ = context.run(Default::default(), |_| {});
    let mut app = ingetrazo_gui::IngeTrazoApp::new();
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &out.join("session"),
    ));
    app.set_language("en");
    let mut records = Vec::new();
    for row in reference["images"]
        .as_array()
        .ok_or("Missing annotation cases")?
    {
        let name = row["case"].as_str().ok_or("case")?;
        let sheet = row["sheet_index"].as_u64().ok_or("sheet_index")? as usize;
        let dpi: u16 = row["dpi"].as_u64().ok_or("dpi")?.try_into()?;
        app.mark_document_clean();
        app.open_document(&source.join(row["scene"].as_str().ok_or("scene")?));
        assert!(!app.document_session.confirm_unsaved);
        assert_eq!(app.scene.sheets.len(), sheet + 1);
        // The unchanged source fixture supplies the same document field context.
        app.current_file = Some("fixture-project.igz".into());
        let before = app.scene.clone();
        let png = out.join(row["image"]["file"].as_str().ok_or("png")?);
        let jpeg = out.join(row["jpeg"].as_str().ok_or("jpeg")?);
        let pdf = out.join(row["pdf"].as_str().ok_or("pdf")?);
        let plan = prepare(&mut app, &context, sheet, &png, dpi)?;
        renderer.save(&plan, &png)?;
        renderer.save(&plan, &jpeg)?;
        // The prepared plan owns all geometry and glyph resources. Reuse it after
        // detaching authored sheets; this is a plan-lifetime proof, not a GPU upload budget.
        let held = std::mem::take(&mut app.scene.sheets);
        let warm = out.join(format!("{name}-warm.png"));
        let warm_result = renderer.save(&plan, &warm);
        app.scene.sheets = held;
        warm_result?;
        assert_eq!(
            std::fs::read(&png)?,
            std::fs::read(&warm)?,
            "Prepared annotation plan changed after author detachment"
        );
        app.export_composer_pdf(Some(sheet), &pdf)?;
        assert_eq!(
            app.scene, before,
            "Annotation export mutated authored document"
        );
        let saved = out.join(format!("{name}.igz"));
        app.save_to_path(&saved);
        assert!(saved.exists());
        let reopened = ingetrazo_formats::igz::load_igz(&saved)?;
        assert_eq!(reopened.sheets.len(), before.sheets.len());
        for (actual, expected) in reopened.sheets.iter().zip(&before.sheets) {
            for family in FAMILIES {
                assert_eq!(
                    actual.native_document.get(family),
                    expected.native_document.get(family),
                    "Opaque annotation ownership: {family}"
                );
            }
            assert_eq!(
                actual.native_document.get("vendor_annotations"),
                expected.native_document.get("vendor_annotations")
            );
        }
        let dimensions = image::image_dimensions(&png)?;
        records.push(serde_json::json!({
            "case":name,"sheet_index":sheet,"dpi":dpi,
            "file":row["image"]["file"],"pixels":[dimensions.0,dimensions.1],
            "jpeg":row["jpeg"],"pdf":row["pdf"],
            "portable":saved.file_name().unwrap().to_string_lossy(),
            "nonmutating":true,"snapshot_owner":true,"warm_reused":true,
            "warm_scope":"Same prepared plan after authored sheets detached; identical PNG bytes"
        }));
        std::fs::write(
            out.join("native.partial.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"images":records}))?,
        )?;
    }
    app.mark_document_clean();
    app.open_document(&source.join("all-six-defaults.igz"));
    assert_eq!(app.scene.sheets.len(), 1);
    let baseline = app.scene.sheets[0].clone();
    let mut refusals = Vec::new();
    for family in FAMILIES {
        app.scene.sheets[0] = baseline.clone();
        app.scene.sheets[0].native_document[family][0]["x_mm"] =
            serde_json::json!("invalid coordinate");
        let before = app.scene.clone();
        let png = out.join(format!("refusal-{family}.png"));
        let pdf = out.join(format!("refusal-{family}.pdf"));
        std::fs::write(&png, b"prior destination")?;
        std::fs::write(&pdf, b"prior destination")?;
        assert!(
            prepare(&mut app, &context, 0, &png, 100).is_err(),
            "Invalid {family} accepted for PNG"
        );
        assert!(
            app.export_composer_pdf(Some(0), &pdf).is_err(),
            "Invalid {family} accepted for PDF"
        );
        assert_eq!(std::fs::read(&png)?, b"prior destination");
        assert_eq!(std::fs::read(&pdf)?, b"prior destination");
        assert_eq!(app.scene, before);
        refusals.push(serde_json::json!({"family":family,"png_destination_preserved":true,"pdf_destination_preserved":true,"scene_preserved":true}));
    }
    renderer.cleanup()?;
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "images":records,"atomic_refusal":true,"refusals":refusals,
            "scope":"Actual App PNG/JPEG/PDF export and save. Same prepared immutable plan survives authored-sheet detachment. No annotation authoring or Model View synchronization claim."
        }))?,
    )?;
    Ok(())
}
