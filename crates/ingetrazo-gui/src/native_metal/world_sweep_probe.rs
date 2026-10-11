//! Actual tool acquisition/hover -> native viewport -> borrowed preview readback.
use super::*;
use glam::DVec3;
use std::sync::Arc;
fn vector(v: &serde_json::Value) -> Result<DVec3, String> {
    let a = v.as_array().ok_or("Vector array")?;
    if a.len() != 3 {
        return Err("Vector length".into());
    }
    Ok(DVec3::new(
        a[0].as_f64().ok_or("Vector x")?,
        a[1].as_f64().ok_or("Vector y")?,
        a[2].as_f64().ok_or("Vector z")?,
    ))
}
fn frame(
    app: &mut IngeTrazoApp,
    metal: &NativeMetal,
    ctx: &egui::Context,
    painter: &mut ingetrazo_metal::egui_painter::EguiPainter,
    route: bool,
) -> Result<Vec<u8>, String> {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    if route {
        if let Some(error) = world_viewport::route(app, rect, 1.) {
            return Err(error);
        }
    }
    let f = app
        .native_metal_world
        .as_ref()
        .and_then(|v| v.frame.as_ref())
        .ok_or("No sweep frame")?;
    if f.sample_count() != 4 {
        return Err("Sweep requires4samples".into());
    }
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            painter.image(
                world_viewport::TEXTURE,
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                egui::Color32::WHITE,
            );
            if let Some(shape) = &app.native_gpu_annotation_shape {
                painter.add(shape.clone());
            }
        },
    );
    painter.update_textures(metal, &output.textures_delta)?;
    world_viewport::install(painter, metal, app)?;
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
    let paint = painter.prepare(metal, &primitives, output.pixels_per_point)?;
    painter.free_textures(&output.textures_delta.free);
    painter
        .submit_diagnostic(metal, paint, 800, 600, false)?
        .wait_readback(Duration::from_secs(5))
}
pub(crate) fn run(input: PathBuf, output: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    if std::fs::read_dir(&output)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("Fresh sweep output required".into());
    }
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(input.join("original.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let metal = Arc::new(NativeMetal::new()?);
    let mut results = Vec::new();
    for c in source["cases"].as_array().ok_or("Sweep cases")? {
        let name = c["name"].as_str().ok_or("Sweep name")?;
        let ctx = egui::Context::default();
        let mut painter = if ingetrazo_metal::MetalCad2d::supported(&metal) {
            ingetrazo_metal::egui_painter::EguiPainter::new(&metal)?
        } else {
            ingetrazo_metal::egui_painter::EguiPainter::new_compute(&metal)?
        };
        let mut app = IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&input.join(c["igz"].as_str().ok_or("IGZ")?))
            .map_err(|e| e.to_string())?;
        app.scene.active_sheet_index = None;
        app.apply_document_presentation();
        app.style.show_grid = false;
        let cam = &c["camera"];
        app.camera.target = vector(&cam["target"])?;
        app.camera.distance = cam["distance"].as_f64().ok_or("Distance")?;
        app.camera.yaw = cam["yaw"].as_f64().ok_or("Yaw")?;
        app.camera.pitch = cam["pitch"].as_f64().ok_or("Pitch")?;
        app.camera.perspective = false;
        app.camera.aspect = 800. / 600.;
        app.native_metal_world = Some(world_viewport::Viewport::new(metal.clone()));
        let mesh_before = app.scene.mesh.clone();
        let history_before = app.undo_stack.len();
        let anchor = vector(&c["anchor"])?;
        let normal = vector(&c["normal"])?;
        let context = |world| {
            ingetrazo_tools::ToolContext::new(
                world,
                ingetrazo_core::math::Ray::new(anchor + normal * 0.001, -normal),
            )
        };
        let mut phases = Vec::new();
        let mut base = None;
        let mut previous = None;
        let mut previous_stats = None;
        let mut refusal = false;
        let mut old_label = None;
        for source_frame in c["frames"].as_array().ok_or("Frames")? {
            let phase = source_frame["phase"].as_str().ok_or("Phase")?;
            if phase == "base" {
                app.set_tool(ingetrazo_tools::ToolType::PushPull);
                app.dispatch_tool_move(&context(anchor));
                app.dispatch_tool_click(&context(anchor));
            }
            if matches!(phase, "base" | "preview" | "offset" | "cancel") {
                let d = source_frame["distance"].as_f64().ok_or("Offset")?;
                app.dispatch_tool_move(&context(anchor + normal * d));
                let p = app
                    .active_tool
                    .face_sweep_preview()
                    .ok_or("No acquired sweep")?;
                if p.face != c["face"].as_u64().ok_or("Face")? as usize
                    || p.attached != c["attached"].as_bool().ok_or("Attached")?
                    || p.prism_cap != c["prism_cap"].as_bool().ok_or("Prism")?
                {
                    return Err(format!("{name} source acquisition mismatch"));
                }
            }
            // Source base/cancel retain acquisition at zero offset (and its label).
            // Tool cancellation itself is a separate lifecycle control.
            if phase == "offset" {
                old_label = Some(Arc::downgrade(
                    &app.native_metal_world
                        .as_ref()
                        .unwrap()
                        .label_owner()
                        .ok_or("No old native label")?,
                ));
                let charge = {
                    let b = metal.budget.lock().map_err(|_| "Budget")?;
                    b.heaps
                        .iter()
                        .map(|h| h.committed + h.reserved)
                        .sum::<u64>()
                };
                app.native_metal_world
                    .as_mut()
                    .unwrap()
                    .probe_font_budget(true)?;
                let reason = world_viewport::route(
                    &mut app,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
                    1.,
                );
                if reason.is_none() {
                    return Err("Real font budget refusal missing".into());
                }
                let after = {
                    let b = metal.budget.lock().map_err(|_| "Budget")?;
                    b.heaps
                        .iter()
                        .map(|h| h.committed + h.reserved)
                        .sum::<u64>()
                };
                if after != charge
                    || previous.as_ref()
                        != Some(&frame(&mut app, &metal, &ctx, &mut painter, false)?)
                {
                    return Err("Font refusal changed accepted frame or GPU charges".into());
                }
                if old_label.as_ref().unwrap().upgrade().is_none() {
                    return Err("Font refusal released accepted label owner".into());
                }
                app.native_metal_world
                    .as_mut()
                    .unwrap()
                    .probe_font_budget(false)?;
            }
            if phase == "offset" {
                let (old_limit, charge) = {
                    let mut b = metal.budget.lock().map_err(|_| "Budget")?;
                    let charge = b
                        .heaps
                        .iter()
                        .map(|h| h.committed + h.reserved)
                        .sum::<u64>();
                    let old = b.total_limit;
                    b.total_limit = Some(charge);
                    (old, charge)
                };
                let reason = world_viewport::route(
                    &mut app,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
                    1.,
                );
                let retained = {
                    let b = metal.budget.lock().map_err(|_| "Budget")?;
                    b.heaps
                        .iter()
                        .map(|h| h.committed + h.reserved)
                        .sum::<u64>()
                        == charge
                };
                metal.budget.lock().map_err(|_| "Budget")?.total_limit = old_limit;
                if reason.is_none() || !retained {
                    return Err("Preview pressure did not preserve charge".into());
                }
                let old = frame(&mut app, &metal, &ctx, &mut painter, false)?;
                if previous.as_ref() != Some(&old) {
                    return Err("Refusal changed old preview pixels".into());
                }
                if old_label.as_ref().unwrap().upgrade().is_none() {
                    return Err("CAD refusal released accepted label owner".into());
                }
                refusal = true;
            }
            let rgba = frame(&mut app, &metal, &ctx, &mut painter, true)
                .map_err(|e| format!("{name}/{phase}: {e}"))?;
            if phase == "offset" && old_label.as_ref().unwrap().upgrade().is_some() {
                return Err("Completed replacement retained retired label owner".into());
            }
            if phase == "base" {
                base = Some(rgba.clone());
            }
            if phase == "cancel" && base.as_ref() != Some(&rgba) {
                return Err(format!("{name} Cancel changed base pixels"));
            }
            let stats = app.native_metal_world.as_ref().unwrap().sweep_stats();
            let label_native = app.native_metal_world.as_ref().unwrap().label_stats();
            let label_uploads = painter.cad_stats().uploads;
            let label_compute = painter
                .compute_stats()
                .map(|s| (s.plan_uploads, s.palette_uploads));
            if frame(&mut app, &metal, &ctx, &mut painter, true)? != rgba
                || app.native_metal_world.as_ref().unwrap().label_stats() != label_native
                || painter.cad_stats().uploads != label_uploads
                || painter
                    .compute_stats()
                    .map(|s| (s.plan_uploads, s.palette_uploads))
                    != label_compute
                || app
                    .native_metal_world
                    .as_ref()
                    .unwrap()
                    .sweep_stats()
                    .geometry_uploads
                    != stats.geometry_uploads
                || app
                    .native_metal_world
                    .as_ref()
                    .unwrap()
                    .sweep_stats()
                    .preview_line_uploads
                    != stats.preview_line_uploads
                || app
                    .native_metal_world
                    .as_ref()
                    .unwrap()
                    .sweep_stats()
                    .base_line_uploads
                    != stats.base_line_uploads
            {
                return Err("Warm sweep uploaded or changed".into());
            }
            std::fs::write(output.join(format!("{name}-{phase}.rgba")), rgba)
                .map_err(|e| e.to_string())?;
            if let Some(old) = previous_stats {
                let old: ingetrazo_metal::world::Stats = old;
                if phase == "offset"
                    && (stats.geometry_uploads - old.geometry_uploads != 1
                        || stats.base_line_uploads != old.base_line_uploads
                        || stats.texture_uploads != old.texture_uploads)
                {
                    return Err("Offset rebuilt base geometry/texture".into());
                }
                if phase == "cancel"
                    && (stats.geometry_uploads != old.geometry_uploads
                        || stats.base_line_uploads != old.base_line_uploads)
                {
                    return Err("Cancel failed cached base restore".into());
                }
            }
            previous = Some(frame(&mut app, &metal, &ctx, &mut painter, true)?);
            previous_stats = Some(stats);
            phases.push(serde_json::json!({"phase":phase,"geometry_uploads":stats.geometry_uploads,"preview_builds":stats.preview_builds,"preview_line_uploads":stats.preview_line_uploads,"base_line_uploads":stats.base_line_uploads,"label_native_builds_bytes":label_native,"label_uploads":label_uploads,"label_compute_uploads":label_compute}));
        }
        // Source's phase named cancel is zero-offset acquisition. Exercise real
        // native Tool cancellation separately, without comparing it to that image.
        let cancelled_label = Arc::downgrade(&app.native_metal_world.as_ref()
            .unwrap().label_owner().ok_or("No zero-offset label owner")?);
        let before_cancel = app.native_metal_world.as_ref().unwrap().sweep_stats();
        app.set_tool(ingetrazo_tools::ToolType::Select);
        if let Some(reason) = world_viewport::route(&mut app,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)), 1.) {
            return Err(format!("Actual Tool cancellation: {reason}"));
        }
        let after_cancel = app.native_metal_world.as_ref().unwrap().sweep_stats();
        if app.active_tool.face_sweep_preview().is_some()
            || app.active_tool.distance_label().is_some()
            || app.native_gpu_annotation_shape.is_some()
            || app.native_metal_world.as_ref().unwrap().label_owner().is_some()
            || before_cancel.geometry_uploads != after_cancel.geometry_uploads
            || before_cancel.base_line_uploads != after_cancel.base_line_uploads {
            return Err("Actual Tool cancellation failed cached base/label retirement".into());
        }
        let _ = frame(&mut app, &metal, &ctx, &mut painter, true)?;
        if cancelled_label.upgrade().is_some() {
            return Err("Actual Tool cancellation retained completed label owner".into());
        }
        if app.scene.mesh != mesh_before || app.undo_stack.len() != history_before {
            return Err("Preview mutated model/history".into());
        }
        drop(app);
        drop(painter);
        results.push(serde_json::json!({"name":name,"phases":phases,"model_unchanged":true,"warm_uploads_zero":true,"pressure_old_pixels_charge_retry":refusal,"font_refusal_retained":true,"actual_tool_cancel":true}));
    }
    let ledger = metal
        .budget
        .lock()
        .map_err(|_| "Budget")?
        .heaps
        .iter()
        .map(|h| h.committed + h.reserved)
        .sum::<u64>();
    if ledger != 0 {
        return Err(format!("Sweep owner leak {ledger}"));
    }
    std::fs::write(output.join("results.json"),serde_json::to_vec_pretty(&serde_json::json!({"device":metal.info.name,"samples":4,"cases":results,"ledger":ledger})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}
