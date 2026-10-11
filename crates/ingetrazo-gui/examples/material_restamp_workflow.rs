// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual App material/history/save consumer; egui controls tested separately.
use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group,
    materials::Material,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn mesh() -> Mesh {
    let mut m = Mesh::new();
    let ids =
        [DVec3::ZERO, DVec3::X * 2., DVec3::new(2., 1., 0.), DVec3::Y].map(|p| m.add_vertex(p));
    let f = m.add_face(ids.to_vec(), vec![]).unwrap();
    m.faces[f].attrs=serde_json::from_value(json!({"mat":"Oak","color":[0.1,0.2,0.3],"texture":{"path":"old.png","sw":2.,"sh":3.,"uvw":[1,0,0,0,0,1,0,0]},"opacity":0.4,"back":{"mat":"Oak","color":[0.8,0.7,0.6]},"vendor":{"id":71},"ifc":{"class":"IfcSlab","id":9}})).unwrap();
    m
}
fn groups(gs: &[Group]) -> Value {
    json!(gs.iter().map(|g|json!({"name":g.name,"faces":g.mesh.faces.iter().map(|f|&f.attrs).collect::<Vec<_>>(),"material":g.native_document.get("material"),"children":groups(&g.children)})).collect::<Vec<_>>())
}
fn state(app: &IngeTrazoApp, phase: &str) -> Value {
    let loose = app.material_loose_mesh();
    json!({"phase":phase,"registry":app.scene.materials.iter().map(|(n,m)|(n.clone(),m.registry_recipe())).collect::<serde_json::Map<String,Value>>(),"active":app.scene.mesh.faces.iter().map(|f|&f.attrs).collect::<Vec<_>>(),"loose":loose.faces.iter().map(|f|&f.attrs).collect::<Vec<_>>(),"groups":groups(&app.bim_document_groups())})
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    let mut cases = vec![];
    for case in ["root", "classic", "placed", "nested"] {
        eprintln!("Material case: {case}");
        let mut app = IngeTrazoApp::new();
        app.scene = Scene::new();
        app.scene.mesh = mesh();
        app.scene.materials.insert("Oak".into(),Material::from_recipe(json!({"name":"Oak","color":[0.1,0.2,0.3],"texture":{"path":"old.png","sw":2.,"sh":3.},"opacity":0.4,"finish":"wood"})).unwrap());
        let mut g = Group::with_mesh("target", mesh());
        g.native_document = json!({"material":{"mat":"Oak","color":[0.1,0.2,0.3],"vendor":72}});
        let token = uuid::Uuid::new_v4();
        app.scene
            .register_mesh_share(token, g.mesh.clone())
            .unwrap();
        g.mesh_share = Some(token);
        if case == "placed" || case == "nested" {
            g.transform = DMat4::from_translation(DVec3::new(4., 5., 0.));
        }
        let mut sibling = g.duplicate_instance();
        sibling.name = "sibling".into();
        if case == "nested" {
            let mut parent = Group::new("parent");
            parent.children.push(g);
            app.scene.groups = vec![parent, sibling];
        } else {
            app.scene.groups = vec![g, sibling];
        }
        if case != "root" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        }
        if case == "nested" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        }
        let mut phases = vec![state(&app, "before")];
        app.edit_named_material(json!({"name":"Oak","color":[0.6,0.5,0.4],"finish":"wood"}))
            .unwrap();
        phases.push(state(&app, "edited"));
        app.undo();
        assert_eq!(app.status_message, "Undo");
        phases.push(state(&app, "undo"));
        app.redo();
        phases.push(state(&app, "redo"));
        if case != "root" {
            while !app.component_edit_stack.is_empty() {
                app.finish_component_edit();
                assert!(
                    !app.status_message.contains("refused"),
                    "{}",
                    app.status_message
                );
            }
            phases.push(state(&app, "finished"));
            app.undo();
            phases.push(state(&app, "finished-undo"));
            app.redo();
            phases.push(state(&app, "finished-redo"));
        }
        app.save_to_path(&out.join(format!("{case}.igz")));
        assert!(
            out.join(format!("{case}.igz")).exists(),
            "{}",
            app.status_message
        );
        cases.push(json!({"case":case,"phases":phases}));
    }
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&json!({"cases":cases})).unwrap(),
    )
    .unwrap();
}
