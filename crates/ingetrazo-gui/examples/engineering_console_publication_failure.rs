//! Actual GUI consumer with a real allocator returning null at target reserve.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {static FAIL:Cell<bool>=const{Cell::new(false)};static REFUSALS:Cell<usize>=const{Cell::new(0)};}
struct Allocator;
fn refuse() -> bool {
    FAIL.try_with(|v| v.replace(false)).unwrap_or(false)
}
fn count() {
    REFUSALS.with(|v| v.set(v.get() + 1));
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if refuse() {
            count();
            std::ptr::null_mut()
        } else {
            System.alloc(l)
        }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if refuse() {
            count();
            std::ptr::null_mut()
        } else {
            System.realloc(p, l, n)
        }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}
#[global_allocator]
static ALLOC: Allocator = Allocator;
fn main() {
    let mut app = ingetrazo_gui::IngeTrazoApp::new();
    let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
        .into_iter()
        .map(|p| app.scene.mesh.add_vertex(p))
        .collect();
    app.scene.mesh.add_face(ids, vec![]).unwrap();
    app.engineering_python_enabled = true;
    app.run_script("held=mesh.faces[0]; held.attrs['mat']='Before'");
    app.undo();
    // Keep a real preexisting Redo entry, and force the application target to grow.
    app.scene.mesh.vertices.shrink_to_fit();
    app.scene.mesh.edges.shrink_to_fit();
    app.scene.mesh.faces.shrink_to_fit();
    let before = app.scene.clone();
    let undo = app.undo_stack.clone();
    let redo = app.redo_stack.clone();
    let revision = app.model_revision;
    let budget = app.engineering_python_budget().unwrap();
    app.engineering_python_publication_probe(Some(|| FAIL.with(|v| v.set(true))));
    app.run_script("refused=mesh.add_face([QVector3D(3,0,0),QVector3D(4,0,0),QVector3D(3,1,0)])");
    assert_eq!(REFUSALS.with(Cell::get), 1);
    assert!(!FAIL.with(Cell::get));
    let scene_unchanged = app.scene == before;
    let undo_redo_preserved = app.undo_stack == undo && app.redo_stack == redo;
    let revision_unchanged = app.model_revision == revision;
    assert!(scene_unchanged);
    assert!(undo_redo_preserved);
    assert!(revision_unchanged);
    assert!(app
        .status_message
        .contains("Application row capacity refused"));
    assert!(app.status_message.contains("scope retired"));
    let refused_scope_charge = budget.snapshot().total();
    assert_eq!(refused_scope_charge, 0);
    // With no live VM revision, old saved app commands remain executable.
    app.redo();
    assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "Before");
    assert_eq!((app.undo_stack.len(), app.redo_stack.len()), (1, 0));
    app.undo();
    assert_eq!(app.scene.mesh, before.mesh);
    assert_eq!((app.undo_stack.len(), app.redo_stack.len()), (0, 1));
    assert_eq!(budget.snapshot().total(), 0);
    app.run_script("added=mesh.add_face([QVector3D(3,0,0),QVector3D(4,0,0),QVector3D(3,1,0)])");
    assert_eq!(app.scene.mesh.faces.len(), 2);
    assert_eq!(app.undo_stack.len(), 1);
    assert_eq!(app.redo_stack.len(), 0);
    app.undo();
    app.redo();
    app.run_script("assert added is mesh.faces[1]");
    assert!(!app.console_logs.last().unwrap().contains("refused"));
    let budget = app.engineering_python_budget().unwrap();
    drop(app);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"real_allocator_refusals":REFUSALS.with(Cell::get),"scene_unchanged":scene_unchanged,"undo_redo_preserved":undo_redo_preserved,"revision_unchanged":revision_unchanged,"refused_scope_charge":refused_scope_charge,"retired_scope_document_history":true,"retry_one_command":true,"retry_alias_undo_redo":true,"final_owner_charge":budget.snapshot().total()})
    );
}
