//! Bounded actual shared application UI -> viewport route -> native paint readback.
//! Constructor-authored changes exercise real render dispatch; no OS gesture claim.
use super::*;
use egui::{Event, Pos2};
use glam::{DMat4, DVec3};
use ingetrazo_core::{camera::StandardView, groups::Group, mesh::Mesh};
use std::sync::Arc;
const WIDTH: usize = 1280;
const HEIGHT: usize = 800;
pub(crate) fn run(root: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("World app probe requires fresh output".into());
    }
    let metal = Arc::new(NativeMetal::new()?);
    let ctx = egui::Context::default();
    let mut painter = EguiPainter::new(&metal)?;
    let mut app = IngeTrazoApp::new();
    app.scene.mesh = Mesh::new();
    app.scene.groups.clear();
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/audits/metal-world-scene/material-source/source.json"
    ))
    .map_err(|e| e.to_string())?;
    let heldout: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/audits/metal-world-scene/material-shaded-side-source/source.json"
    ))
    .map_err(|e| e.to_string())?;
    recipe["cases"]
        .as_array_mut()
        .ok_or("Recipe cases missing")?
        .extend(
            heldout["cases"]
                .as_array()
                .ok_or("Held-out cases missing")?
                .iter()
                .cloned(),
        );
    let alpha: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/audits/metal-world-scene/material-alpha-source/source.json"
    ))
    .map_err(|e| e.to_string())?;
    recipe["cases"]
        .as_array_mut()
        .ok_or("Recipe cases missing")?
        .extend(
            alpha["cases"]
                .as_array()
                .ok_or("Alpha cases missing")?
                .iter()
                .cloned(),
        );
    const ASSET: &[u8] =
        include_bytes!("../../../../docs/audits/metal-world-scene/material-source/quadrants.png");
    app.scene
        .embedded_textures
        .insert("quadrants".into(), ASSET.to_vec());
    app.scene.active_sheet_index = None;
    app.scene.shadows.enabled = false;
    app.shadows_enabled = false;
    app.style.show_edges = false;
    app.style.show_profiles = false;
    app.style.show_sky = false;
    app.style.show_grid = false;
    app.style.background_color = [0., 0., 1., 1.];
    app.camera.set_standard_view(StandardView::Top);
    app.camera.perspective = false;
    app.camera.distance = 8.;
    app.camera.target = DVec3::ZERO;
    app.native_metal_world = Some(world_viewport::Viewport::new(metal.clone()));
    app.bump_model_revision();
    let mut observations = Vec::new();
    let mut time = 0.;
    let mut render = |app: &mut IngeTrazoApp,
                      name: &str,
                      events: Vec<Event>|
     -> Result<(), String> {
        time += 1. / 60.;
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(WIDTH as f32, HEIGHT as f32),
        ));
        raw.time = Some(time);
        raw.events = events;
        let output = ctx.run(raw, |ctx| app.update_ui(ctx));
        let viewport = app.native_metal_world.as_ref().ok_or("Bridge retired")?;
        if viewport.frame.is_none() || viewport.last_route_reason.is_some() {
            let rejection = serde_json::json!({"passed":false,"phase":name,"frame_present":viewport.frame.is_some(),"route_reason":viewport.last_route_reason,"scope":"Actual shared UI route refusal; no native frame acceptance"});
            std::fs::write(
                root.join("refusal.json"),
                serde_json::to_vec_pretty(&rejection).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            return Err(format!(
                "Material frame {name} was not accepted: {:?}",
                viewport.last_route_reason
            ));
        }
        painter.update_textures(&metal, &output.textures_delta)?;
        world_viewport::install(&mut painter, &metal, app)?;
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        let paint = painter.prepare(&metal, &primitives, output.pixels_per_point)?;
        painter.free_textures(&output.textures_delta.free);
        let mut job = painter.submit_diagnostic(&metal, paint, WIDTH, HEIGHT, false)?;
        let rgba = job.wait_readback(Duration::from_secs(5))?;
        std::fs::write(root.join(format!("{name}.rgba")), rgba).map_err(|e| e.to_string())?;
        let viewport = app.native_metal_world.as_ref().ok_or("Bridge retired")?;
        let rect = app
            .native_gpu_viewport_rect
            .ok_or("Viewport layout absent")?;
        let matrix = app.camera.projection_matrix() * app.camera.view_matrix();
        let sample = DVec3::new(0., -0.2, 0.);
        let red = ingetrazo_render::pipeline::project_point(sample, &matrix, rect)
            .ok_or("Classic observation outside projection")?
            .0;
        let old =
            ingetrazo_render::pipeline::project_point(DVec3::new(0., -0.2, 0.), &matrix, rect)
                .ok_or("Old observation outside projection")?
                .0;
        observations.push(serde_json::json!({"name":name,"native":app.native_gpu_viewport_enabled,"frame_present":viewport.frame.is_some(),"route_reason":viewport.last_route_reason,"selection_count":app.scene.selection.len(),"rect":[rect.min.x,rect.min.y,rect.width(),rect.height()],"red_pixel":[red.x,red.y],"old_pixel":[old.x,old.y],"catalog_builds":viewport.catalog_builds(),"model_refreshes":viewport.model_refreshes(),"matrix_bytes":viewport.matrix_bytes(),"geometry_uploads":viewport.geometry_uploads(),"texture_uploads":viewport.texture_uploads(),"material_order_builds":viewport.material_order_builds()}));
        std::fs::write(
            root.join("observations.json"),
            serde_json::to_vec_pretty(&observations).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    };
    for case in recipe["cases"].as_array().ok_or("Recipe cases missing")? {
        let name = case["name"].as_str().ok_or("Recipe name missing")?;
        let mut positions = vec![
            DVec3::new(-0.6, -0.6, 0.),
            DVec3::new(0.6, -0.6, 0.),
            DVec3::new(0., 0.6, 0.),
        ];
        if case["reverse"] == true {
            positions.reverse();
        }
        let mut mesh = Mesh::new();
        let ids = positions.into_iter().map(|p| mesh.add_vertex(p)).collect();
        let face = mesh
            .add_face(ids, vec![])
            .ok_or("Recipe triangle invalid")?;
        for (key, value) in case["attrs"].as_object().ok_or("Recipe attrs missing")? {
            let mut value = value.clone();
            if key == "texture" {
                value["path"] = serde_json::json!("quadrants");
            }
            mesh.faces[face].attrs.insert(key.clone(), value);
        }
        app.scene.groups.clear();
        if name.starts_with("group-") {
            let material: serde_json::Map<String, serde_json::Value> =
                mesh.faces[face].attrs.drain().collect();
            let mut group = Group::with_mesh("material", mesh);
            group.native_document =
                serde_json::json!({"xform":DMat4::IDENTITY.to_cols_array(),"material":material});
            if name == "group-reflected-texture" {
                group.transform = DMat4::from_scale(DVec3::new(-1., 1., 1.));
            }
            app.scene.mesh = Mesh::new();
            app.scene.groups.push(group);
        } else {
            app.scene.mesh = mesh;
        }
        app.style.show_textures = case["mode"] != "shaded";
        app.bump_model_revision();
        render(&mut app, name, vec![])?;
        assert!(
            app.native_gpu_viewport_enabled,
            "Material route did not activate: {name}"
        );
        render(&mut app, &format!("{name}-warm"), vec![])?;
        render(&mut app, &format!("{name}-warm2"), vec![])?;
    }
    drop(render);
    let builds = app.native_metal_world.as_ref().unwrap().catalog_builds();
    drop(app);
    drop(painter);
    let budget = metal.budget.lock().map_err(|_| "Budget poisoned")?;
    let committed: u64 = budget.heaps.iter().map(|h| h.committed).sum();
    let reserved: u64 = budget.heaps.iter().map(|h| h.reserved).sum();
    assert_eq!((committed, reserved), (0, 0));
    let report = serde_json::json!({"passed":true,"width":WIDTH,"height":HEIGHT,"frames":observations,"catalog_builds":builds,"ledger_committed":committed,"ledger_reserved":reserved,"scope":"Actual shared update_ui + viewport route + production host ingress/native Metal UI readback; constructor-authored source material/color/alpha/front/back/UV/image mean recipes through production notification hooks, no OS gesture/window/presentation/full-world claim"});
    std::fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
