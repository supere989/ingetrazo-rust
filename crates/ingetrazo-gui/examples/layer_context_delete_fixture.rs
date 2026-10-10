use glam::DVec3;
use ingetrazo_core::{
    groups::Group,
    layers::{Layer, DEFAULT_LAYER},
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", out.join("config"));
    let mut rows = serde_json::Map::new();
    for (nested, linked) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        a.scene.layers.push(Layer::new("Tagged"));
        let x = a.scene.mesh.add_vertex(DVec3::new(10., 0., 0.));
        let y = a.scene.mesh.add_vertex(DVec3::new(11., 0., 0.));
        let e = a.scene.mesh.add_edge(x, y).unwrap();
        a.scene.mesh.edges[e].layer = Some("Tagged".into());
        let mut mesh = Mesh::new();
        let v = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        let f = mesh.add_face(v.to_vec(), vec![]).unwrap();
        mesh.faces[f].layer = Some("Tagged".into());
        mesh.faces[f].attrs.insert("layer".into(), "Tagged".into());
        let g = Group::with_mesh("Inside", mesh);
        let id = g.id;
        a.scene.groups.push(g);
        a.scene.make_component(id).unwrap();
        let sibling = if linked {
            Some(a.scene.groups[0].duplicate_instance())
        } else {
            None
        };
        if nested {
            let child = a.scene.groups.remove(0);
            let mut parent = Group::new("Parent");
            parent.children.push(child);
            a.scene.groups.push(parent);
        }
        if let Some(sibling) = sibling {
            a.scene.groups.push(sibling);
        }
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        if nested {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.delete_layer(1);
        let mut states = vec![
            serde_json::json!({"layers":a.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"inside":a.scene.mesh.faces[0].layer.as_deref().unwrap_or(DEFAULT_LAYER),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()}),
        ];
        let state = |a: &IngeTrazoApp| serde_json::json!({"layers":a.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"outside":a.scene.mesh.edges[0].layer.as_deref().unwrap_or(DEFAULT_LAYER),"inside":if nested{a.scene.groups[0].children[0].mesh.faces[0].layer.as_deref().unwrap_or(DEFAULT_LAYER)}else{a.scene.groups[0].mesh.faces[0].layer.as_deref().unwrap_or(DEFAULT_LAYER)},"sibling":if linked{Some(a.scene.groups[1].mesh.faces[0].layer.as_deref().unwrap_or(DEFAULT_LAYER))}else{None},"undo":a.undo_stack.len(),"redo":a.redo_stack.len()});
        a.finish_component_edit();
        if nested {
            a.finish_component_edit();
        }
        states.push(state(&a));
        a.undo();
        states.push(state(&a));
        a.redo();
        states.push(state(&a));
        rows.insert(
            format!(
                "{}{}",
                if nested { "nested_" } else { "" },
                if linked { "linked" } else { "single" }
            ),
            serde_json::json!(states),
        );
    }
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
