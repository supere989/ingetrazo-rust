//! Bounded actual shared application UI -> viewport route -> native paint readback.
//! Synthetic egui pointer delivery is explicit; no own-window/presentation claim.
use super::*;
use egui::{Event, PointerButton, Pos2};
use glam::{DMat4, DVec3};
use ingetrazo_core::{camera::StandardView, groups::Group, mesh::Mesh, scene::SelectionItem};
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
    let group = |name: &str, color: [f64; 3], x: f64| -> Result<Group, String> {
        let mut mesh = Mesh::new();
        let ids = [
            DVec3::new(-0.6, -0.6, 0.),
            DVec3::new(0.6, -0.6, 0.),
            DVec3::new(0., 0.6, 0.),
        ]
        .map(|p| mesh.add_vertex(p));
        let face = mesh
            .add_face(ids.to_vec(), vec![])
            .ok_or("Fixture triangle invalid")?;
        mesh.faces[face]
            .attrs
            .insert("color".into(), serde_json::json!(color));
        let mut g = Group::with_mesh(name, mesh);
        g.mesh_share = Some(g.id);
        g.component_definition = Some(g.id);
        g.native_document = serde_json::json!({"proto":0,"xform":DMat4::IDENTITY.to_cols_array()});
        g.transform = DMat4::from_translation(DVec3::new(x, 0., 0.));
        Ok(g)
    };
    app.scene.groups = vec![
        group("red", [1., 0., 0.], 0.)?,
        group("green", [0., 1., 0.], 2.)?,
    ];
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
        let red = ingetrazo_render::pipeline::project_point(
            app.scene.groups[0]
                .transform
                .transform_point3(DVec3::new(0., -0.2, 0.)),
            &matrix,
            rect,
        )
        .ok_or("Red observation outside projection")?
        .0;
        observations.push(serde_json::json!({"name":name,"native":app.native_gpu_viewport_enabled,"selection_count":app.scene.selection.len(),"rect":[rect.min.x,rect.min.y,rect.width(),rect.height()],"red_pixel":[red.x,red.y],"catalog_builds":viewport.catalog_builds(),"model_refreshes":viewport.model_refreshes(),"matrix_bytes":viewport.matrix_bytes(),"geometry_uploads":viewport.geometry_uploads()}));
        Ok(())
    };
    render(&mut app, "initial", vec![])?;
    render(&mut app, "warm1", vec![])?;
    render(&mut app, "warm2", vec![])?;
    let before = app.native_metal_world.as_ref().unwrap().matrix_bytes();
    let refresh = app.native_metal_world.as_ref().unwrap().model_refreshes();
    app.camera.target.x += 0.1;
    render(&mut app, "camera", vec![])?;
    assert_eq!(
        app.native_metal_world.as_ref().unwrap().matrix_bytes(),
        before
    );
    assert_eq!(
        app.native_metal_world.as_ref().unwrap().model_refreshes(),
        refresh
    );
    app.scene.groups[0].transform = DMat4::from_translation(DVec3::new(0.8, 0., 0.));
    app.bump_placement_revision();
    render(&mut app, "model", vec![])?;
    assert_eq!(
        app.native_metal_world.as_ref().unwrap().matrix_bytes() - before,
        64
    );
    let rect = app.native_gpu_viewport_rect.unwrap();
    let matrix = app.camera.projection_matrix() * app.camera.view_matrix();
    let hit = ingetrazo_render::pipeline::project_point(DVec3::new(0.8, -0.2, 0.), &matrix, rect)
        .unwrap()
        .0;
    let click = |pos: Pos2, pressed: bool| {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            },
        ]
    };
    render(&mut app, "select-press", click(hit, true))?;
    render(&mut app, "selected", click(hit, false))?;
    assert!(app.scene.selection.contains(&SelectionItem::Group(0)));
    assert!(!app.native_gpu_viewport_enabled);
    let blank = rect.right_top() + egui::vec2(-12., 12.);
    render(&mut app, "clear-press", click(blank, true))?;
    render(&mut app, "cleared", click(blank, false))?;
    assert!(app.scene.selection.is_empty());
    assert!(app.native_gpu_viewport_enabled);
    app.scene.groups[0].native_document["material"] = serde_json::json!({"opacity":0.5});
    app.bump_render_paint_revision();
    render(&mut app, "alpha", vec![])?;
    assert!(!app.native_gpu_viewport_enabled);
    app.scene.groups[0]
        .native_document
        .as_object_mut()
        .unwrap()
        .remove("material");
    app.bump_render_paint_revision();
    render(&mut app, "restored", vec![])?;
    assert!(app.native_gpu_viewport_enabled);
    drop(render);
    let builds = app.native_metal_world.as_ref().unwrap().catalog_builds();
    drop(app);
    drop(painter);
    let budget = metal.budget.lock().map_err(|_| "Budget poisoned")?;
    let committed: u64 = budget.heaps.iter().map(|h| h.committed).sum();
    let reserved: u64 = budget.heaps.iter().map(|h| h.reserved).sum();
    assert_eq!((committed, reserved), (0, 0));
    let report = serde_json::json!({"passed":true,"width":WIDTH,"height":HEIGHT,"frames":observations,"catalog_builds":builds,"ledger_committed":committed,"ledger_reserved":reserved,"scope":"Actual shared update_ui + viewport route + production host ingress/native Metal UI readback; synthetic egui pointer events, no OS pointer/window/presentation/full-world claim"});
    std::fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
