//! Real native host and TCP screenshot workflow; root alone executes.
#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "exchange_import_workflows.rs"]
mod exchange;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod fixture {
    use crate::exchange;
    use ingetrazo_core::{guides::GuideType, scene::SelectionItem};
    use ingetrazo_gui::{
        native_vulkan::{App, System},
        IngeTrazoApp,
    };
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpStream,
        path::PathBuf,
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };
    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    fn observe(a: &IngeTrazoApp) -> Result<Value, String> {
        let s = &a.scene;
        let c = &a.camera;
        let mut selection = Vec::new();
        for x in &s.selection {
            let (kind, i) = match x {
                SelectionItem::Dimension(i) => ("dimensions", *i),
                SelectionItem::TextLabel(i) => ("text_labels", *i),
                SelectionItem::Face(i) => ("faces", *i),
                SelectionItem::Edge(i) => ("edges", *i),
                SelectionItem::Vertex(i) => ("vertices", *i),
                SelectionItem::Guide(i) => ("guides", *i),
                SelectionItem::Group(i) => ("groups", *i),
                SelectionItem::SectionPlane(i) => ("section_planes", *i),
                SelectionItem::GeoPath(i) => ("geo_paths", *i),
                SelectionItem::GeoPoint(i) => ("geo_points", *i),
                _ => return Err("Undeclared screenshot selection owner".into()),
            };
            selection.push((kind, i));
        }
        selection.sort();
        let dimensions=s.dimensions.iter().map(|d|json!({"a":d.start.to_array(),"b":d.end.to_array(),"offset":d.offset.to_array(),"axis":d.axis,"layer":d.layer,"text":d.text_override})).collect::<Vec<_>>();
        let texts=s.text_labels.iter().map(|t|{let mut v=json!({"anchor":t.anchor.to_array(),"offset":t.offset.to_array(),"text":t.text});if let Some(layer)=&t.layer{if !layer.is_empty(){v["layer"]=json!(layer);}}v}).collect::<Vec<_>>();
        let guides = s
            .guides
            .iter()
            .map(|g| {
                let mut v = match &g.kind {
                    GuideType::Point(p) => json!({"point":p.to_array()}),
                    GuideType::InfiniteLine { point, dir } => {
                        json!({"point":point.to_array(),"direction":dir.to_array()})
                    }
                    GuideType::Segment { a, b } => {
                        json!({"point":a.to_array(),"direction":(*b-*a).to_array()})
                    }
                };
                if let Some(origin) = g.origin {
                    v["origin"] = json!(origin.to_array());
                }
                v
            })
            .collect::<Vec<_>>();
        let sections = s
            .section_planes
            .iter()
            .map(|p| {
                let mut v =
                    json!({"point":p.point.to_array(),"normal":p.normal.to_array(),"uid":p.native_uid()});
                if !p.name.is_empty() {
                    v["name"] = json!(p.name);
                }
                if !p.symbol.is_empty() {
                    v["symbol"] = json!(p.symbol);
                }
                if p.active {
                    v["active"] = json!(true);
                }
                v
            })
            .collect::<Vec<_>>();
        let paths = s
            .geo_paths
            .iter()
            .map(|p| {
                let mut v =
                    json!({"points":p.points.iter().map(|p|p.to_array()).collect::<Vec<_>>()});
                if p.closed {
                    v["closed"] = json!(true);
                }
                if !p.name.is_empty() {
                    v["name"] = json!(p.name);
                }
                if let Some(surface) = &p.surface {
                    v["surface"] = json!(surface);
                }
                v
            })
            .collect::<Vec<_>>();
        let points = s
            .geo_points
            .iter()
            .map(|p| {
                let mut v = json!({"position":p.position.to_array()});
                if !p.name.is_empty() {
                    v["name"] = json!(p.name);
                }
                if !p.desc.is_empty() {
                    v["desc"] = json!(p.desc);
                }
                v
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"selection":selection,"scene":exchange::scene(a)?,"overlays":{"dimensions":dimensions,"text_labels":texts,"guides":guides,"section_planes":sections,"geo_paths":paths,"geo_points":points},"camera":{"target":c.target.to_array(),"distance":c.distance,"yaw":c.yaw,"pitch":c.pitch,"perspective":c.perspective,"fov":c.fov_deg,"aspect":c.aspect,"up":c.up.to_array(),"znear":c.znear,"zfar":c.zfar,"two_point":c.two_point},"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"edit_context":a.component_edit_stack.len(),"current_file":a.current_file,"dirty":a.document_dirty(),"scene_version":s.version}),
        )
    }
    struct Reply {
        index: usize,
        request: Value,
        result: Result<Value, String>,
    }
    struct Fixture {
        app: IngeTrazoApp,
        source: PathBuf,
        out: PathBuf,
        cases: Vec<Value>,
        case: usize,
        warm: usize,
        started: Instant,
        worker: Option<thread::JoinHandle<()>>,
        commands: Option<mpsc::Sender<(usize, Value)>>,
        replies: Option<mpsc::Receiver<Reply>>,
        before: Option<Value>,
        next: usize,
        report: Value,
        failed: Option<String>,
        resize_attempts: usize,
    }
    impl Fixture {
        fn load_case(&mut self) -> Result<(), String> {
            let case = &self.cases[self.case];
            let name = case["name"].as_str().ok_or("Missing case name")?;
            self.app = IngeTrazoApp::new();
            self.app.scene = ingetrazo_formats::load_file(
                &self
                    .source
                    .join(case["input"].as_str().ok_or("Missing input")?),
            )
            .map_err(|e| e.to_string())?;
            self.app.native_gpu_viewport_enabled = true;
            self.app.current_file = Some(
                self.source.join(
                    case["initial_file"]
                        .as_str()
                        .ok_or("Missing declared initial_file")?,
                ),
            );
            let c = &case["initial_camera"];
            self.app.camera.target = glam::DVec3::new(
                c["target"][0].as_f64().ok_or("target")?,
                c["target"][1].as_f64().ok_or("target")?,
                c["target"][2].as_f64().ok_or("target")?,
            );
            self.app.camera.distance = c["distance"].as_f64().ok_or("distance")?;
            self.app.camera.yaw = c["yaw"].as_f64().ok_or("yaw")?;
            self.app.camera.pitch = c["pitch"].as_f64().ok_or("pitch")?;
            self.app.camera.perspective = c["perspective"].as_bool().ok_or("perspective")?;
            self.app.camera.fov_deg = c["fov"].as_f64().ok_or("fov")?;
            for i in case["edit_path"].as_array().ok_or("edit_path")? {
                let i = i.as_u64().ok_or("edit index")? as usize;
                self.app.scene.selection.clear();
                self.app.scene.selection.insert(SelectionItem::Group(i));
                self.app.begin_component_edit();
            }
            if case["selected_annotations"].as_bool() == Some(true) {
                self.app.scene.selection.insert(SelectionItem::Dimension(0));
                self.app.scene.selection.insert(SelectionItem::TextLabel(0));
            }
            if !self.app.document_dirty() {
                return Err(format!("{name}: declared dirty fixture was not operative"));
            }
            self.warm = 0;
            self.next = 0;
            self.resize_attempts = 0;
            self.started = Instant::now();
            Ok(())
        }
        fn start(&mut self, ctx: &egui::Context) -> Result<(), String> {
            let port = self.app.start_ai_bridge(ctx, 0)?;
            let (tx, rx) = mpsc::channel::<(usize, Value)>();
            let (replies, receive) = mpsc::channel();
            let ctx = ctx.clone();
            self.worker = Some(thread::spawn(move || {
                let result = (|| {
                    let mut socket =
                        TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
                    socket
                        .set_read_timeout(Some(Duration::from_secs(15)))
                        .map_err(|e| e.to_string())?;
                    socket
                        .set_write_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    let mut reader = BufReader::new(socket.try_clone().map_err(|e| e.to_string())?);
                    while let Ok((index, request)) = rx.recv() {
                        let result = (|| {
                            let mut bytes =
                                serde_json::to_vec(&request).map_err(|e| e.to_string())?;
                            bytes.push(b'\n');
                            socket.write_all(&bytes).map_err(|e| e.to_string())?;
                            let mut line = String::new();
                            reader.read_line(&mut line).map_err(|e| e.to_string())?;
                            serde_json::from_str(&line).map_err(|e| e.to_string())
                        })();
                        if replies
                            .send(Reply {
                                index,
                                request,
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                        ctx.request_repaint();
                    }
                    Ok::<(), String>(())
                })();
                if let Err(error) = result {
                    let _ = replies.send(Reply {
                        index: usize::MAX,
                        request: Value::Null,
                        result: Err(error),
                    });
                    ctx.request_repaint();
                }
            }));
            self.commands = Some(tx);
            self.replies = Some(receive);
            Ok(())
        }
        fn advance(&mut self) -> Result<(), String> {
            if self.before.is_some() {
                return Ok(());
            }
            let case = &self.cases[self.case];
            let requests = case["requests"].as_array().ok_or("requests")?;
            if self.next >= requests.len() {
                return Ok(());
            }
            let request = json!({"id":format!("{}-{}",case["name"].as_str().unwrap(),self.next),"tool":"screenshot","args":requests[self.next]});
            self.before = Some(observe(&self.app)?);
            self.commands
                .as_ref()
                .ok_or("No TCP owner")?
                .send((self.next, request))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        fn receive(&mut self) -> Result<bool, String> {
            let Some(rx) = &self.replies else {
                return Ok(false);
            };
            let reply = match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return Ok(false),
                Err(e) => return Err(e.to_string()),
            };
            let result = reply.result?;
            if reply.index != self.next {
                return Err("TCP reply order mismatch".into());
            }
            let before = self.before.take().ok_or("No observed request owner")?;
            let after = observe(&self.app)?;
            if before != after {
                return Err("Actual capture changed model/history/camera/context".into());
            }
            let name = self.cases[self.case]["name"].as_str().unwrap();
            let mut row = json!({"case":name,"index":reply.index,"request":reply.request,"reply":result,"before":before,"after":after});
            if name == "invalid-dimensions" {
                if row["reply"]["ok"].as_bool() != Some(false) {
                    return Err("Invalid dimension unexpectedly accepted".into());
                }
            } else {
                if row["reply"]["ok"].as_bool() != Some(true) {
                    return Err(format!("Actual screenshot refused: {}", row["reply"]));
                }
                let path = PathBuf::from(
                    row["reply"]["result"]["path"]
                        .as_str()
                        .ok_or("No actual PNG path")?,
                );
                let encoded = std::fs::read(path).map_err(|e| e.to_string())?;
                let image = image::load_from_memory(&encoded)
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                let size = [image.width(), image.height()];
                if json!(size)
                    != json!([
                        row["reply"]["result"]["width"],
                        row["reply"]["result"]["height"]
                    ])
                {
                    return Err("Actual PNG extent differs from reply".into());
                }
                let raw = image.into_raw();
                if !raw.chunks_exact(4).any(|p| p != &raw[..4]) {
                    return Err("Actual captured viewport is uniform".into());
                }
                let png = format!("{name}-{}.png", reply.index);
                let rgba = format!("{name}-{}.rgba", reply.index);
                std::fs::write(self.out.join(&png), &encoded).map_err(|e| e.to_string())?;
                std::fs::write(self.out.join(&rgba), &raw).map_err(|e| e.to_string())?;
                row["image"] = json!({"file":png,"rgba":rgba,"size":size,"encoded_sha256":sha(&encoded),"rgba_sha256":sha(&raw)});
            }
            self.report["states"].as_array_mut().unwrap().push(row);
            self.next += 1;
            self.persist()?;
            Ok(true)
        }
        fn persist(&self) -> Result<(), String> {
            std::fs::write(
                self.out.join("native-ai-bridge-capture.partial.json"),
                serde_json::to_vec_pretty(&self.report).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        }
        fn retire(&mut self) -> Result<(), String> {
            self.commands = None;
            if let Some(worker) = self.worker.take() {
                worker.join().map_err(|_| "TCP worker panicked")?;
            }
            self.replies = None;
            self.app.stop_ai_bridge();
            Ok(())
        }
    }
    impl App for Fixture {
        fn title() -> &'static str {
            "IngeTrazo actual bridge viewport capture"
        }
        fn new(_: &mut System, _: Option<PathBuf>) -> Self {
            let source = PathBuf::from(std::env::var_os("INGETRAZO_CAPTURE_SOURCE").unwrap());
            let out = PathBuf::from(std::env::var_os("INGETRAZO_CAPTURE_OUTPUT").unwrap());
            let original: Value = serde_json::from_slice(
                &std::fs::read(source.join("original-ai-bridge-capture.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                original["source_pin"],
                "6be29fe437a7cd992d4e079f71720821b25216b9"
            );
            let cases = original["cases"].as_array().unwrap().clone();
            assert_eq!(cases.len(), 6);
            let mut f = Self {
                app: IngeTrazoApp::new(),
                source,
                out,
                cases: cases.clone(),
                case: 0,
                warm: 0,
                started: Instant::now(),
                worker: None,
                commands: None,
                replies: None,
                before: None,
                next: 0,
                report: json!({"source_pin":original["source_pin"],"cases":cases,"states":[],"controls":{}}),
                failed: None,
                resize_attempts: 0,
            };
            f.load_case().unwrap();
            f
        }
        fn build_ui(&mut self, ctx: &egui::Context) {
            self.app.update_ui(ctx);
            let result = (|| {
                if self.started.elapsed() > Duration::from_secs(45) {
                    return Err("Actual-host case deadline".into());
                }
                if self.failed.is_some() {
                    return Ok(());
                }
                let Some(rect) = self.app.native_gpu_viewport_rect else {
                    return Ok(());
                };
                let wanted = &self.cases[self.case]["logical_viewport"];
                let width = wanted[0].as_f64().ok_or("logical width")? as f32;
                let height = wanted[1].as_f64().ok_or("logical height")? as f32;
                if rect.width() != width || rect.height() != height {
                    if self.resize_attempts > 20 {
                        return Err("Actual logical viewport calibration refused".into());
                    }
                    let Some(window) = ctx.input(|i| i.viewport().inner_rect) else {
                        // First host frame may precede actual window metadata.
                        // Retain the bounded case deadline and repaint; never fabricate it.
                        self.warm = 0;
                        return Ok(());
                    };
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                        window.size() + egui::vec2(width - rect.width(), height - rect.height()),
                    ));
                    self.resize_attempts += 1;
                    self.warm = 0;
                    return Ok(());
                }
                if ctx.pixels_per_point() != 1.0 {
                    return Err("Fixture requires actual DPR1".into());
                }
                self.warm += 1;
                if self.warm < 8 {
                    return Ok(());
                }
                if self.commands.is_none() {
                    self.start(ctx)?;
                }
                let _ = self.receive()?;
                let count = self.cases[self.case]["requests"].as_array().unwrap().len();
                if self.next == count {
                    self.retire()?;
                    self.case += 1;
                    if self.case == self.cases.len() {
                        self.report["controls"] = json!({"actual_socket":true,"actual_host_renderer":true,"requested_rerender":true,"readonly_model_history_camera":true,"nonuniform_actual_png":true,"worker_joined":true});
                        std::fs::write(
                            self.out.join("native-ai-bridge-capture.json"),
                            serde_json::to_vec_pretty(&self.report).unwrap(),
                        )
                        .map_err(|e| e.to_string())?;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return Ok(());
                    }
                    self.load_case()?;
                    return Ok(());
                }
                self.advance()?;
                Ok::<(), String>(())
            })();
            if let Err(error) = result {
                self.failed = Some(error.clone());
                self.report["error"] = json!(error);
                let _ = self.persist();
                let _ = self.retire();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(10));
        }
        fn after_ui_gpu(&mut self, system: &mut System) {
            App::after_ui_gpu(&mut self.app, system);
        }
        fn prepare_gpu(&mut self, system: &mut System, resize: bool) {
            App::prepare_gpu(&mut self.app, system, resize);
        }
        fn gpu_error(&mut self, error: &str) {
            self.failed = Some(error.to_owned());
            self.report["error"] = json!(error);
            let _ = self.persist();
        }
    }
    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let source =
            PathBuf::from(std::env::args_os().nth(1).ok_or("SOURCE required")?).canonicalize()?;
        let out = PathBuf::from(std::env::args_os().nth(2).ok_or("OUTPUT required")?);
        std::fs::create_dir_all(&out)?;
        for (key, folder) in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
        ] {
            std::env::set_var(key, out.join(folder));
        }
        std::env::set_var("INGETRAZO_CAPTURE_SOURCE", source);
        std::env::set_var("INGETRAZO_CAPTURE_OUTPUT", &out);
        std::env::set_var("INGETRAZO_VULKAN_WORLD", "1");
        ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)?;
        if !out.join("native-ai-bridge-capture.json").is_file() {
            return Err(
                "Actual-host screenshot workflow incomplete; partial report retained".into(),
            );
        }
        Ok(())
    }
}
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fixture::main()
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!("This consumer requires the native Vulkan capture host; Metal remains a separate backend acceptance.");
    std::process::exit(2);
}
