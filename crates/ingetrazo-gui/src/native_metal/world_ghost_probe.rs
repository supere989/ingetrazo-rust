//! Actual app flag/recall/action -> production native world route -> GPU readback.
use super::*;
use ingetrazo_core::camera::StandardView;
use std::sync::Arc;
fn capture(app: &mut IngeTrazoApp, metal: &NativeMetal) -> Result<Vec<u8>, String> {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    if let Some(reason) = world_viewport::route(app, rect, 1.) {
        return Err(reason);
    }
    let frame = app
        .native_metal_world
        .as_ref()
        .and_then(|v| v.frame.as_ref())
        .ok_or("No Metal world frame")?;
    if frame.sample_count() != 4 {
        return Err("Ghost fixture requires actual4sample target".into());
    }
    frame
        .submit_diagnostic(metal)?
        .readback(Duration::from_secs(5))
}
pub(crate) fn run(input: PathBuf, output: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    if std::fs::read_dir(&output)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("Fresh ghost output required".into());
    }
    let reference: serde_json::Value = serde_json::from_slice(
        &std::fs::read(input.join("original-alpha-results.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let metal = Arc::new(NativeMetal::new()?);
    let mut rows = Vec::new();
    for case in reference["cases"]
        .as_array()
        .ok_or("Missing original cases")?
    {
        let name = case["case"].as_str().ok_or("Missing case name")?;
        let mut app = IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&input.join(format!("{name}.igz")))
            .map_err(|e| e.to_string())?;
        app.scene.active_sheet_index = None;
        app.scene.shadows.enabled = false;
        app.shadows_enabled = false;
        app.style.show_edges = false;
        app.style.show_profiles = false;
        app.style.show_sky = false;
        app.style.show_grid = false;
        app.style.background_color = [1.; 4];
        app.camera.set_standard_view(StandardView::Top);
        app.camera.perspective = false;
        app.camera.distance = 4.;
        app.camera.target = glam::DVec3::ZERO;
        app.camera.aspect = 800. / 600.;
        app.native_metal_world = Some(world_viewport::Viewport::new(metal.clone()));
        let rgba = capture(&mut app, &metal).map_err(|e| format!("{name} initial: {e}"))?;
        std::fs::write(output.join(format!("{name}.rgba")), &rgba).map_err(|e| e.to_string())?;
        let before = app.native_metal_world.as_ref().unwrap().ghost_metrics();
        let before_geometry = app.native_metal_world.as_ref().unwrap().geometry_uploads();
        let warm = capture(&mut app, &metal)?;
        if warm != rgba
            || app.native_metal_world.as_ref().unwrap().ghost_metrics() != before
            || app.native_metal_world.as_ref().unwrap().geometry_uploads() != before_geometry
        {
            return Err("Warm ghost changed pixels or uploaded".into());
        }
        let counts = app
            .native_metal_world
            .as_ref()
            .unwrap()
            .frame
            .as_ref()
            .unwrap()
            .ghost_counts();
        // Real shared allocator admission pressure, not a fabricated Vulkan/Metal error.
        let (old_limit, committed) = {
            let mut b = metal.budget.lock().map_err(|_| "Budget lock")?;
            let n = b.heaps.iter().map(|h| h.committed).sum::<u64>();
            let old = b.total_limit;
            b.total_limit = Some(n);
            (old, n)
        };
        app.scene
            .set_hidden_view(true, !app.scene.show_hidden_objects);
        app.scene
            .set_hidden_view(false, !app.scene.show_hidden_geometry);
        // Choose an on-state when off so that cold admission necessarily has payload.
        app.scene.set_hidden_view(true, true);
        app.scene.set_hidden_view(false, true);
        app.native_render_revisions.model_replaced();
        let refused = world_viewport::route(
            &mut app,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
            1.,
        )
        .is_some();
        let unchanged_charge = metal
            .budget
            .lock()
            .map_err(|_| "Budget lock")?
            .heaps
            .iter()
            .map(|h| h.committed)
            .sum::<u64>()
            == committed;
        metal.budget.lock().map_err(|_| "Budget lock")?.total_limit = old_limit;
        if !refused || !unchanged_charge {
            return Err("Pressure failed to preserve budget/frame".into());
        }
        let old = app
            .native_metal_world
            .as_ref()
            .unwrap()
            .frame
            .as_ref()
            .unwrap()
            .submit_diagnostic(&metal)?
            .readback(Duration::from_secs(5))?;
        if old != rgba {
            return Err("Refusal replaced old pixels".into());
        }
        capture(&mut app, &metal)?;
        // Consumer actions/recall exercise the same app entrypoints as Vulkan host.
        app.add_saved_view("ghost".into());
        let index = app.selected_saved_view.ok_or("View capture absent")?;
        app.scene.set_hidden_view(true, false);
        app.scene.set_hidden_view(false, false);
        app.apply_saved_view(index);
        capture(&mut app, &metal)?;
        app.unhide_all();
        let after = ingetrazo_render::hidden_ghosts::Packet::build(&app.scene)?;
        if !after.faces.is_empty() || !after.textured.is_empty() || !after.edges.is_empty() {
            return Err("Unhide left ghost members".into());
        }
        let unhide_reason = world_viewport::route(
            &mut app,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
            1.,
        );
        let mut unhide_wire_positive = None;
        if name.starts_with("edge-") {
            if unhide_reason.is_some() {
                return Err("Unhide wire invisible-style route refused".into());
            }
            // This additional ordinary-wire control exercises the admitted one-pixel path.
            // Ghost source12 frames above retain their original edge-disabled style.
            app.style.line_width = 1.;
            app.style.show_edges = true;
            let wire = capture(&mut app, &metal).map_err(|e| format!("{name} ordinary width1 wire: {e}"))?;
            if wire.chunks_exact(4).all(|p| p == [255, 255, 255, 255]) {
                return Err("Visible ordinary wire absent".into());
            }
            unhide_wire_positive = Some(true);
        } else if name.starts_with("textured-object-") {
            if !unhide_reason
                .as_ref()
                .is_some_and(|r| r.contains("missing-ghost-image.png"))
            {
                return Err("Missing ordinary texture refusal".into());
            }
        } else if let Some(error) = &unhide_reason {
            return Err(error.clone());
        }
        rows.push(serde_json::json!({"case":name,"samples":4,"counts":counts,"warm_zero_upload":true,"pressure_refused":refused,"old_pixels_retained":true,"flags_recalled":app.scene.show_hidden_objects&&app.scene.show_hidden_geometry,"unhide_empty_ghosts":true,"unhide_route_reason":unhide_reason,"unhide_wire_positive":unhide_wire_positive}));
        drop(app);
    }
    // Separate valid sampled asset: post-Unhide success, no source12 input changes.
    let valid_texture_unhide = {
        let mut app = IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&input.join("textured-object-on.igz"))
            .map_err(|e| e.to_string())?;
        const ASSET: &[u8] = include_bytes!(
            "../../../../docs/audits/metal-world-scene/material-source/quadrants.png"
        );
        app.scene
            .embedded_textures
            .insert("valid-metal-ghost.png".into(), ASSET.to_vec());
        app.scene.groups[0].mesh.faces[0]
            .attrs
            .get_mut("texture")
            .ok_or("Missing textured face")?["path"] = serde_json::json!("valid-metal-ghost.png");
        app.scene.active_sheet_index = None;
        app.scene.shadows.enabled = false;
        app.shadows_enabled = false;
        app.style.show_edges = false;
        app.style.show_profiles = false;
        app.style.show_sky = false;
        app.style.show_grid = false;
        app.style.background_color = [1.; 4];
        app.camera.set_standard_view(StandardView::Top);
        app.camera.perspective = false;
        app.camera.distance = 4.;
        app.camera.target = glam::DVec3::ZERO;
        app.camera.aspect = 800. / 600.;
        app.native_metal_world = Some(world_viewport::Viewport::new(metal.clone()));
        let ghost = capture(&mut app, &metal)?;
        app.unhide_all();
        let ordinary = capture(&mut app, &metal)?;
        if ordinary == ghost
            || ordinary.chunks_exact(4).all(|p| p == [255, 255, 255, 255])
            || app.native_metal_world.as_ref().unwrap().texture_uploads() == 0
        {
            return Err("Valid texture Unhide did not draw native sampled surface".into());
        }
        let uploads = app.native_metal_world.as_ref().unwrap().texture_uploads();
        if capture(&mut app, &metal)? != ordinary
            || app.native_metal_world.as_ref().unwrap().texture_uploads() != uploads
        {
            return Err("Valid textured Unhide warm cache changed".into());
        }
        true
    };
    let (committed, reserved) = {
        let b = metal.budget.lock().map_err(|_| "Budget lock")?;
        (
            b.heaps.iter().map(|h| h.committed).sum::<u64>(),
            b.heaps.iter().map(|h| h.reserved).sum::<u64>(),
        )
    };
    if committed != 0 || reserved != 0 {
        return Err(format!("Ghost owner leak {committed}/{reserved}"));
    }
    let report = serde_json::json!({"device":metal.info.name,"backend":"native-metal","width":800,"height":600,"cases":rows,"committed_bytes":committed,"reserved_bytes":reserved,"valid_texture_unhide":valid_texture_unhide});
    std::fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
