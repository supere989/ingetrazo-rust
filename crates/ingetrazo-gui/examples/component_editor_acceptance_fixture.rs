use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::igz::load_igz(&dir.join("component-source.igz")).unwrap();
    let mut parent = Group::new("parent");
    let mut child = app.scene.groups[0].duplicate_instance();
    child.transform = DMat4::from_translation(DVec3::new(-2., 1., 0.));
    parent.children.push(child);
    parent.transform = DMat4::from_translation(DVec3::new(4., 5., 6.));
    let parentid = parent.id;
    app.scene.groups.push(parent);
    app.scene.make_component(parentid).unwrap();
    let mut copy = app.scene.groups[3].duplicate_instance();
    copy.transform = DMat4::from_translation(DVec3::new(14., 15., 16.));
    app.scene.groups.push(copy);
    app.scene.clear_selection();
    app.scene.select(SelectionItem::Group(3));
    app.scene.migrate_legacy_mesh_shares().unwrap();
    let before = app.scene.clone();
    ingetrazo_formats::igz::save_igz(&app.scene, &dir.join("editor-before.igz")).unwrap();
    // Source container entry bakes its transform outside command history.
    let mut baked = IngeTrazoApp::new();
    baked.scene = before.clone();
    baked.begin_component_edit();
    baked.finish_component_edit();
    assert!(baked.undo_stack.is_empty());
    let mut undo_before = baked.scene.clone();
    undo_before.selection = before.selection.clone();
    undo_before.selection_order = before.selection_order.clone();
    undo_before.version = before.version;
    undo_before.view_version = before.view_version;
    let child_world = before.groups[3].transform * before.groups[3].children[0].transform;
    app.begin_component_edit();
    app.scene.select(SelectionItem::Group(0));
    app.begin_component_edit();
    app.scene.mesh.vertices[0].position = child_world.transform_point3(DVec3::new(0., 0., 5.));
    app.finish_component_edit();
    app.finish_component_edit();
    assert_eq!(app.undo_stack.len(), 1);
    for index in [3, 4] {
        assert_eq!(
            app.scene.groups[index].children[0].id,
            before.groups[index].children[0].id
        );
        assert_eq!(
            app.scene.groups[index].transform,
            if index == 3 {
                DMat4::IDENTITY
            } else {
                before.groups[index].transform
            }
        );
        assert_eq!(
            app.scene.groups[index].children[0].transform,
            if index == 3 {
                child_world
            } else {
                before.groups[index].children[0].transform
            }
        );
    }
    let committed = app.scene.clone();
    app.save_to_path(&dir.join("editor-committed.igz"));
    app.undo();
    assert_eq!(app.scene, undo_before);
    app.save_to_path(&dir.join("editor-undone.igz"));
    app.redo();
    assert_eq!(app.scene, committed);
    let mut unique_parent_app = IngeTrazoApp::new();
    unique_parent_app.scene = app.scene.clone();
    unique_parent_app.scene.clear_selection();
    unique_parent_app.scene.select(SelectionItem::Group(4));
    unique_parent_app.make_component_unique();
    unique_parent_app.scene.clear_selection();
    unique_parent_app.scene.select(SelectionItem::Group(0));
    unique_parent_app.begin_component_edit();
    unique_parent_app.scene.mesh.vertices[0].position.z = 6.;
    unique_parent_app.finish_component_edit();
    unique_parent_app.save_to_path(&dir.join("editor-parent-unique.igz"));
    app.scene.clear_selection();
    app.scene.select(SelectionItem::Group(0));
    let cancelbefore = app.scene.clone();
    app.begin_component_edit();
    app.scene.mesh.vertices[0].position.z = 9.;
    let blocked = dir.join("fragment.igz");
    app.save_to_path(&blocked);
    assert!(!blocked.exists());
    app.cancel_component_edit();
    assert_eq!(app.scene, cancelbefore);
    app.make_component_unique();
    app.begin_component_edit();
    app.scene.mesh.vertices[0].position.z = 7.;
    app.finish_component_edit();
    app.save_to_path(&dir.join("editor-unique.igz"));
    app.scene.clear_selection();
    app.scene.select(SelectionItem::Group(0));
    app.copy_selection();
    app.paste_in_place();
    app.scene.clear_selection();
    app.scene.select(SelectionItem::Group(1));
    app.copy_selection();
    app.paste_in_place();
    app.save_to_path(&dir.join("editor-pasted.igz"));
}
