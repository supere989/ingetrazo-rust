use ingetrazo_gui::{appearance_preferences as appearance, IngeTrazoApp};
use serde_json::{json, Value};
use std::path::PathBuf;
fn frame(app: &mut IngeTrazoApp, ctx: &egui::Context, theme: Option<egui::Theme>, time: &mut f64) {
    for _ in 0..2 {
        *time += 0.1;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400., 900.),
                )),
                time: Some(*time),
                system_theme: theme,
                ..Default::default()
            },
            |ctx| app.update_ui(ctx),
        );
    }
}
fn record(app: &IngeTrazoApp, ctx: &egui::Context, phase: &str) -> Value {
    let p = &app.document_session.preferences;
    let v = ctx.style().visuals.clone();
    let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
    let settings = [
        "general/theme",
        "ui/toolbar_icon_px",
        "ui/large_toolbar_icons",
    ]
    .into_iter()
    .filter_map(|key| p.extra.get(key).map(|v| (key.to_owned(), v.clone())))
    .collect::<serde_json::Map<_, _>>();
    json!({"phase":phase,"theme":appearance::theme(p),"dark":v.dark_mode,"icon_px":appearance::toolbar_size(p),
           "window":rgb(v.window_fill),"text":rgb(v.override_text_color.unwrap()),"base":rgb(v.extreme_bg_color),
           "highlight":rgb(v.selection.bg_fill),"link":rgb(v.hyperlink_color),"settings":settings})
}
fn choose(app: &mut IngeTrazoApp, theme: &str, px: u32) {
    app.begin_preferences();
    let draft = app.document_session.preferences_draft.as_mut().unwrap();
    draft
        .preferences
        .extra
        .insert("general/theme".into(), json!(theme));
    draft
        .preferences
        .extra
        .insert("ui/toolbar_icon_px".into(), json!(px));
}
pub fn run() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap());
    let store = root.join("native-appearance-settings");
    if store.exists() {
        std::fs::remove_dir_all(&store).unwrap();
    }
    let mut app = IngeTrazoApp::new();
    app.set_language("en");
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    app.document_session
        .preferences
        .extra
        .insert("proof/opaque".into(), json!({"vendor":71}));
    let original_scene = app.scene.clone();
    let original_history = app.undo_stack.len();
    let ctx = egui::Context::default();
    let mut time = 0.;
    let mut rows = Vec::new();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "default-dark"));
    choose(&mut app, "light", 40);
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "draft-unpublished"));
    app.cancel_preferences();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "cancel"));
    choose(&mut app, "light", 40);
    app.accept_preferences();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "light-40"));
    choose(&mut app, "system", 20);
    app.accept_preferences();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "system-light"));
    // Unknown immediately after Light must fall back Dark, not retain the previous scheme.
    frame(&mut app, &ctx, None, &mut time);
    assert!(ctx.style().visuals.dark_mode);
    frame(&mut app, &ctx, Some(egui::Theme::Dark), &mut time);
    rows.push(record(&app, &ctx, "system-dark"));
    frame(&mut app, &ctx, None, &mut time);
    rows.push(record(&app, &ctx, "system-unknown"));
    choose(&mut app, "dark", 24);
    app.accept_preferences();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "fixed-dark"));
    app.document_session
        .preferences
        .extra
        .remove("ui/toolbar_icon_px");
    app.document_session
        .preferences
        .extra
        .insert("ui/large_toolbar_icons".into(), json!("1"));
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "legacy-large"));
    choose(&mut app, "dark", 40);
    app.accept_preferences();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "migrated-40"));
    app.document_session
        .preferences
        .extra
        .insert("general/theme".into(), json!("invalid"));
    app.document_session
        .preferences
        .extra
        .insert("ui/toolbar_icon_px".into(), json!("invalid"));
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    rows.push(record(&app, &ctx, "invalid-fallback"));
    assert_eq!(app.scene, original_scene);
    assert_eq!(app.undo_stack.len(), original_history);
    assert_eq!(
        app.document_session.preferences.extra["proof/opaque"],
        json!({"vendor":71})
    );
    app.persist_preferences();
    let mut restarted = IngeTrazoApp::new();
    restarted.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    assert_eq!(
        restarted.document_session.preferences.extra["ui/toolbar_icon_px"],
        json!("invalid")
    );
    assert_eq!(
        appearance::toolbar_size(&restarted.document_session.preferences),
        24
    );
    // Assets are embedded source PNGs, decoded by the native image path. Avoid megabyte JSON arrays.
    let expected: Value =
        serde_json::from_slice(&std::fs::read(root.join("source-icons.json")).unwrap()).unwrap();
    let mut icons = serde_json::Map::new();
    for (key, _) in expected.as_object().unwrap() {
        let (mode, name) = key.split_once('/').unwrap();
        let dark = mode == "dark";
        let bytes = ingetrazo_gui::icons::get_themed_icon_bytes(name, dark).unwrap();
        let decoded = image::load_from_memory(bytes).unwrap().to_rgba8();
        let folder = root.join("native-icons").join(mode);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(format!("{name}.rgba")), decoded.as_raw()).unwrap();
        std::fs::write(folder.join(format!("{name}.png")), bytes).unwrap();
        icons.insert(
            key.clone(),
            json!({"size":[decoded.width(),decoded.height()]}),
        );
    }
    std::fs::write(
        root.join("native-icons.json"),
        serde_json::to_vec_pretty(&icons).unwrap(),
    )
    .unwrap();
    let count = app.icons.cached_icons();
    frame(&mut app, &ctx, Some(egui::Theme::Light), &mut time);
    assert_eq!(app.icons.cached_icons(), count);
    std::fs::write(
        root.join("native-appearance.json"),
        serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("native-controls.json"),serde_json::to_vec_pretty(&json!({"unknown_after_light_falls_back_dark":true,"unchanged_scene_history":true,"unknown_extra_preserved":true,"restart_invalid_value_preserved":true,"warm_icon_cache_unchanged":true,"cached_icons":count})).unwrap()).unwrap();
}
