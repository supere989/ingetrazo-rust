use glam::DVec3;
use ingetrazo_core::{image_plane::ImagePlane, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use std::path::Path;
fn state(a: &IngeTrazoApp) -> serde_json::Value {
    let planes: Vec<_> = a
        .scene
        .image_planes
        .iter()
        .map(|p| {
            let mut v = p.to_json();
            v["texture"]["path"] = serde_json::json!("scan.png");
            v
        })
        .collect();
    let mut selected_images: Vec<_> = a
        .scene
        .selection
        .iter()
        .filter_map(|item| {
            if let SelectionItem::ImagePlane(i) = item {
                a.scene.image_planes.get(*i).map(|p| p.name.clone())
            } else {
                None
            }
        })
        .collect();
    selected_images.sort();
    serde_json::json!({"selected_images":selected_images,"planes":planes,"layers":a.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", out.join("config"));
    let mut rows = serde_json::Map::new();
    for (nested, folded) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.open_document(&input.join(if nested {
            "nested-input.igz"
        } else {
            "direct-input.igz"
        }));
        if nested {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        let mut states = vec![state(&a)];
        let id = a.scene.image_planes[0].id;
        a.move_image_planes(&[id], DVec3::new(1., 2., 3.)).unwrap();
        a.set_image_plane_opacity(id, 0.375).unwrap();
        let mut new = ImagePlane::new(
            a.scene.image_planes[0].path.clone(),
            DVec3::new(8., 9., 10.),
            DVec3::new(2., 0., 0.),
            DVec3::Y,
        );
        new.name = "New".into();
        new.aspect = 0.5;
        a.add_image_plane(new);
        states.push(state(&a));
        if folded {
            a.save_undo_state();
            let i = a
                .scene
                .mesh
                .vertices
                .iter()
                .position(|v| v.position == DVec3::new(10., 0., 0.))
                .unwrap();
            a.scene.mesh.vertices[i].position += DVec3::new(2., 0., 0.);
            a.scene.mesh.rebuild_spatial_index();
            a.scene.bump_version();
        }
        a.finish_component_edit();
        if nested {
            a.finish_component_edit();
        }
        states.push(state(&a));
        for _ in 0..if folded { 4 } else { 3 } {
            a.undo();
            states.push(state(&a));
        }
        for _ in 0..if folded { 4 } else { 3 } {
            a.redo();
            states.push(state(&a));
        }
        a.scene.clear_selection();
        let index = a
            .scene
            .image_planes
            .iter()
            .position(|p| p.id == id)
            .unwrap();
        a.scene.select(SelectionItem::ImagePlane(index));
        a.set_image_plane_locked(id, false);
        states.push(state(&a));
        a.delete_image_planes(&[id]);
        states.push(state(&a));
        a.undo();
        states.push(state(&a));
        a.redo();
        states.push(state(&a));
        let key = format!(
            "{}{}",
            if nested { "nested_" } else { "" },
            if folded { "folded" } else { "ordinary" }
        );
        a.save_to_path(&out.join(format!("{key}.igz")));
        rows.insert(key, serde_json::json!(states));
    }
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
