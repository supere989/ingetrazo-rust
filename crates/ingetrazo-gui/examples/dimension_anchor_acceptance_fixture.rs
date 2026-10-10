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
    assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.mesh_shares,before.mesh_shares);
    assert_eq!(app.scene.dimensions.len(),2);assert_eq!(app.scene.dimensions[1].end,DVec3::new(14.,20.,30.));assert!(app.scene.dimensions[1].anchor_b.is_none());
    app.redo();
    assert_eq!(app.scene.groups,after.groups);assert_eq!(app.scene.mesh_shares,after.mesh_shares);assert_eq!(app.scene.dimensions[1].end,DVec3::new(14.,20.,30.));assert!(app.scene.dimensions[1].anchor_b.is_none());
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
