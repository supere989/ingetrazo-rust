use glam::{DMat4, DVec3};
use ingetrazo_core::{
    composition::Sheet,
    georef::{GeoPath, SurfaceFillMode, TerrainObject},
    groups::Group,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::app::IngeTrazoApp;
fn state(a: &IngeTrazoApp) -> serde_json::Value {
    serde_json::json!({"views":a.scene.saved_views.iter().map(|v|(&v.name,v.distance)).collect::<Vec<_>>(),"sheets":a.scene.sheets.iter().map(|s|&s.name).collect::<Vec<_>>(),"terrain":a.scene.terrain.as_ref().map(|t|t.visible),"path":a.scene.geo_paths.iter().map(|p|{let mut v=serde_json::json!({"points":p.points.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"name":p.name});if p.closed{v["closed"]=true.into();}if p.surface.is_some(){v["surface"]="flat".into();}v}).collect::<Vec<_>>()})
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", out.join("config"));
    let mut rows = serde_json::Map::new();
    for folded in [false, true] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        let mut m = Mesh::new();
        let v = m.add_vertex(DVec3::ZERO);
        let w = m.add_vertex(DVec3::X);
        m.add_edge(v, w);
        let mut g = Group::with_mesh("Leaf", m);
        g.transform = DMat4::from_translation(DVec3::new(10., 20., 30.));
        a.scene.groups.push(g);
        let mut sheet = Sheet::default();
        sheet.name = "Sheet".into();
        a.scene.sheets.push(sheet);
        a.scene.terrain = Some(TerrainObject {
            history_id: uuid::Uuid::new_v4(),
            vertices: vec![DVec3::ZERO, DVec3::X, DVec3::Y],
            uvs: vec![(0., 0.), (1., 0.), (0., 1.)],
            triangles: vec![(0, 1, 2)],
            tile_range: (0, 0, 1, 1, 0),
            nx: 0,
            ny: 0,
            bbox: (0., 0., 0., 0.),
            visible: true,
        });
        a.scene.geo_paths.push(GeoPath::new(
            vec![DVec3::new(2., 3., 4.), DVec3::new(5., 6., 7.)],
            false,
            "Route",
        ));
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        let entry = state(&a);
        if folded {
            a.save_undo_state();
            a.scene.mesh.vertices[1].position.x += 3.;
            a.scene.bump_version();
        }
        a.save_undo_state();
        a.scene.geo_paths[0].points[1] = DVec3::new(8., 9., 10.);
        a.scene.bump_version();
        a.save_undo_state();
        a.scene.geo_paths[0].surface = Some(SurfaceFillMode::Flat);
        a.scene.geo_paths[0].closed = true;
        a.scene.bump_version();
        a.add_saved_view("Scene 1".into());
        a.camera.distance = 33.;
        a.update_selected_saved_view();
        a.scene.sheets[0].name = "Edited sheet".into();
        a.set_terrain_visibility(false);
        let before = a.undo_stack.len();
        a.finish_component_edit();
        assert!(a.component_edit_stack.is_empty());
        let mut states = vec![state(&a)];
        for _ in 0..if folded { 1 } else { 2 } {
            a.undo();
            states.push(state(&a));
        }
        for _ in 0..if folded { 1 } else { 2 } {
            a.redo();
            states.push(state(&a));
        }
        rows.insert(
            if folded { "folded" } else { "ordinary" }.into(),
            serde_json::json!({"entry":entry,"states":states,"history_before_finish":before}),
        );
    }
    let mut a = IngeTrazoApp::new();
    a.scene = Scene::new();
    a.save_undo_state();
    a.scene
        .saved_views
        .push(ingetrazo_core::saved_views::SavedView::new("Imported"));
    a.scene.bump_version();
    a.selected_saved_view = Some(0);
    a.rename_selected_saved_view("Renamed".into());
    a.remove_selected_saved_view();
    let views = |a: &IngeTrazoApp| {
        a.scene
            .saved_views
            .iter()
            .map(|v| serde_json::json!([v.name, v.distance]))
            .collect::<Vec<_>>()
    };
    let mut states = vec![views(&a)];
    a.undo();
    states.push(views(&a));
    a.redo();
    states.push(views(&a));
    a.undo();
    a.add_saved_view("Scene 1".into());
    let branch = a.redo_stack.len();
    a.redo();
    states.push(views(&a));
    rows.insert(
        "import_object_lifetime".into(),
        serde_json::json!({"states":states,"redo_retained_after_ui_add":branch}),
    );
    let terrain = |z: f64| TerrainObject {
        history_id: uuid::Uuid::new_v4(),
        vertices: vec![
            DVec3::new(0., 0., z),
            DVec3::new(1., 0., z),
            DVec3::new(0., 1., z),
        ],
        uvs: vec![(0., 0.), (1., 0.), (0., 1.)],
        triangles: vec![(0, 1, 2)],
        tile_range: (0, 0, 1, 1, 0),
        nx: 0,
        ny: 0,
        bbox: (0., 0., 0., 0.),
        visible: true,
    };
    let mut a = IngeTrazoApp::new();
    a.scene = Scene::new();
    a.scene.terrain = Some(terrain(1.));
    a.save_undo_state();
    a.scene.terrain = None;
    a.scene.bump_version();
    let ts = |a: &IngeTrazoApp| {
        a.scene
            .terrain
            .as_ref()
            .map(|t| serde_json::json!([t.vertices[0].z, t.visible]))
    };
    let mut states = vec![ts(&a)];
    a.publish_runtime_terrain(Some(terrain(9.)));
    a.set_terrain_visibility(false);
    states.push(ts(&a));
    a.undo();
    states.push(ts(&a));
    a.redo();
    states.push(ts(&a));
    a.publish_runtime_terrain(Some(terrain(5.)));
    a.set_terrain_visibility(false);
    a.undo();
    states.push(ts(&a));
    a.redo();
    states.push(ts(&a));
    rows.insert(
        "terrain_owner_runtime_rebuild".into(),
        serde_json::json!(states),
    );
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
