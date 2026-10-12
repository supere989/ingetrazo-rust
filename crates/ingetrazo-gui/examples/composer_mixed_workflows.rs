//! Actual finite heterogeneous sheet workflows; no physical/GPU pixel claim.
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
) -> egui::FullOutput {
    *time += 0.1;
    ctx.run(
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
    )
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

fn compositions(app: &IngeTrazoApp) -> Value {
    let mut all = ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"].clone();
    for comp in all.as_array_mut().unwrap() {
        let object = comp.as_object_mut().unwrap();
        object.remove("vendor_mixed");
        if let Some(images) = object.get_mut("images").and_then(Value::as_array_mut) {
            for image in images {
                image.as_object_mut().unwrap().remove("vendor_mixed_image");
            }
        }
        for family in [
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
            if object
                .get(family)
                .is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
            {
                object.remove(family);
            }
        }
        if object.get("cajetin").is_some_and(Value::is_null) {
            object.remove("cajetin");
        }
    }
    all
}
fn state(app: &IngeTrazoApp, name: &str, phase: &str) -> Value {
    let i = app.scene.active_sheet_index.unwrap();
    let history = app
        .composer_state
        .sheet_history
        .get(&app.scene.sheets[i].id);
    let all = compositions(app);
    let owners = app.composer_selection_observation();
    let mut sorted = owners["selection_order"].as_array().unwrap().clone();
    sorted.sort_by(|a, b| {
        a["family"]
            .as_str()
            .cmp(&b["family"].as_str())
            .then(a["index"].as_u64().cmp(&b["index"].as_u64()))
    });
    json!({"case":name,"phase":phase,"composition":all[i],"all_compositions":all,
        "sheet_index":i,"selection_order":owners["selection_order"],"selection":sorted,"panel_owner":owners["panel_owner"],
        "undo":history.map_or(0,|h|h.0.len()),"redo":history.map_or(0,|h|h.1.len())})
}
fn focus_view(ctx: &egui::Context) {
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
    let mods = modifiers(a);
    if op == "press" {
        focus_view(ctx);
        frame(ctx, app, time, vec![egui::Event::PointerMoved(p)], mods);
    }
    let event = if op == "move" {
        egui::Event::PointerMoved(p)
    } else {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: op == "press",
            modifiers: mods,
        }
    };
    frame(ctx, app, time, vec![event], mods);
    Ok(())
}
fn property(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    a: &Value,
) -> Result<(), Error> {
    let widget = a["widget"].as_str().ok_or("Property widget")?;
    let id = match widget {
        "text_size" => "mixed-ui-size",
        "text_edit" => "mixed-ui-text",
        "img_opacity" => "picture-ui-opacity",
        _ => return Err("Undeclared mixed property".into()),
    };
    click(ctx, app, time, id)?;
    let mods = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let mut select = key(egui::Key::A);
    if let egui::Event::Key { modifiers, .. } = &mut select {
        *modifiers = mods;
    }
    frame(ctx, app, time, vec![select], mods);
    let text = if widget == "text_edit" {
        a["value"].as_str().ok_or("Text value")?.to_owned()
    } else {
        a["value"].as_f64().ok_or("Numeric value")?.to_string()
    };
    frame(
        ctx,
        app,
        time,
        vec![egui::Event::Text(text)],
        Default::default(),
    );
    if widget != "text_edit" {
        frame(
            ctx,
            app,
            time,
            vec![key(egui::Key::Enter)],
            Default::default(),
        );
    }
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
        op @ ("press" | "move" | "release") => pointer(ctx, app, time, a, op)?,
        "click" => {
            pointer(ctx, app, time, a, "press")?;
            pointer(ctx, app, time, a, "release")?;
        }
        "select_fixture" => app.composer_select_sheet_item(
            a["family"].as_str().ok_or("Family")?,
            a["index"].as_u64().ok_or("Index")? as usize,
            false,
        )?,
        "property" => property(ctx, app, time, a)?,
        "key" => {
            focus_view(ctx);
            let k = match a["key"].as_str() {
                Some("Delete") => egui::Key::Delete,
                Some("Escape") => egui::Key::Escape,
                _ => return Err("Undeclared key".into()),
            };
            frame(ctx, app, time, vec![key(k)], Default::default());
        }
        "undo" => {
            focus_view(ctx);
            app.undo_composer_edit();
        }
        "redo" => {
            focus_view(ctx);
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
        op => return Err(format!("Undeclared mixed action {op}").into()),
    }
    frame(ctx, app, time, vec![], modifiers(a));
    Ok(())
}
fn foreground_guard(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    name: &str,
    at: &Value,
) -> Result<Value, Error> {
    focus_view(ctx);
    let at = pixel(ctx, at)?;
    let before = state(app, name, "foreground");
    let raw = ingetrazo_formats::igz::scene_to_igz_document(&app.scene);
    let id = egui::Id::new("mixed-consumer-foreground");
    let mut owned = false;
    for (i, events) in [
        vec![],
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
        vec![egui::Event::PointerMoved(at + egui::vec2(8., 6.))],
        vec![egui::Event::PointerButton {
            pos: at + egui::vec2(8., 6.),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    ]
    .into_iter()
    .enumerate()
    {
        *time += 0.1;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600., 1000.),
                )),
                time: Some(*time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::Area::new(id)
                    .order(egui::Order::Foreground)
                    .movable(false)
                    .fixed_pos(at - egui::vec2(24., 16.))
                    .show(ctx, |ui| {
                        let (_, response) = ui.allocate_exact_size(
                            egui::vec2(96., 64.),
                            egui::Sense::click_and_drag(),
                        );
                        owned |= i == 1
                            && ctx.layer_id_at(at)
                                == Some(egui::LayerId::new(egui::Order::Foreground, id))
                            && ctx.input(|input| input.pointer.primary_down())
                            && response.contains_pointer();
                    });
                app.update_ui(ctx);
            },
        );
    }
    frame(ctx, app, time, vec![], Default::default());
    if !owned
        || before != state(app, name, "foreground")
        || raw != ingetrazo_formats::igz::scene_to_igz_document(&app.scene)
    {
        return Err("Actual foreground ownership leaked to mixed selection/model/history".into());
    }
    Ok(
        json!({"actual_foreground_layer":true,"pointer_down_contains":true,"model_history_selection_unchanged":true}),
    )
}
fn main() -> Result<(), Error> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() != 3 && (args.len() != 5 || args[3] != "--case") {
        return Err("composer_mixed_workflows SOURCE OUTPUT [--case NAME]".into());
    }
    let only = args.get(4).and_then(|s| s.to_str());
    let source = Path::new(&args[1]).canonicalize()?;
    std::fs::create_dir_all(&args[2])?;
    let out = Path::new(&args[2]).canonicalize()?;
    std::env::set_current_dir(&source)?;
    unsafe {
        std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    }
    let report: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-mixed.json"))?)?;
    let mut cases = Vec::new();
    let mut states = Vec::new();
    let mut controls = Vec::new();
    let mut failures = Vec::new();
    for recipe in report["cases"].as_array().ok_or("Case roster")? {
        let name = recipe["name"].as_str().ok_or("Case name")?;
        if only.is_some_and(|s| s != name) {
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
            let actions = recipe["actions"].as_array().ok_or("Actions")?;
            let mut pointers = 0usize;
            let mut properties = 0usize;
            for a in actions {
                phase = a["phase"].as_str().ok_or("Phase")?.to_owned();
                action(&ctx, &mut app, &mut time, a)?;
                if matches!(
                    a["op"].as_str(),
                    Some("press" | "move" | "release" | "click")
                ) {
                    pointers += 1;
                }
                if a["op"] == "property" {
                    properties += 1;
                }
                states.push(state(&app, name, &phase));
            }
            if pointers == 0 {
                return Err("Case lacked actual mixed pointer ingress".into());
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
            let before = state(&app, name, "control");
            let raw = ingetrazo_formats::igz::scene_to_igz_document(&app.scene);
            let stats = app.composer_sheet_selection_stats();
            for _ in 0..8 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            if stats != app.composer_sheet_selection_stats()
                || before != state(&app, name, "control")
                || raw != ingetrazo_formats::igz::scene_to_igz_document(&app.scene)
            {
                return Err("Idle mixed selection rebuilt or mutated model/history".into());
            }
            if app
                .composer_sheet_pointer_for_control([f64::NAN, 0.])
                .is_ok()
                || before != state(&app, name, "control")
                || raw != ingetrazo_formats::igz::scene_to_igz_document(&app.scene)
            {
                return Err("Invalid mixed pointer did not refuse atomically".into());
            }
            // Use current actual typed geometry, not a stale source press position
            // after the cohort has moved or changed sheets.
            let sheet = &app.scene.sheets[app.scene.active_sheet_index.unwrap()];
            let target = sheet
                .items
                .iter()
                .find(|item| {
                    let hidden = sheet
                        .native_items
                        .get(&item.id())
                        .is_some_and(|v| v["hidden"] == true || v["locked"] == true);
                    let r = item.rect();
                    !hidden && r.w > 0. && r.h > 0.
                })
                .ok_or("Foreground control needs actual visible typed owner")?
                .rect();
            let anchor = json!([target.x + target.w / 2., target.y + target.h / 2.]);
            let foreign = foreground_guard(&ctx, &mut app, &mut time, name, &anchor)?;
            controls.push(json!({"case":name,"idle_reused":true,"atomic_refusal":true,"actual_pointer_ingress":true,"foreign_ui_owned":true,"pointer_events":pointers,"property_events":properties,"foreground":foreign,"cohort_cap_atomic":false,"cohort_cap_scope":"Not exercised by this finite consumer"}));
            Ok(())
        })();
        if let Err(error) = result {
            failures.push(json!({"case":name,"phase":phase,"error":error.to_string()}));
        }
        std::fs::write(
            out.join("native-mixed.partial.json"),
            serde_json::to_vec_pretty(
                &json!({"cases":cases,"states":states,"control_cases":controls,"failures":failures}),
            )?,
        )?;
    }
    if only.is_some() && cases.is_empty() && failures.is_empty() {
        return Err("Unknown diagnostic case".into());
    }
    let flags = json!({"idle_reused":!controls.is_empty()&&controls.iter().all(|v|v["idle_reused"]==true),"atomic_refusal":!controls.is_empty()&&controls.iter().all(|v|v["atomic_refusal"]==true),"actual_pointer_ingress":!controls.is_empty()&&controls.iter().all(|v|v["actual_pointer_ingress"]==true),"foreign_ui_owned":!controls.is_empty()&&controls.iter().all(|v|v["foreign_ui_owned"]==true),"cohort_cap_atomic":false});
    let payload = json!({"source_pin":report["source_pin"],"cases":cases,"states":states,"controls":flags,"control_cases":controls,"failures":failures,"diagnostic_case":only,"scope":"Actual heterogeneous App pointer/widget semantics and bounded controls. selection_order remains diagnostic; no Qt selectedItems order guarantee, physical pixel, profile painter or cohort-cap execution claim."});
    std::fs::write(
        out.join("native-mixed.partial.json"),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    if !failures.is_empty() {
        return Err(format!(
            "{} mixed cases failed; partial report retained",
            failures.len()
        )
        .into());
    }
    std::fs::write(
        out.join(if only.is_some() {
            "native-mixed.diagnostic.json"
        } else {
            "native-mixed.json"
        }),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    Ok(())
}
