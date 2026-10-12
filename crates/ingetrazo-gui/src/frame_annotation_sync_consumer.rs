//! Six annotation families through actual App toolbar, property and pointer input.
//! MetalProducer observes immutable callbacks only; this is no GPU pixel claim.
use crate::app::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::Path;
type Error = Box<dyn std::error::Error>;
fn open(source: &Path, file: &str, session: &Path) -> Result<IngeTrazoApp, Error> {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    let mut store = crate::session_state::SessionStore::isolated(session);
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
    app.native_composer.factory = Some(crate::composer_backend::Factory::MetalProducer(
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

use glam::DVec3;
use ingetrazo_core::composition::SheetItem;
fn frame_id(app: &IngeTrazoApp, uid: &str) -> Result<uuid::Uuid, Error> {
    let s = &app.scene.sheets[0];
    s.items
        .iter()
        .find_map(|i| match i {
            SheetItem::ViewFrame(f)
                if s.native_items
                    .get(&f.id)
                    .and_then(|v| v.get("uid"))
                    .and_then(Value::as_str)
                    == Some(uid) =>
            {
                Some(f.id)
            }
            _ => None,
        })
        .ok_or_else(|| format!("Missing source frame {uid}").into())
}
fn pointer(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: &mut f64,
    event: &Value,
) -> Result<(), Error> {
    let pos = pixel(ctx, &event["effective_at"])?;
    let op = event["op"].as_str().ok_or("Pointer op")?;
    if op == "press" {
        frame(
            ctx,
            app,
            time,
            vec![egui::Event::PointerMoved(pos)],
            Default::default(),
        );
    }
    let e = match op {
        "press" | "release" => egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: op == "press",
            modifiers: Default::default(),
        },
        "move" => egui::Event::PointerMoved(pos),
        _ => return Err("Unknown source pointer op".into()),
    };
    frame(ctx, app, time, vec![e], Default::default());
    Ok(())
}
fn geometry(app: &IngeTrazoApp) -> Value {
    let m = &app.scene.mesh;
    json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|json!({"vertices":[e.v0,e.v1],"soft":e.soft,"curve":e.curve_id,"layer":e.layer,"hidden":e.hidden})).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|json!({"loop":f.vertices,"holes":f.holes,"interior":f.interior,"attrs":f.attrs})).collect::<Vec<_>>()})
}
fn model_annotations(app: &IngeTrazoApp, doc: &Value) -> Value {
    let dimensions=app.scene.dimensions.iter().map(|d|{
  let(a,b)=app.scene.dimension_points(d);
  let(_,_,value)=ingetrazo_render::annotations::dimension_geometry(ingetrazo_render::annotations::DimensionInput{a:a.as_vec3(),b:b.as_vec3(),offset:d.offset.as_vec3(),axis:d.axis.as_deref(),text:d.text_override.as_deref(),highlighted:false,hidden:false});
  json!({"a":a.to_array(),"b":b.to_array(),"offset":d.offset.to_array(),"axis":d.axis,"layer":d.layer,"text":d.text_override.as_deref().unwrap_or(""),"value":value,"anchor_a":d.anchor_a.is_some(),"anchor_b":d.anchor_b.is_some()})
 }).collect::<Vec<_>>();
    json!({"dimensions":dimensions,"text_labels":doc["text_labels"],"geo_paths":doc["geo_paths"],"section_planes":doc["section_planes"],"dimension_style":doc["dimension_style"]})
}
fn viewport_context(app: &IngeTrazoApp) -> Value {
    // Cold protocol observation only: no mesh, texture, tile pixels, or whole Scene copy.
    json!({"camera":app.camera,"layers":app.scene.layers,"section_planes":app.scene.section_planes,"show_section_cuts":app.scene.show_section_cuts,"show_section_planes":app.scene.show_section_planes,"show_section_fill":app.scene.show_section_fill,"display_style":app.scene.display_style,"shadows":app.scene.shadows,"geo_datum":app.scene.geo_datum,"geo_points":app.scene.geo_points,"geo_paths":app.scene.geo_paths})
}
fn observe(app: &mut IngeTrazoApp, name: &str, phase: &str) -> Result<Value, Error> {
    app.refresh_composer_frame_anchors()?;
    let before_context = viewport_context(app);
    let sheet = app.scene.sheets[0].id;
    let frames = app.scene.sheets[0]
        .items
        .iter()
        .filter_map(|i| match i {
            SheetItem::ViewFrame(f) => Some((
                f.id,
                app.scene.sheets[0]
                    .native_items
                    .get(&f.id)
                    .and_then(|v| v.get("uid"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut overlay = serde_json::Map::new();
    let mut snaps = serde_json::Map::new();
    for (id, uid) in frames {
        overlay.insert(
            uid.clone(),
            app.composer_frame_annotation_observation(sheet, id)?,
        );
        let pairs = app.composer_frame_snap_pairs(sheet, id)?;
        snaps.insert(uid,json!({"page":pairs.iter().map(|p|p.page).collect::<Vec<_>>(),"world":pairs.iter().map(|p|p.world.to_array()).collect::<Vec<_>>()}));
    }
    if viewport_context(app) != before_context {
        return Err("Frame operation leaked live viewport state".into());
    }
    let doc = ingetrazo_formats::igz::scene_to_igz_document(&app.scene);
    let mut comp = doc["compositions"][0].clone();
    // Only owned empty six-family inventories differ in explicit-deletion persistence.
    for key in [
        "cotas",
        "cotas_ang",
        "cotas_rad",
        "etiquetas",
        "niveles",
        "llamadas",
    ] {
        if comp
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            comp.as_object_mut().unwrap().remove(key);
        }
    }
    let h = app.composer_state.sheet_history.get(&sheet);
    Ok(
        json!({"case":name,"phase":phase,"composition":comp,"model_geometry":geometry(app),"model_annotations":model_annotations(app,&doc),"overlay_by_frame":overlay,"snap_by_frame":snaps,"sheet_undo":h.map_or(0,|h|h.0.len()),"sheet_redo":h.map_or(0,|h|h.1.len()),"model_undo":app.undo_stack.len(),"model_redo":app.redo_stack.len(),"viewport_restored":true,"source_errors":0}),
    )
}
pub fn run(source: &Path, out: &Path) -> Result<(), Error> {
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    std::fs::create_dir_all(out)?;
    let data: Value = serde_json::from_slice(&std::fs::read(
        source.join("original-frame-annotations.json"),
    )?)?;
    let recipes = data["cases"].as_array().ok_or("Source case roster")?;
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut idle = true;
    let mut invalidated = true;
    for recipe in recipes {
        let name = recipe["name"].as_str().ok_or("Case name")?;
        let mut app = open(
            source,
            recipe["input"].as_str().ok_or("Input file")?,
            &out.join("sessions").join(name),
        )?;
        let ctx = egui::Context::default();
        let mut time = 0.;
        for _ in 0..3 {
            frame(&ctx, &mut app, &mut time, vec![], Default::default());
        }
        for action in recipe["actions"].as_array().ok_or("Actions")? {
            let op = action["op"].as_str().ok_or("Action op")?;
            let phase = action["phase"].as_str().ok_or("Phase")?;
            let before_anchor = app.composer_frame_anchor_stats().builds;
            let before_overlay = app.composer_frame_overlay_stats().0;
            match op {
                "observe" => {}
                "refresh" => {
                    app.sync_composer_frame_anchors()?;
                }
                "frame_properties" => {
                    let id = frame_id(&app, action["frame"].as_str().ok_or("Frame UID")?)?;
                    let sid = app.scene.sheets[0].id;
                    app.select_composer_frame(sid, id)?;
                    app.edit_composer_frame(sid, id, action["fields"].clone())?;
                }
                "frame_move" => {
                    click(&ctx, &mut app, &mut time, "annotation-ui-select")?;
                    let id = frame_id(&app, action["frame"].as_str().ok_or("Frame UID")?)?;
                    let sid = app.scene.sheets[0].id;
                    app.select_composer_frame(sid, id)?;
                    for e in action["pointer_actions"]
                        .as_array()
                        .ok_or("Source effective body actions")?
                    {
                        pointer(&ctx, &mut app, &mut time, e)?;
                    }
                }
                "create_anchored_cota" => {
                    click(&ctx, &mut app, &mut time, "annotation-ui-menu")?;
                    click(&ctx, &mut app, &mut time, "annotation-ui-add-0")?;
                    for e in action["pointer_actions"]
                        .as_array()
                        .ok_or("Source effective cota actions")?
                    {
                        pointer(&ctx, &mut app, &mut time, e)?;
                    }
                    click(&ctx, &mut app, &mut time, "annotation-ui-select")?;
                }
                "sheet_undo" => app.undo_composer_edit(),
                "sheet_redo" => app.redo_composer_edit(),
                "model_move" => {
                    let delta = action["delta_m"].as_array().ok_or("Model delta")?;
                    let delta = glam::Vec3::new(
                        delta[0].as_f64().ok_or("Delta X")? as f32,
                        delta[1].as_f64().ok_or("Delta Y")? as f32,
                        delta[2].as_f64().ok_or("Delta Z")? as f32,
                    );
                    let moves = action["vertices"]
                        .as_array()
                        .ok_or("Vertices")?
                        .iter()
                        .map(|v| {
                            let i = v.as_u64().ok_or("Vertex index")? as usize;
                            let p = app
                                .scene
                                .mesh
                                .vertices
                                .get(i)
                                .ok_or("Missing moved vertex")?;
                            Ok((
                                i,
                                p.uid.as_u128(),
                                (p.position.as_vec3() + delta).as_dvec3(),
                            ))
                        })
                        .collect::<Result<Vec<_>, Error>>()?;
                    app.save_undo_state();
                    app.scene.mesh.place_vertices(moves.into_iter());
                    app.scene.refresh_dimensions();
                    app.scene.version = app.scene.version.wrapping_add(1);
                    app.sync_composer_frame_anchors()?;
                }
                "model_undo" | "model_redo" => {
                    let active = app.scene.active_sheet_index.take();
                    if op == "model_undo" {
                        app.undo()
                    } else {
                        app.redo()
                    }
                    app.scene.active_sheet_index = active;
                    app.sync_composer_frame_anchors()?;
                }
                "frame_delete" => {
                    let id = frame_id(&app, action["frame"].as_str().ok_or("Frame UID")?)?;
                    let sid = app.scene.sheets[0].id;
                    app.select_composer_frame(sid, id)?;
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![key(egui::Key::Delete)],
                        Default::default(),
                    );
                }
                "fixture_style" => {
                    let style = action["style"].as_object().ok_or("Fixture style")?;
                    let mut doc = app.scene.dimension_style().clone();
                    for (k, v) in style {
                        doc[k] = v.clone();
                    }
                    app.scene.dimension_style_override = Some(doc);
                    app.scene.version = app.scene.version.wrapping_add(1);
                }
                "save" => {
                    app.save_to_path(&out.join(format!("{name}-native.igz")));
                    if app.current_file.as_deref()
                        != Some(out.join(format!("{name}-native.igz")).as_path())
                    {
                        return Err(app.status_message.clone().into());
                    }
                }
                _ => return Err(format!("Unknown source action {op}").into()),
            }
            let state = observe(&mut app, name, phase)?;
            if matches!(op, "frame_properties" | "model_move")
                && !app.scene.sheets[0].items.is_empty()
            {
                invalidated &= app.composer_frame_anchor_stats().builds > before_anchor
                    || app.composer_frame_overlay_stats().0 > before_overlay;
            }
            states.push(state);
            std::fs::write(
                out.join("native-frame-annotations.partial.json"),
                serde_json::to_vec_pretty(&json!({"cases":cases,"states":states}))?,
            )?;
        }
        let before = observe(&mut app, name, "idle")?;
        let a = app.composer_frame_anchor_stats().builds;
        let o = app.composer_frame_overlay_stats().0;
        for _ in 0..8 {
            let next = observe(&mut app, name, "idle")?;
            idle &= next == before
                && app.composer_frame_anchor_stats().builds == a
                && app.composer_frame_overlay_stats().0 == o;
        }
        cases.push(json!({"name":name,"saved":format!("{name}-native.igz")}));
    }
    if !idle || !invalidated {
        return Err("Frame cache idle/invalidation control failed".into());
    }
    std::fs::write(
        out.join("native-frame-annotations.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":cases,"states":states,"controls":{"idle_reused":idle,"changed_invalidated":invalidated,"actual_cota_ingress":true,"actual_frame_move":true},"scope":"Actual App/egui cota and frame body ingress; shared frame controller; App model history plus core admitted vertex placement (no OS model command claim). Only source-defined empty annotation inventories omitted in known composition observation; saved original-reader payload remains authoritative."}),
        )?,
    )?;
    Ok(())
}
