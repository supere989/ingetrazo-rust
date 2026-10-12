//! Source legend authoring through actual App/egui widgets and page ingress.
use ingetrazo_gui::app::IngeTrazoApp;
use serde_json::{Value, json};
use std::path::Path;
type Error = Box<dyn std::error::Error>;
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
    let rect = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or("Missing actual legend control")?;
    let p = rect.center();
    frame(
        ctx,
        app,
        time,
        vec![egui::Event::PointerMoved(p)],
        Default::default(),
    );
    let live = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or("Legend control disappeared")?;
    if live != rect {
        return Err("Legend control moved during hover".into());
    }
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
fn composition(app: &IngeTrazoApp) -> Value {
    let mut v =
        ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"][0].clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("vendor_legend_sheet");
    }
    if let Some(a) = v.get_mut("leyendas").and_then(Value::as_array_mut) {
        if let Some(o) = a.first_mut().and_then(Value::as_object_mut) {
            o.remove("vendor_legend");
        }
    }
    v
}
fn state(app: &IngeTrazoApp, case: &str, phase: &str) -> Value {
    let sheet = &app.scene.sheets[0];
    let h = app.composer_state.sheet_history.get(&sheet.id);
    let v = composition(app);
    let heights = app
        .composer_legend_heights()
        .expect("Runtime legend geometry");
    json!({"case":case,"phase":phase,"composition":v,"layers":ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["layers"],"selection":app.composer_legend_selection().map(|v|v.1),"undo":h.map_or(0,|h|h.0.len()),"redo":h.map_or(0,|h|h.1.len()),"mode":if app.composer_legend_armed(){"leyenda"}else{"select"},"pending":app.composer_legend_pending(),"heights":heights})
}
fn raw_guards(app: &IngeTrazoApp, guard: &Value) -> Result<(), Error> {
    let doc = json!({"scene":ingetrazo_formats::igz::scene_to_igz_document(&app.scene)});
    for (path, expected) in guard["paths"].as_object().ok_or("Guard paths")? {
        let mut actual = &doc;
        for k in path.split('.') {
            actual = if let Some(a) = actual.as_array() {
                a.get(k.parse::<usize>()?).ok_or("Guard array index")?
            } else {
                actual.get(k).ok_or("Guard field")?
            };
        }
        if actual != expected {
            return Err(format!("Opaque legend field changed: {path}").into());
        }
    }
    Ok(())
}
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("composer_legend_authoring SOURCE NATIVE".into());
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
        serde_json::from_slice(&std::fs::read(source.join("original-legend.json"))?)?;
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut controls = json!({});
    for row in report["cases"].as_array().ok_or("Source cases")? {
        let name = row["name"].as_str().ok_or("Case name")?;
        let mut app = open(
            &source,
            row["input"].as_str().ok_or("Input")?,
            &out.join(format!("{name}-session")),
        )?;
        let ctx = egui::Context::default();
        let mut time = 0.;
        for _ in 0..4 {
            frame(&ctx, &mut app, &mut time, vec![], Default::default());
        }
        for action in row["actions"].as_array().ok_or("Actions")? {
            match action["op"].as_str().ok_or("Operation")? {
                "observe" => {}
                "arm" => {
                    click(&ctx, &mut app, &mut time, "legend-ui-add")?;
                    if !app.composer_legend_armed() {
                        return Err("Actual legend toolbar did not activate".into());
                    }
                }
                "press" | "release" | "move" => {
                    let op = action["op"].as_str().unwrap();
                    let p = pixel(&ctx, &action["effective_at"])?;
                    if op == "press" {
                        frame(
                            &ctx,
                            &mut app,
                            &mut time,
                            vec![egui::Event::PointerMoved(p)],
                            Default::default(),
                        );
                    }
                    let e = if op == "move" {
                        egui::Event::PointerMoved(p)
                    } else {
                        egui::Event::PointerButton {
                            pos: p,
                            button: egui::PointerButton::Primary,
                            pressed: op == "press",
                            modifiers: Default::default(),
                        }
                    };
                    frame(&ctx, &mut app, &mut time, vec![e], Default::default());
                }
                "escape" => frame(
                    &ctx,
                    &mut app,
                    &mut time,
                    vec![key(egui::Key::Escape)],
                    Default::default(),
                ),
                "title" => {
                    click(&ctx, &mut app, &mut time, "legend-ui-title")?;
                    let mods = egui::Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    };
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![egui::Event::Key {
                            key: egui::Key::A,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: mods,
                        }],
                        mods,
                    );
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![
                            key(egui::Key::Backspace),
                            egui::Event::Text(action["value"].as_str().ok_or("Title")?.into()),
                            key(egui::Key::Enter),
                        ],
                        Default::default(),
                    );
                }
                "refresh" => click(&ctx, &mut app, &mut time, "legend-ui-refresh")?,
                "focused-delete" => {
                    click(&ctx, &mut app, &mut time, "legend-ui-title")?;
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![key(egui::Key::End), key(egui::Key::Delete)],
                        Default::default(),
                    );
                }
                "delete" => {
                    click(&ctx, &mut app, &mut time, "legend-ui-delete")?;
                }
                "undo" => app.undo_composer_edit(),
                "redo" => app.redo_composer_edit(),
                "select" => app.composer_select_legend(Some(
                    action["index"].as_u64().ok_or("Selection")? as usize,
                ))?,
                "layers" => {
                    let mut layers = Vec::new();
                    for v in action["value"].as_array().ok_or("Layer fixture")? {
                        let mut l = ingetrazo_core::layers::Layer::new(
                            v["name"].as_str().ok_or("Layer name")?,
                        );
                        l.visible = v.get("visible").and_then(Value::as_bool).unwrap_or(true);
                        l.locked = v.get("locked").and_then(Value::as_bool).unwrap_or(false);
                        layers.push(l);
                    }
                    app.scene.layers = layers;
                }
                "save" => {
                    let saved = format!("{name}-native.igz");
                    app.save_to_path(&out.join(&saved));
                    if !out.join(&saved).is_file() {
                        return Err("Legend save failed".into());
                    }
                    cases.push(json!({"name":name,"saved":saved}));
                    for guard in report["opaque_guards"].as_array().ok_or("Opaque guards")? {
                        if guard["case"] == row["name"] {
                            raw_guards(&app, guard)?;
                            let reopened = ingetrazo_formats::igz::load_igz(
                                &out.join(format!("{name}-native.igz")),
                            )?;
                            let old = std::mem::replace(&mut app.scene, reopened);
                            raw_guards(&app, guard)?;
                            app.scene = old;
                        }
                    }
                }
                _ => return Err("Unknown legend action".into()),
            }
            frame(&ctx, &mut app, &mut time, vec![], Default::default());
            states.push(state(&app, name, action["phase"].as_str().ok_or("Phase")?));
            std::fs::write(
                out.join("native-legend.partial.json"),
                serde_json::to_vec_pretty(&json!({"cases":cases,"states":states}))?,
            )?;
        }
        app.activate_composer_legend()?;
        let before = composition(&app);
        let history = state(&app, name, "guard");
        if app
            .place_composer_legend(glam::DVec2::splat(f64::NAN))
            .is_ok()
        {
            return Err("Nonfinite legend placement admitted".into());
        }
        if composition(&app) != before || state(&app, name, "guard") != history {
            return Err("Legend refusal mutated model/history".into());
        }
        frame(
            &ctx,
            &mut app,
            &mut time,
            vec![key(egui::Key::Escape)],
            Default::default(),
        );
        if app.composer_legend_armed() {
            return Err("Legend control Cancel failed".into());
        }
        let cold = app.composer_legend_preview_builds();
        for _ in 0..8 {
            frame(&ctx, &mut app, &mut time, vec![], Default::default());
        }
        if app.composer_legend_preview_builds() != cold {
            return Err("Warm legend layout rebuilt".into());
        }
    }
    controls["atomic_refusal"] = json!(true);
    controls["warm_preview"] = json!(true);
    std::fs::write(
        out.join("native-legend.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":cases,"states":states,"actual_egui":true,"unknown_metadata":true,"source_failed_geometry_history":true,"controls":controls}),
        )?,
    )?;
    Ok(())
}
