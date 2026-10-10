use glam::DVec3;
use ingetrazo_core::composition::{SheetItem, ViewFrame};
use ingetrazo_gui::{app::IngeTrazoApp, composer_compute::ComposerCompute};
fn paint(a: &mut IngeTrazoApp, c: &egui::Context) {
    let _ = c.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 900.),
            )),
            ..Default::default()
        },
        |c| {
            egui::CentralPanel::default().show(c, |ui| {
                ingetrazo_gui::composer::render_sheet_composer(a, ui, 0)
            });
        },
    );
}
fn verify(a: &mut IngeTrazoApp) -> Vec<usize> {
    let frames: Vec<_> = a.scene.sheets[0]
        .items
        .iter()
        .filter_map(|x| {
            if let SheetItem::ViewFrame(f) = x {
                Some(f.clone())
            } else {
                None
            }
        })
        .collect();
    let jobs = a.composer_compute.completed_jobs;
    let mut counts = vec![];
    for f in frames {
        let mut s = a.scene.clone();
        if let Some(name) = f.view_key.strip_prefix("scene:") {
            let v = s
                .saved_views
                .iter()
                .find(|v| v.name == name)
                .unwrap()
                .clone();
            let mut c = s.camera.clone();
            v.apply_scene(&mut s, &mut c);
        }
        let camera = ingetrazo_gui::composer::camera_for_frame(&a.scene, &f);
        let actual = a
            .composer_compute
            .segments(
                &s,
                &camera,
                a.clipboard_document,
                a.model_revision,
                &f.view_key,
                f.profiles,
                f.hidden_lines,
            )
            .unwrap();
        let expected = ingetrazo_render::vector_hlr::parallel_segments(
            &s,
            &camera,
            f.profiles,
            f.hidden_lines,
        );
        assert_eq!(actual.len(), expected.len());
        for (x, y) in actual.iter().zip(&expected) {
            assert_eq!(x.kind, y.kind);
            assert!(x.a.distance(y.a) < 1e-10 && x.b.distance(y.b) < 1e-10);
        }
        counts.push(actual.len());
    }
    assert_eq!(
        a.composer_compute.completed_jobs, jobs,
        "panel failed to populate exact frame cache"
    );
    counts
}
fn main() {
    let worker = std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
    let mut a = IngeTrazoApp::new();
    a.composer_compute = ComposerCompute::with_worker(&worker);
    let ids =
        [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| a.scene.mesh.add_vertex(p));
    a.scene.mesh.add_face(ids.to_vec(), vec![]);
    a.set_layer_visibility(0, false);
    a.add_saved_view("Hidden".into());
    a.set_layer_visibility(0, true);
    a.scene.sheets.push(Default::default());
    a.scene.sheets[0].items.clear();
    for key in ["__current__", "std:top", "scene:Hidden"] {
        let mut f = ViewFrame::default();
        f.style = "vectorial".into();
        f.view_key = key.into();
        f.cam_target = Some([0.5, 0.5, 0.]);
        f.cam_yaw = Some(0.7);
        f.cam_pitch = Some(0.6);
        f.cam_distance = Some(10.);
        a.scene.sheets[0].items.push(SheetItem::ViewFrame(f));
    }
    let c = egui::Context::default();
    paint(&mut a, &c);
    let first = verify(&mut a);
    assert!(first[0] > 0 && first[1] > 0 && first[2] == 0);
    let pids = a.composer_compute.worker_ids();
    assert_eq!(pids.len(), 2);
    let jobs = a.composer_compute.completed_jobs;
    for _ in 0..5 {
        paint(&mut a, &c);
        verify(&mut a);
    }
    assert_eq!(jobs, a.composer_compute.completed_jobs);
    let v = a.scene.view_version;
    assert!(!a.set_layer_visibility(0, true));
    assert_eq!(v, a.scene.view_version);
    a.set_layer_visibility(0, false);
    paint(&mut a, &c);
    assert_eq!(verify(&mut a), vec![0, 0, 0]);
    a.set_layer_visibility(0, true);
    paint(&mut a, &c);
    assert_eq!(verify(&mut a), first);
    a.scene.active_sheet_index = None;
    a.save_undo_state();
    let x = a.scene.mesh.add_vertex(DVec3::new(2., 0., 0.));
    let y = a.scene.mesh.add_vertex(DVec3::new(2., 1., 0.));
    a.scene.mesh.add_edge(x, y);
    a.scene.bump_version();
    paint(&mut a, &c);
    let edited = verify(&mut a);
    assert!(edited[0] > first[0]);
    a.scene.active_sheet_index = None;
    a.undo();
    paint(&mut a, &c);
    assert_eq!(verify(&mut a), first);
    assert_eq!(a.scene.sheets[0].items.len(), 3);
    a.scene.active_sheet_index = None;
    a.redo();
    paint(&mut a, &c);
    assert_eq!(verify(&mut a), edited);
    assert_eq!(a.composer_compute.worker_ids(), pids);
    println!("PASS independent production panel: 3 fixed-camera frames, saved hidden snapshot, exact serial endpoints/classes, 5 warm redraws, visibility/noop, model Undo/Redo with sheets retained; persistent PIDs {:?}, jobs {}",pids,a.composer_compute.completed_jobs);
}
