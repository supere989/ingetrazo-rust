use glam::{DMat4, DVec3};
use ingetrazo_core::{dimensions::Dimension, mesh::Mesh, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
fn save(app: &IngeTrazoApp, dir: &std::path::Path, name: &str) {
    ingetrazo_formats::igz::save_igz(&app.scene, &dir.join(format!("{name}.igz"))).unwrap();
}
fn points(app: &IngeTrazoApp, a: DVec3, b: DVec3) {
    assert_eq!(app.scene.dimension_points(&app.scene.dimensions[0]), (a, b));
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    if std::env::args().nth(2).as_deref() == Some("--preview") {
        dimension_preview(&dir);
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--admission") {
        admission(&dir);
        return;
    }
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::igz::load_igz(&dir.join("source.igz")).unwrap();
    let vertex = app
        .scene
        .mesh
        .vertices
        .iter()
        .position(|v| v.position == DVec3::X)
        .unwrap();
    app.save_undo_state();
    app.scene.mesh.vertices[vertex].position = DVec3::new(3., 0., 0.);
    app.scene.refresh_dimensions();
    points(&app, DVec3::ZERO, DVec3::new(3., 0., 0.));
    save(&app, &dir, "moved");
    app.scene.select(SelectionItem::Edge(0));
    app.make_group();
    app.scene.refresh_dimensions();
    assert_eq!(
        app.scene.dimensions[0].anchor_b.as_ref().unwrap().path,
        vec![app.scene.groups[0].id]
    );
    let grouped = app.scene.clone();
    app.save_undo_state();
    app.scene.groups[0].transform = DMat4::from_scale_rotation_translation(
        DVec3::splat(2.),
        glam::DQuat::IDENTITY,
        DVec3::new(10., 20., 30.),
    );
    app.scene.refresh_dimensions();
    points(&app, DVec3::new(10., 20., 30.), DVec3::new(16., 20., 30.));
    let transformed = app.scene.clone();
    app.undo();
    assert_eq!(app.scene, grouped);
    app.redo();
    assert_eq!(app.scene, transformed);
    save(&app, &dir, "scaled");
    app.explode_group();
    app.scene.refresh_dimensions();
    points(&app, DVec3::new(10., 20., 30.), DVec3::new(16., 20., 30.));
    assert!(app.scene.dimensions[0]
        .anchor_b
        .as_ref()
        .unwrap()
        .path
        .is_empty());
    save(&app, &dir, "exploded");
    app.scene.mesh = Mesh::new();
    app.scene.refresh_dimensions();
    assert!(app.scene.dimensions[0].anchor_b.is_none());
    points(&app, DVec3::new(10., 20., 30.), DVec3::new(16., 20., 30.));
    save(&app, &dir, "deleted");
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::igz::load_igz(&dir.join("context-source.igz")).unwrap(); // Normalize loader-era mesh identities before the exact model Undo baseline.
    app.scene.migrate_legacy_mesh_shares().unwrap();
    app.scene.select(SelectionItem::Group(0));
    let before = app.scene.clone();
    app.begin_component_edit();
    assert_eq!(app.component_edit_stack.len(), 1);
    assert!(
        !app.scene
            .mesh
            .vertices
            .iter()
            .any(|v| v.position == DVec3::X),
        "Original placed leaf entry exposes world geometry"
    );
    let vertex = app
        .scene
        .mesh
        .vertices
        .iter()
        .position(|v| v.position == DVec3::new(11., 20., 30.))
        .unwrap();
    app.scene.mesh.vertices[vertex].position = DVec3::new(14., 20., 30.);
    app.scene.refresh_dimensions();
    let mut d = Dimension::new(
        DVec3::new(10., 20., 30.),
        DVec3::new(14., 20., 30.),
        DVec3::new(0., 2., 0.),
    );
    d.text_override = Some("Local <>".into());
    app.scene.dimensions.push(d);
    app.scene.bind_dimensions();
    app.finish_component_edit();
    assert!(app.component_edit_stack.is_empty());
    points(&app, DVec3::new(10., 20., 30.), DVec3::new(11., 20., 30.));
    save(&app, &dir, "context");
    let after = app.scene.clone();
    app.undo();
    assert_eq!(app.scene.groups, before.groups);
    assert_eq!(app.scene.mesh_shares, before.mesh_shares);
    assert_eq!(app.scene.dimensions.len(), 2);
    assert_eq!(app.scene.dimensions[1].end, DVec3::new(14., 20., 30.));
    assert!(app.scene.dimensions[1].anchor_b.is_none());
    app.redo();
    assert_eq!(app.scene.groups, after.groups);
    assert_eq!(app.scene.mesh_shares, after.mesh_shares);
    assert_eq!(app.scene.dimensions[1].end, DVec3::new(14., 20., 30.));
    assert!(app.scene.dimensions[1].anchor_b.is_none());
    assert!(!app.edit_dimension(0, None, Some("invalid".into()), None));
    let before = app.scene.clone();
    assert!(app.edit_dimension(
        0,
        Some("Edited <>".into()),
        Some("x".into()),
        Some("Survey".into())
    ));
    app.undo();
    assert_eq!(app.scene, before);
    assert!(app.remove_dimension(0));
    app.undo();
    assert_eq!(app.scene, before);
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::igz::load_igz(&dir.join("outliner-source.igz")).unwrap();
    let id = app.scene.groups[0].children[0].children[0].id;
    let before = app.scene.clone();
    app.set_outliner_visibility(id, false);
    assert!(!app.select_outliner_group(id, false));
    app.undo();
    assert_eq!(app.scene, before);
    assert!(app.select_outliner_group(id, false));
    assert_eq!(app.component_edit_stack.len(), 2);
    assert_eq!(app.scene.groups[0].id, id);
    assert!(app.scene.selection.contains(&SelectionItem::Group(0)));
    app.rename_outliner_group(id, "Edited leaf".into());
    app.begin_component_edit();
    assert!(
        !app.scene
            .mesh
            .vertices
            .iter()
            .any(|v| v.position == DVec3::X),
        "Nested entry must retain original world bake"
    );
    let vertex = app
        .scene
        .mesh
        .vertices
        .iter()
        .position(|v| v.position == DVec3::new(112., 222., 333.))
        .unwrap();
    app.scene.mesh.vertices[vertex].position = DVec3::new(117., 222., 333.);
    app.finish_component_edit();
    app.finish_component_edit();
    app.finish_component_edit();
    save(&app, &dir, "outliner");
    app.undo();
    let parent = &app.scene.groups[0];
    let middle = &parent.children[0];
    let leaf = &middle.children[0];
    assert_eq!(parent.transform, DMat4::IDENTITY);
    assert_eq!(middle.transform, DMat4::IDENTITY);
    assert_eq!(
        leaf.transform,
        DMat4::from_translation(DVec3::new(111., 222., 333.))
    );
    assert_eq!(leaf.id, id);
    assert_eq!(
        leaf.name, "Edited leaf",
        "Original mesh shareback undo preserves preceding rename"
    );
    assert!(leaf.mesh.vertices.iter().any(|v| v.position == DVec3::X));
    points(
        &app,
        DVec3::new(111., 222., 333.),
        DVec3::new(112., 222., 333.),
    );
    app.undo();
    assert_eq!(app.scene.groups[0].children[0].children[0].name, "Leaf");
    app.redo();
    assert_eq!(
        app.scene.groups[0].children[0].children[0].name,
        "Edited leaf"
    );
    app.redo();
    points(
        &app,
        DVec3::new(111., 222., 333.),
        DVec3::new(112., 222., 333.),
    );
}

fn admission(dir: &std::path::Path) {
    use ingetrazo_tools::base::{ToolContext, ToolType};
    use serde_json::{json, Value};
    let source: Value =
        serde_json::from_slice(&std::fs::read(dir.join("admission-original.json")).unwrap())
            .unwrap();
    let mut rows = Vec::new();
    for c in source["cases"].as_array().unwrap() {
        let case = c["case"].as_str().unwrap();
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&dir.join(format!("{case}-input.igz"))).unwrap();
        let record = |a: &mut IngeTrazoApp, phase: &str| {
            a.scene.refresh_dimensions();
            json!({"case":case,"phase":phase,"dimensions":a.scene.dimensions.iter().map(|d|json!({"a":d.start.to_array(),"b":d.end.to_array(),"offset":d.offset.to_array(),"axis":d.axis,"anchored":[d.anchor_a.is_some(),d.anchor_b.is_some()]})).collect::<Vec<_>>(),"selection":a.scene.selection.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
        };
        rows.push(record(&mut a, "before"));
        a.set_tool(ToolType::Dimension);
        for p in c["clicks"].as_array().unwrap() {
            let p = DVec3::new(
                p[0].as_f64().unwrap(),
                p[1].as_f64().unwrap(),
                p[2].as_f64().unwrap(),
            );
            a.dispatch_tool_click(&ToolContext::new(
                p,
                ingetrazo_core::math::Ray::new(DVec3::new(50., 50., 50.), -DVec3::Z),
            ));
        }
        rows.push(record(&mut a, "create"));
        a.undo();
        rows.push(record(&mut a, "create-undo"));
        a.redo();
        rows.push(record(&mut a, "create-redo"));
        a.scene.select(if case == "near" || case == "outside" {
            SelectionItem::Edge(0)
        } else {
            SelectionItem::Group(0)
        });
        a.set_tool(ToolType::Move);
        a.dispatch_tool_click(&ToolContext::new(
            DVec3::ZERO,
            ingetrazo_core::math::Ray::new(DVec3::new(50., 50., 50.), -DVec3::Z),
        ));
        let m = ToolContext::new(
            DVec3::new(1., 2., 0.),
            ingetrazo_core::math::Ray::new(DVec3::new(50., 50., 50.), -DVec3::Z),
        );
        a.dispatch_tool_move(&m);
        a.dispatch_tool_click(&m);
        rows.push(record(&mut a, "move"));
        a.undo();
        rows.push(record(&mut a, "undo"));
        a.redo();
        rows.push(record(&mut a, "redo"));
        a.save_to_path(&dir.join(format!("{case}-native.igz")));
        assert!(dir.join(format!("{case}-native.igz")).exists());
    }
    std::fs::write(
        dir.join("admission-native.json"),
        serde_json::to_vec_pretty(&json!({"cases":source["cases"],"states":rows})).unwrap(),
    )
    .unwrap();
}

fn dimension_preview(dir: &std::path::Path) {
    use ingetrazo_tools::base::{ToolContext, ToolType};
    use serde_json::{json, Value};
    let source: Value =
        serde_json::from_slice(&std::fs::read(dir.join("preview-original.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    for c in source["cases"].as_array().unwrap() {
        let case = c["case"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.scene =
            ingetrazo_formats::igz::load_igz(&dir.join(format!("{case}-input.igz"))).unwrap();
        if case == "nested" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        }
        app.camera.target = DVec3::ZERO;
        app.camera.distance = 20.;
        app.camera.yaw = c["yaw"].as_f64().unwrap();
        app.camera.pitch = c["pitch"].as_f64().unwrap();
        app.camera.perspective = false;
        app.camera.aspect = 800. / 600.;
        app.set_tool(ToolType::Dimension);
        let context = |name: &str| {
            let p = &c[name];
            ToolContext::new(
                DVec3::new(
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                ),
                ingetrazo_core::math::Ray::new(DVec3::new(50., 50., 50.), -DVec3::Z),
            )
        };
        let record = |app: &IngeTrazoApp, phase: &str| {
            let label = app.active_tool.distance_label().map(|(_, p)| {
                let plan = ingetrazo_gui::annotation_plan_for_probe(
                    app,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
                )
                .unwrap();
                let texts: Vec<_> = plan
                    .operations
                    .iter()
                    .filter_map(|o| {
                        if let ingetrazo_render::annotations::Primitive::Text {
                            text, color, ..
                        } = &o.primitive
                        {
                            if *color == [15, 20, 27, 255] {
                                Some(text.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(texts.len(), 1);
                json!({"text":texts[0],"point":p.to_array()})
            });
            json!({"case":case,"phase":phase,"label":label,"lines":app.active_tool.rubber_band_lines().iter().map(|(a,b)|[a.to_array(),b.to_array()]).collect::<Vec<_>>(),"dimensions":app.scene.dimensions.iter().map(|d|json!({"a":d.start.to_array(),"b":d.end.to_array(),"offset":d.offset.to_array(),"axis":d.axis})).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()})
        };
        app.dispatch_tool_move(&context("a"));
        rows.push(record(&app, "before"));
        app.dispatch_tool_click(&context("a"));
        rows.push(record(&app, "first"));
        app.dispatch_tool_move(&context("b"));
        rows.push(record(&app, "span"));
        app.dispatch_tool_click(&context("b"));
        rows.push(record(&app, "second"));
        app.dispatch_tool_move(&context("cursor"));
        rows.push(record(&app, "preview"));
        app.dispatch_tool_move(&context("cursor"));
        rows.push(record(&app, "same"));
        app.dispatch_tool_cancel();
        rows.push(record(&app, "cancel"));
        app.dispatch_tool_move(&context("a"));
        app.dispatch_tool_click(&context("a"));
        app.dispatch_tool_move(&context("b"));
        app.dispatch_tool_click(&context("b"));
        app.dispatch_tool_move(&context("cursor"));
        app.dispatch_tool_click(&context("cursor"));
        rows.push(record(&app, "commit"));
        app.undo();
        rows.push(record(&app, "undo"));
        app.redo();
        rows.push(record(&app, "redo"));
        if case == "nested" {
            while !app.component_edit_stack.is_empty() {
                app.finish_component_edit();
            }
        }
        app.save_to_path(&dir.join(format!("{case}-native.igz")));
        assert!(dir.join(format!("{case}-native.igz")).exists());
    }
    std::fs::write(
        dir.join("preview-native.json"),
        serde_json::to_vec_pretty(&json!({"cases":source["cases"],"states":rows})).unwrap(),
    )
    .unwrap();
}
