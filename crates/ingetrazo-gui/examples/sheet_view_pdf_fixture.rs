use ingetrazo_core::{
    composition::{Orientation, PaperSize, RectMm, ShapeItem, Sheet, SheetItem, ViewFrame},
    sheet_shape::SheetShape,
};
use ingetrazo_gui::{composer_compute::ComposerCompute, IngeTrazoApp};
// Boundary equality is independent of triangulation fan order. The paper source
// f64 clipper changes chord/ring start order versus the viewport f32 cap API.
fn material_boundary(triangles: &[[glam::DVec3; 3]]) -> Vec<[[f64; 3]; 2]> {
    let mut pairs = Vec::new();
    for triangle in triangles {
        for i in 0..3 {
            let mut pair = [triangle[i].to_array(), triangle[(i + 1) % 3].to_array()];
            pair.sort_by(|a, b| a.partial_cmp(b).unwrap());
            pairs.push(pair);
        }
    }
    pairs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut boundary = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        let mut j = i + 1;
        while j < pairs.len() && pairs[j] == pairs[i] {
            j += 1;
        }
        if (j - i) % 2 == 1 {
            boundary.push(pairs[i]);
        }
        i = j;
    }
    boundary
}
fn canonical_rings(rings: &[Vec<glam::DVec3>]) -> Vec<Vec<[f64; 3]>> {
    let mut result = Vec::new();
    for ring in rings {
        let points = ring.iter().map(|p| p.to_array()).collect::<Vec<_>>();
        let mut variants = Vec::new();
        for reverse in [false, true] {
            for start in 0..points.len() {
                variants.push(
                    (0..points.len())
                        .map(|i| {
                            points[if reverse {
                                (start + points.len() - i) % points.len()
                            } else {
                                (start + i) % points.len()
                            }]
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
        variants.sort_by(|a, b| a.partial_cmp(b).unwrap());
        result.push(variants.remove(0));
    }
    result.sort_by(|a, b| a.partial_cmp(b).unwrap());
    result
}
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
                hidden_lines: hidden,
                annotations: false,
                section_marks: false,
                view_key: reference["paper_view_key"]
                    .as_str()
                    .unwrap_or("__current__")
                    .into(),
                cut_fill: reference["paper_cut_fill"]
                    .as_str()
                    .unwrap_or("none")
                    .into(),
                scale_n: reference["paper_scale_n"].as_f64().unwrap_or(100.),
                cut_hatch_mm: reference["paper_cut_hatch_mm"].as_f64().unwrap_or(1.5),
                cut_fill_color: reference["paper_cut_fill_color"]
                    .as_str()
                    .unwrap_or("#595e69")
                    .into(),
                border: true,
                pen_edge_mm: if hidden { 0.37 } else { 0.18 },
                pen_profile_mm: if hidden { 0.62 } else { 0.35 },
                pen_cut_mm: if hidden { 0.83 } else { 0.5 },
                ..Default::default()
            };
            sheet.native_items.insert(
                frame.id,
                serde_json::json!({"border_mm":0.43,"border_color":"#246891","grid_m":reference.get("paper_grid_m").and_then(|v| v.as_f64()).unwrap_or(0.),"z":0.}),
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
        // Test-only application copy independently checks the borrowed production
        // adapter against the model's saved-view application contract.
        let mut observation_scene = app.scene.clone();
        if let Some(name) = reference["paper_view_key"]
            .as_str()
            .and_then(|s| s.strip_prefix("scene:"))
        {
            let saved = app
                .scene
                .saved_views
                .iter()
                .find(|s| s.name == name)
                .unwrap();
            let mut camera = app.camera.clone();
            saved.apply_scene(&mut observation_scene, &mut camera);
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
        let (_, observed) = ingetrazo_render::vector_hlr::original_qt_paper_observation(
            &observation_scene,
            &app.camera,
        );
        let source_hard = operative["geometry"][1].as_array().unwrap();
        if observation_scene.active_section().is_none() {
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
        let paper = app
            .composer_compute
            .original_qt_paper_drawing(
                &app.scene,
                &app.camera,
                app.clipboard_document,
                app.model_revision,
                "__current__",
                true,
                false,
            )
            .unwrap();
        if app.scene.active_section().is_none_or(|p| {
            p.normal
                .abs()
                .to_array()
                .iter()
                .filter(|&&v| v != 0.)
                .count()
                == 1
        }) {
            assert_eq!(
                material_boundary(&paper.section_material),
                material_boundary(&ingetrazo_render::vector_hlr::paper_section_triangles(
                    &app.scene
                )),
                "cached admission material {name}, source loops {:?}",
                paper.section_loops
            );
        }
        let cache_jobs = app.composer_compute.completed_jobs;
        let warm = app
            .composer_compute
            .original_qt_paper_drawing(
                &app.scene,
                &app.camera,
                app.clipboard_document,
                app.model_revision,
                "__current__",
                true,
                false,
            )
            .unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&paper, &warm),
            "warm admission returns the complete cached record, not rebuilt caps"
        );
        assert_eq!(cache_jobs, app.composer_compute.completed_jobs);
        if !paper.section_material.is_empty() {
            let previous = app.scene.sheets.clone();
            for sheet in &mut app.scene.sheets {
                for item in &mut sheet.items {
                    if let SheetItem::ViewFrame(frame) = item {
                        frame.cut_fill = "hatch".into();
                        frame.cut_hatch_mm = f64::NAN;
                    }
                }
            }
            let sentinel = root.join(format!("{name}.invalid-hatch.pdf"));
            std::fs::write(&sentinel, b"section admission sentinel").unwrap();
            let before_jobs = app.composer_compute.completed_jobs;
            let error = app.export_composer_pdf(None, &sentinel).unwrap_err();
            assert!(error.contains("hatch step"), "{error}");
            assert_eq!(before_jobs, app.composer_compute.completed_jobs);
            assert_eq!(
                std::fs::read(&sentinel).unwrap(),
                b"section admission sentinel"
            );
            std::fs::remove_file(sentinel).unwrap();
            app.scene.sheets = previous;
        }
        if let Some(name) = reference["paper_view_key"]
            .as_str()
            .and_then(|s| s.strip_prefix("scene:"))
        {
            let saved = app
                .scene
                .saved_views
                .iter()
                .find(|v| v.name == name)
                .unwrap();
            let view = ingetrazo_render::paper_scene::PaperSceneView::new(&app.scene, Some(saved));
            let record = app
                .composer_compute
                .original_qt_paper_drawing_for_view(
                    &view,
                    &app.camera,
                    app.clipboard_document,
                    app.model_revision,
                    name,
                    true,
                    false,
                )
                .unwrap();
            let source_caps =
                ingetrazo_render::vector_hlr::paper_section_triangles(&observation_scene);
            if observation_scene.active_section().is_none_or(|p| {
                p.normal
                    .abs()
                    .to_array()
                    .iter()
                    .filter(|&&v| v != 0.)
                    .count()
                    == 1
            }) {
                assert_eq!(
                    material_boundary(&record.section_material),
                    material_boundary(&source_caps)
                );
            }
            let mut ghosts = saved.clone();
            ghosts.hidden_shown = Some(serde_json::json!({"objects":false,"geometry":false}));
            let ghost_view =
                ingetrazo_render::paper_scene::PaperSceneView::new(&app.scene, Some(&ghosts));
            let unchanged = app
                .composer_compute
                .original_qt_paper_drawing_for_view(
                    &ghost_view,
                    &app.camera,
                    app.clipboard_document,
                    app.model_revision,
                    name,
                    true,
                    false,
                )
                .unwrap();
            assert!(
                std::sync::Arc::ptr_eq(&record, &unchanged),
                "ghost presentation flags do not alter paper visibility or cache identity"
            );
            let original_jobs = app.composer_compute.completed_jobs;
            let mut altered = saved.clone();
            altered.hidden_layers = app.scene.layers.iter().map(|l| l.name.clone()).collect();
            let altered_view =
                ingetrazo_render::paper_scene::PaperSceneView::new(&app.scene, Some(&altered));
            let changed = app
                .composer_compute
                .original_qt_paper_drawing_for_view(
                    &altered_view,
                    &app.camera,
                    app.clipboard_document,
                    app.model_revision,
                    name,
                    true,
                    false,
                )
                .unwrap();
            assert_eq!(
                app.composer_compute.completed_jobs,
                original_jobs + 1,
                "saved visibility must not reuse old ink at unchanged model revision"
            );
            assert!(
                changed.segments.is_empty(),
                "hidden layers remain hidden despite ghost flags"
            );
            assert!(!std::sync::Arc::ptr_eq(&record, &changed));
            let restored = app
                .composer_compute
                .original_qt_paper_drawing_for_view(
                    &view,
                    &app.camera,
                    app.clipboard_document,
                    app.model_revision,
                    name,
                    true,
                    false,
                )
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(&record, &restored));
            assert_eq!(app.composer_compute.completed_jobs, original_jobs + 1);
            assert_eq!(
                app.scene, before,
                "borrowed saved-view computations cannot mutate scene"
            );
        }
        if let Some(source_loops) = operative.get("world_section_loops") {
            let expected = source_loops
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    r.as_array()
                        .unwrap()
                        .iter()
                        .map(|p| {
                            glam::DVec3::from_array(std::array::from_fn(|i| p[i].as_f64().unwrap()))
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let key = reference["paper_view_key"]
                .as_str()
                .unwrap_or("__current__");
            let saved = key
                .strip_prefix("scene:")
                .and_then(|n| app.scene.saved_views.iter().find(|s| s.name == n));
            let view = ingetrazo_render::paper_scene::PaperSceneView::new(&app.scene, saved);
            let record = app
                .composer_compute
                .original_qt_paper_drawing_for_view(
                    &view,
                    &app.camera,
                    app.clipboard_document,
                    app.model_revision,
                    key,
                    true,
                    false,
                )
                .unwrap();
            assert_eq!(
                canonical_rings(&record.section_loops),
                canonical_rings(&expected),
                "actual original WORLD section loops {name}; no coordinate tolerance"
            );
        }
        rows.push(serde_json::json!({"name":name,"pages":2,"warm_cache_no_jobs":true,"worker_pids":pids,"scene_unchanged":true,"cache_policy_separation":true,"cached_section_admission":true,"operative_camera_basis":true,"operative_hard_world_exact":app.scene.active_section().is_none()}));
    }
    assert_eq!(rows.len(), 13);
    std::fs::write(
        root.join("native-controls.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
