//! Retained original picture authoring through App transactions and real egui controls.
use ingetrazo_gui::app::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
type Error = Box<dyn std::error::Error>;
const PROPS: [&str; 6] = [
    "shape",
    "radius_mm",
    "feather_mm",
    "fit",
    "border",
    "border_mm",
];
fn composition(app: &IngeTrazoApp) -> Value {
    ingetrazo_formats::igz::scene_to_igz_document(&app.scene)["compositions"][0].clone()
}
fn state(app: &IngeTrazoApp, case: &str, phase: &str) -> Value {
    let sheet = &app.scene.sheets[0];
    let history = app.composer_state.sheet_history.get(&sheet.id);
    json!({"case":case,"phase":phase,"composition":composition(app),
        "selection":app.composer_picture_selection().map(|v|v.1),
        "undo":history.map_or(0,|v|v.0.len()),"redo":history.map_or(0,|v|v.1.len())})
}
fn open(source: &Path, input: &str, session: &Path) -> Result<IngeTrazoApp, Error> {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        session,
    ));
    app.mark_document_clean();
    app.open_document(&source.join(input));
    if app.document_session.confirm_unsaved || app.scene.sheets.len() != 1 {
        return Err("Original authoring input was not opened".into());
    }
    app.enter_composer_sheet(0);
    app.scene.active_sheet_index = Some(0);
    Ok(app)
}
fn action(app: &mut IngeTrazoApp, row: &Value, out: &Path, name: &str) -> Result<(), Error> {
    let path = row["path"].as_str().map(Path::new);
    match row["op"].as_str().ok_or("Missing authoring operation")? {
        "observe" => {}
        "add" => {
            app.composer_add_picture(path)?;
        }
        "select" => app.composer_select_picture(Some(
            row["index"].as_u64().ok_or("Picture index")? as usize,
        ))?,
        "replace" => app.composer_replace_picture(path)?,
        "property" | "property-noop" => {
            let index = app
                .composer_picture_selection()
                .ok_or("Missing selected picture")?
                .1;
            let field = row["field"].as_str();
            let record = &app.scene.sheets[0].native_document["images"][index];
            let mut changes = serde_json::Map::new();
            if field == Some("opacity") {
                changes.insert(
                    "opacity".into(),
                    json!(row["value"].as_f64().ok_or("Opacity percent")? / 100.),
                );
            } else {
                for key in PROPS {
                    changes.insert(key.into(), record[key].clone());
                }
                if let Some(field) = field {
                    changes.insert(field.into(), row["value"].clone());
                }
            }
            app.composer_edit_picture(&Value::Object(changes))?;
        }
        "undo" => app.undo_composer_edit(),
        "redo" => app.redo_composer_edit(),
        "save" => {
            let path = out.join(format!("{name}-native.igz"));
            app.save_to_path(&path);
            if !path.is_file() {
                return Err(format!("Native App save failed: {}", app.status_message).into());
            }
        }
        _ => return Err("Unknown authoring operation".into()),
    }
    Ok(())
}
fn frame(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    *time += 0.1;
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280., 900.),
            )),
            time: Some(*time),
            events,
            ..Default::default()
        },
        |ctx| app.update_ui(ctx),
    )
}
fn click(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    id: &str,
) -> Result<(), Error> {
    let rect = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or_else(|| format!("Missing actual egui control: {id}"))?;
    let p = rect.center();
    frame(
        ctx,
        app,
        time,
        vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        ctx,
        app,
        time,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    Ok(())
}
#[cfg(unix)]
fn ui_controls(source: &Path, out: &Path, input: &str) -> Result<Value, Error> {
    use std::os::unix::fs::PermissionsExt;
    struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Environment {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                if let Some(v) = v {
                    std::env::set_var(k, v)
                } else {
                    std::env::remove_var(k)
                }
            }
        }
    }
    let vars = [
        "PATH",
        "INGETRAZO_PDF_UI_OBSERVE",
        "INGETRAZO_AUTHORING_PICKER_REPLY",
        "INGETRAZO_AUTHORING_PICKER_LOG",
    ];
    let _restore = Environment(vars.into_iter().map(|k| (k, std::env::var_os(k))).collect());
    let bin = out.join("owned-picker");
    std::fs::create_dir_all(&bin)?;
    let script = r#"#!/bin/sh
