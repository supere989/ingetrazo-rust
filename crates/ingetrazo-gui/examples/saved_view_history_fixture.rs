use ingetrazo_core::{groups::Group, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::json;
fn rows(gs: &[Group]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for g in gs {
        out.push(json!([g.name, g.hidden]));
        out.extend(rows(&g.children));
    }
    out
}
fn rec(a: &IngeTrazoApp, label: &str, out: &mut Vec<serde_json::Value>) {
    out.push(json!({"label":label,"groups":rows(&a.scene.groups),"distance":a.camera.distance,"target":a.camera.target.to_array(),"edges":a.scene.mesh.edges.len(),"vertices":a.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edge_endpoints":a.scene.mesh.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"views":a.scene.saved_views.iter().map(|v|&v.name).collect::<Vec<_>>(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()}));
}
fn main() {
    let d = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut a = IngeTrazoApp::new();
    a.scene = ingetrazo_formats::igz::load_igz(&d.join("input.igz")).unwrap();
    a.camera.distance = 11.;
    let mut out = Vec::new();
    a.add_edge_command(glam::DVec3::ZERO, glam::DVec3::X, None, None)
        .unwrap();
    a.apply_saved_view(0);
    rec(&a, "geometry-apply", &mut out);
    a.undo();
    rec(&a, "geometry-undo", &mut out);
    a.redo();
    rec(&a, "geometry-redo", &mut out);
    a.camera.distance = 38.;
    a.selected_saved_view = Some(0);
    a.update_selected_saved_view();
    a.rename_selected_saved_view("updated".into());
    a.apply_saved_view(1);
    rec(&a, "update-rename-apply", &mut out);
    a.undo();
    rec(&a, "updated-geometry-undo", &mut out);
    a.redo();
    rec(&a, "updated-geometry-redo", &mut out);
    a.scene.select(SelectionItem::Group(0));
    a.hide_selection_command();
    a.apply_saved_view(0);
    rec(&a, "hide-then-recall", &mut out);
    a.undo();
    rec(&a, "hide-undo", &mut out);
    a.redo();
    rec(&a, "hide-redo", &mut out);
    a.apply_saved_view(1);
    a.undo();
    rec(&a, "hide-second-undo", &mut out);
    a.redo();
    rec(&a, "hide-second-redo", &mut out);
    a.scene.select(SelectionItem::Group(0));
    a.hide_selection_command();
    a.apply_saved_view(1);
    rec(&a, "already-hidden-recall", &mut out);
    a.undo();
    rec(&a, "already-hidden-undo", &mut out);
    a.redo();
    rec(&a, "already-hidden-redo", &mut out);
    a.apply_saved_view(0);
    a.undo();
    rec(&a, "already-hidden-later-recall-undo", &mut out);
    a.redo();
    rec(&a, "already-hidden-later-redo", &mut out);
    a.unhide_all();
    a.apply_saved_view(0);
    rec(&a, "unhide-recall", &mut out);
    a.undo();
    rec(&a, "unhide-undo", &mut out);
    a.redo();
    rec(&a, "unhide-redo", &mut out);
    a.apply_saved_view(0);
    a.undo();
    rec(&a, "unhide-second-undo", &mut out);
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Group(0));
    a.delete_selection();
    a.apply_saved_view(1);
    rec(&a, "delete-recall", &mut out);
    a.undo();
    rec(&a, "delete-undo", &mut out);
    a.redo();
    rec(&a, "delete-redo", &mut out);
    a.camera.distance = 90.;
    a.undo();
    rec(&a, "manual-camera-undo", &mut out);
    a.redo();
    rec(&a, "manual-camera-redo", &mut out);
    a.selected_saved_view = Some(1);
    a.remove_selected_saved_view();
    a.undo();
    rec(&a, "panel-delete-undo", &mut out);
    a.redo();
    rec(&a, "panel-delete-redo", &mut out);
    a.undo();
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    assert_eq!(a.component_edit_stack.len(), 2);
    a.add_edge_command(
        glam::DVec3::new(2., 0., 0.),
        glam::DVec3::new(3., 0., 0.),
        None,
        None,
    )
    .unwrap();
    a.camera.target = glam::DVec3::new(7., 8., 9.);
    a.camera.distance = 23.;
    rec(&a, "nested-context-camera", &mut out);
    a.undo();
    rec(&a, "nested-context-camera-undo", &mut out);
    a.redo();
    rec(&a, "nested-context-camera-redo", &mut out);
    println!("{}", json!(out));
}
