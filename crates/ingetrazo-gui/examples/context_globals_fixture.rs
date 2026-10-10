use glam::{DMat4, DVec3};
use ingetrazo_core::{
    georef::{GeoPoint, SceneDatum},
    groups::Group,
    guides::{Guide, GuideType},
    mesh::Mesh,
    scene::{Scene, SelectionItem},
    textlabel::TextLabel,
};
use ingetrazo_gui::app::IngeTrazoApp;
fn state(s: &Scene) -> serde_json::Value {
    serde_json::json!({"guides":s.guides.iter().map(|g|match g.kind{GuideType::Point(p)=>serde_json::json!({"point":p.to_array()}),_=>panic!()}).collect::<Vec<_>>(),"texts":s.text_labels,"points":s.geo_points.iter().map(|p|serde_json::json!({"position":p.position.to_array(),"name":p.name,"desc":p.desc})).collect::<Vec<_>>(),"datum":s.geo_datum.as_ref().map(|d|[d.lat,d.lon,d.alt,d.north])})
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", out.join("config"));
    let mut rows = serde_json::Map::new();
    for (name, folded, nested) in [
        ("ordinary", false, false),
        ("folded", true, false),
        ("nested_ordinary", false, true),
        ("nested_folded", true, true),
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(DVec3::ZERO);
        let v1 = mesh.add_vertex(DVec3::X);
        mesh.add_edge(v0, v1);
        let mut group = Group::with_mesh("Leaf", mesh);
        group.transform = DMat4::from_translation(DVec3::new(10., 20., 30.));
        a.scene.groups.push(group);
        a.scene.guides.push(Guide::point(DVec3::new(2., 3., 4.)));
        a.scene.text_labels.push(TextLabel::new(
            DVec3::new(5., 6., 7.),
            DVec3::Y * 2.,
            "Existing",
        ));
        a.scene
            .geo_points
            .push(GeoPoint::new(DVec3::new(8., 9., 10.), "Existing", "Survey"));
        let before = state(&a.scene);
        if nested {
            let leaf = a.scene.groups.remove(0);
            let mut parent = Group::new("Parent");
            parent.children.push(leaf);
            a.scene.groups.push(parent);
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        let entry = state(&a.scene);
        assert_eq!(entry, before);
        if folded {
            a.save_undo_state();
            a.scene.mesh.vertices[1].position.x += 3.;
            a.scene.bump_version();
        }
        a.save_undo_state();
        a.scene.guides.push(Guide::point(DVec3::new(20., 21., 22.)));
        a.scene.bump_version();
        a.save_undo_state();
        a.scene.text_labels.push(TextLabel::new(
            DVec3::new(23., 24., 25.),
            DVec3::Y * 3.,
            "Added",
        ));
        a.scene.bump_version();
        a.save_undo_state();
        a.scene
            .geo_points
            .push(GeoPoint::new(DVec3::new(26., 27., 28.), "Added", "Survey"));
        a.scene.geo_datum = Some(SceneDatum::new(37., -122., 15., 20.));
        a.scene.bump_version();
        a.finish_component_edit();
        if nested {
            a.finish_component_edit();
        }
        assert!(a.component_edit_stack.is_empty(), "{}", a.status_message);
        let mut states = vec![state(&a.scene)];
        for _ in 0..if folded { 1 } else { 3 } {
            a.undo();
            states.push(state(&a.scene));
        }
        for _ in 0..if folded { 1 } else { 3 } {
            a.redo();
            states.push(state(&a.scene));
        }
        rows.insert(
            name.into(),
            serde_json::json!({"entry":entry,"states":states}),
        );
    }
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
