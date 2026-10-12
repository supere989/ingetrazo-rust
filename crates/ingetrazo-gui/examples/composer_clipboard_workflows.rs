//! Frozen sheet clipboard actions through actual App input.
//! The integrated composer session reset is an explicit lifecycle boundary,
//! not evidence of native standalone-window close/reopen.
//! MetalProducer observes immutable callbacks only; this is no GPU pixel claim.
use ingetrazo_gui::app::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::Path;
type Error = Box<dyn std::error::Error>;
fn open(source: &Path, file: &str, session: &Path) -> Result<IngeTrazoApp, Error> {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    let mut store = ingetrazo_gui::session_state::SessionStore::isolated(session);
    // The source creates new windows in one preferences namespace. Recovery data stays case-local.
    store.config_dir = session
        .parent()
        .ok_or("Missing case session parent")?
        .join("shared-preferences");
    app.attach_session_store(store);
    app.mark_document_clean();
    app.open_document(&source.join(file));
    if app.document_session.confirm_unsaved
        || app.scene.sheets.is_empty()
        || app.current_file.as_deref() != Some(source.join(file).as_path())
    {
        return Err("Original gesture fixture was not opened".into());
    }
    app.enter_composer_sheet(0);
    app.scene.active_sheet_index = Some(0);
    app.composer_state.zoom = 2.;
    app.native_composer.factory = Some(ingetrazo_gui::composer_backend::Factory::MetalProducer(
        Default::default(),
    ));
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
    let output = ctx.run(
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
    if !output.platform_output.copied_text.is_empty() {
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("clipboard-consumer-system-text"),
                output.platform_output.copied_text,
            )
        });
    }
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

