//! Six annotation families through actual App toolbar, property and pointer input.
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
        || app.scene.sheets.len() != 1
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
        ctx.data_mut(|d| d.remove_temp::<egui::Rect>(egui::Id::new(id)));
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
fn composition(app: &IngeTrazoApp) -> Value {
    let mut v =
        ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"][0].clone();
    let o = v.as_object_mut().unwrap();
    o.remove("vendor_annotation_sheet");
    for f in FAMILIES {
        if let Some(a) = o.get_mut(f).and_then(Value::as_array_mut) {
            for v in a.iter_mut() {
                v.as_object_mut().unwrap().remove("vendor_annotation");
            }
            if a.is_empty() {
                o.remove(f);
            }
        }
    }
    v
}
fn state(app: &IngeTrazoApp, name: &str, phase: &str) -> Value {
    let (armed, points) = app.composer_annotation_pending();
    let h = app
        .composer_state
        .sheet_history
        .get(&app.scene.sheets[0].id);
    let mut pending = json!({"start":null,"second":null,"angular":[],"radial":null});
    match armed {
        Some(0 | 3 | 4 | 5) => {
            pending["start"] = json!(points.first());
            if armed == Some(0) {
                pending["second"] = json!(points.get(1));
            }
        }
        Some(1) => pending["angular"] = json!(points),
        Some(2) => pending["radial"] = json!(points.first()),
        _ => {}
    }
    json!({"case":name,"phase":phase,"composition":composition(app),"selection":app.composer_annotation_selection().map(|(_,f,i)|json!({"family":FAMILIES[f],"index":i})),"undo":h.map_or(0,|h|h.0.len()),"redo":h.map_or(0,|h|h.1.len()),"mode":armed.map_or("select",|f|MODES[f]),"pending":pending,"callback_error_count":0})
}
fn raw_guards(app: &IngeTrazoApp, guard: &Value) -> Result<(), Error> {
    let doc = json!({"scene":ingetrazo_formats::igz::scene_to_igz_document(&app.scene)});
    for (path, expected) in guard["paths"].as_object().ok_or("Guard paths")? {
        let mut actual = &doc;
        for k in path.split('.') {
            actual = if let Some(a) = actual.as_array() {
                a.get(k.parse::<usize>()?).ok_or("Guard index")?
            } else {
                actual.get(k).ok_or("Guard key")?
            };
        }
        if actual != expected {
            return Err(format!("Opaque annotation changed: {path}").into());
        }
    }
    Ok(())
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
        frame(ctx, app, time, vec![egui::Event::PointerMoved(p)], m);
    }
    let e = if op == "move" {
        egui::Event::PointerMoved(p)
    } else {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: op == "press",
            modifiers: m,
        }
    };
    frame(ctx, app, time, vec![e], m);
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
            let id = if mode == "select" {
                "annotation-ui-select".to_string()
            } else {
                format!(
                    "annotation-ui-add-{}",
                    MODES
                        .iter()
                        .position(|x| *x == mode)
                        .ok_or("Unknown tool")?
                )
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
        "select" => {
            app.composer_select_annotation(
                family(a["family"].as_str().ok_or("Select family")?)?,
                Some(a["index"].as_u64().ok_or("Select index")? as usize),
            )?;
        }
        "property" => {
            let widget = a["widget"].as_str().ok_or("Property widget")?;
            click(
                ctx,
                app,
                time,
                if widget == "ll_number" {
                    "annotation-ui-property-number"
                } else {
                    "annotation-ui-property-text"
                },
            )?;
            let m = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            let mut select = key(egui::Key::A);
            if let egui::Event::Key { modifiers, .. } = &mut select {
                *modifiers = m;
            }
            frame(ctx, app, time, vec![select], m);
            // Replacement is one edit event, preventing an artificial empty-text transaction.
            frame(
                ctx,
                app,
                time,
                vec![egui::Event::Text(
                    a["value"].as_str().ok_or("Property value")?.into(),
                )],
                Default::default(),
            );
            if a["multiline"] != true {
                frame(
                    ctx,
                    app,
                    time,
                    vec![key(egui::Key::Enter)],
                    Default::default(),
                );
            }
        }
        "escape" | "delete" => {
            view_focus(ctx);
            frame(
                ctx,
                app,
                time,
                vec![key(if a["op"] == "escape" {
                    egui::Key::Escape
                } else {
                    egui::Key::Delete
                })],
                Default::default(),
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
        op => return Err(format!("Unknown annotation operation {op}").into()),
    }
    frame(ctx, app, time, vec![], Default::default());
    Ok(())
}
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("sheet_annotation_authoring SOURCE NATIVE".into());
    }
    let source = Path::new(&args[1]).canonicalize()?;
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    std::env::set_current_dir(&source)?;
    unsafe {
        std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    }
    let report: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-annotations.json"))?)?;
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut failures = Vec::new();
    let mut controls = Vec::new();
    for row in report["cases"].as_array().ok_or("Source cases")? {
        let name = row["name"].as_str().ok_or("Case name")?;
        let mut phase = "open".to_string();
        let result = (|| -> Result<(), Error> {
            let mut app = open(
                &source,
                row["input"].as_str().ok_or("Input filename")?,
                &out.join(format!("{name}-session")),
            )?;
            let ctx = egui::Context::default();
            let mut time = 0.;
            for _ in 0..4 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            for a in row["actions"].as_array().ok_or("Actions")? {
                phase = a["phase"].as_str().ok_or("Action phase")?.into();
                action(&ctx, &mut app, &mut time, a)?;
                states.push(state(&app, name, &phase));
            }
            phase = "save".into();
            let saved = format!("{name}-native.igz");
            app.save_to_path(&out.join(&saved));
            if !out.join(&saved).is_file() {
                return Err("Annotation save failed".into());
            }
            states.push(state(&app, name, "save"));
            cases.push(json!({"name":name,"saved":saved}));
            for guard in report["opaque_guards"]
                .as_array()
                .ok_or("Opaque guards")?
                .iter()
                .filter(|g| g["case"] == name)
            {
                raw_guards(&app, guard)?;
                let reopened = ingetrazo_formats::igz::load_igz(&out.join(&saved))?;
                let old = std::mem::replace(&mut app.scene, reopened);
                raw_guards(&app, guard)?;
                app.scene = old;
            }
            phase = "controls".into();
            let builds = app.composer_annotation_preview_stats().0;
            if name.ends_with("-lifecycle") && builds == 0 {
                return Err("Annotation canvas produced no geometry".into());
            }
            for _ in 0..8 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            if app.composer_annotation_preview_stats().0 != builds {
                return Err("Idle annotation geometry rebuilt".into());
            }
            let selected = app.composer_annotation_selection().or_else(|| {
                let sheet = &app.scene.sheets[0];
                FAMILIES.iter().enumerate().find_map(|(f, key)| {
                    sheet
                        .native_document
                        .get(*key)
                        .and_then(Value::as_array)
                        .and_then(|items| {
                            items.iter().position(|v| {
                                v.get("hidden").and_then(Value::as_bool) != Some(true)
                            })
                        })
                        .map(|i| (sheet.id, f, i))
                })
            });
            app.activate_composer_annotation(0)?;
            let before = state(&app, name, "refusal");
            if app
                .place_composer_annotation(&[[f64::NAN, 0.], [1., 1.], [2., 2.]], false)
                .is_ok()
                || state(&app, name, "refusal") != before
            {
                return Err("Invalid placement mutated model/history".into());
            }
            if let Some((_, f, i)) = selected {
                app.composer_select_annotation(f, Some(i))?;
                let before = state(&app, name, "refusal");
                if app
                    .edit_composer_annotation(json!({"hidden":"invalid"}))
                    .is_ok()
                    || state(&app, name, "refusal") != before
                {
                    return Err("Invalid property mutated model/history".into());
                }
                // Exercise immediate actual Delete/save against the imported fallback inventory.
                app.composer_select_annotation(f, Some(i))?;
                app.delete_composer_annotation()?;
                let path = out.join(format!("{name}-delete-control.igz"));
                app.save_to_path(&path);
                let reopened = ingetrazo_formats::igz::load_igz(&path)?;
                let expected = app.scene.sheets[0].native_document.get(FAMILIES[f]);
                let actual = reopened.sheets[0].native_document.get(FAMILIES[f]);
                if actual != expected {
                    return Err("Deleted raw annotation resurrected on save".into());
                }
            }
            controls.push(json!({"case":name,"idle_reuse":true,"placement_refusal":true,"property_refusal":selected.map(|_|true),"delete_persistence":selected.map(|_|true),"cache_builds":builds}));
            Ok(())
        })();
        if let Err(e) = result {
            failures.push(json!({"case":name,"phase":phase,"error":e.to_string()}));
        }
        std::fs::write(
            out.join("native-annotations.partial.json"),
            serde_json::to_vec_pretty(&json!({"cases":cases,"states":states,"failures":failures}))?,
        )?;
    }
    let payload = json!({"cases":cases,"states":states,"controls":controls,"failures":failures,"actual_egui":true,"unknown_metadata":true,"callback_scope":"Native producer payloads; no hardware pixel acceptance"});
    std::fs::write(
        out.join("native-annotations.partial.json"),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    if !failures.is_empty() {
        return Err(format!(
            "{} annotation cases failed; see native-annotations.partial.json",
            failures.len()
        )
        .into());
    }
    std::fs::write(
        out.join("native-annotations.json"),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    Ok(())
}
