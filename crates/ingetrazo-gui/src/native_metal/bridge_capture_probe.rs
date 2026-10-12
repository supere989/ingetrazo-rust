//! Real host/TCP capture admission and bounded owner-lifecycle probe.
use crate::IngeTrazoApp;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::PathBuf,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) struct Probe {
    pub input: PathBuf,
    output: PathBuf,
    recipe: Value,
    started: Instant,
    worker: Option<JoinHandle<()>>,
    receiver: Option<mpsc::Receiver<Result<Vec<Value>, String>>>,
    baseline: Option<Value>,
    done: bool,
    port: Option<u16>,
    phase: u8,
    rows: Vec<Value>,
}
fn observation(app: &IngeTrazoApp) -> Value {
    let r = app.native_render_revisions;
    json!({"document":ingetrazo_formats::igz::scene_to_igz_document(&app.scene),
        "undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"dirty":app.document_dirty(),
        "file":app.current_file,"camera":app.camera,"edit_depth":app.component_edit_stack.len(),
        "scene_version":app.scene.version,"view_version":app.scene.view_version,
        "model_revision":app.model_revision,"native_geometry_revision":app.native_geometry_revision,
        "render_epochs":[r.static_domains.geometry,r.static_domains.paint,r.static_domains.style,
            r.static_domains.layers,r.static_domains.topology,r.poses]})
}

