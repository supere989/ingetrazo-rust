use ingetrazo_core::{
    composition::{Orientation, PaperSize, RectMm, ShapeItem, Sheet, SheetItem, ViewFrame},
    sheet_shape::SheetShape,
};
use ingetrazo_gui::{composer_compute::ComposerCompute, IngeTrazoApp};
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let worker = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    let mut rows = vec![];
    for entry in std::fs::read_dir(root.join("corpus")).unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".reference.json") {
            continue;
        }
        let reference: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let name = reference["name"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.scene =
            ingetrazo_formats::load_igz(&root.join("corpus").join(format!("{name}.igz"))).unwrap();
        app.camera = serde_json::from_value(reference["camera"].clone()).unwrap();
        app.scene.camera = app.camera.clone();
        app.composer_compute = ComposerCompute::with_worker(&worker);
        for hidden in [false, true] {
            let mut sheet = Sheet::new("View", PaperSize::A4, Orientation::Portrait);
            sheet.items.clear();
            let frame = ViewFrame {
                rect: RectMm::new(20., 30., 100., 100.),
                style: "vectorial".into(),
                view_key: "__current__".into(),
                hidden_lines: hidden,
                annotations: false,
                section_marks: false,
                cut_fill: "none".into(),
                border: true,
                pen_edge_mm: if hidden { 0.37 } else { 0.18 },
                pen_profile_mm: if hidden { 0.62 } else { 0.35 },
                pen_cut_mm: if hidden { 0.83 } else { 0.5 },
                ..Default::default()
            };
            sheet.native_items.insert(
                frame.id,
                serde_json::json!({"border_mm":0.43,"border_color":"#246891","z":0.}),
            );
            let behind = SheetShape {
                kind: "rect".into(),
                x_mm: 15.,
                y_mm: 25.,
                w_mm: 110.,
                h_mm: 110.,
                fill: true,
                fill_color: "#99aacc".into(),
                color: "#112233".into(),
                z: -1.,
                ..Default::default()
            };
            let over = SheetShape {
                kind: "linea".into(),
                x_mm: 20.,
                y_mm: 70.,
                w_mm: 100.,
                h_mm: 0.,
                color: "#dd3344".into(),
                stroke_mm: 0.8,
                z: 1.,
                ..Default::default()
            };
            sheet.items = vec![
                SheetItem::ViewFrame(frame),
                SheetItem::Shape(ShapeItem {
                    id: uuid::Uuid::new_v4(),
                    shape: behind,
                }),
                SheetItem::Shape(ShapeItem {
                    id: uuid::Uuid::new_v4(),
                    shape: over,
                }),
            ];
            app.scene.sheets.push(sheet);
        }
        let operative: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("{name}.operative-paper.json"))).unwrap(),
        )
        .unwrap();
        let basis = ingetrazo_render::vector_hlr::OriginalQtPaperBasis::from_camera(&app.camera);
        for (i, v) in [basis.eye, basis.right, basis.up, basis.forward]
            .iter()
            .enumerate()
        {
            for axis in 0..3 {
                assert!(
                    (v[axis] - operative["basis"][i][axis].as_f64().unwrap()).abs() < 2e-15,
                    "operative camera {name}"
                );
            }
        }
        let (_, observed) =
            ingetrazo_render::vector_hlr::original_qt_paper_observation(&app.scene, &app.camera);
        let source_hard = operative["geometry"][1].as_array().unwrap();
        if app.scene.active_section().is_none() {
            let mut native = observed
                .iter()
                .filter(|e| e.kind == ingetrazo_render::vector_hlr::LineKind::Edge)
                .map(|e| {
                    let mut pair = [e.a.to_array(), e.b.to_array()];
                    pair.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    pair
                })
                .collect::<Vec<_>>();
            let mut source = source_hard
                .iter()
                .map(|edge| {
                    let mut pair = [[0.; 3]; 2];
                    for p in 0..2 {
                        for axis in 0..3 {
                            pair[p][axis] = edge[p][axis].as_f64().unwrap();
                        }
                    }
                    pair.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    pair
                })
                .collect::<Vec<_>>();
            native.sort_by(|a, b| a.partial_cmp(b).unwrap());
            source.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(
                native, source,
                "actual viewport hard WORLD endpoint arrays {name}"
            );
            let mut native_soft = observed
                .iter()
                .filter(|e| e.kind == ingetrazo_render::vector_hlr::LineKind::Profile)
                .map(|e| {
                    let mut pair = [e.a.to_array(), e.b.to_array()];
                    pair.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    pair
                })
                .collect::<Vec<_>>();
            let mut source_soft = Vec::new();
            for (index, edge) in operative["geometry"][2]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let normals = &operative["geometry"][3][index];
                let vector = |index: usize| {
                    glam::DVec3::new(
                        normals[index][0].as_f64().unwrap(),
                        normals[index][1].as_f64().unwrap(),
                        normals[index][2].as_f64().unwrap(),
                    )
                };
                if (vector(0).dot(basis.forward) < 0.) == (vector(1).dot(basis.forward) < 0.) {
                    continue;
                }
                let mut pair = [[0.; 3]; 2];
                for p in 0..2 {
                    for axis in 0..3 {
                        pair[p][axis] = edge[p][axis].as_f64().unwrap();
                    }
                }
                pair.sort_by(|a, b| a.partial_cmp(b).unwrap());
                source_soft.push(pair);
            }
            native_soft.sort_by(|a, b| a.partial_cmp(b).unwrap());
            source_soft.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(
                native_soft, source_soft,
                "actual viewport soft silhouette WORLD endpoints {name}"
            );
        }
        // Admission failure after selecting a genuine supported View leaves the
        // destination and Scene untouched and does not start a fresh worker.
        let invalid_dest = root.join(format!("{name}.invalid.pdf"));
        std::fs::write(&invalid_dest, b"sentinel").unwrap();
        let mut invalid = app.scene.sheets[0].items.clone();
        if let SheetItem::ViewFrame(f) = &mut app.scene.sheets[0].items[0] {
            f.rect.w = f64::NAN;
        }
        assert!(app.export_composer_pdf(Some(0), &invalid_dest).is_err());
        assert_eq!(std::fs::read(&invalid_dest).unwrap(), b"sentinel");
        assert!(app.composer_compute.worker_ids().is_empty());
        std::mem::swap(&mut app.scene.sheets[0].items, &mut invalid);
        std::fs::remove_file(invalid_dest).unwrap();
        let before = app.scene.clone();
        let dest = root.join(format!("{name}.native.pdf"));
        assert_eq!(app.export_composer_pdf(None, &dest).unwrap(), 2);
        assert_eq!(app.scene, before);
        let pids = app.composer_compute.worker_ids();
        let jobs = app.composer_compute.completed_jobs;
        let bytes = std::fs::read(&dest).unwrap();
        assert_eq!(app.export_composer_pdf(None, &dest).unwrap(), 2);
        assert_eq!(bytes, std::fs::read(&dest).unwrap());
        assert_eq!(jobs, app.composer_compute.completed_jobs);
        assert_eq!(pids, app.composer_compute.worker_ids());
        assert_eq!(pids.len(), 2);
        let old = app.composer_compute.completed_jobs;
        let _ = app
            .composer_compute
            .segments(
                &app.scene,
                &app.camera,
                app.clipboard_document,
                app.model_revision,
                "__current__",
                true,
                false,
            )
            .unwrap();
        assert_eq!(
            app.composer_compute.completed_jobs,
            old + 1,
            "native policy must not reuse paper ink"
        );
        assert_eq!(app.composer_compute.worker_ids(), pids);
        let _ = app
            .composer_compute
            .segments(
                &app.scene,
                &app.camera,
                app.clipboard_document,
                app.model_revision,
                "__current__",
                true,
                false,
            )
            .unwrap();
        assert_eq!(app.composer_compute.completed_jobs, old + 1);
        let _ = app
            .composer_compute
            .original_qt_paper_segments(
                &app.scene,
                &app.camera,
                app.clipboard_document,
                app.model_revision,
                "__current__",
                true,
                false,
            )
            .unwrap();
        // The exporter key camera is source-frame adjusted; regardless of whether this
        // explicit raw-camera key is already present, a second identical policy is warm.
        let qt_jobs = app.composer_compute.completed_jobs;
        let _ = app
            .composer_compute
            .original_qt_paper_segments(
                &app.scene,
                &app.camera,
                app.clipboard_document,
                app.model_revision,
                "__current__",
                true,
                false,
            )
            .unwrap();
        assert_eq!(app.composer_compute.completed_jobs, qt_jobs);
        assert_eq!(app.composer_compute.worker_ids(), pids);
        rows.push(serde_json::json!({"name":name,"pages":2,"warm_cache_no_jobs":true,"worker_pids":pids,"scene_unchanged":true,"cache_policy_separation":true,"operative_camera_basis":true,"operative_hard_world_exact":app.scene.active_section().is_none()}));
    }
    assert_eq!(rows.len(), 13);
    std::fs::write(
        root.join("native-controls.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
