#[path="line_command_history_fixture/measurements.rs"]
mod measurements;
use glam::DVec3;
use ingetrazo_core::{math::Ray, mesh::Mesh};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
fn state(m: &Mesh) -> Value {
    json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|&f.vertices).collect::<Vec<_>>()})
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    if std::env::args().nth(2).as_deref()==Some("--measurements"){measurements::run(&out);return;}
    let source: Vec<Value> =
        serde_json::from_slice(&std::fs::read(out.join("original.json")).unwrap()).unwrap();
    let mut rows = vec![];
    for row in source {
        let name = row["case"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&out.join(format!("{name}.igz"))).unwrap();
        app.set_tool(ToolType::Line);
        let point = |value: &Value| {
            DVec3::new(
                value[0].as_f64().unwrap(),
                value[1].as_f64().unwrap(),
                value[2].as_f64().unwrap(),
            )
        };
        let a = point(&row["a"]);
        let b = point(&row["b"]);
        let candidate =
            ingetrazo_core::drafting::add_segments(&app.scene.mesh, &[(a, b)], None, None).unwrap();
        let class = if ingetrazo_core::drafting::source_bare_edge_history(
            &app.scene.mesh,
            &candidate,
            a,
            b,
        ) {
            "AddEdgeCommand"
        } else {
            "SnapshotCompound"
        };
        let before = state(&app.scene.mesh);
        let ctx = |p| ToolContext::new(p, Ray::new(p + DVec3::Z, -DVec3::Z));
        app.dispatch_tool_click(&ctx(a));
        app.dispatch_tool_click(&ctx(b));
        if name == "midchain-branch" {
            app.dispatch_tool_vcb("0;3;0").unwrap();
        }
        let after = state(&app.scene.mesh);
        app.undo();
        let undone = state(&app.scene.mesh);
        app.redo();
        let redone = state(&app.scene.mesh);
        let branch = if name == "midchain-branch" {
            app.undo();
            app.dispatch_tool_vcb("0;-1;0").unwrap();
            Some(state(&app.scene.mesh))
        } else {
            None
        };
        app.dispatch_tool_cancel();
        rows.push(json!({"case":name,"a":row["a"],"b":row["b"],"class":class,"before":before,"after":after,"undo":undone,"redo":redone,"branch":branch,"cancel":state(&app.scene.mesh),"anchor":app.active_tool.snap_anchor().map(|p|p.to_array()),"history":[app.undo_stack.len(),app.redo_stack.len()]}));
    }
    println!("{}", json!(rows));
}
