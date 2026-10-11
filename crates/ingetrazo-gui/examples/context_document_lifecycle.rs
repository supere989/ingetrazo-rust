use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group,
    math::Ray,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::{document_session::DocumentAction, IngeTrazoApp};
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
use std::path::Path;
fn mesh_state(mesh: &Mesh, m: DMat4) -> Value {
    let point = |i: usize| m.transform_point3(mesh.vertices[i].position).to_array();
    let ring = |ids: &Vec<usize>| {
        let p = ids.iter().map(|i| point(*i)).collect::<Vec<_>>();
        (0..p.len())
            .map(|i| p[i..].iter().chain(&p[..i]).copied().collect::<Vec<_>>())
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    };
    let mut points = (0..mesh.vertices.len()).map(point).collect::<Vec<_>>();
    points.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut edges = mesh
        .edges
        .iter()
        .map(|e| {
            let mut p = [point(e.v0), point(e.v1)];
            p.sort_by(|a, b| a.partial_cmp(b).unwrap());
            p
        })
        .collect::<Vec<_>>();
    edges.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut faces = mesh
        .faces
        .iter()
        .map(|f| {
            let mut holes = f.holes.iter().map(&ring).collect::<Vec<_>>();
            holes.sort_by(|a, b| a.partial_cmp(b).unwrap());
            json!({"outer":ring(&f.vertices),"holes":holes,"attrs":f.attrs})
        })
        .collect::<Vec<_>>();
    faces.sort_by_key(|f| f.to_string());
    json!({"points":points,"edges":edges,"faces":faces})
}
fn state(s: &Scene) -> Value {
    fn visit(g: &Group, parent: DMat4, rows: &mut Vec<Value>) {
        let m = parent * g.transform;
        rows.push(json!({"name":g.name,"uid":g.native_uid(),"mesh":mesh_state(&g.mesh,m),"ext":g.native_document.get("ext").cloned().unwrap_or(Value::Null)}));
        for c in &g.children {
            visit(c, m, rows);
        }
    }
    let mut rows = vec![];
    for g in &s.groups {
        visit(g, DMat4::IDENTITY, &mut rows);
    }
    json!({"root_mesh":mesh_state(&s.mesh,DMat4::IDENTITY),"groups":rows})
}
fn setup(out: &Path) -> IngeTrazoApp {
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.set_language("en");
    a.open_document(&out.join("input.igz"));
    a.current_file = None;
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    assert_eq!(a.component_edit_stack.len(), 2);
    a.set_tool(ToolType::Rectangle);
    a.save_undo_state();
    for v in &mut a.scene.mesh.vertices {
        v.position.z += 2.;
    }
    a.camera.target = DVec3::new(7., 8., 9.);
    a.camera.distance = 23.;
    a
}
fn snapshot(a: &IngeTrazoApp, phase: &str) -> Value {
    json!({"phase":phase,"state":state(&a.scene),"depth":a.component_edit_stack.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"tool":format!("{:?}",a.active_tool_type).to_lowercase(),"camera_target":a.camera.target.to_array(),"camera_distance":a.camera.distance})
}
fn frame(a: &mut IngeTrazoApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000., 700.),
    ));
    input.events = events;
    ctx.run(input, |ctx| {
        a.document_session_ui(ctx);
        ingetrazo_gui::dialogs::render_dialogs(a, ctx);
    })
}
fn click(a: &mut IngeTrazoApp, ctx: &egui::Context, label: &str) {
    frame(a, ctx, vec![]);
    let output = frame(a, ctx, vec![]);
    let ps = output
        .shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                if t.galley.text() == label {
                    return Some(t.pos + t.galley.rect.center().to_vec2());
                }
                None
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(ps.len(), 1, "button {label}");
    let pos = ps[0];
    for pressed in [true, false] {
        frame(
            a,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut rows = vec![];
    for case in [
        "new_cancel",
        "open_cancel",
        "open_picker_cancel",
        "open_discard",
        "open_failed",
        "new_discard",
        "close_cancel",
        "close_discard",
        "close_save",
        "close_save_cancel",
        "close_save_failed",
    ] {
        let mut a = setup(&out);
        let ctx = egui::Context::default();
        let path = out.join(format!("{case}.native.igz"));
        if case == "close_save" {
            a.current_file = Some(path.clone());
        } else if case == "close_save_failed" {
            a.current_file = Some(out.join("missing/failed.igz"));
        }
        if case.starts_with("new") {
            a.new_document();
        } else if case.starts_with("open") {
            a.begin_open_document();
        } else {
            a.request_document_action(DocumentAction::Close);
        }
        assert!(a.document_session.confirm_unsaved, "{case}");
        let prompts = vec![a.component_edit_stack.len()];
        let mut dialogs = vec![];
        let choice = if ["new_cancel", "open_cancel", "close_cancel"].contains(&case) {
            "Cancel"
        } else if case.starts_with("close_save") {
            "Save"
        } else {
            "Discard"
        };
        click(&mut a, &ctx, choice);
        if a.show_open_dialog {
            dialogs.push(a.component_edit_stack.len());
            if case == "open_picker_cancel" {
                click(&mut a, &ctx, "Cancel");
            } else {
                a.dialog_path_input = out
                    .join(if case == "open_failed" {
                        "corrupt.igz"
                    } else {
                        "replacement.igz"
                    })
                    .display()
                    .to_string();
                click(&mut a, &ctx, "Open");
            }
        }
        if a.show_save_dialog {
            assert_eq!(case, "close_save_cancel");
            dialogs.push(a.component_edit_stack.len());
            click(&mut a, &ctx, "Cancel");
        }
        let error = a.status_message.starts_with("Failed to open")
            || a.status_message.starts_with("Save failed");
        let closed = a.document_session.close_allowed;
        let prompt_open = a.document_session.confirm_unsaved;
        let mut phases = vec![snapshot(&a, "after")];
        if !closed {
            a.undo();
            phases.push(snapshot(&a, "undo"));
            a.redo();
            phases.push(snapshot(&a, "redo"));
        }
        let final_document = if !a.component_edit_stack.is_empty() {
            a.save_document_as();
            a.show_save_dialog = false;
            Some(state(&a.scene))
        } else {
            None
        };
        rows.push(json!({"case":case,"phases":phases,"prompt_depths":prompts,"dialog_depths":dialogs,"closed":closed,"error":error,"file_exists":path.exists(),"prompt_open":prompt_open,"final_document":final_document}));
    }
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap(),
    )
    .unwrap();
    let mut pending = vec![];
    for (tool, chain) in [
        (ToolType::Line, false),
        (ToolType::Line, true),
        (ToolType::Rectangle, false),
    ] {
        let mut a = setup(&out);
        a.set_tool(tool);
        let context = |p| ToolContext::new(p, Ray::new(p + DVec3::Z * 10., -DVec3::Z));
        a.dispatch_tool_click(&context(DVec3::new(30., 40., 50.)));
        if chain {
            a.dispatch_tool_click(&context(DVec3::new(31., 40., 50.)));
        }
        a.dispatch_tool_move(&context(DVec3::new(32., 41., 50.)));
        let name = format!(
            "{}-{}",
            format!("{tool:?}").to_lowercase(),
            if chain { "chain" } else { "first" }
        );
        a.current_file = Some(out.join(format!("{name}.native.igz")));
        a.save_document();
        let mut phases = vec![snapshot(&a, "after")];
        let anchor = a.active_tool.snap_anchor().map(|p| p.to_array());
        a.dispatch_tool_click(&context(DVec3::new(32., 41., 50.)));
        phases.push(snapshot(&a, "continued"));
        a.undo();
        phases.push(snapshot(&a, "undo"));
        a.redo();
        phases.push(snapshot(&a, "redo"));
        pending.push(json!({"case":name,"anchor_after":anchor,"phases":phases}));
    }
    std::fs::write(
        out.join("native-pending.json"),
        serde_json::to_vec_pretty(&json!({"rows":pending})).unwrap(),
    )
    .unwrap();
}
