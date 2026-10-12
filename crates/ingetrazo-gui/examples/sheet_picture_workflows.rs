//! Actual App imported picture ownership, native raster, PDF and saved-reader consumer.
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
    fn counters(&self) -> [u64; 4] {
        let s = self.exporter.as_ref().unwrap().picture_stats();
        [
            s.image_uploads,
            s.row_uploads,
            s.uploaded_bytes,
            s.warm_hits,
        ]
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
    fn counters(&self) -> [u64; 4] {
        let s = self.0.picture_stats();
        [
            s.image_uploads,
            s.row_uploads,
            s.uploaded_bytes,
            s.warm_hits,
        ]
    }
    fn cleanup(&mut self) -> Result<(), String> {
        Ok(())
    }
}
// Temporarily hide extracted fixture assets only. A prepared export must retain
// immutable bytes; Drop restores paths even when a native export returns an error.
struct HiddenAssets(Vec<(std::path::PathBuf, std::path::PathBuf)>);
impl HiddenAssets {
    fn new(images: &serde_json::Value, source: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut held = Self(Vec::new());
        let mut unique = std::collections::HashSet::new();
        for value in images.as_array().ok_or("images")? {
            let Some(raw) = value["path"].as_str() else {
                continue;
            };
            let path = source.join(raw);
            if !path.is_file() || !unique.insert(path.clone()) {
                continue;
            }
            path.canonicalize()?.strip_prefix(source.canonicalize()?)?;
            let backup = path.with_file_name(format!(".ingetrazo-proof-held-{}", held.0.len()));
            if backup.exists() {
                return Err("Asset hold destination already exists".into());
            }
            std::fs::rename(&path, &backup)?;
            held.0.push((path, backup));
        }
        Ok(held)
    }
    fn restore(&mut self) -> Result<(), std::io::Error> {
        while let Some((path, backup)) = self.0.last() {
            std::fs::rename(backup, path)?;
            self.0.pop();
        }
        Ok(())
    }
}
impl Drop for HiddenAssets {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
fn prepare(
    app: &mut ingetrazo_gui::IngeTrazoApp,
    ctx: &egui::Context,
    path: &Path,
) -> Result<ingetrazo_gui::sheet_image::Plan, String> {
    app.begin_sheet_image_export(0);
    app.choose_sheet_image_path(0, Some(path), Format::Png);
    app.choose_sheet_image_resolution(0, path.to_owned(), Some(100))?;
    app.prepare_sheet_image_request(ctx)
        .ok_or("Missing prepared picture request")?
        .map(|(_, plan)| plan)
}
fn count(data: &[u8], token: &[u8]) -> usize {
    data.windows(token.len()).filter(|s| *s == token).count()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let source = Path::new(args.first().ok_or("Missing frozen source")?).canonicalize()?;
    let out = Path::new(args.get(1).ok_or("Missing output")?);
    std::fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    // Original image paths are external and relative to the source working directory.
    std::env::set_current_dir(&source)?;
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
    for row in source_json["cases"].as_array().ok_or("cases")? {
        let name = row["case"].as_str().ok_or("case")?;
        app.mark_document_clean();
        app.open_document(&source.join(row["scene"].as_str().ok_or("scene")?));
        assert!(!app.document_session.confirm_unsaved);
        assert_eq!(app.scene.sheets.len(), 1);
        for picture in app.scene.sheets[0].native_document["images"]
            .as_array_mut()
            .ok_or("images")?
        {
            picture["vendor_probe"] =
                serde_json::json!({"opaque":"preserve","nested":[1,false,null]});
            // The original has no operative image rotation field; preserve metadata.
            picture["rotation_deg"] = serde_json::json!(17.);
        }
        let before = app.scene.clone();
        let png = out.join(format!("{name}.png"));
        let jpg = out.join(format!("{name}.jpg"));
        let plan = prepare(&mut app, &ctx, &png)?;
        let cpu = app.sheet_picture_cache_stats();
        let mut held = HiddenAssets::new(&before.sheets[0].native_document["images"], &source)?;
        renderer.save(&plan, &png)?;
        let cold = renderer.counters();
        renderer.save(&plan, &jpg)?;
        let warm_path = out.join(format!("{name}-warm.png"));
        renderer.save(&plan, &warm_path)?;
        let warm = renderer.counters();
        assert_eq!(
            &cold[..3],
            &warm[..3],
            "Warm export reuploaded immutable picture buffers"
        );
        assert_eq!(
            std::fs::read(&png)?,
            std::fs::read(&warm_path)?,
            "Prepared image changed between exports"
        );
        held.restore()?;
        app.export_composer_pdf(Some(0), &out.join(format!("{name}.pdf")))?;
        assert_eq!(
            cpu,
            app.sheet_picture_cache_stats(),
            "PDF decoded/composited the already prepared picture again"
        );
        assert_eq!(app.scene, before, "Export mutated authored scene");
        let mut images = serde_json::Map::new();
        for (extension, path) in [("png", &png), ("jpg", &jpg)] {
            let decoded = image::open(path)?;
            images.insert(extension.into(), serde_json::json!({"file":format!("{name}.{extension}"),"pixels":[decoded.width(),decoded.height()]}));
        }
        let saved = out.join(format!("{name}.igz"));
        app.save_to_path(&saved);
        assert!(saved.exists());
        let reopened = ingetrazo_formats::igz::load_igz(&saved)?;
        assert_eq!(
            reopened.sheets[0].native_document["images"],
            before.sheets[0].native_document["images"],
            "External paths and presentation metadata changed on save"
        );
        records.push(serde_json::json!({"case":name,"dpi":100,"images":images,"pdf":format!("{name}.pdf"),"portable":format!("{name}.igz"),"nonmutating":true,"external_paths":true,"snapshot_assets":true,"warm_images":true,"cpu_cache":cpu,"cold_gpu":cold,"warm_gpu":warm}));
    }
    // A second placement must share one immutable image and PDF XObject set.
    app.mark_document_clean();
    app.open_document(&source.join("picture-import-plain.igz"));
    let images = app.scene.sheets[0].native_document["images"]
        .as_array_mut()
        .ok_or("duplicate images")?;
    let mut duplicate = images.first().ok_or("first picture")?.clone();
    duplicate["x_mm"] = serde_json::json!(82.);
    images.push(duplicate);
    let gpu_before = renderer.counters();
    let duplicate_png = out.join("duplicate.png");
    let plan = prepare(&mut app, &ctx, &duplicate_png)?;
    renderer.save(&plan, &duplicate_png)?;
    let gpu_after = renderer.counters();
    assert!(
        gpu_after[0] - gpu_before[0] <= 1,
        "Duplicate placement uploaded pixels twice"
    );
    let duplicate_pdf = out.join("duplicate.pdf");
    app.export_composer_pdf(Some(0), &duplicate_pdf)?;
    let single_pdf = std::fs::read(out.join("picture-import-plain.pdf"))?;
    let double_pdf = std::fs::read(duplicate_pdf)?;
    let image_objects = count(&single_pdf, b"/Subtype /Image");
    assert!(image_objects > 0);
    assert_eq!(image_objects, count(&double_pdf, b"/Subtype /Image"));
    assert_eq!(count(&double_pdf, b"/Im0 Do"), 2);
    // Invalid presentation refuses before opening/replacing either destination.
    app.scene.sheets[0].native_document["images"][0]["shape"] = serde_json::json!("invalid-shape");
    let before = app.scene.clone();
    let png = out.join("refusal.png");
    let pdf = out.join("refusal.pdf");
    std::fs::write(&png, b"prior PNG destination")?;
    std::fs::write(&pdf, b"prior PDF destination")?;
    assert!(prepare(&mut app, &ctx, &png).is_err());
    assert!(app.export_composer_pdf(Some(0), &pdf).is_err());
    assert_eq!(std::fs::read(png)?, b"prior PNG destination");
    assert_eq!(std::fs::read(pdf)?, b"prior PDF destination");
    assert_eq!(app.scene, before);
    renderer.cleanup()?;
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"cases":records,"atomic_refusal":true,"duplicate_owner":true,"duplicate_gpu_before":gpu_before,"duplicate_gpu_after":gpu_after,"duplicate_pdf_image_objects":image_objects}),
        )?,
    )?;
    Ok(())
}
