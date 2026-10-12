//! Actual App imported legend snapshot PDF/native image/save consumers.
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let source = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let source_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(source.join("source.json"))?)?;
    let mut renderer = Renderer::new(args.iter().any(|a| a == "--software"))?;
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |_| {});
    let mut app = ingetrazo_gui::IngeTrazoApp::new();
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &out.join("session"),
    ));
    let mut records = Vec::new();
    for row in source_json["images"].as_array().ok_or("images")? {
        let name = row["case"].as_str().ok_or("case")?;
        app.mark_document_clean();
        app.open_document(&source.join(row["scene"].as_str().ok_or("scene")?));
        assert!(!app.document_session.confirm_unsaved);
        assert_eq!(app.scene.sheets.len(), 1);
        app.set_language(row["language"].as_str().ok_or("language")?);
        // Snapshot names are independent of current model layer names.
        if let Some(layer) = app.scene.layers.first_mut() {
            layer.name = "Changed live layer after snapshot".into();
        }
        app.scene.sheets[0].native_document["leyendas"][0]["vendor_probe"] =
            serde_json::json!({"opaque":"preserve"});
        let before = app.scene.clone();
        let path = out.join(row["image"]["file"].as_str().ok_or("image")?);
        let dpi = row["dpi"].as_u64().ok_or("dpi")? as u16;
        app.begin_sheet_image_export(0);
        app.choose_sheet_image_path(0, Some(&path), Format::Png);
        app.choose_sheet_image_resolution(0, path.clone(), Some(dpi))?;
        let (request, plan) = app.prepare_sheet_image_request(&ctx).ok_or("request")??;
        renderer.save(&plan, &request.path)?;
        app.export_composer_pdf(Some(0), &out.join(row["pdf"].as_str().ok_or("pdf")?))?;
        assert_eq!(app.scene, before, "export mutates authored scene");
        let image = image::open(&path)?.to_rgba8();
        std::fs::write(out.join(format!("{name}.rgba")), image.as_raw())?;
        let saved = out.join(format!("{name}.igz"));
        app.save_to_path(&saved);
        assert!(saved.exists());
        let reopened = ingetrazo_formats::igz::load_igz(&saved)?;
        assert_eq!(
            reopened.sheets[0].native_document["leyendas"],
            before.sheets[0].native_document["leyendas"],
            "legend portable metadata ownership"
        );
        records.push(serde_json::json!({"case":name,"file":row["image"]["file"],"pixels":[image.width(),image.height()],"dpi":dpi,"nonmutating":true,"snapshot_owner":true,"portable":saved.file_name().unwrap().to_string_lossy(),"pdf":row["pdf"]}));
    }
    // Invalid visible row payload refuses atomically; opaque snapshot metadata remains.
    app.mark_document_clean();
    app.open_document(&source.join("legend-rows.igz"));
    app.scene.sheets[0].native_document["leyendas"][0]["rows"][0] =
        serde_json::json!({"invalid":"row"});
    let target = out.join("refusal.png");
    std::fs::write(&target, b"prior destination")?;
    let before = app.scene.clone();
    app.begin_sheet_image_export(0);
    app.choose_sheet_image_path(0, Some(&target), Format::Png);
    app.choose_sheet_image_resolution(0, target.clone(), Some(100))?;
    assert!(app
        .prepare_sheet_image_request(&ctx)
        .ok_or("refusal request")?
        .is_err());
    assert_eq!(std::fs::read(&target)?, b"prior destination");
    assert!(app.export_composer_pdf(Some(0), &target).is_err());
    assert_eq!(std::fs::read(&target)?, b"prior destination");
    assert_eq!(app.scene, before);
    renderer.cleanup()?;
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"images":records,"atomic_refusal":true}))?,
    )?;
    Ok(())
}
