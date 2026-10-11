use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, scene::{Scene, SelectionItem}};
use ingetrazo_gui::IngeTrazoApp;
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

fn phase(a: &IngeTrazoApp, name: &str) -> Value {
    json!({"phase":name,"state":state(&a.scene),"depth":a.component_edit_stack.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"pending":a.paste_preview.is_some(),
    "reference":a.paste_preview.as_ref().map(|p|p.reference.to_array()),"offset":a.paste_preview.as_ref().map(|p|p.offset.to_array())})
}
fn setup(out: &std::path::Path) -> IngeTrazoApp {
    let mut a=IngeTrazoApp::new();a.system_clipboard=None;a.open_document(&out.join("input.igz"));
    a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
    a.scene.select(SelectionItem::Group(0));a.begin_component_edit();assert_eq!(a.component_edit_stack.len(),2);
    a.save_undo_state();for v in &mut a.scene.mesh.vertices {v.position.z+=2.;}
    a.scene.select(SelectionItem::Face(0));a.copy_selection();a.paste_selection();a.update_paste_preview(DVec3::new(30.,40.,50.));
    assert!(a.paste_preview.is_some());a
}
fn main() {
    let out=PathBuf::from(std::env::args().nth(1).unwrap());let mut rows=vec![];
    for save in ["save","save_as","save_as_cancel","failed"] {
        for finish in ["continue","cancel"] {
            let case=format!("{save}-{finish}");let mut a=setup(&out);
            let p=a.paste_preview.as_ref().unwrap();let owner=p.scene.mesh.vertices.as_ptr();
            let preview=p.scene.clone();let reference=p.reference;let offset=p.offset;
            let path=out.join(format!("{case}.native.igz"));let mut dialogs=vec![];
            match save {
                "save"=>{a.current_file=Some(path.clone());a.save_document();}
                "failed"=>{a.current_file=Some(out.join("missing/failed.igz"));a.save_document();}
                _=>{a.current_file=None;a.save_document_as();assert!(a.show_save_dialog);dialogs.push(a.component_edit_stack.len());
                    if save=="save_as" {a.save_to_path(&path);}else{a.cancel_document_action();}a.show_save_dialog=false;}
            }
            let p=a.paste_preview.as_ref().expect("Save discarded pending Paste");
            assert_eq!(p.scene.mesh.vertices.as_ptr(),owner,"Save cloned the immutable preview owner");
            assert_eq!(p.scene,preview);assert_eq!(p.reference,reference);assert_eq!(p.offset,offset);
            assert_eq!(a.document_dirty(),save=="failed"||save=="save_as_cancel");
            let error=a.status_message.starts_with("Save failed:");let before=state(&a.scene);let history=a.undo_stack.len();
            let mut phases=vec![phase(&a,"after")];
            if finish=="continue" {assert!(a.commit_paste_preview());assert_eq!(a.undo_stack.len(),history+1);}
            else {a.cancel_paste_preview();assert_eq!(state(&a.scene),before);assert_eq!(a.undo_stack.len(),history);}
            phases.push(phase(&a,finish));
            if finish=="continue" {a.undo();assert_eq!(state(&a.scene),before);phases.push(phase(&a,"undo"));a.redo();phases.push(phase(&a,"redo"));}
            rows.push(json!({"case":case,"phases":phases,"file_exists":path.exists(),"error":error,"dialog_depths":dialogs}));
        }
    }
    // A refused Finish must leave the live context and pending owner untouched.
    let mut a=setup(&out);let owner=a.paste_preview.as_ref().unwrap().scene.mesh.vertices.as_ptr();
    let reference=a.paste_preview.as_ref().unwrap().reference;let offset=a.paste_preview.as_ref().unwrap().offset;
    let history=a.undo_stack.len();a.scene.mesh.vertices[0].position.x=f64::NAN;
    let path=out.join("invalid.native.igz");a.save_to_path(&path);
    assert!(!path.exists());assert_eq!(a.component_edit_stack.len(),2);assert_eq!(a.undo_stack.len(),history);
    assert!(a.scene.mesh.vertices[0].position.x.is_nan());let p=a.paste_preview.as_ref().unwrap();
    assert_eq!(p.scene.mesh.vertices.as_ptr(),owner);assert_eq!(p.reference,reference);assert_eq!(p.offset,offset);
    std::fs::write(out.join("native.json"),serde_json::to_vec_pretty(&json!({"rows":rows,"invalid_refusal":true,"immutable_preview_owner":true})).unwrap()).unwrap();
    println!("8 pending Paste Save workflows passed");
}