fn compositions(app: &IngeTrazoApp) -> Value {
    let mut rows =
        ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"].clone();
    // Literal Composicion.to_dict observation: optional empty lists and absent
    // title are omitted. The saved IGZ remains unmodified and is reopened by Python.
    for comp in rows.as_array_mut().unwrap() {
        let fields = comp.as_object_mut().unwrap();
        for key in [
            "texts",
            "images",
            "scalebars",
            "nortes",
            "leyendas",
            "shapes",
            "cotas",
            "cotas_ang",
            "cotas_rad",
            "etiquetas",
            "perfiles",
            "niveles",
            "llamadas",
        ] {
            if fields
                .get(key)
                .is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
            {
                fields.remove(key);
            }
        }
        if fields.get("cajetin").is_some_and(Value::is_null) {
            fields.remove("cajetin");
        }
    }
    rows
}
fn state(ctx: &egui::Context, app: &IngeTrazoApp, name: &str, phase: &str) -> Value {
    let index = app.scene.active_sheet_index.unwrap();
    let h = app
        .composer_state
        .sheet_history
        .get(&app.scene.sheets[index].id);
    let all = compositions(app);
    let selection = app.composer_selection_observation();
    let mut sorted = selection["selection_order"].as_array().unwrap().clone();
    sorted.sort_by(|a, b| {
        a["family"]
            .as_str()
            .cmp(&b["family"].as_str())
            .then(a["index"].as_u64().cmp(&b["index"].as_u64()))
    });
    let clipboard = app.composer_clipboard_observation();
    let source = clipboard["source_sheet"]
        .as_str()
        .and_then(|id| app.scene.sheets.iter().position(|s| s.id.to_string() == id));
    let text = ctx
        .data(|d| d.get_temp::<String>(egui::Id::new("clipboard-consumer-system-text")))
        .unwrap_or_default();
    json!({"case":name,"phase":phase,"composition":all[index],"all_compositions":all,
        "sheet_index":index,"selection_order":selection["selection_order"],"selection":sorted,
        "panel_owner":selection["panel_owner"],"undo":h.map_or(0,|h|h.0.len()),"redo":h.map_or(0,|h|h.1.len()),
        "clipboard":clipboard["records"],"clipboard_from":source,"system_text":text})
}
fn view_focus(ctx: &egui::Context) {
    ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
}
fn modifiers(a: &Value) -> egui::Modifiers {
    egui::Modifiers {
        ctrl: a["ctrl"] == true,
        command: a["ctrl"] == true,
        shift: a["shift"] == true,
        alt: a["alt"] == true,
        ..Default::default()
    }
}
fn pointer(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    a: &Value,
    op: &str,
) -> Result<(), Error> {
    let p = pixel(ctx, &a["effective_at"])?;
    let m = modifiers(a);
    if op == "press" {
        view_focus(ctx);
        frame(ctx, app, time, vec![egui::Event::PointerMoved(p)], m);
    }
    let event = if op == "move" {
        egui::Event::PointerMoved(p)
    } else {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: op == "press",
            modifiers: m,
        }
    };
    frame(ctx, app, time, vec![event], m);
    Ok(())
}
fn action(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    a: &Value,
) -> Result<(), Error> {
    match a["op"].as_str().ok_or("Operation")? {
        "observe" => {}
        "press" | "move" | "release" => pointer(ctx, app, time, a, a["op"].as_str().unwrap())?,
        "click" => {
            for op in ["move", "press", "release"] {
                pointer(ctx, app, time, a, op)?;
            }
        }
        "select_fixture" => app.composer_select_sheet_item(
            a["family"].as_str().ok_or("Family")?,
            a["index"].as_u64().ok_or("Index")? as usize,
            false,
        )?,
        "clipboard" => {
            view_focus(ctx);
            let event = match a["key"].as_str().ok_or("Clipboard key")? {
                "C" => egui::Event::Copy,
                "X" => egui::Event::Cut,
                "V" => egui::Event::Paste(String::new()),
                _ => return Err("Undeclared clipboard key".into()),
            };
            frame(
                ctx,
                app,
                time,
                vec![event],
                egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            );
        }
        "undo" => {
            view_focus(ctx);
            app.undo_composer_edit();
        }
        "redo" => {
            view_focus(ctx);
            app.redo_composer_edit();
        }
        "sheet_switch" => {
            let i = a["index"].as_u64().ok_or("Sheet index")? as usize;
            if i >= app.scene.sheets.len() {
                return Err("Missing sheet".into());
            }
            app.enter_composer_sheet(i);
            app.scene.active_sheet_index = Some(i);
        }
        "reopen_window" => {
            // Original creates a fresh standalone composer. Rust's composer is
            // integrated: exercise the matching ephemeral-session reset only.
            let before = app.composer_clipboard_observation();
            app.finish_composer_edit();
            app.composer_state = Default::default();
            for sheet in &mut app.scene.sheets {
                sheet.selected_item = None;
            }
            app.scene.active_sheet_index = Some(0);
            app.enter_composer_sheet(0);
            app.composer_state.zoom = 2.;
            if app.composer_clipboard_observation() != before {
                return Err("Clipboard lost at composer-session boundary".into());
            }
        }
        op => return Err(format!("Unknown clipboard action {op}").into()),
    }
    frame(ctx, app, time, vec![], modifiers(a));
    Ok(())
}
fn main() -> Result<(), Error> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() != 3 && (args.len() != 5 || args[3] != "--case") {
        return Err("composer_clipboard_workflows SOURCE OUTPUT [--case NAME]".into());
    }
    let only = args.get(4).and_then(|x| x.to_str());
    let source = Path::new(&args[1]).canonicalize()?;
    std::fs::create_dir_all(&args[2])?;
    let out = Path::new(&args[2]).canonicalize()?;
    std::env::set_current_dir(&source)?;
    unsafe {
        std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    }
    let report: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-clipboard.json"))?)?;
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut failures = Vec::new();
    let mut controls = Vec::new();
    for recipe in report["cases"].as_array().ok_or("Source cases")? {
        let name = recipe["name"].as_str().ok_or("Case name")?;
        if only.is_some_and(|c| c != name) {
            continue;
        }
        let mut phase = "open".to_string();
        let result = (|| -> Result<(), Error> {
            let mut app = open(
                &source,
                recipe["input"].as_str().ok_or("Input")?,
                &out.join("sessions").join(name),
            )?;
            let ctx = egui::Context::default();
            let mut time = 0.;
            for _ in 0..4 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            let mut keys = 0;
            let mut copies = 0;
            let mut cuts = 0;
            let mut pastes = 0;
            let mut persistence = false;
            for a in recipe["actions"].as_array().ok_or("Actions")? {
                phase = a["phase"].as_str().ok_or("Phase")?.into();
                let op = a["op"].as_str().ok_or("Operation")?;
                let before = state(&ctx, &app, name, &phase);
                action(&ctx, &mut app, &mut time, a)?;
                let after = state(&ctx, &app, name, &phase);
                if op == "clipboard" {
                    keys += 1;
                    let selection = before["selection"].as_array().unwrap().len();
                    let clip = before["clipboard"].as_array().unwrap().len();
                    let key = a["key"].as_str().unwrap();
                    if key == "C" {
                        if before["all_compositions"] != after["all_compositions"]
                            || before["undo"] != after["undo"]
                        {
                            return Err("Copy mutated document/history".into());
                        }
                        if selection > 0 {
                            if after["clipboard"].as_array().unwrap().len() != selection {
                                return Err("Copy did not capture actual selection".into());
                            }
                            copies += 1;
                        }
                    }
                    if (key == "X" && selection == 0) || (key == "V" && clip == 0) {
                        if before["all_compositions"] != after["all_compositions"]
                            || before["undo"] != after["undo"]
                        {
                            return Err("Empty clipboard action mutated document/history".into());
                        }
                    } else if key != "C" {
                        if after["undo"].as_u64() != before["undo"].as_u64().map(|n| n + 1) {
                            return Err("Clipboard edit did not commit one history step".into());
                        }
                        if key == "X" {
                            cuts += 1;
                        } else {
                            pastes += 1;
                        }
                    }
                }
                if op == "reopen_window" {
                    persistence = true;
                }
                states.push(after);
            }
            phase = "save".into();
            let saved = format!("{name}-native.igz");
            app.save_to_path(&out.join(&saved));
            if !out.join(&saved).is_file()
                || app.current_file.as_deref() != Some(out.join(&saved).as_path())
            {
                return Err(app.status_message.clone().into());
            }
            states.push(state(&ctx, &app, name, "save"));
            cases.push(json!({"name":name,"saved":saved}));
            phase = "controls".into();
            let mut refused = false;
            if !app.composer_clipboard_observation()["records"]
                .as_array()
                .unwrap()
                .is_empty()
            {
                let i = app.scene.active_sheet_index.unwrap();
                let previous = app.scene.sheets[i].native_document["images"].clone();
                app.scene.sheets[i].native_document["images"] =
                    json!("invalid destination inventory");
                let before = state(&ctx, &app, name, "refusal");
                if app.paste_composer_items().is_ok()
                    || state(&ctx, &app, name, "refusal") != before
                {
                    return Err("Refused paste mutated sheet/history/clipboard".into());
                }
                app.scene.sheets[i].native_document["images"] = previous;
                refused = true;
            }
            if keys == 0 {
                return Err("Case lacked clipboard ingress".into());
            }
            controls.push(json!({"case":name,"actual_clipboard_ingress":true,"clipboard_events":keys,"operative_copies":copies,"operative_cuts":cuts,"operative_pastes":pastes,"atomic_refusal":refused,"clipboard_persistence":persistence}));
            Ok(())
        })();
        if let Err(e) = result {
            failures.push(json!({"case":name,"phase":phase,"error":e.to_string()}));
        }
        std::fs::write(
            out.join("native-clipboard.partial.json"),
            serde_json::to_vec_pretty(
                &json!({"cases":cases,"states":states,"control_cases":controls,"failures":failures}),
            )?,
        )?;
    }
    if only.is_some() && cases.is_empty() && failures.is_empty() {
        return Err("Unknown diagnostic case".into());
    }
    let summary = json!({"actual_clipboard_ingress":!controls.is_empty()&&controls.iter().all(|v|v["actual_clipboard_ingress"]==true),"atomic_refusal":controls.iter().any(|v|v["atomic_refusal"]==true),"clipboard_persistence":controls.iter().any(|v|v["clipboard_persistence"]==true)});
    let payload = json!({"source_pin":report["source_pin"],"cases":cases,"states":states,"failures":failures,"controls":summary,"control_cases":controls,"callback_errors":[],"opaque_guards":[],"diagnostic_case":only,"scope":"Actual app Copy/Cut/Paste ingress and literal full states. Plain text is observed egui platform output, not OS clipboard acceptance. Reopen is explicit ephemeral composer-session reset within one app, not standalone window lifecycle. No profile-painting claim. Actual order retained; strict comparisons may report differences."});
    std::fs::write(
        out.join("native-clipboard.partial.json"),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    if !failures.is_empty() {
        return Err(format!(
            "{} clipboard cases failed; see partial report",
            failures.len()
        )
        .into());
    }
    std::fs::write(
        out.join(if only.is_some() {
            "native-clipboard.diagnostic.json"
        } else {
            "native-clipboard.json"
        }),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    Ok(())
}
