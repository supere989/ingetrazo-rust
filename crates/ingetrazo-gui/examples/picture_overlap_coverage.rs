//! One actual chamfer export: immutable prepared spans and actual prefix GPU paint.
#[path = "sheet_symbols_workflows.rs"]
mod symbols;
use ingetrazo_formats::sheet_image::Format;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
fn image(path: &Path, out: &Path, name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let image = image::open(path)?.to_rgba8();
    let raw = image.as_raw();
    std::fs::write(out.join(format!("{name}.rgba")), raw)?;
    Ok(
        json!({"file":path.file_name().unwrap().to_string_lossy(),"rgba":format!("{name}.rgba"),"size":[image.width(),image.height()],"rgba_sha256":format!("{:x}",Sha256::digest(raw))}),
    )
}
fn queue(a: &mut IngeTrazoApp, path: &Path) -> Result<(), String> {
    a.begin_sheet_image_export(0);
    a.choose_sheet_image_path(0, Some(path), Format::Png);
    a.choose_sheet_image_resolution(0, path.into(), Some(100))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err("picture_overlap_coverage SOURCE OUTPUT [--software]".into());
    }
    let source = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let input: Value = serde_json::from_slice(&std::fs::read(
        source.join("original-picture-overlap.json"),
    )?)?;
    let mut renderer = symbols::Renderer::new(args.iter().any(|a| a == "--software"))?;
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.set_language("en");
    app.native_gpu_viewport_enabled = true;
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &out.join("state"),
    ));
    app.open_document(&source.join("picture-overlap.igz"));
    if app.scene.sheets.len() != 1 {
        return Err("one frozen sheet required".into());
    }
    let before = app.scene.clone();
    let history = (
        app.undo_stack.len(),
        app.redo_stack.len(),
        app.composer_state.sheet_history.len(),
    );
    let ctx = egui::Context::default();
    let ordinary_path = out.join("ordinary-before.png");
    queue(&mut app, &ordinary_path)?;
    let (request, ordinary) = app
        .prepare_sheet_image_request(&ctx)
        .ok_or("ordinary request missing")??;
    renderer.save(&ordinary, &request.path)?;
    let ordinary_image = image(&ordinary_path, out, "ordinary-before")?;
    let observed_path = out.join("ordinary-observed.png");
    queue(&mut app, &observed_path)?;
    let (result, observations) = app.prepare_sheet_image_request_observed(&ctx);
    let (request, plan) = result.ok_or("observed request missing")??;
    let heavy = observations
        .iter()
        .filter(|v| v["stroke"][1].as_f64() == input["stages"][0]["pen"]["width"].as_f64())
        .collect::<Vec<_>>();
    if heavy.len() != 1 {
        return Err(format!("ambiguous actual heavy geometry {}", heavy.len()).into());
    }
    let id = heavy[0]["geometry_id"].as_u64().ok_or("geometry ID")?;
    let records = plan.diagnostic_records();
    let rows = records.as_array().ok_or("records")?;
    let index = rows
        .iter()
        .position(|r| r["geometry_id"].as_u64() == Some(id))
        .ok_or("published heavy absent")?;
    if rows[index]["source_spans"] != heavy[0]["postmask_spans"] {
        return Err("trace not actual published spans".into());
    }
    let prefix = plan.diagnostic_prefix(id)?;
    let prefix_records = prefix.diagnostic_records();
    if prefix_records.as_array() != Some(&rows[..index].to_vec()) {
        return Err("prefix did not retain actual immutable owners".into());
    }
    let preceding_id = index
        .checked_sub(1)
        .and_then(|i| rows[i]["geometry_id"].as_u64())
        .ok_or("preceding fill owner missing")?;
    let before_preceding = plan.diagnostic_prefix(preceding_id)?;
    let preceding_path = out.join("before-preceding-record.png");
    renderer.save(&before_preceding, &preceding_path)?;
    let preceding_image = image(&preceding_path, out, "before-preceding-record")?;
    let prefix_path = out.join("prefix.png");
    renderer.save(&prefix, &prefix_path)?;
    let prefix_image = image(&prefix_path, out, "prefix")?;
    renderer.save(&plan, &request.path)?;
    let observed_image = image(&observed_path, out, "ordinary-observed")?;
    if ordinary_image["rgba_sha256"] != observed_image["rgba_sha256"] {
        return Err("native observation altered ordinary frame".into());
    }
    if app.scene != before
        || history
            != (
                app.undo_stack.len(),
                app.redo_stack.len(),
                app.composer_state.sheet_history.len(),
            )
    {
        return Err("diagnostic changed authored model/history".into());
    }
    renderer.cleanup()?;
    std::fs::write(
        out.join("native-picture-overlap.json"),
        serde_json::to_vec_pretty(
            &json!({"source_pin":input["source_pin"],"case":"picture-overlap","dpi":100,"ordinary_before":ordinary_image,"ordinary_observed":observed_image,"prefix":prefix_image,"before_preceding_record":preceding_image,"preceding_record":rows[index-1],"heavy":heavy[0],"prepared_records":records,"heavy_record_index":index,"controls":{"observer_ordinary_equal":true,"model_history_unchanged":true,"actual_published_spans":true,"prefix_immutable_owner_reuse":true},"scope":"Actual App normal export and same prepared native records; prefix shares Arc owners. No probe geometry producer or CPU replacement renderer, no private Qt span claim."}),
        )?,
    )?;
    Ok(())
}
