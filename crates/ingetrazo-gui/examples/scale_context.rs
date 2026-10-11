// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual app Scale dispatch plus nested context lifecycle; no OS capture.
use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group, image_plane::ImagePlane, math::Ray, mesh::Mesh, scene::SelectionItem,
};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
fn vertices(m: &Mesh) -> Value {
    json!(m
        .vertices
        .iter()
        .map(|v| v.position.to_array())
        .collect::<Vec<_>>())
}
fn group(g: &Group) -> Value {
    json!({"name":g.name,"vertices":vertices(&g.mesh),"transform":if g.native_document.get("xform").is_some_and(|v|v.is_null()){None}else{Some(g.transform.to_cols_array())},"children":g.children.iter().map(group).collect::<Vec<_>>()})
}
fn state(a: &IngeTrazoApp) -> Value {
    json!({"mesh":vertices(&a.scene.mesh),"context_groups":a.scene.groups.iter().map(group).collect::<Vec<_>>(),"images":a.scene.image_planes.iter().map(|im|[im.origin.to_array(),im.u.to_array(),im.v.to_array()]).collect::<Vec<_>>(),"depth":a.component_edit_stack.len(),"active":a.active_tool.pending_model_preview(),"tool":format!("{:?}",a.active_tool_type),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"status":a.status_message})
}
fn point(v: &Value) -> DVec3 {
    DVec3::from_array(std::array::from_fn(|i| v[i].as_f64().unwrap()))
}
fn ctx(p: DVec3) -> ToolContext {
    ToolContext::with_modifiers(
        p,
        Ray::new(p + DVec3::Z * 10., -DVec3::Z),
        false,
        false,
        false,
    )
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for row in source["cases"].as_array().unwrap() {
        let kind = row["name"].as_str().unwrap();
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let mut mesh = Mesh::new();
        let ids = [
            DVec3::ZERO,
            DVec3::new(2., 0., 0.),
            DVec3::new(2., 3., 0.),
            DVec3::new(0., 3., 0.),
        ]
        .map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(), vec![]);
        let mut child = Group::with_mesh("child", mesh);
        child.transform = DMat4::from_translation(DVec3::new(1., 2., 0.));
        child.native_document = json!({"component":true,"xform":child.transform.to_cols_array()});
        let mut outer = Group::with_mesh("outer", Mesh::new());
        outer.transform = DMat4::from_translation(DVec3::new(4., 5., 0.));
        outer.native_document = json!({"component":true,"xform":outer.transform.to_cols_array()});
        outer.children.push(child);
        a.scene.groups.push(outer);
        a.scene.image_planes.push(ImagePlane::new(
            "scan",
            DVec3::new(1., 2., 0.),
            DVec3::new(2., 0., 0.),
            DVec3::new(0., 3., 0.),
        ));
        if kind != "entry-preview" {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.scene.clear_selection();
        if kind == "entry-preview" {
            a.scene.select(SelectionItem::Group(0));
        } else if kind == "image-preview" {
            a.scene.select(SelectionItem::ImagePlane(0));
        } else {
            a.scene.select(SelectionItem::Face(0));
            if kind == "mixed-preview" {
                a.scene.select(SelectionItem::ImagePlane(0));
            }
        }
        a.set_tool(ToolType::Scale);
        let mut states = serde_json::Map::new();
        states.insert("before".into(), state(&a));
        a.dispatch_tool_click(&ctx(point(&row["grab"])));
        a.dispatch_tool_move(&ctx(point(&row["hover"])));
        states.insert("preview".into(), state(&a));
        let mut accepted = vec![];
        if kind == "nested-commit" {
            accepted.push(a.dispatch_tool_vcb("2").is_ok());
            states.insert("commit".into(), state(&a));
        }
        if kind == "entry-preview" {
            a.begin_component_edit();
        } else {
            a.finish_component_edit();
        }
        states.insert("boundary".into(), state(&a));
        if kind == "nested-commit" {
            accepted.push(a.dispatch_tool_vcb("3").is_ok());
            states.insert("retype".into(), state(&a));
            a.undo();
            states.insert("undo".into(), state(&a));
            a.redo();
            states.insert("redo".into(), state(&a));
        } else {
            a.dispatch_tool_cancel();
            states.insert("tool-cancel".into(), state(&a));
        }
        // Explicit native whole-session Cancel is an integrity boundary absent in source.
        a.cancel_component_edit();
        states.insert("context-cancel".into(), state(&a));
        rows.push(json!({"name":kind,"states":states,"accepted":accepted}));
    }
    std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"cases":rows,"scope":"actual app dispatch/context methods, no OS input or rendered claim"})).unwrap()).unwrap();
}
