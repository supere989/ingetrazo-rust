//! Retained source gestures through the real App/egui composer ingress.
use ingetrazo_gui::app::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::Path;
type Error = Box<dyn std::error::Error>;
fn composition(app: &IngeTrazoApp) -> Value {
    let mut v =
        ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"][0].clone();
    // ONLY the two declared native-preservation adversaries are outside Python's dataclass schema.
    if let Some(sheet) = v.as_object_mut() {
        sheet.remove("vendor_gesture_sheet");
    }
    if let Some(images) = v.get_mut("images").and_then(|v| v.as_array_mut()) {
        if let Some(image) = images.first_mut().and_then(|v| v.as_object_mut()) {
            image.remove("vendor_gesture_picture");
        }
    }
    v
}
fn state(app: &IngeTrazoApp, case: &str, phase: &str) -> Value {
    let sheet = &app.scene.sheets[0];
    let h = app.composer_state.sheet_history.get(&sheet.id);
    json!({"case":case,"phase":phase,"composition":composition(app),"selection":app.composer_picture_selection().map(|v|v.1),"undo":h.map_or(0,|h|h.0.len()),"redo":h.map_or(0,|h|h.1.len())})
}
fn open(source: &Path, file: &str, session: &Path) -> Result<IngeTrazoApp, Error> {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        session,
    ));
    app.mark_document_clean();
    app.open_document(&source.join(file));
    if app.document_session.confirm_unsaved
        || app.scene.sheets.len() != 1
        || app.current_file.as_deref() != Some(source.join(file).as_path())
    {
        return Err("Original gesture fixture was not opened".into());
    }
    app.enter_composer_sheet(0);
    app.scene.active_sheet_index = Some(0);
    app.composer_state.zoom = 2.;
    Ok(app)
}
fn frame(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    events: Vec<egui::Event>,
    mods: egui::Modifiers,
) {
    *time += 0.1;
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600., 1000.),
            )),
            time: Some(*time),
            events,
            modifiers: mods,
            ..Default::default()
        },
        |ctx| app.update_ui(ctx),
    );
}
fn pixel(ctx: &egui::Context, at: &Value) -> Result<egui::Pos2, Error> {
    let transform = ctx
        .data(|d| d.get_temp::<[f32; 3]>(egui::Id::new("pdf-ui-paper-transform")))
        .ok_or("Missing actual paper transform")?;
    Ok(egui::pos2(
        transform[0] + at[0].as_f64().ok_or("Gesture X")? as f32 * transform[2],
        transform[1] + at[1].as_f64().ok_or("Gesture Y")? as f32 * transform[2],
    ))
}
fn event(ctx: &egui::Context, e: &Value) -> Result<(Vec<egui::Event>, egui::Modifiers), Error> {
    let mut mods = egui::Modifiers::default();
    match e["mods"].as_str().unwrap_or("none") {
        "ctrl" => mods.ctrl = true,
        "shift" => mods.shift = true,
        "none" => {}
        _ => return Err("Unsupported gesture modifier".into()),
    };
    let op = e["op"].as_str().ok_or("Gesture operation")?;
    if op == "escape" {
        return Ok((
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: mods,
            }],
            mods,
        ));
    }
    let p = pixel(ctx, &e["effective_at"])?;
    let event = match op {
        "press" | "release" => egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: op == "press",
            modifiers: mods,
        },
        "move" => egui::Event::PointerMoved(p),
        _ => return Err("Unknown pointer operation".into()),
    };
    Ok((vec![event], mods))
}
fn raw_guards(app: &IngeTrazoApp, guards: &Value) -> Result<(), Error> {
    let doc = json!({"scene":ingetrazo_formats::igz::scene_to_igz_document(&app.scene)});
    for (path, expected) in guards["paths"].as_object().ok_or("Opaque guards")? {
        let mut actual = &doc;
        for k in path.split('.') {
            actual = if let Some(a) = actual.as_array() {
                a.get(k.parse::<usize>()?).ok_or("Opaque array path")?
            } else {
                actual.get(k).ok_or("Opaque object path")?
            };
        }
        if actual != expected {
            return Err(format!("Opaque guard changed: {path}").into());
        }
    }
    Ok(())
}
fn extras(source: &Path, out: &Path) -> Result<Value, Error> {
    let mut app = open(
        source,
        "unselected-drag-input.igz",
        &out.join("controls-session"),
    )?;
    let ctx = egui::Context::default();
    let mut time = 0.;
    for _ in 0..2 {
        frame(&ctx, &mut app, &mut time, vec![], Default::default());
    }
    let before = composition(&app);
    let history = state(&app, "control", "before");
    let drag_warm = app.composer_picture_preview_stats();
    let mut events = Vec::new();
    for (op, at) in [
        ("press", [55., 50.]),
        ("move", [65., 60.]),
        ("release", [65., 60.]),
        ("press", [65., 60.]),
        ("move", [75., 70.]),
        ("release", [75., 70.]),
    ] {
        let e = json!({"op":op,"mods":"none","effective_at":at});
        events.extend(event(&ctx, &e)?.0);
    }
    frame(&ctx, &mut app, &mut time, events, Default::default());
    frame(&ctx, &mut app, &mut time, vec![], Default::default());
    assert_eq!(
        drag_warm,
        app.composer_picture_preview_stats(),
        "Position-only drag prepared/read/uploaded pictures"
    );
    let two = state(&app, "control", "two-release");
    assert_eq!(
        two["undo"],
        json!(2),
        "Two releases must own two undo entries"
    );
    assert_eq!(two["composition"]["images"][0]["x_mm"], json!(60.));
    app.undo_composer_edit();
    assert_eq!(composition(&app)["images"][0]["x_mm"], json!(50.));
    app.undo_composer_edit();
    assert_eq!(composition(&app), before);
    app.redo_composer_edit();
    app.redo_composer_edit();
    assert_eq!(composition(&app), two["composition"]);
    frame(&ctx, &mut app, &mut time, vec![], Default::default());
    let warm = app.composer_picture_preview_stats();
    for _ in 0..8 {
        frame(&ctx, &mut app, &mut time, vec![], Default::default());
    }
    assert_eq!(
        warm,
        app.composer_picture_preview_stats(),
        "Warm frame prepared/read/uploaded an image"
    );
    // Explicit transaction Cancel restores geometry; Escape has its separate source rule.
    let before = composition(&app);
    for (op, at) in [("press", [75., 70.]), ("move", [85., 80.])] {
        let e = json!({"op":op,"mods":"none","effective_at":at});
        let (v, m) = event(&ctx, &e)?;
        frame(&ctx, &mut app, &mut time, v, m);
    }
    assert_ne!(composition(&app), before);
    app.cancel_composer_edit();
    assert_eq!(composition(&app), before);
    let e = json!({"op":"release","mods":"none","effective_at":[85.,80.]});
    let (v, m) = event(&ctx, &e)?;
    frame(&ctx, &mut app, &mut time, v, m);
    assert_eq!(composition(&app), before);
    // Losing the pointer owns the completed draft and never moves on later hover.
    for (op, at) in [("press", [75., 70.]), ("move", [85., 80.])] {
        let e = json!({"op":op,"mods":"none","effective_at":at});
        let (v, m) = event(&ctx, &e)?;
        frame(&ctx, &mut app, &mut time, v, m);
    }
    let lost = composition(&app);
    frame(
        &ctx,
        &mut app,
        &mut time,
        vec![egui::Event::PointerGone],
        Default::default(),
    );
    let e = json!({"op":"move","mods":"none","effective_at":[120.,110.]});
    let (v, m) = event(&ctx, &e)?;
    frame(&ctx, &mut app, &mut time, v, m);
    assert_eq!(composition(&app), lost);
    Ok(
        json!({"two_release_entries":true,"explicit_cancel":true,"lost_pointer":true,"warm_counters":warm,"initial_history":history}),
    )
}
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let source = Path::new(args.first().ok_or("Source directory")?).canonicalize()?;
    let out = Path::new(args.get(1).ok_or("Native output")?);
    std::fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    std::env::set_current_dir(&source)?;
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    let report: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-gestures.json"))?)?;
    let guards: Value = serde_json::from_slice(&std::fs::read(source.join("opaque-guards.json"))?)?;
    let mut cases = Vec::new();
    let mut states = Vec::new();
    for row in report["cases"].as_array().ok_or("Source cases")? {
        let name = row["name"].as_str().ok_or("Case name")?;
        let mut app = open(
            &source,
            row["input"].as_str().ok_or("Case input")?,
            &out.join(format!("session-{name}")),
        )?;
        if let Some(i) = row["selected"].as_u64() {
            app.composer_select_picture(Some(i as usize))?;
        }
        let ctx = egui::Context::default();
        let mut time = 0.;
        for _ in 0..2 {
            frame(&ctx, &mut app, &mut time, vec![], Default::default());
        }
        states.push(state(&app, name, "before"));
        for (i, e) in row["events"]
            .as_array()
            .ok_or("Case events")?
            .iter()
            .enumerate()
        {
            let (v, m) = event(&ctx, e)?;
            frame(&ctx, &mut app, &mut time, v, m);
            states.push(state(
                &app,
                name,
                &format!("{i}-{}", e["op"].as_str().unwrap()),
            ));
        }
        app.undo_composer_edit();
        frame(&ctx, &mut app, &mut time, vec![], Default::default());
        states.push(state(&app, name, "undo"));
        app.redo_composer_edit();
        frame(&ctx, &mut app, &mut time, vec![], Default::default());
        states.push(state(&app, name, "redo"));
        let saved = format!("{name}-native.igz");
        app.save_to_path(&out.join(&saved));
        if !out.join(&saved).is_file() {
            return Err("Native gesture save failed".into());
        }
        if guards["case"] == row["name"] {
            raw_guards(&app, &guards)?;
            let reopened = ingetrazo_formats::igz::load_igz(&out.join(&saved))?;
            let original = std::mem::replace(&mut app.scene, reopened);
            raw_guards(&app, &guards)?;
            app.scene = original;
        }
        states.push(state(&app, name, "save"));
        cases.push(json!({"name":name,"saved":saved}));
        std::fs::write(
            out.join("native-gestures.partial.json"),
            serde_json::to_vec_pretty(&json!({"cases":cases,"states":states}))?,
        )?;
    }
    let controls = extras(&source, &out)?;
    std::fs::write(
        out.join("native-gestures.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":cases,"states":states,"actual_egui":true,"unknown_metadata":true,"controls":controls}),
        )?,
    )?;
    Ok(())
}
