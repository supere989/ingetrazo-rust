use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
fn target(groups: &[Group]) -> &Group {
    if groups[0].children.is_empty() {
        &groups[0]
    } else {
        target(&groups[0].children)
    }
}
fn main() {
    let source = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let dir = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
    let mut donor = IngeTrazoApp::new();
    donor.system_clipboard = None;
    donor.scene = ingetrazo_formats::igz::load_igz(&source.join("source-clipboard.igz")).unwrap();
    donor.scene.select(SelectionItem::Face(0));
    donor.copy_selection();
    let clipboard = donor.clipboard.clone().unwrap();
    assert!(!clipboard.resources.embedded_textures.is_empty());
    let mut cases = serde_json::Map::new();
    for nested in [false, true] {
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.scene = Scene::new();
        let mut mesh = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(), vec![]).unwrap();
        let mut leaf = Group::with_mesh("Target", mesh);
        leaf.transform = DMat4::from_translation(DVec3::X * 10.);
        if nested {
            let mut parent = Group::new("Parent");
            parent.transform = DMat4::from_translation(DVec3::X * 100.);
            parent.children.push(leaf);
            app.scene.groups.push(parent);
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        } else {
            app.scene.groups.push(leaf);
        }
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        app.clipboard = Some(clipboard.clone());
        let before = app.scene.clone();
        let depth = app.undo_stack.len();
        app.begin_paste_preview();
        assert!(app.paste_preview.is_some());
        assert_eq!(app.scene, before);
        app.cancel_paste_preview();
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack.len(), depth);
        app.begin_paste_preview();
        assert!(app.commit_paste_preview());
        assert!(!app.scene.embedded_textures.is_empty());
        app.finish_component_edit();
        if nested {
            app.finish_component_edit();
        }
        let label = if nested { "nested" } else { "direct" };
        let mut rows = vec![];
        for phase in ["finish", "undo", "redo"] {
            if phase == "undo" {
                app.undo();
            }
            if phase == "redo" {
                app.redo();
            }
            let group = target(&app.scene.groups);
            let faces = group
                .mesh
                .faces
                .iter()
                .filter(|f| f.attrs.contains_key("texture"))
                .collect::<Vec<_>>();
            rows.push(serde_json::json!({"phase":phase,"faces":group.mesh.faces.len(),"textured_faces":faces.len(),"asset_refs":faces.iter().map(|f|f.attrs["texture"]["embed"].clone()).collect::<Vec<_>>(),"uvw":faces.iter().map(|f|f.attrs["texture"]["uvw"].clone()).collect::<Vec<_>>(),"vendor":faces.iter().map(|f|f.attrs.get("vendor").cloned().unwrap_or_default()).collect::<Vec<_>>(),"live_asset_bytes":app.scene.embedded_textures.values().map(|v|v.len()).sum::<usize>()}));
            ingetrazo_formats::igz::save_igz(&app.scene, &dir.join(format!("{label}-{phase}.igz")))
                .unwrap();
        }
        cases.insert(label.into(), serde_json::json!(rows));
    }
    std::fs::write(dir.join("native.json"),serde_json::to_vec_pretty(&serde_json::json!({"cases":cases,"system_clipboard_disabled":true,"preview_cancel_verified":true})).unwrap()).unwrap();
}