impl Probe {
    pub fn new(source: PathBuf, output: PathBuf) -> Result<Self, String> {
        let source = source.canonicalize().map_err(|e| e.to_string())?;
        if output.exists() {
            return Err("Metal capture probe requires fresh output".into());
        }
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        let report: Value = serde_json::from_slice(
            &std::fs::read(source.join("original-ai-bridge-capture.json"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if report["source_pin"] != "6be29fe437a7cd992d4e079f71720821b25216b9" {
            return Err("Original source pin mismatch".into());
        }
        let recipe = report["cases"]
            .as_array()
            .ok_or("Source cases absent")?
            .iter()
            .find(|c| c["name"] == "committed-selected-overlays")
            .ok_or("Source overlay fixture absent")?
            .clone();
        let input = source.join(recipe["input"].as_str().ok_or("Source input absent")?);
        Ok(Self {
            input,
            output,
            recipe,
            started: Instant::now(),
            worker: None,
            receiver: None,
            baseline: None,
            done: false,
            port: None,
            phase: 0,
            rows: vec![],
        })
    }
    pub fn setup(&mut self, app: &mut IngeTrazoApp) -> Result<(), String> {
        let camera = &self.recipe["initial_camera"];
        let nums = |v: &Value| v.as_f64().ok_or("Declared camera number absent");
        app.camera.target = glam::DVec3::new(
            nums(&camera["target"][0])?,
            nums(&camera["target"][1])?,
            nums(&camera["target"][2])?,
        );
        app.camera.distance = nums(&camera["distance"])?;
        app.camera.yaw = nums(&camera["yaw"])?;
        app.camera.pitch = nums(&camera["pitch"])?;
        app.camera.fov_deg = nums(&camera["fov"])?;
        app.camera.perspective = camera["perspective"]
            .as_bool()
            .ok_or("Declared camera perspective absent")?;
        if app.scene.dimensions.is_empty() || app.scene.text_labels.is_empty() {
            return Err("Actual committed dimension/text fixture required".into());
        }
        if self.recipe["selected_annotations"].as_bool() == Some(true) {
            app.scene
                .selection
                .insert(ingetrazo_core::scene::SelectionItem::Dimension(0));
            app.scene
                .selection
                .insert(ingetrazo_core::scene::SelectionItem::TextLabel(0));
        }
        app.current_file = self.recipe["initial_file"].as_str().map(PathBuf::from);
        if self.recipe["initial_dirty"].as_bool() == Some(true) {
            app.document_session.clean = None;
        }
        Ok(())
    }
    pub fn tick(&mut self, state: &mut super::State, ctx: &egui::Context) -> Result<bool, String> {
        let app = &mut state.app;
        if self.started.elapsed() > Duration::from_secs(45) {
            return Err("Metal host/TCP capture probe deadline exceeded".into());
        }
        if self.done {
            return Ok(true);
        }
        if self.worker.is_none() {
            if !app.ai_bridge_metal_capture_ready {
                return Ok(false);
            }
            let baseline = observation(app);
            // Actual production protocol with a declared shorter UI deadline for
            // the owned blocked-submission negative control. Shipping default
            // 120-second bridge admission is unchanged.
            let wake = ctx.clone();
            let bridge = ingetrazo_core::ai_bridge::Bridge::start(
                0,
                ingetrazo_core::ai_bridge::Limits {
                    ui_timeout: Duration::from_secs(5),
                    ..Default::default()
                },
                move || wake.request_repaint(),
            )
            .map_err(|e| e.to_string())?;
            let port = bridge.address().port();
            app.ai_bridge = Some(bridge);
            let output = self.output.clone();
            let (tx, rx) = mpsc::channel();
            self.worker = Some(thread::spawn(move || {
                let result = (|| -> Result<Vec<Value>, String> {
                    let mut socket =
                        TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
                    socket
                        .set_read_timeout(Some(Duration::from_secs(12)))
                        .map_err(|e| e.to_string())?;
                    let mut reader = BufReader::new(socket.try_clone().map_err(|e| e.to_string())?);
                    let mut rows = vec![];
                    for (index, (width, height)) in
                        [(1, 1), (120, 180), (640, 96)].into_iter().enumerate()
                    {
                        let request = json!({"id":index,"tool":"screenshot","args":{"width":width,"height":height}});
                        let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
                        // Actual newline framing through the registered serial bridge.
                        socket
                            .write_all(&bytes[..bytes.len() / 2])
                            .map_err(|e| e.to_string())?;
                        socket
                            .write_all(&bytes[bytes.len() / 2..])
                            .map_err(|e| e.to_string())?;
                        socket.write_all(b"\n").map_err(|e| e.to_string())?;
                        let mut line = String::new();
                        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                            return Err("TCP bridge closed without reply".into());
                        }
                        let reply: Value =
                            serde_json::from_str(&line).map_err(|e| e.to_string())?;
                        if reply["id"] != index {
                            return Err("Actual capture wire id mismatch".into());
                        }
                        let name = format!("capture-{index}.png");
                        if reply["ok"] == true {
                            let file = PathBuf::from(
                                reply["result"]["path"]
                                    .as_str()
                                    .ok_or("Actual PNG path absent")?,
                            );
                            let encoded = std::fs::read(&file).map_err(|e| e.to_string())?;
                            std::fs::write(output.join(&name), &encoded)
                                .map_err(|e| e.to_string())?;
                            let image = image::load_from_memory(&encoded)
                                .map_err(|e| e.to_string())?
                                .into_rgba8();
                            if image.width() != width.max(64) || image.height() != height.max(64) {
                                return Err("Actual capture clamp/extent mismatch".into());
                            }
                            if image
                                .as_raw()
                                .chunks_exact(4)
                                .all(|p| p == &image.as_raw()[..4])
                            {
                                return Err("Actual screenshot unexpectedly uniform".into());
                            }
                            std::fs::write(
                                output.join(format!("capture-{index}.rgba")),
                                image.as_raw(),
                            )
                            .map_err(|e| e.to_string())?;
                        }
                        rows.push(json!({"request":request,"reply":reply,"file":name}));
                    }
                    Ok(rows)
                })();
                let _ = tx.send(result);
            }));
            self.receiver = Some(rx);
            self.baseline = Some(baseline);
            self.port = Some(port);
            return Ok(false);
        }
        match self.receiver.as_ref().unwrap().try_recv() {
            Ok(result) => {
                self.worker
                    .take()
                    .unwrap()
                    .join()
                    .map_err(|_| "Capture TCP worker panicked")?;
                let rows = result?;
                let after = observation(app);
                let readonly = self.baseline.as_ref() == Some(&after);
                if self.phase == 0 {
                    self.rows = rows;
                    if !readonly || !self.rows.iter().all(|r| r["reply"]["ok"] == true) {
                        std::fs::write(
                            self.output.join("partial.json"),
                            serde_json::to_vec_pretty(
                                &json!({"rows":self.rows,"before":self.baseline,"after":after}),
                            )
                            .unwrap(),
                        )
                        .map_err(|e| e.to_string())?;
                        return Err(
                            "Actual positive capture refused or mutated document; retained replies"
                                .into(),
                        );
                    }
                    state.capture.block_next_for_probe();
                    let port = self.port.unwrap();
                    let (tx, rx) = mpsc::channel();
                    self.worker = Some(thread::spawn(move || {
                        let result = (|| -> Result<Vec<Value>, String> {
                            let mut socket = TcpStream::connect(("127.0.0.1", port))
                                .map_err(|e| e.to_string())?;
                            socket
                                .set_read_timeout(Some(Duration::from_secs(12)))
                                .map_err(|e| e.to_string())?;
                            let mut reader =
                                BufReader::new(socket.try_clone().map_err(|e| e.to_string())?);
                            let mut rows = vec![];
                            for id in ["blocked-deadline", "poisoned-backend"] {
                                let request = json!({"id":id,"tool":"screenshot","args":{"width":64,"height":64}});
                                socket
                                    .write_all(serde_json::to_string(&request).unwrap().as_bytes())
                                    .map_err(|e| e.to_string())?;
                                socket.write_all(b"\n").map_err(|e| e.to_string())?;
                                let started = Instant::now();
                                let mut line = String::new();
                                if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                                    return Err("Negative TCP reply absent".into());
                                }
                                let reply: Value =
                                    serde_json::from_str(&line).map_err(|e| e.to_string())?;
                                rows.push(json!({"request":request,"reply":reply,"seconds":started.elapsed().as_secs_f64()}));
                            }
                            Ok(rows)
                        })();
                        let _ = tx.send(result);
                    }));
                    self.receiver = Some(rx);
                    self.phase = 1;
                    return Ok(false);
                }
                self.rows.extend(rows);
                let events = state.capture.diagnostics().to_vec();
                let expected_extents = [[64u32, 64], [120, 180], [640, 96], [64, 64]];
                let inventory = events.len() == 4
                    && events.iter().zip(expected_extents).enumerate().all(
                        |(i, (event, extent))| {
                            event["id"].as_u64() == Some(i as u64 + 1)
                                && event["requested"] == json!(extent)
                                && event["live_depth_extent"].as_array().is_some_and(|v| {
                                    v.len() == 2
                                        && v.iter().all(|n| n.as_u64().is_some_and(|n| n > 0))
                                })
                                && event["live_depth_samples"].as_u64() == Some(4)
                        },
                    );
                let timeout = inventory
                    && events[3]["blocked_gate"] == true
                    && events[3]["retirement"] == "Quarantined"
                    && self.rows[3]["reply"]["id"] == "blocked-deadline"
                    && self.rows[3]["reply"]["ok"] == false
                    && self.rows[3]["reply"]["error"]
                        .as_str()
                        .is_some_and(|e| e.contains("timed out"));
                let poison = self.rows.last().is_some_and(|row| {
                    row["reply"]["id"] == "poisoned-backend"
                        && row["reply"]["ok"] == false
                        && row["reply"]["error"]
                            == "Native viewport capture requires an admitted live backend"
                        && row["seconds"].as_f64().is_some_and(|s| s < 2.)
                });
                let live_depth = inventory
                    && events[..3].iter().all(|event| {
                        event["blocked_gate"] == false
                            && event["retirement"] == "Completed"
                            && event["source_frame"] != event["target_frame"]
                            && event["live_depth_extent"] != event["requested"]
                    });
                state.capture.release_gate_for_probe();
                app.stop_ai_bridge();
                let report = json!({"scope":"Actual native Metal host/TCP finite capture probe; no source pixel acceptance inferred",
                    "port":self.port,"fixture_ui_timeout_seconds":5,"rows":self.rows,"before":self.baseline,"after":after,"backend_events":events,
                    "controls":{"actual_host":true,"actual_tcp":true,"readonly_model_history_camera":readonly,"worker_joined":true,"bridge_stopped":true,"blocked_deadline_quarantine":timeout,"poisoned_backend_quick_refusal":poison,"retained_live_depth_owner":live_depth},
                    "all_positive_requests_successful":true});
                std::fs::write(
                    self.output.join("report.json"),
                    serde_json::to_vec_pretty(&report).unwrap(),
                )
                .map_err(|e| e.to_string())?;
                self.done = true;
                if !readonly || !timeout || !poison || !live_depth {
                    return Err("Actual Metal capture lifecycle guard failed".into());
                }
                Ok(true)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn finish(&mut self) -> Result<(), String> {
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| "Capture worker cleanup panicked")?;
        }
        if !self.done {
            return Err("Metal capture probe did not reach terminal report".into());
        }
        Ok(())
    }
}