printf '%s\n' "${0##*/}" >> "$INGETRAZO_AUTHORING_PICKER_LOG"
if [ "$INGETRAZO_AUTHORING_PICKER_REPLY" = '__cancel__' ]; then exit 1; fi
printf '%s\n' "$INGETRAZO_AUTHORING_PICKER_REPLY"
"#;
    for name in ["kdialog", "zenity"] {
        let file = bin.join(name);
        std::fs::write(&file, script)?;
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut paths = vec![bin.clone()];
    if let Some(old) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&old));
    }
    std::env::set_var("PATH", std::env::join_paths(paths)?);
    let log = out.join("picker-calls.txt");
    std::env::set_var("INGETRAZO_AUTHORING_PICKER_LOG", &log);
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    let mut app = open(source, input, &out.join("ui-session"))?;
    let ctx = egui::Context::default();
    let mut time = 0.;
    frame(&ctx, &mut app, &mut time, vec![]);
    frame(&ctx, &mut app, &mut time, vec![]);
    let before = state(&app, "ui", "unchanged");
    std::env::set_var("INGETRAZO_AUTHORING_PICKER_REPLY", "__cancel__");
    click(&ctx, &mut app, &mut time, "picture-ui-add")?;
    assert_eq!(
        state(&app, "ui", "unchanged"),
        before,
        "Chooser Cancel mutated state/history"
    );
    assert_eq!(
        std::fs::read_to_string(&log)?,
        "kdialog\n",
        "Cancel launched a second chooser"
    );
    std::env::set_var("INGETRAZO_AUTHORING_PICKER_REPLY", "assets/checker.png");
    let count = composition(&app)["images"]
        .as_array()
        .ok_or("Image array")?
        .len();
    click(&ctx, &mut app, &mut time, "picture-ui-add")?;
    assert_eq!(
        composition(&app)["images"].as_array().unwrap().len(),
        count + 1
    );
    app.composer_select_picture(Some(count))?;
    frame(&ctx, &mut app, &mut time, vec![]);
    frame(&ctx, &mut app, &mut time, vec![]);
    let old = composition(&app)["images"][count].clone();
    click(&ctx, &mut app, &mut time, "picture-ui-border")?;
    assert_eq!(
        composition(&app)["images"][count]["border"],
        json!(!old["border"].as_bool().unwrap())
    );
    let old = composition(&app)["images"][count].clone();
    std::env::set_var("INGETRAZO_AUTHORING_PICKER_REPLY", "assets/person.png");
    click(&ctx, &mut app, &mut time, "picture-ui-replace")?;
    let mut expected = old.clone();
    expected["path"] = json!("assets/person.png");
    assert_eq!(
        composition(&app)["images"][count],
        expected,
        "Replace changed picture geometry/properties"
    );
    assert_eq!(
        std::fs::read_to_string(&log)?,
        "kdialog\nkdialog\nkdialog\n"
    );
    frame(&ctx, &mut app, &mut time, vec![]);
    let before = state(&app, "ui", "warm");
    let pixels = app.sheet_picture_cache_stats();
    let warm = app.composer_picture_preview_stats();
    for _ in 0..8 {
        frame(&ctx, &mut app, &mut time, vec![]);
    }
    assert_eq!(state(&app, "ui", "warm"), before);
    assert_eq!(app.sheet_picture_cache_stats(), pixels);
    assert_eq!(
        app.composer_picture_preview_stats(),
        warm,
        "Unchanged preview repeated preparation, file reads or upload"
    );
    let before = state(&app, "ui", "refusal");
    for invalid in [
        json!({"w_mm":0.}),
        json!({"shape":"invalid"}),
        json!({"opacity":true}),
        json!({"rotation_deg":90.}),
    ] {
        assert!(app.composer_edit_picture(&invalid).is_err());
        assert_eq!(state(&app, "ui", "refusal"), before);
    }
    Ok(
        json!({"actual_egui_add":true,"actual_egui_border":true,"actual_egui_replace":true,"cancel_no_second_chooser":true,"cancel_unchanged":true,"replace_geometry_preserved":true,"warm_scene_and_decode_composite_unchanged":true,"warm_io_and_upload_unchanged":true,"warm_counters_before":warm,"warm_counters_after":app.composer_picture_preview_stats(),"invalid_edits_preserve_history":true,"scope":"Actual App egui controls with owned external-picker response; no physical desktop chooser or raster pixel acceptance"}),
    )
}
#[cfg(not(unix))]
fn ui_controls(_: &Path, _: &Path, _: &str) -> Result<Value, Error> {
    Ok(json!({"scope":"Controlled Unix picker ingress unavailable on this platform"}))
}
fn z_controls(source: &Path, out: &Path) -> Result<Vec<Value>, Error> {
    let controls: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-z-controls.json"))?)?;
    let base = ingetrazo_formats::igz::load_igz(&source.join("authoring-input.igz"))?;
    let original = ingetrazo_formats::igz::scene_to_igz_document(&base);
    let mut rows = vec![];
    for row in controls["rows"]
        .as_array()
        .ok_or("Missing category controls")?
    {
        let category = row["category"].as_str().ok_or("Category")?;
        let mut document = original.clone();
        let item = if category == "cajetin" {
            &mut document["compositions"][0][category]
        } else {
            &mut document["compositions"][0][category][0]
        };
        item["z"] = row["fixture_z"].clone();
        item["hidden"] = json!(true);
        let input = out.join(format!("z-input-{category}.igz"));
        std::fs::write(
            &input,
            serde_json::to_vec(&json!({"igz_format":1,"scene":document}))?,
        )?;
        let mut app = open(
            out,
            input.file_name().unwrap().to_str().unwrap(),
            &out.join(format!("z-{category}-session")),
        )?;
        let before = composition(&app);
        let index = app
            .composer_add_picture(Some(Path::new("assets/checker.png")))?
            .ok_or("No new image")?;
        let added = state(&app, "z", "added");
        let new_z = composition(&app)["images"][index]["z"].clone();
        app.undo_composer_edit();
        let undone = state(&app, "z", "undo");
        rows.push(json!({"category":category,"fixture_z":row["fixture_z"],"new_image_z":new_z,"added_undo":added["undo"],"added_redo":added["redo"],"undo_restored":composition(&app)==before,"undo":undone["undo"],"redo":undone["redo"]}));
    }
    Ok(rows)
}
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let source = PathBuf::from(args.first().ok_or("Missing source directory")?).canonicalize()?;
    let out = PathBuf::from(args.get(1).ok_or("Missing native output")?);
    std::fs::create_dir_all(&out)?;
    let out = out.canonicalize()?;
    std::env::set_current_dir(&source)?;
    let fixture: Value = serde_json::from_slice(&std::fs::read(
        source.join("original-picture-authoring.json"),
    )?)?;
    let cases = fixture["cases"]
        .as_array()
        .ok_or("Missing authoring cases")?;
    assert_eq!(cases.len(), 3);
    let mut states = vec![];
    let mut records = vec![];
    for case in cases {
        let name = case["name"].as_str().ok_or("Case name")?;
        let mut app = open(
            &source,
            case["input"].as_str().ok_or("Case input")?,
            &out.join(format!("{name}-session")),
        )?;
        for row in case["actions"].as_array().ok_or("Case actions")? {
            action(&mut app, row, &out, name)?;
            states.push(state(&app, name, row["phase"].as_str().ok_or("Phase")?));
        }
        records.push(json!({"name":name,"saved":format!("{name}-native.igz")}));
    }
    let mut result = json!({"cases":records,"states":states,"controls":{"result":"incomplete"},"fresh_source_captures":0});
    std::fs::write(out.join("native.json"), serde_json::to_vec_pretty(&result)?)?;
    result["z_controls"] = json!(z_controls(&source, &out)?);
    std::fs::write(out.join("native.json"), serde_json::to_vec_pretty(&result)?)?;
    let controls = ui_controls(&source, &out, cases[1]["input"].as_str().unwrap())?;
    result["controls"] = controls.clone();
    std::fs::write(out.join("native.json"), serde_json::to_vec_pretty(&result)?)?;
    println!(
        "{}",
        json!({"cases":records.len(),"states":states.len(),"controls":controls})
    );
    Ok(())
}
