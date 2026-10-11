use glam::DVec3;
use ingetrazo_core::{groups::Group, math::Ray, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::{ToolContext, ToolType};
use serde_json::{json, Value};
fn rows(gs: &[Group]) -> Vec<Value> {
    let mut out = vec![];
    for g in gs {
        let instance = g.component_definition.is_some()
            || g.native_document.get("xform").is_some_and(|v| !v.is_null())
            || g.transform != glam::DMat4::IDENTITY;
        out.push(json!({"name":g.name,"xform":if instance{json!(g.transform.to_cols_array())}else{Value::Null},"axes":g.native_document.get("axes").unwrap_or(&Value::Null),"vertices":g.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":g.mesh.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"faces":g.mesh.faces.iter().map(|f|json!({"outer":f.vertices,"holes":f.holes,"attrs":f.attrs})).collect::<Vec<_>>(),"ext":g.native_document.get("ext").unwrap_or(&Value::Null)}));
        out.extend(rows(&g.children))
    }
    out
}
fn rec(a: &IngeTrazoApp, case: &str, phase: &str, out: &mut Vec<Value>) {
    out.push(json!({"case":case,"phase":phase,"groups":rows(&a.scene.groups),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"selection":a.scene.selection.iter().filter_map(|s|if let SelectionItem::Group(i)=s{a.scene.groups.get(*i).map(|g|g.name.clone())}else{None}).collect::<Vec<_>>()}));
}
fn click(a: &mut IngeTrazoApp, p: [f64; 3]) {
    let p = DVec3::from_array(p);
    a.dispatch_tool_click(&ToolContext::new(p, Ray::new(p + DVec3::Z, -DVec3::Z)));
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut out = vec![];
    for case in ["classic", "shared", "nested", "oblique", "context"] {
        let mut a = IngeTrazoApp::new();
        a.scene = ingetrazo_formats::igz::load_igz(&dir.join(format!("{case}.input.igz"))).unwrap();
        a.scene.selection.insert(SelectionItem::Group(0));
        if case == "context" {
            a.begin_component_edit();
            a.scene.selection.insert(SelectionItem::Group(0));
        }
        rec(&a, case, "before", &mut out);
        assert!(a.start_change_axes());
        let (o, r, g) = if case == "oblique" {
            ([1.1, -0.3, 0.7], [3.2, 1.4, 1.1], [-0.2, 2.3, 0.8])
        } else {
            ([2., 0., 0.], [2., 5., 0.], [0., 3., 0.])
        };
        click(&mut a, o);
        click(&mut a, o);
        click(&mut a, r);
        click(&mut a, r);
        assert_eq!(a.undo_stack.len(), 0);
        rec(&a, case, "invalid", &mut out);
        click(&mut a, g);
        assert_eq!(a.active_tool_type, ToolType::Select);
        rec(&a, case, "changed", &mut out);
        if case != "context" {
            ingetrazo_formats::igz::save_igz(&a.scene, &dir.join(format!("{case}.native.igz")))
                .unwrap();
        }
        a.undo();
        rec(&a, case, "undo", &mut out);
        a.redo();
        rec(&a, case, "redo", &mut out);
        assert!(a.start_change_axes());
        click(&mut a, [8., 9., 0.]);
        a.dispatch_tool_cancel();
        assert_eq!(a.active_tool_type, ToolType::Select);
        rec(&a, case, "cancel", &mut out);
        if case == "context" {
            a.finish_component_edit();
            rec(&a, case, "finish", &mut out);
            ingetrazo_formats::igz::save_igz(&a.scene, &dir.join("context.native.igz")).unwrap();
            a.undo();
            rec(&a, case, "finish-undo", &mut out);
            a.redo();
            rec(&a, case, "finish-redo", &mut out);
        }
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
