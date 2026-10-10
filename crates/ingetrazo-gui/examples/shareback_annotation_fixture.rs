use glam::{DMat4, DVec3};
use ingetrazo_core::{
    dimensions::Dimension,
    groups::Group,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
fn state(app: &IngeTrazoApp) -> serde_json::Value {
    serde_json::json!({"mesh":app.scene.groups[0].mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"dimensions":app.scene.dimensions.iter().map(|d| {let(a,b)=app.scene.dimension_points(d);serde_json::json!({"a":a.to_array(),"b":b.to_array(),"offset":d.offset.to_array(),"text":d.text_override,"axis":d.axis,"anchor_a":d.anchor_a.is_some(),"anchor_b":d.anchor_b.is_some()})}).collect::<Vec<_>>()})
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = IngeTrazoApp::new();
    app.scene = Scene::new();
    let mut mesh = Mesh::new();
    let a = mesh.add_vertex(DVec3::ZERO);
    let b = mesh.add_vertex(DVec3::X);
    mesh.add_edge(a, b);
    let mut group = Group::with_mesh("Translated", mesh);
    group.transform = DMat4::from_translation(DVec3::new(10., 20., 30.));
    app.scene.groups.push(group);
    app.scene.dimensions.push(Dimension::new(
        DVec3::new(10., 20., 30.),
        DVec3::new(11., 20., 30.),
        DVec3::Y,
    ));
    app.scene.bind_dimensions();
    app.scene.select(SelectionItem::Group(0));
    app.begin_component_edit();
    app.save_undo_state();
    let vertex = app
        .scene
        .mesh
        .vertices
        .iter()
        .position(|v| v.position == DVec3::new(11., 20., 30.))
        .unwrap();
    app.scene.mesh.vertices[vertex].position.x = 14.;
    app.scene.refresh_dimensions();
    app.save_undo_state();
    let mut dimension = Dimension::new(
        DVec3::new(10., 20., 30.),
        DVec3::new(14., 20., 30.),
        DVec3::new(0., 2., 0.),
    );
    dimension.text_override = Some("Local <>".into());
    let id = dimension.id;
    app.scene.dimensions.push(dimension);
    app.scene.bind_dimensions();
    app.finish_component_edit();
    let mut rows = serde_json::Map::new();
    rows.insert("finish".into(), state(&app));
    ingetrazo_formats::igz::save_igz(&app.scene, &dir.join("finish.igz")).unwrap();
    for i in 0..2 {
        app.undo();
        if let Some(d) = app.scene.dimensions.get(1) {
            assert_eq!(d.id, id);
        }
        rows.insert(format!("undo{i}"), state(&app));
        ingetrazo_formats::igz::save_igz(&app.scene, &dir.join(format!("undo{i}.igz"))).unwrap();
        app.redo();
        rows.insert(format!("redo{i}"), state(&app));
        ingetrazo_formats::igz::save_igz(&app.scene, &dir.join(format!("redo{i}.igz"))).unwrap();
    }
    let mut only = IngeTrazoApp::new();
    only.scene = Scene::new();
    let mut m = Mesh::new();
    let a = m.add_vertex(DVec3::ZERO);
    let b = m.add_vertex(DVec3::X);
    m.add_edge(a, b);
    let mut g = Group::with_mesh("Annotation only", m);
    g.transform = DMat4::from_translation(DVec3::new(10., 20., 30.));
    only.scene.groups.push(g);
    only.scene.select(SelectionItem::Group(0));
    only.begin_component_edit();
    only.save_undo_state();
    let mut d = Dimension::new(
        DVec3::new(10., 20., 30.),
        DVec3::new(11., 20., 30.),
        DVec3::Y * 2.,
    );
    d.text_override = Some("Only <>".into());
    only.scene.dimensions.push(d);
    only.scene.bind_dimensions();
    only.finish_component_edit();
    assert!(only.undo_stack.last().unwrap().history_shareback.is_none());
    let mut counts = vec![only.scene.dimensions.len()];
    only.undo();
    counts.push(only.scene.dimensions.len());
    only.redo();
    counts.push(only.scene.dimensions.len());
    rows.insert("annotation_only_history".into(), serde_json::json!(counts));
    let mut section = IngeTrazoApp::new();
    section.scene = Scene::new();
    let mut m = Mesh::new();
    let a = m.add_vertex(DVec3::ZERO);
    let b = m.add_vertex(DVec3::X);
    m.add_edge(a, b);
    let mut g = Group::with_mesh("Section", m);
    g.transform = DMat4::from_translation(DVec3::new(10., 20., 30.));
    section.scene.groups.push(g);
    section.scene.select(SelectionItem::Group(0));
    section.begin_component_edit();
    section.save_undo_state();
    section.scene.mesh.vertices[1].position.x = 14.;
    section.save_undo_state();
    section
        .scene
        .section_planes
        .push(ingetrazo_core::section::SectionPlane::new(
            DVec3::new(10., 20., 30.),
            DVec3::Z,
            "Section",
            "A",
        ));
    section.finish_component_edit();
    let mut counts = vec![section.scene.section_planes.len()];
    section.undo();
    counts.push(section.scene.section_planes.len());
    section.redo();
    counts.push(section.scene.section_planes.len());
    rows.insert("section_retained".into(), serde_json::json!(counts));
    let reopened = ingetrazo_formats::igz::load_igz(&dir.join("redo1.igz")).unwrap();
    assert!(reopened.history_shareback.is_none());
    assert_eq!(reopened.dimensions[1].end, DVec3::new(14., 20., 30.));
    assert!(reopened.dimensions[1].anchor_b.is_some());
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
