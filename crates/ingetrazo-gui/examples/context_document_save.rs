use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, scene::{Scene, SelectionItem}};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::ToolType;
use serde_json::{json, Value};
use std::path::PathBuf;
fn state(s: &Scene) -> Value {
    fn points(mesh: &ingetrazo_core::mesh::Mesh, m: DMat4) -> Vec<[f64;3]> {
        let mut pts=mesh.vertices.iter().map(|v|m.transform_point3(v.position).to_array()).collect::<Vec<_>>();
        pts.sort_by(|a,b|a.partial_cmp(b).unwrap());pts
    }
    fn visit(g: &Group, parent: DMat4, rows: &mut Vec<Value>) {
        let m=parent*g.transform;
        rows.push(json!({"name":g.name,"uid":g.native_uid(),"points":points(&g.mesh,m),"ext":g.native_document.get("ext").cloned().unwrap_or(Value::Null)}));
        for child in &g.children {visit(child,m,rows);}
    }
    let mut rows=vec![];for g in &s.groups {visit(g,DMat4::IDENTITY,&mut rows);}
    json!({"root_points":points(&s.mesh,DMat4::IDENTITY),"groups":rows})
}
fn main() {
    let out=PathBuf::from(std::env::args().nth(1).unwrap());
    let mut rows=vec![];
    for case in ["save","save_as_cancel","save_as","failed"] {
        let mut a=IngeTrazoApp::new();a.system_clipboard=None;
        a.open_document(&out.join("input.igz"));
        a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
        a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
        assert_eq!(a.component_edit_stack.len(),2);a.set_tool(ToolType::Rectangle);
        a.save_undo_state();for v in &mut a.scene.mesh.vertices {v.position.z+=2.;}
        a.camera.target=DVec3::new(7.,8.,9.);a.camera.distance=23.;
        let path=out.join(format!("{case}.native.igz"));
        let mut dialogs=vec![];
        match case {
            "save"=>{a.current_file=Some(path.clone());a.save_document();}
            "failed"=>{a.current_file=Some(out.join("missing/failed.igz"));a.save_document();}
            _=>{a.current_file=None;a.save_document_as();assert!(a.show_save_dialog);dialogs.push(a.component_edit_stack.len());
                if case=="save_as" {a.save_to_path(&path);}else{a.cancel_document_action();}
                a.show_save_dialog=false;
            }
        }
        let error=a.status_message.starts_with("Save failed:");
        assert_eq!(a.document_dirty(),case=="failed"||case=="save_as_cancel");
        let mut phases=vec![];
        for phase in ["after","undo","redo"] {
            if phase=="undo" {a.undo();}if phase=="redo" {a.redo();}
            phases.push(json!({"phase":phase,"state":state(&a.scene),"depth":a.component_edit_stack.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"tool":format!("{:?}",a.active_tool_type).to_lowercase(),"camera_target":a.camera.target.to_array(),"camera_distance":a.camera.distance}));
        }
        rows.push(json!({"case":case,"phases":phases,"file_exists":path.exists(),"error":error,"dialog_depths":dialogs}));
    }
    // Invalid edited geometry must never serialize a fragment or alter history.
    let mut a=IngeTrazoApp::new();a.system_clipboard=None;a.open_document(&out.join("input.igz"));
    a.scene.select(SelectionItem::Group(0));a.begin_component_edit();a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
    a.scene.mesh.vertices[0].position.x=f64::NAN;
    let depth=a.component_edit_stack.len();let undo=a.undo_stack.len();
    let path=out.join("invalid.native.igz");a.save_to_path(&path);
    assert!(!path.exists());assert_eq!(a.component_edit_stack.len(),depth);assert_eq!(a.undo_stack.len(),undo);assert!(a.scene.mesh.vertices[0].position.x.is_nan());
    std::fs::write(out.join("native.json"),serde_json::to_vec_pretty(&json!({"rows":rows,"invalid_refusal":true})).unwrap()).unwrap();
}
