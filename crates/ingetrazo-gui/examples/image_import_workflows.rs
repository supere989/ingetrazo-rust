//! Actual Image chooser, pointer/VCB, history and portable asset workflows.
#[allow(dead_code)]
#[path = "geotiff_import_workflows.rs"]
mod retained;
use glam::DVec3;
use ingetrazo_core::{scene::SelectionItem, screen_pick::PixelViewport};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::ToolType;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
const PIN: &str = "6be29fe437a7cd992d4e079f71720821b25216b9";
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn resource(a: &IngeTrazoApp, key: &str) -> Result<Value, String> {
    let b = a
        .image_import
        .prepared_bytes(key)
        .or_else(|| a.scene.embedded_textures.get(key).map(Vec::as_slice))
        .ok_or("Image resource is not owned")?;
    let p = ingetrazo_render::source_texture_decode::decode(b, Default::default())?;
    Ok(json!({"encoded_sha256":sha(b),"rgba_sha256":sha(&p.rgba),"size":[p.width,p.height]}))
}
fn tool(a: &IngeTrazoApp) -> Result<Value, String> {
    let Some(s) = a.active_tool.image_placement_state() else {
        return Ok(
            json!({"name":a.active_tool.name(),"armed":false,"aspect":null,"resource":null,"start":null,"hover":null,"work_plane":null,"free_aspect":false,"preview_lines":[],"value_label":null}),
        );
    };
    Ok(
        json!({"name":a.active_tool.name(),"armed":s.path.is_some(),"aspect":s.aspect,"resource":s.path.map(|p|resource(a,p)).transpose()?,"start":s.start.map(|p|p.to_array()),"hover":s.hover.map(|p|p.to_array()),"work_plane":s.work_plane.map(|(a,b)|[a.to_array(),b.to_array()]),"free_aspect":s.free_aspect,"preview_lines":a.active_tool.rubber_band_lines().iter().map(|(a,b)|[a.to_array(),b.to_array()]).collect::<Vec<_>>(),"value_label":a.image_placement_label().map(|(text,p)|json!([text,p.to_array()]))}),
    )
}
fn record(a: &IngeTrazoApp, name: &str, phase: &str) -> Result<Value, String> {
    Ok(
        json!({"case":name,"phase":phase,"scene":retained::scene(a)?,"tool":tool(a)?,"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"camera":retained::camera(a)}),
    )
}
struct Ui {
    ctx: egui::Context,
    time: f64,
    size: egui::Vec2,
    modifiers: egui::Modifiers,
}
impl Ui {
    fn new() -> Self {
        Self {
            ctx: Default::default(),
            time: 0.,
            size: egui::vec2(1280., 800.),
            modifiers: Default::default(),
        }
    }
    fn frame(&mut self, a: &mut IngeTrazoApp, events: Vec<egui::Event>) {
        self.time += 0.02;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            modifiers: self.modifiers,
            events,
            ..Default::default()
        };
        let _ = self.ctx.run(input, |ctx| a.update_ui(ctx));
    }
    fn warm(&mut self, a: &mut IngeTrazoApp) {
        for _ in 0..3 {
            self.frame(a, vec![]);
        }
    }
    fn layout(&mut self, a: &mut IngeTrazoApp, w: f32, h: f32) -> Result<(), String> {
        self.warm(a);
        let r = a.native_gpu_viewport_rect.ok_or("Actual viewport rect")?;
        self.size += egui::vec2(w - r.width(), h - r.height());
        self.warm(a);
        Ok(())
    }
    fn pointer(
        &mut self,
        a: &mut IngeTrazoApp,
        op: &str,
        pixel: &Value,
        shift: bool,
    ) -> Result<(), String> {
        let r = a
            .native_gpu_viewport_rect
            .ok_or("Actual viewport geometry")?;
        let x = pixel[0].as_f64().ok_or("Recorded pointer x")?;
        let y = pixel[1].as_f64().ok_or("Recorded pointer y")?;
        if (r.width() - 800.).abs() > 0.1 || (r.height() - 600.).abs() > 0.1 {
            return Err(format!("Actual viewport mismatch {r:?}"));
        }
        let pos = r.min + egui::vec2(x as f32, y as f32);
        self.modifiers.shift = shift;
        let events = match op {
            "move" => vec![egui::Event::PointerMoved(pos)],
            "press" | "release" => vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: op == "press",
                modifiers: self.modifiers,
            }],
            _ => return Err("pointer action".into()),
        };
        self.frame(a, events);
        Ok(())
    }
    fn key(&mut self, a: &mut IngeTrazoApp, key: egui::Key) {
        let e = egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: self.modifiers,
        };
        self.frame(a, vec![e]);
        self.frame(
            a,
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: false,
                repeat: false,
                modifiers: self.modifiers,
            }],
        );
    }
    fn click(&mut self, a: &mut IngeTrazoApp, pos: egui::Pos2) {
        self.frame(a, vec![egui::Event::PointerMoved(pos)]);
        self.frame(
            a,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
        );
        self.frame(
            a,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }
    fn vcb(&mut self, a: &mut IngeTrazoApp, text: &str) -> Result<(), String> {
        let images = a.scene.image_planes.len();
        let history = a.undo_stack.len();
        self.frame(a, vec![egui::Event::Text(text.to_owned())]);
        if a.vcb_text != text {
            return Err(format!(
                "Actual viewport VCB did not accept input: {:?}",
                a.vcb_text
            ));
        }
        self.key(a, egui::Key::Enter);
        if a.scene.image_planes.len() != images + 1 || a.undo_stack.len() != history + 1 {
            return Err(format!("Actual VCB did not Add: {}", a.status_message));
        }
        Ok(())
    }
    fn close_error(&mut self, a: &mut IngeTrazoApp) -> Result<(), String> {
        self.warm(a);
        let r = self
            .ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("image-import-error-ok")))
            .ok_or("Actual Image error OK")?;
        self.click(a, r.center());
        if a.image_import.error.is_some() {
            return Err("Image modal did not dismiss".into());
        }
        Ok(())
    }
}
struct Env {
    path: Option<std::ffi::OsString>,
    pick: Option<std::ffi::OsString>,
    observe: Option<std::ffi::OsString>,
    marker: Option<std::ffi::OsString>,
}
impl Drop for Env {
    fn drop(&mut self) {
        for (k, v) in [
            ("PATH", self.path.take()),
            ("INGETRAZO_IMAGE_FIXTURE_PICK", self.pick.take()),
            ("INGETRAZO_UI_OBSERVE", self.observe.take()),
            ("INGETRAZO_IMAGE_PICKER_MARKER", self.marker.take()),
        ] {
            if let Some(v) = v {
                std::env::set_var(k, v)
            } else {
                std::env::remove_var(k)
            }
        }
    }
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let original: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-image-import.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if original["source_pin"] != PIN {
        return Err("Source pin".into());
    }
    let _env = Env {
        path: std::env::var_os("PATH"),
        pick: std::env::var_os("INGETRAZO_IMAGE_FIXTURE_PICK"),
        observe: std::env::var_os("INGETRAZO_UI_OBSERVE"),
        marker: std::env::var_os("INGETRAZO_IMAGE_PICKER_MARKER"),
    };
    let picker = out.join("picker");
    std::fs::create_dir_all(&picker).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            std::env::current_exe().map_err(|e| e.to_string())?,
            picker.join("kdialog"),
        )
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    {
        return Err("Owned picker fixture requires Unix CI".into());
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::set_var(
        "PATH",
        format!("{}:{}", picker.display(), path.to_string_lossy()),
    );
    std::env::set_var("INGETRAZO_UI_OBSERVE", "1");
    let marker = out.join("picker-invocations.log");
    std::env::set_var("INGETRAZO_IMAGE_PICKER_MARKER", &marker);
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut headers = Vec::new();
    let mut guards = json!({"actual_picker":false,"actual_pointer":false,"actual_vcb":false,"modal_ownership":false,"external_independence":true,"resource_reuse":false,"refusal_preserved":false,"undo_armed_resource":false,"idle_readonly":true});
    for row in original["headers"]
        .as_array()
        .ok_or("Header fixture inventory")?
    {
        let name = row["name"].as_str().ok_or("Header name")?;
        let file = row["file"].as_str().ok_or("Header file")?;
        let bytes = std::fs::read(source.join(file)).map_err(|e| e.to_string())?;
        let dims = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .into_dimensions();
        let mut h = json!({"name":name,"file":file,"aspect":1.,"size":[0,0],"readable":false,"encoded_sha256":sha(&bytes),"decoded":null});
        if let Ok((w, height)) = dims {
            let p = ingetrazo_render::source_texture_decode::decode(&bytes, Default::default())?;
            let relative = format!("assets/{name}.rgba");
            std::fs::create_dir_all(out.join("assets")).map_err(|e| e.to_string())?;
            std::fs::write(out.join(&relative), &p.rgba).map_err(|e| e.to_string())?;
            h["aspect"] = json!(height as f64 / w as f64);
            h["size"] = json!([w, height]);
            h["readable"] = json!(true);
            h["decoded"] = json!({"rgba":relative,"size":[p.width,p.height],"sha256":sha(&p.rgba)});
        }
        headers.push(h);
    }
    for c in original["cases"].as_array().ok_or("Cases")? {
        let name = c["name"].as_str().ok_or("Case name")?;
        let mut a = IngeTrazoApp::new();
        a.native_gpu_viewport_enabled = true;
        a.system_clipboard = None;
        let input = source.join(c["input"].as_str().ok_or("Initial IGZ")?);
        a.open_document(&input);
        for index in c["edit_path"].as_array().ok_or("Edit path")? {
            a.scene.clear_selection();
            a.scene.select(SelectionItem::Group(
                index.as_u64().ok_or("Edit index")? as usize
            ));
            a.begin_component_edit();
        }
        retained::initial_camera(&mut a, &c["initial_camera"])?;
        let mut ui = Ui::new();
        ui.layout(&mut a, 800., 600.)?;
        states.push(record(&a, name, "before")?);
        let ext = out.join("external").join(name);
        std::fs::create_dir_all(&ext).map_err(|e| e.to_string())?;
        let mut keys = Vec::new();
        for action in c["actions"].as_array().ok_or("Actions")? {
            let op = action["op"].as_str().ok_or("Action op")?;
            let before_vcb = (a.scene.image_planes.len(), a.undo_stack.len());
            match op {
                "import" => {
                    let previous = a.scene.clone();
                    let history = (a.undo_stack.len(), a.redo_stack.len());
                    if let Some(asset) = action["asset"].as_str() {
                        let filename = action["filename"].as_str().ok_or("Filename")?;
                        let p = ext.join(filename);
                        std::fs::copy(source.join(asset), &p).map_err(|e| e.to_string())?;
                        std::env::set_var("INGETRAZO_IMAGE_FIXTURE_PICK", p);
                    } else {
                        std::env::remove_var("INGETRAZO_IMAGE_FIXTURE_PICK");
                    }
                    a.begin_image_import_dialog();
                    ui.warm(&mut a);
                    if a.image_import.error.is_some() {
                        assert_eq!(a.scene, previous);
                        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
                        ui.key(&mut a, egui::Key::Z);
                        ui.key(&mut a, egui::Key::Delete);
                        assert_eq!(a.scene, previous);
                        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
                        guards["modal_ownership"] = json!(true);
                        ui.close_error(&mut a)?;
                    }
                    if let Some(s) = a.active_tool.image_placement_state() {
                        if let Some(key) = s.path {
                            keys.push(key.to_owned());
                        }
                    }
                }
                "move" | "press" | "release" => ui.pointer(
                    &mut a,
                    op,
                    &action["pixel"],
                    action["shift"].as_bool().unwrap_or(false),
                )?,
                "vcb" => ui.vcb(&mut a, action["text"].as_str().ok_or("Typed value")?)?,
                "undo" => a.undo(),
                "redo" => a.redo(),
                "escape" => ui.key(&mut a, egui::Key::Escape),
                "tool" => {
                    if action["name"] == "Line" {
                        a.set_tool(ToolType::Line);
                    } else {
                        return Err("Fixture tool".into());
                    }
                }
                "remove-external" => {
                    for p in std::fs::read_dir(&ext).map_err(|e| e.to_string())? {
                        std::fs::remove_file(p.map_err(|e| e.to_string())?.path())
                            .map_err(|e| e.to_string())?;
                    }
                }
                _ => return Err(format!("Unknown Image action {op}")),
            }
            if op == "vcb" {
                assert_eq!(a.scene.image_planes.len(), before_vcb.0 + 1, "Typed image placement must publish one image");
                assert_eq!(a.undo_stack.len(), before_vcb.1 + 1, "Typed image placement must publish one Add command");
                guards["actual_vcb"] = json!(true);
            }
            if op == "press"
                && a.active_tool
                    .image_placement_state()
                    .is_some_and(|v| v.start.is_some())
            {
                guards["actual_pointer"] = json!(true);
            }
            states.push(record(&a, name, action["phase"].as_str().ok_or("Phase")?)?);
        }
        for p in std::fs::read_dir(&ext).map_err(|e| e.to_string())? {
            std::fs::remove_file(p.map_err(|e| e.to_string())?.path())
                .map_err(|e| e.to_string())?;
        }
        states.push(record(&a, name, "external-removed")?);
        if !c["edit_path"].as_array().unwrap().is_empty() {
            a.finish_component_edit();
            states.push(record(&a, name, "finish")?);
        }
        let saved = c["saved"].as_str().ok_or("Saved filename")?;
        a.save_to_path(&out.join(saved));
        if !out.join(saved).is_file() {
            return Err("Native save missing".into());
        }
        states.push(record(&a, name, "save")?);
        let before = a.scene.clone();
        let hist = (a.undo_stack.len(), a.redo_stack.len());
        let reads = a.image_import.byte_reads;
        ui.warm(&mut a);
        assert_eq!(a.scene, before);
        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), hist);
        assert_eq!(a.image_import.byte_reads, reads);
        if keys.len() >= 3 {
            assert_eq!(keys[0], keys[1]);
            assert_ne!(keys[0], keys[2]);
            assert_eq!(a.scene.embedded_textures.len(), 2);
            guards["resource_reuse"] = json!(true);
        }
        cases.push(json!({"name":name,"saved":saved,"phases":states.iter().filter(|r|r["case"]==name).map(|r|r["phase"].clone()).collect::<Vec<_>>(),"cache_reuse":{"same_name_equal":keys.len()<2||keys[0]==keys[1],"renamed_distinct":keys.len()<3||keys[0]!=keys[2]}}));
        std::fs::write(out.join("native-image-import.partial.json"),serde_json::to_vec_pretty(&json!({"source_pin":PIN,"cases":cases,"states":states,"headers":headers,"guards":guards})).unwrap()).map_err(|e|e.to_string())?;
    }
    // Explicit native refusals and resource ownership through a document Undo while armed.
    let c = &original["cases"][0];
    let mut a = IngeTrazoApp::new();
    a.open_document(&source.join(c["input"].as_str().unwrap()));
    a.begin_image_import(&source.join("assets/scan.png"))?;
    let ctx = |p| {
        ingetrazo_tools::ToolContext::new(
            p,
            ingetrazo_core::math::Ray::new(p + DVec3::Z, -DVec3::Z),
        )
    };
    a.dispatch_tool_click(&ctx(DVec3::new(-4., -2., 0.)));
    a.dispatch_tool_move(&ctx(DVec3::new(-1., 2., 0.)));
    let before = a.scene.clone();
    let pending = tool(&a)?;
    let history = (a.undo_stack.len(), a.redo_stack.len());
    assert!(a.dispatch_tool_vcb("-1m").is_err());
    assert_eq!(a.scene, before);
    assert_eq!(tool(&a)?, pending);
    assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
    guards["refusal_preserved"] = json!(true);
    let prepared_before = a.image_import.pending_bytes();
    assert!(prepared_before > 0);
    a.dispatch_tool_vcb("2m")?;
    a.begin_image_import(&source.join("assets/scan.png"))?;
    a.undo();
    assert!(a.scene.image_planes.is_empty());
    a.dispatch_tool_click(&ctx(DVec3::new(-4., -2., 0.)));
    a.dispatch_tool_move(&ctx(DVec3::new(-1., 2., 0.)));
    a.dispatch_tool_vcb("2m")?;
    assert_eq!(a.scene.image_planes.len(), 1);
    assert!(a
        .scene
        .embedded_textures
        .contains_key(&a.scene.image_planes[0].path));
    guards["undo_armed_resource"] = json!(true);
    a.native_gpu_viewport_enabled = true;
    let mut modal_ui = Ui::new();
    modal_ui.layout(&mut a, 800., 600.)?;
    let prior = a.scene.clone();
    let prior_history = (a.undo_stack.len(), a.redo_stack.len());
    let prior_tool = tool(&a)?;
    let prior_camera = retained::camera(&a);
    std::env::set_var(
        "INGETRAZO_IMAGE_FIXTURE_PICK",
        source.join("assets/invalid.png"),
    );
    a.begin_image_import_dialog();
    assert!(a.image_import.error.is_some());
    modal_ui.warm(&mut a);
    modal_ui.modifiers.ctrl = true;
    modal_ui.key(&mut a, egui::Key::Z);
    modal_ui.modifiers.ctrl = false;
    modal_ui.key(&mut a, egui::Key::Delete);
    modal_ui.frame(
        &mut a,
        vec![
            egui::Event::PointerMoved(egui::pos2(500., 400.)),
            egui::Event::PointerButton {
                pos: egui::pos2(500., 400.),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    assert_eq!(a.scene, prior);
    assert_eq!((a.undo_stack.len(), a.redo_stack.len()), prior_history);
    assert_eq!(tool(&a)?, prior_tool);
    assert_eq!(retained::camera(&a), prior_camera);
    modal_ui.close_error(&mut a)?;
    guards["modal_ownership"] = json!(true);
    let child_lines = std::fs::read_to_string(&marker)
        .map_err(|e| e.to_string())?
        .lines()
        .count();
    let expected_children = original["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            c["actions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| a["op"] == "import")
                .count()
        })
        .sum::<usize>()
        + 1;
    assert_eq!(child_lines, expected_children);
    guards["actual_picker"] = json!(true);
    let result = json!({"source_pin":PIN,"cases":cases,"states":states,"headers":headers,"guards":guards,"actual_egui":true,"scope":"Actual App picker and pointer/VCB/history, production header/asset decoders; no GPU image presentation claim"});
    std::fs::write(
        out.join("native-image-import.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "{}",
        json!({"cases":result["cases"].as_array().unwrap().len(),"states":result["states"].as_array().unwrap().len(),"headers":result["headers"].as_array().unwrap().len()})
    );
    Ok(())
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if Path::new(&args[0])
        .file_name()
        .is_some_and(|p| p == "kdialog")
    {
        if let Some(marker) = std::env::var_os("INGETRAZO_IMAGE_PICKER_MARKER") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(marker)
                .unwrap();
            writeln!(file, "{}", serde_json::to_string(&args).unwrap()).unwrap();
        }
        if let Some(p) = std::env::var_os("INGETRAZO_IMAGE_FIXTURE_PICK") {
            println!("{}", p.to_string_lossy());
            return;
        }
        std::process::exit(1);
    }
    if args.len() != 3 {
        eprintln!("image_import_workflows SOURCE NATIVE");
        std::process::exit(2)
    }
    let source = PathBuf::from(&args[1]).canonicalize().unwrap();
    let out = PathBuf::from(&args[2]);
    if let Err(error) = run(&source, &out) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
