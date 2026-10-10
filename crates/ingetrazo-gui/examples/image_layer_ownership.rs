use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group, image_plane::ImagePlane, layers::Layer, mesh::Mesh, scene::SelectionItem,
};
use ingetrazo_gui::IngeTrazoApp;
fn state(a: &IngeTrazoApp) -> serde_json::Value {
    serde_json::json!({"layers":a.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"planes":a.scene.image_planes.iter().map(|p|serde_json::json!({"name":p.name,"layer":p.layer,"origin":p.origin})).collect::<Vec<_>>(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
}
fn main() {
    let mut rows = serde_json::Map::new();
    for case in ["rename", "existing_redo", "delete", "nested_delete"] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene.layers[0].name = "Layer 0".into();
        let mut im = ImagePlane::new("scan.png", DVec3::ZERO, DVec3::X, DVec3::Y);
        im.name = "New".into();
        let id = im.id;
        let mut states;
        if case == "delete" || case == "nested_delete" {
            a.scene.layers.push(Layer::new("Images"));
            a.scene.image_planes.push(im);
            let mut m = Mesh::new();
            m.add_vertex(DVec3::ZERO);
            m.add_vertex(DVec3::X);
            m.add_vertex(DVec3::Y);
            m.add_face(vec![0, 1, 2], vec![]);
            let mut g = Group::new("Leaf");
            g.mesh = m;
            g.transform = DMat4::IDENTITY;
            if case == "nested_delete" {
                let mut p = Group::new("Outer");
                p.children.push(g);
                a.scene.groups.push(p);
                a.scene.select(SelectionItem::Group(0));
                a.begin_component_edit();
            } else {
                a.scene.groups.push(g);
            }
            a.scene.clear_selection();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.move_image_planes(&[id], DVec3::new(2., 3., 4.)).unwrap();
            let i = a
                .scene
                .layers
                .iter()
                .position(|l| l.name == "Images")
                .unwrap();
            a.delete_layer(i);
            states = vec![state(&a)];
            a.finish_component_edit();
            if case == "nested_delete" {
                a.finish_component_edit();
            }
            states.push(state(&a));
            a.undo();
            states.push(state(&a));
            a.redo();
            states.push(state(&a));
        } else {
            a.add_image_plane(im);
            states = vec![state(&a)];
            if case == "rename" {
                let i = a
                    .scene
                    .layers
                    .iter()
                    .position(|l| l.name == "Images")
                    .unwrap();
                a.rename_layer(i, "Renamed".into());
                states.push(state(&a));
                a.undo();
                states.push(state(&a));
                a.redo();
                states.push(state(&a));
                a.undo();
                states.push(state(&a));
            } else {
                a.undo();
                states.push(state(&a));
                a.scene.layers.push(Layer::new("Images"));
                states.push(state(&a));
                a.redo();
                states.push(state(&a));
                a.undo();
                states.push(state(&a));
                a.redo();
                states.push(state(&a));
            }
        }
        rows.insert(case.into(), serde_json::json!(states));
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
