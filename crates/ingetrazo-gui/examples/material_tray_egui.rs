// SPDX-License-Identifier: GPL-3.0-or-later
//! Real egui input through the production tray/dialog. No OS/GPU claim.
use glam::DVec3;
use ingetrazo_core::{
    materials::Material,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn frame(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 1400.),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| app.update_ui(ctx),
    )
}
fn find_shape(shape: &egui::epaint::Shape, name: &str) -> Option<egui::Pos2> {
    match shape {
        egui::epaint::Shape::Text(text) if text.galley.text() == name => {
            Some(text.visual_bounding_rect().center())
        }
        egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| find_shape(s, name)),
        _ => None,
    }
}
fn click(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    out: &egui::FullOutput,
    time: &mut f64,
    name: &str,
) -> egui::FullOutput {
    let pos = out
        .shapes
        .iter()
        .find_map(|s| find_shape(&s.shape, name))
        .unwrap_or_else(|| panic!("UI text not visible: {name}"));
    *time += 0.1;
    frame(
        ctx,
        app,
        *time,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    *time += 0.1;
    frame(
        ctx,
        app,
        *time,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
}
fn snapshot(app: &IngeTrazoApp, phase: &str) -> Value {
    json!({"phase":phase,"recipe":app.scene.materials["Oak"].registry_recipe(),"attrs":app.scene.mesh.faces[0].attrs,"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"selection":app.scene.selection.len(),"revision":app.model_revision})
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    let mut app = IngeTrazoApp::new();
    app.scene = Scene::new();
    let v = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| app.scene.mesh.add_vertex(p));
    let f = app.scene.mesh.add_face(v.to_vec(), vec![]).unwrap();
    let recipe = json!({"name":"Oak","color":[0.,0.,0.],"texture":{"path":"old.png","sw":2.,"sh":3.,"rot":14,"vendor":71},"opacity":0.4,"finish":"wood","vendor":72});
    let m = Material::from_recipe(recipe).unwrap();
    app.scene.mesh.faces[f].attrs = m
        .face_recipe()
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    app.scene.mesh.faces[f]
        .attrs
        .insert("back".into(), json!({"color":[0.8,0.7,0.6]}));
    app.scene.materials.insert("Oak".into(), m);
    app.active_material = "Oak".into();
    app.scene.select(SelectionItem::Face(f));
    let ctx = egui::Context::default();
    let mut time = 0.;
    let mut output = frame(&ctx, &mut app, time, vec![]);
    for _ in 0..3 {
        time += 0.1;
        output = frame(&ctx, &mut app, time, vec![]);
    }
    let mut phases = vec![snapshot(&app, "before")];
    output = click(&ctx, &mut app, &output, &mut time, "Edit material");
    time += 0.1;
    output = frame(&ctx, &mut app, time, vec![]);
    output = click(&ctx, &mut app, &output, &mut time, "Cancel");
    phases.push(snapshot(&app, "cancelled"));
    time+=0.1;output=frame(&ctx,&mut app,time,vec![]);
    output = click(&ctx, &mut app, &output, &mut time, "Edit material");
    time += 0.1;
    output = frame(&ctx, &mut app, time, vec![]);
    output = click(&ctx, &mut app, &output, &mut time, "Texture");
    let _ = click(&ctx, &mut app, &output, &mut time, "Apply and restamp");
    phases.push(snapshot(&app, "edited"));
    assert!(app.scene.mesh.faces[f].attrs.get("texture").is_none());
    assert_eq!(app.undo_stack.len(), 1);
    app.undo();
    phases.push(snapshot(&app, "undo"));
    app.redo();
    phases.push(snapshot(&app, "redo"));
    app.save_to_path(&out.join("tray.igz"));
    assert!(out.join("tray.igz").exists(), "{}", app.status_message);
    std::fs::write(
        out.join("tray.json"),
        serde_json::to_vec_pretty(
            &json!({"scope":"production egui input; no OS or GPU claim","phases":phases}),
        )
        .unwrap(),
    )
    .unwrap();
}
