//! Frozen specialized composer workflows through actual App input and controls.
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

fn key(k: egui::Key) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}
fn click(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    id: &str,
) -> Result<(), Error> {
    let mut rect = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or("Missing actual annotation control")?;
    let mut stable = false;
    for _ in 0..4 {
        // A closed panel must not leave an old observation usable as a live widget.
        ctx.data_mut(|d| d.remove::<egui::Rect>(egui::Id::new(id)));
        frame(
            ctx,
            app,
            time,
            vec![egui::Event::PointerMoved(rect.center())],
            Default::default(),
        );
        let live = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
            .ok_or("Annotation control disappeared")?;
        if live == rect {
            stable = true;
            break;
        }
        rect = live;
    }
    if !stable {
        return Err("Annotation control did not settle during hover".into());
    }
    let p = rect.center();
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            time,
            vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }],
            Default::default(),
        );
    }
    Ok(())
}

const FAMILIES: [&str; 6] = [
    "cotas",
    "cotas_ang",
    "cotas_rad",
    "etiquetas",
    "niveles",
    "llamadas",
];
const MODES: [&str; 6] = [
    "cota",
    "cota_ang",
    "cota_radio",
    "etiqueta",
    "nivel",
    "llamada",
];
fn family(key: &str) -> Result<usize, Error> {
    FAMILIES
        .iter()
        .position(|x| *x == key)
        .ok_or_else(|| format!("Unknown family {key}").into())
}
fn compositions(app: &IngeTrazoApp) -> Value {
    let mut rows =
        ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"].clone();
    for comp in rows.as_array_mut().unwrap() {
        let o = comp.as_object_mut().unwrap();
        // Original dataclasses omit only these declared vendor fields. Their
        // preservation is checked independently in the actual native save.
        o.remove("vendor_specialized");
        for family in FAMILIES {
            if let Some(items) = o.get_mut(family).and_then(Value::as_array_mut) {
                for item in items.iter_mut() {
                    item.as_object_mut().unwrap().remove("vendor_record");
                }
                if items.is_empty() {
                    o.remove(family);
                }
            }
        }
    }
    rows
}
fn state(app: &IngeTrazoApp, name: &str, phase: &str) -> Value {
    let index = app.scene.active_sheet_index.unwrap();
    let sheet = &app.scene.sheets[index];
    let h = app.composer_state.sheet_history.get(&sheet.id);
    let all = compositions(app);
    let mut selected = app
        .composer_annotation_selections()
        .into_iter()
        .map(|(f, i)| (FAMILIES[f], i))
        .collect::<Vec<_>>();
    selected.sort_unstable();
    json!({"case":name,"phase":phase,"composition":all[index],"all_compositions":all,
        "sheet_index":index,"selection":selected.iter().map(|(f,i)|json!({"family":f,"index":i})).collect::<Vec<_>>(),
        "panel_owner":app.composer_annotation_selection().map(|(_,f,i)|json!({"family":FAMILIES[f],"index":i})),
        "undo":h.map_or(0,|h|h.0.len()),"redo":h.map_or(0,|h|h.1.len()),
        "mode":app.composer_annotation_mode(),"pending":app.composer_specialized_pending()})
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
        shift: a["shift"] == true,
        command: a["ctrl"] == true,
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
    match a["op"].as_str().ok_or("Action operation")? {
        "observe" => {}
        "arm" => {
            let mode = a["mode"].as_str().ok_or("Tool mode")?;
            let id = match mode {
                "select" => "annotation-ui-select".to_string(),
                "cota_cadena" => "annotation-ui-add-chain".into(),
                "cota_base" => "annotation-ui-add-baseline".into(),
                _ => format!(
                    "annotation-ui-add-{}",
                    MODES
                        .iter()
                        .position(|x| *x == mode)
                        .ok_or("Unknown tool")?
                ),
            };
            if mode != "select" {
                click(ctx, app, time, "annotation-ui-menu")?;
            }
            click(ctx, app, time, &id)?;
        }
        "click" => {
            pointer(ctx, app, time, a, "press")?;
            pointer(ctx, app, time, a, "release")?;
        }
        op @ ("press" | "release" | "move") => pointer(ctx, app, time, a, op)?,
        "select_fixture" => app.composer_select_annotation(
            family(a["family"].as_str().ok_or("Select family")?)?,
            Some(a["index"].as_u64().ok_or("Select index")? as usize),
        )?,
        "property" => {
            let key = match a["widget"].as_str().ok_or("Property widget")? {
                "cota_axis" => "axis",
                "cota_units" => "units",
                _ => return Err("Undeclared property widget".into()),
            };
            let value = a
                .get("data")
                .unwrap_or(&a["value"])
                .as_str()
                .ok_or("Property value")?;
            click(ctx, app, time, &format!("annotation-ui-property-{key}"))?;
            click(
                ctx,
                app,
                time,
                &format!("annotation-ui-choice-{key}-{value}"),
            )?;
        }
        "button" => {
            if a["widget"] != "cota_text_reset" {
                return Err("Undeclared button".into());
            }
            click(ctx, app, time, "annotation-ui-reset-text")?;
        }
        "key" => {
            view_focus(ctx);
            let k = match a["key"].as_str().ok_or("Key")? {
                "Escape" => egui::Key::Escape,
                "Delete" => egui::Key::Delete,
                _ => return Err("Undeclared key".into()),
            };
            frame(ctx, app, time, vec![key(k)], Default::default());
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
            let index = a["index"].as_u64().ok_or("Sheet index")? as usize;
            if index >= app.scene.sheets.len() {
                return Err("Missing target sheet".into());
            }
            app.enter_composer_sheet(index);
            app.scene.active_sheet_index = Some(index);
        }
        op => return Err(format!("Unknown specialized action {op}").into()),
    }
    frame(ctx, app, time, vec![], modifiers(a));
    Ok(())
}
fn main() -> Result<(), Error> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() != 3 && (args.len() != 5 || args[3] != "--case") {
        return Err("composer_specialized_workflows SOURCE OUTPUT [--case NAME]".into());
    }
    let only_case = args.get(4).and_then(|s| s.to_str());
    let source = Path::new(&args[1]).canonicalize()?;
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    std::env::set_current_dir(&source)?;
    unsafe {
        std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    }
    let report: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-specialized.json"))?)?;
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut failures = Vec::new();
    let mut control_cases = Vec::new();
    for recipe in report["cases"].as_array().ok_or("Source case roster")? {
        let name = recipe["name"].as_str().ok_or("Case name")?;
        if only_case.is_some_and(|c| c != name) {
            continue;
        }
        let mut phase = String::from("open");
        let result = (|| -> Result<(), Error> {
            let mut app = open(
                &source,
                recipe["input"].as_str().ok_or("Input file")?,
                &out.join("sessions").join(name),
            )?;
            let ctx = egui::Context::default();
            let mut time = 0.;
            for _ in 0..4 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            let mut gestures = 0usize;
            let mut pointer_events = 0usize;
            let mut press: Option<(Value, usize)> = None;
            for a in recipe["actions"].as_array().ok_or("Actions")? {
                phase = a["phase"].as_str().ok_or("Phase")?.into();
                let op = a["op"].as_str().ok_or("Action operation")?;
                if op == "press" && a.get("end_local").is_some() {
                    let sid = app.scene.sheets[app.scene.active_sheet_index.unwrap()].id;
                    press = Some((
                        compositions(&app),
                        app.composer_state
                            .sheet_history
                            .get(&sid)
                            .map_or(0, |h| h.0.len()),
                    ));
                }
                action(&ctx, &mut app, &mut time, a)?;
                if matches!(op, "press" | "move" | "release" | "click") {
                    pointer_events += 1;
                }
                if op == "release" && a.get("end_local").is_some() {
                    let (before, undo) = press
                        .take()
                        .ok_or("Gesture released without actual press")?;
                    let sid = app.scene.sheets[app.scene.active_sheet_index.unwrap()].id;
                    let after_undo = app
                        .composer_state
                        .sheet_history
                        .get(&sid)
                        .map_or(0, |h| h.0.len());
                    if a["end_local"].is_null() {
                        if compositions(&app) != before || after_undo != undo {
                            return Err("No-op handle changed model/history".into());
                        }
                    } else {
                        if compositions(&app) == before || after_undo != undo + 1 {
                            return Err("Actual specialized gesture did not change model with one Undo step".into());
                        }
                        gestures += 1;
                    }
                }
                states.push(state(&app, name, &phase));
            }
            if pointer_events == 0 {
                return Err("Case exercised no actual pointer ingress".into());
            }
            phase = "save".into();
            let saved = format!("{name}-native.igz");
            app.save_to_path(&out.join(&saved));
            if !out.join(&saved).is_file()
                || app.current_file.as_deref() != Some(out.join(&saved).as_path())
            {
                return Err(app.status_message.clone().into());
            }
            states.push(state(&app, name, "save"));
            cases.push(json!({"name":name,"saved":saved}));
            phase = "controls".into();
            let builds = app.composer_annotation_preview_stats().0;
            for _ in 0..8 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            if app.composer_annotation_preview_stats().0 != builds {
                return Err("Idle annotation geometry rebuilt".into());
            }
            app.activate_composer_annotation(0)?;
            let before = state(&app, name, "refusal");
            if app
                .place_composer_annotation(&[[f64::NAN, 0.], [1., 1.], [2., 2.]], false)
                .is_ok()
                || state(&app, name, "refusal") != before
            {
                return Err("Invalid placement mutated state/history".into());
            }
            let sid = app.scene.active_sheet_index.unwrap();
            let candidate = FAMILIES.iter().enumerate().find_map(|(f, k)| {
                app.scene.sheets[sid]
                    .native_document
                    .get(*k)
                    .and_then(Value::as_array)
                    .and_then(|a| a.iter().position(|v| v["hidden"] != true))
                    .map(|i| (f, i))
            });
            if let Some((f, i)) = candidate {
                app.composer_select_annotation(f, Some(i))?;
                let before = state(&app, name, "refusal");
                if app
                    .edit_composer_annotation(json!({"hidden":"invalid"}))
                    .is_ok()
                    || state(&app, name, "refusal") != before
                {
                    return Err("Invalid property mutated state/history".into());
                }
            }
            control_cases.push(json!({"case":name,"idle_reused":true,"atomic_refusal":true,"actual_pointer_ingress":true,"pointer_events":pointer_events,"operative_gestures":gestures}));
            Ok(())
        })();
        if let Err(e) = result {
            failures.push(json!({"case":name,"phase":phase,"error":e.to_string()}));
        }
        std::fs::write(
            out.join("native-specialized.partial.json"),
            serde_json::to_vec_pretty(
                &json!({"cases":cases,"states":states,"failures":failures,"control_cases":control_cases}),
            )?,
        )?;
    }
    if only_case.is_some() && cases.is_empty() && failures.is_empty() {
        return Err("Unknown diagnostic case".into());
    }
    let controls = json!({"idle_reused":control_cases.iter().all(|c|c["idle_reused"]==true)&&!control_cases.is_empty(),"atomic_refusal":control_cases.iter().all(|c|c["atomic_refusal"]==true)&&!control_cases.is_empty(),"actual_pointer_ingress":control_cases.iter().all(|c|c["actual_pointer_ingress"]==true)&&!control_cases.is_empty()});
    let payload = json!({"cases":cases,"states":states,"controls":controls,"control_cases":control_cases,"failures":failures,"diagnostic_case":only_case});
    std::fs::write(
        out.join("native-specialized.partial.json"),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    if !failures.is_empty() {
        return Err(format!(
            "{} specialized cases failed; see partial report",
            failures.len()
        )
        .into());
    }
    std::fs::write(
        out.join(if only_case.is_some() {
            "native-specialized.diagnostic.json"
        } else {
            "native-specialized.json"
        }),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    Ok(())
}
