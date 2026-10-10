use glam::DVec3;
use ingetrazo_core::{
    composition::{SheetItem, ViewFrame},
    scene::Scene,
};
use ingetrazo_gui::{app::IngeTrazoApp, composer_compute::ComposerCompute};
fn paint(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000., 800.),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ingetrazo_gui::composer::render_sheet_composer(app, ui, 0)
            });
        },
    );
}
fn main() {
    let worker =
        std::path::PathBuf::from(std::env::args_os().nth(1).expect("standalone worker path"));
    let mut app = IngeTrazoApp::new();
    app.scene = Scene::new();
    app.composer_compute = ComposerCompute::with_worker(&worker);
    let ids = [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y]
        .map(|p| app.scene.mesh.add_vertex(p));
    app.scene.mesh.add_face(ids.to_vec(), vec![]);
    let mut frame = ViewFrame::default();
    frame.style = "vectorial".into();
    frame.view_key = "std:front".into();
    app.scene.sheets.push(Default::default());
    app.scene.sheets[0].items.clear();
    app.scene.sheets[0].items.push(SheetItem::ViewFrame(frame));
    let ctx = egui::Context::default();
    paint(&mut app, &ctx);
    let pids = app.composer_compute.worker_ids();
    assert_eq!(pids.len(), 2);
    let first = app.composer_compute.completed_jobs;
    assert_eq!(first, 1);
    for _ in 0..10 {
        paint(&mut app, &ctx);
    }
    assert_eq!(app.composer_compute.completed_jobs, first);
    assert_eq!(app.composer_compute.worker_ids(), pids);
    app.save_undo_state();
    let a = app.scene.mesh.add_vertex(DVec3::new(2., 0., 0.));
    let b = app.scene.mesh.add_vertex(DVec3::new(2., 1., 0.));
    app.scene.mesh.add_edge(a, b);
    app.scene.bump_version();
    paint(&mut app, &ctx);
    assert_eq!(app.composer_compute.completed_jobs, first + 1);
    app.scene.active_sheet_index = None;
    app.undo();
    paint(&mut app, &ctx);
    assert_eq!(app.composer_compute.completed_jobs, first + 2);
    assert_eq!(app.composer_compute.worker_ids(), pids);
    let mut failed =
        ComposerCompute::with_worker(std::path::Path::new("/missing/ingetrazo-render-worker"));
    assert!(failed
        .segments(
            &app.scene,
            &app.camera,
            app.clipboard_document,
            app.model_revision,
            "",
            true,
            true
        )
        .is_err());
    assert!(failed.worker_ids().is_empty());
    println!("PASS productioncomposer: persistent2PIDs,10cachehitsnoadditionaljobs,edit/undoinvalidation,explicitmissingworkerrefusal; metrics {:?}",app.composer_compute.metrics());
}
