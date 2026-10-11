use glam::DVec3;
use ingetrazo_gui::{document_session::DocumentAction, session_state::SessionStore, IngeTrazoApp};
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
fn labels(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn visit(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            egui::Shape::Vec(v) => {
                for s in v {
                    visit(s, out)
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        visit(&s.shape, &mut out)
    }
    out
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 3);
    let root = PathBuf::from(&args[2]);
    let source = root.join("original.igz");
    let mode = &args[1];
    if mode == "init" {
        std::fs::create_dir_all(&root).unwrap();
        let mut scene = ingetrazo_core::scene::Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| scene.mesh.add_vertex(p));
        scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        ingetrazo_formats::igz::save_igz(&scene, &source).unwrap();
        println!("{}", json!({"initialized":true}));
        return;
    }
    let mut app = IngeTrazoApp::new();
    app.attach_session_store(SessionStore::isolated(&root));
    if mode == "writer" {
        app.open_document(&source);
        app.scene.mesh.vertices[0].position = DVec3::new(9., 8., 7.);
        app.scene.bump_version();
        app.document_session.preferences.autosave_minutes = 7;
        app.document_session
            .preferences
            .extra
            .insert("future-setting".into(), json!({"opaque":[1,2]}));
        app.persist_preferences();
        assert!(app
            .autosave_if_due(Instant::now() + Duration::from_secs(1000), false)
            .unwrap());
        let held = app.document_session.store.as_ref().unwrap().clone();
        let slot = held.slot_for(Some(&source)).unwrap();
        println!(
            "{}",
            json!({"ready":true,"pid":std::process::id(),"slot":slot,"session":held.session_id})
        );
        io::stdout().flush().unwrap();
        let mut command = String::new();
        io::stdin().read_line(&mut command).unwrap();
        assert_eq!(command.trim(), "drop-app");
        drop(app);
        println!("{}", json!({"clone_holds_owner":true}));
        io::stdout().flush().unwrap();
        command.clear();
        io::stdin().read_line(&mut command).unwrap();
        drop(held);
        return;
    }
    if mode == "scan" {
        let records = app
            .document_session
            .store
            .as_ref()
            .unwrap()
            .recovery_candidates(false);
        println!(
            "{}",
            json!({"pid":std::process::id(),"show_recovery":app.document_session.show_recovery,"count":records.as_ref().map(|r|r.len()).ok(),"error":records.err(),"status":app.status_message})
        );
        return;
    }
    assert_eq!(mode, "recover");
    assert!(app.document_session.show_recovery);
    assert_eq!(app.document_session.recovery.len(), 1);
    let ctx = egui::Context::default();
    let frame = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.document_session_ui(ctx));
    let out = ctx.run(frame(), |ctx| app.document_session_ui(ctx));
    let ui_labels = labels(&out.shapes);
    assert!(ui_labels.iter().any(|s| s == "Recover an autosave"));
    fn button_center(shape: &egui::Shape, prefix: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.job.text.starts_with(prefix) => {
                Some(t.pos + t.galley.size() / 2.)
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| button_center(s, prefix)),
            _ => None,
        }
    }
    let pos = out
        .shapes
        .iter()
        .find_map(|s| button_center(&s.shape, &source.display().to_string()))
        .expect("Actual recovery button is painted");
    let mut click = frame();
    click.events = vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ];
    let _ = ctx.run(click, |ctx| app.document_session_ui(ctx));
    assert_eq!(app.scene.mesh.vertices[0].position, DVec3::new(9., 8., 7.));
    assert!(app.current_file.is_none() && app.document_dirty());
    assert_eq!(app.document_session.preferences.autosave_minutes, 7);
    assert_eq!(
        app.document_session.preferences.extra["future-setting"],
        json!({"opaque":[1,2]})
    );
    let recovered = app.scene.clone();
    app.request_document_action(DocumentAction::Close);
    assert!(app.document_session.confirm_unsaved);
    app.cancel_document_action();
    assert_eq!(app.scene, recovered);
    assert!(!app.document_session.close_allowed);
    app.request_document_action(DocumentAction::Close);
    app.save_to_path(&root.join("missing/refused.igz"));
    assert_eq!(app.scene, recovered);
    assert!(app.document_dirty() && !app.document_session.close_allowed);
    assert!(app.document_session.pending.is_none());
    let refused_status = app.status_message.clone();
    app.save_to_path(&root.join("recovered.igz"));
    assert!(!app.document_dirty());
    let saved = ingetrazo_formats::igz::load_igz(&root.join("recovered.igz")).unwrap();
    assert_eq!(saved.mesh.vertices[0].position, DVec3::new(9., 8., 7.));
    app.request_document_action(DocumentAction::Close);
    assert!(app.document_session.close_allowed);
    println!(
        "{}",
        json!({"pid":std::process::id(),"recovered":[9,8,7],"preferences":app.document_session.preferences,"picker_labels":ui_labels,"cancel_preserved":true,"failed_save_preserved":true,"failed_save_status":refused_status,"saved_copy":true,"close_allowed":true,"discarded_copies":app.document_session.store.as_ref().unwrap().recovery_candidates(true).unwrap().len()})
    );
}
