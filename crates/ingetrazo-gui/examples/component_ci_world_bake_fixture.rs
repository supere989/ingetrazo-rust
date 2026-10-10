use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
fn points(g: &Group) -> Vec<[f64; 3]> {
    g.mesh
        .vertices
        .iter()
        .map(|v| v.position.to_array())
        .collect()
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let mut rows = vec![];
    for case in source["cases"].as_array().unwrap() {
        let mut app = IngeTrazoApp::new();
        let mut leaf = Group::new("first");
        let mut mesh = Mesh::new();
        let verts = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(verts.to_vec(), vec![]).unwrap();
        leaf.mesh = mesh;
        let id = leaf.id;
        app.scene.groups.push(leaf);
        app.scene.make_component(id).unwrap();
        let other = app.scene.groups[0].duplicate_instance();
        let mut plain = app.scene.groups[0].duplicate_instance();
        plain.component_definition = None;
        plain.native_document = serde_json::json!({"component":false});
        let mut child = app.scene.groups[0].duplicate_instance();
        child.transform = DMat4::from_translation(DVec3::new(-2., 1., 0.));
        let mut parent = Group::new("parent");
        parent.children.push(child);
        let frame: [f64; 16] = serde_json::from_value(case["before"]["selected"].clone()).unwrap();
        parent.transform = DMat4::from_cols_array(&frame);
        let parent_id = parent.id;
        app.scene.groups.extend([other, plain, parent]);
        app.scene.make_component(parent_id).unwrap();
        let mut sibling = app.scene.groups[3].duplicate_instance();
        sibling.transform = DMat4::from_translation(DVec3::splat(10.)) * sibling.transform;
        app.scene.groups.push(sibling);
        app.scene.select(SelectionItem::Group(3));
        let before = app.scene.clone();
        let initial_version=app.scene.version;
        app.begin_component_edit();
        let parent_drawing_frame=app.scene.drawing_frame.unwrap();let combined = app.scene.groups[0].transform;
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        let child_drawing_frame=app.scene.drawing_frame.unwrap();let entry: Vec<_> = app
            .scene
            .mesh
            .vertices
            .iter()
            .map(|v| v.position.to_array())
            .collect();
        let target = combined.transform_point3(DVec3::new(0., 0., 5.));
        app.scene.mesh.vertices[0].position = target;
        app.finish_component_edit();
        app.finish_component_edit();
        assert_eq!(app.undo_stack.len(), 1);
        let committed = serde_json::json!({"selected":app.scene.groups[3].transform.to_cols_array(),"child":app.scene.groups[3].children[0].transform.to_cols_array(),"sibling":app.scene.groups[4].transform.to_cols_array(),"sibling_child":app.scene.groups[4].children[0].transform.to_cols_array(),"prototypes":[points(&app.scene.groups[0]),points(&app.scene.groups[1]),points(&app.scene.groups[2]),points(&app.scene.groups[3].children[0]),points(&app.scene.groups[4].children[0])],"history":app.undo_stack.len(),"shared":app.scene.groups.iter().take(3).all(|g|g.mesh_share==app.scene.groups[0].mesh_share)&&app.scene.groups[3].children[0].mesh_share==app.scene.groups[0].mesh_share&&app.scene.groups[4].children[0].mesh_share==app.scene.groups[0].mesh_share});
        for index in [3, 4] {
            assert_eq!(app.scene.groups[index].id, before.groups[index].id);
            assert_eq!(
                app.scene.groups[index].children[0].id,
                before.groups[index].children[0].id
            );
        }
        let committed_version=app.scene.version;assert!(app.scene.selection.is_empty());
        app.undo();let undo_version=app.scene.version;assert!(app.scene.selection.is_empty()&&undo_version>committed_version);
        let undone = serde_json::json!({"selected":app.scene.groups[3].transform.to_cols_array(),"child":app.scene.groups[3].children[0].transform.to_cols_array(),"points":points(&app.scene.groups[0])});
        app.redo();let redo_version=app.scene.version;assert!(app.scene.selection.is_empty()&&redo_version>undo_version);
        assert_eq!(points(&app.scene.groups[0])[0], [0., 0., 5.]);
        rows.push(serde_json::json!({"name":case["name"],"parent_drawing_frame":parent_drawing_frame.to_cols_array(),"child_drawing_frame":child_drawing_frame.to_cols_array(),"entry_world_points":entry,"authored_world_point":target.to_array(),"committed":committed,"undone":undone,"placement_ids_preserved":true,"history_presentation":{"initial_version":initial_version,"committed_version":committed_version,"undo_version":undo_version,"redo_version":redo_version,"selection_empty_after_commit_undo_redo":true}}));
    }
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
