//! Consumes original actions through actual App frames and submitted paint callbacks.
use super::*;
fn callbacks(
    shape: &egui::Shape,
    ids: &[u64],
    out: &mut Vec<ingetrazo_render::cad_callback::Snapshot>,
) -> Result<(), Error> {
    match shape {
        egui::Shape::Vec(v) => {
            for s in v {
                callbacks(s, ids, out)?
            }
        }
        egui::Shape::Callback(c) => {
            if let Some(c) = c
                .callback
                .downcast_ref::<ingetrazo_render::cad_callback::Callback>()
            {
                let s = c.snapshot()?;
                if ids.contains(&s.geometry.id()) {
                    out.push(s);
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn mul(a: u8, b: u8) -> u8 {
    let v = u32::from(a) * u32::from(b) + 128;
    ((v + (v >> 8)) >> 8) as u8
}
fn capture(
    ctx: &egui::Context,
    output: &egui::FullOutput,
    path: &Path,
) -> Result<(usize, bool), Error> {
    let ids = ctx
        .data(|d| d.get_temp::<Vec<u64>>(egui::Id::new("run-preview-submitted-geometries")))
        .unwrap_or_default();
    let mut submitted = vec![];
    for s in &output.shapes {
        callbacks(&s.shape, &ids, &mut submitted)?;
    }
    let transform = ctx
        .data(|d| d.get_temp::<[f32; 3]>(egui::Id::new("pdf-ui-paper-transform")))
        .ok_or("Missing paper transform")?;
    assert_eq!(transform[2], 2.);
    assert_eq!(transform[0].fract(), 0.);
    assert_eq!(transform[1].fract(), 0.);
    let mut rgb = vec![255u8; 594 * 420 * 3];
    for s in &submitted {
        let spans = s
            .geometry
            .source_spans()
            .ok_or("Actual preview callback lacks source spans")?;
        let translation = spans.translation(s.pose, 1.)?;
        // The submitted packet supports exact byte palettes and straight encoded
        // batch uniforms. The latter uses the production Qt brush conversion.
        s.geometry.validate_batches(&s.batches)?;
        assert_eq!(s.batches.len(), 1);
        let color = if let Some(colors) = s.geometry.premultiplied_palette() {
            assert_eq!(colors.len(), 1);
            colors[0].bytes()
        } else {
            s.batches[0]
                .color
                .qt_premultiplied8()?
                .map(|v| (v * 255.).round() as u8)
        };
        for span in spans.rows() {
            let y = span.y + translation[1] - transform[1] as i32;
            if !(0..420).contains(&y) {
                continue;
            }
            for x in span.x..span.x + span.len {
                let x = x + translation[0] - transform[0] as i32;
                if !(0..594).contains(&x) {
                    continue;
                }
                let i = (y as usize * 594 + x as usize) * 3;
                let alpha = mul(color[3], span.coverage);
                for c in 0..3 {
                    rgb[i + c] =
                        mul(color[c], span.coverage).saturating_add(mul(rgb[i + c], 255 - alpha));
                }
            }
        }
    }
    image::save_buffer(path, &rgb, 594, 420, image::ColorType::Rgb8)?;
    Ok((submitted.len(), rgb.iter().any(|v| *v != 255)))
}
fn observation(app: &IngeTrazoApp) -> Value {
    let s = state(app, "", "");
    let pending = app.composer_specialized_pending();
    json!({"compositions":s["all_compositions"],"sheet_index":s["sheet_index"],"selection":app.composer_annotation_selections().iter().map(|(_,i)|*i).collect::<Vec<_>>(),"panel_owner":app.composer_annotation_selection().map(|(_,_,i)|i),"undo":s["undo"],"redo":s["redo"],"mode":s["mode"],"pending":{"run_points":pending["run_points"],"run_sep":pending["run_sep"],"run_axis":pending["run_axis"],"run_cotas":pending["run_cotas"]}})
}
pub(super) fn run(source: &Path, out: &Path) -> Result<(), Error> {
    let source = source.canonicalize()?;
    std::fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    std::env::set_current_dir(&source)?;
    unsafe {
        std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    }
    let source_report: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-run-previews.json"))?)?;
    let mut states = vec![];
    let mut cases = vec![];
    let mut controls = vec![];
    for recipe in source_report["cases"].as_array().ok_or("Cases")? {
        let name = recipe["name"].as_str().ok_or("Name")?;
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
        // Declared device pose: integer origin at density2, matching actual source item paint.
        let t = ctx
            .data(|d| d.get_temp::<[f32; 3]>(egui::Id::new("pdf-ui-paper-transform")))
            .ok_or("Paper")?;
        app.composer_state.pan_offset += egui::vec2(t[0].round() - t[0], t[1].round() - t[1]);
        frame(&ctx, &mut app, &mut time, vec![], Default::default());
        let mut pointers = 0;
        for a in recipe["actions"].as_array().ok_or("Actions")? {
            let phase = a["phase"].as_str().ok_or("Phase")?;
            let op = a["op"].as_str().ok_or("Op")?;
            let before = app.scene.clone();
            let before_history = observation(&app);
            let ids = app
                .scene
                .sheets
                .iter()
                .map(|s| s.annotation_ids.clone())
                .collect::<Vec<_>>();
            match op {
                "hover" => {
                    let p = pixel(&ctx, &a["effective_at"])?;
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![egui::Event::PointerMoved(p)],
                        modifiers(a),
                    );
                    pointers += 1;
                }
                "shift" => {
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![],
                        egui::Modifiers {
                            shift: a["down"] == true,
                            ..Default::default()
                        },
                    );
                }
                "key" => {
                    view_focus(&ctx);
                    frame(
                        &ctx,
                        &mut app,
                        &mut time,
                        vec![key(egui::Key::Escape)],
                        Default::default(),
                    );
                }
                "select_fixture" => app.composer_select_annotation(
                    0,
                    Some(a["index"].as_u64().ok_or("Index")? as usize),
                )?,
                _ => action(&ctx, &mut app, &mut time, a)?,
            }
            if op == "hover" || op == "shift" {
                assert_eq!(app.scene, before, "Hover document mutation");
                assert_eq!(
                    app.scene
                        .sheets
                        .iter()
                        .map(|s| s.annotation_ids.clone())
                        .collect::<Vec<_>>(),
                    ids,
                    "Hover identity mutation"
                );
                let h = observation(&app);
                assert_eq!(h["undo"], before_history["undo"]);
                assert_eq!(h["redo"], before_history["redo"]);
            }
            ctx.data_mut(|d| {
                d.remove::<Vec<u64>>(egui::Id::new("run-preview-submitted-geometries"))
            });
            let paint = frame(&ctx, &mut app, &mut time, vec![], Default::default());
            let file = format!("{name}-{phase}-preview.png");
            let (submitted, nonempty) = capture(&ctx, &paint, &out.join(&file))?;
            let mut preview = app.composer_run_preview_observation();
            let cache_stats =
                json!({"preparations":preview["preparations"],"canvas":preview["canvas"]});
            preview.as_object_mut().unwrap().remove("preparations");
            preview.as_object_mut().unwrap().remove("canvas");
            let state = observation(&app);
            let version = app.scene.version;
            states.push(json!({"case":name,"phase":phase,"state":state,"preview":preview,"paint":{"file":file,"submitted_callbacks":submitted,"nonempty":nonempty,"density":2.,"dpr":1.,"clip_mm":[0.,0.,297.,210.],"background":[255,255,255,255],"page_to_device":[2.,0.,0.,0.,2.,0.,0.,0.,1.]},"observer_readonly":true,"native_scene_version":version,"cache_stats":cache_stats}));
            let stats = app.composer_run_preview_observation();
            for _ in 0..3 {
                frame(&ctx, &mut app, &mut time, vec![], Default::default());
            }
            let idle = app.composer_run_preview_observation();
            assert_eq!(stats["preparations"], idle["preparations"]);
            assert_eq!(stats["canvas"][0], idle["canvas"][0]);
            assert_eq!(observation(&app), state);
            std::fs::write(
                out.join("native-run-previews.partial.json"),
                serde_json::to_vec_pretty(&json!({"cases":cases,"states":states}))?,
            )?;
        }
        let saved = format!("{name}-native.igz");
        app.save_to_path(&out.join(&saved));
        assert!(out.join(&saved).is_file());
        cases.push(json!({"name":name,"saved":saved}));
        controls.push(json!({"case":name,"actual_pointer_ingress":pointers>0,"hover_readonly":true,"idle_reused":true}));
    }
    std::fs::write(
        out.join("native-run-previews.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":cases,"states":states,"controls":controls,"actual_app_paint":true,"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","failures":[]}),
        )?,
    )?;
    println!("{}", json!({"cases":cases.len(),"states":states.len()}));
    Ok(())
}
