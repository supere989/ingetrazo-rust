//! Operative app-side bridge dispatch. Transport threads never borrow the scene.
use crate::IngeTrazoApp;
use glam::{DVec3, Mat4};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
use serde_json::{json, Value};
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
fn placement(group: &Group) -> bool {
    if group.native_document.is_object() {
        group.native_document["proto"].as_u64().is_some()
            && group.native_document["xform"].is_array()
            || !group.children.is_empty()
    } else {
        group.component_definition.is_some() || !group.children.is_empty()
    }
}
fn visible(scene: &Scene, group: &Group) -> bool {
    !group.hidden && !group.billboard && scene.is_layer_visible(group.layer.as_deref())
}
#[derive(Default)]
pub(crate) struct QueryCache {
    key: Option<(u64, u64, u64)>,
    bounds: Value,
    builds: u64,
}
fn bounds(app: &IngeTrazoApp) -> Value {
    let root = app
        .component_edit_stack
        .first()
        .map(|s| &s.original_scene)
        .unwrap_or(&app.scene);
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    let mut seen = false;
    fn absorb(p: DVec3, lo: &mut DVec3, hi: &mut DVec3, seen: &mut bool) {
        *seen = true;
        *lo = lo.min(p);
        *hi = hi.max(p);
    }
    fn qt(p: DVec3) -> DVec3 {
        p.as_vec3().as_dvec3()
    }
    for edge in &root.mesh.edges {
        if app.scene.is_layer_visible(edge.layer.as_deref()) {
            for id in [edge.v0, edge.v1] {
                if let Some(v) = root.mesh.vertices.get(id) {
                    absorb(qt(v.position), &mut lo, &mut hi, &mut seen);
                }
            }
        }
    }
    for face in &root.mesh.faces {
        if !face.attrs.get("hidden").is_some_and(truth)
            && app.scene.is_layer_visible(
                face.layer
                    .as_deref()
                    .or_else(|| face.attrs.get("layer").and_then(Value::as_str)),
            )
        {
            for id in &face.vertices {
                if let Some(v) = root.mesh.vertices.get(*id) {
                    absorb(qt(v.position), &mut lo, &mut hi, &mut seen);
                }
            }
        }
    }
    fn walk(
        app: &IngeTrazoApp,
        group: &Group,
        parent: Mat4,
        lo: &mut DVec3,
        hi: &mut DVec3,
        seen: &mut bool,
    ) {
        let active = app
            .component_edit_stack
            .iter()
            .position(|s| s.group_id == group.id);
        let (mesh, children, world): (&Mesh, &[Group], Mat4) = if let Some(index) = active {
            let edited = app
                .component_edit_stack
                .get(index + 1)
                .map(|s| &s.original_scene)
                .unwrap_or(&app.scene);
            (&edited.mesh, &edited.groups, Mat4::IDENTITY)
        } else {
            (
                &group.mesh,
                &group.children,
                parent * group.transform.as_mat4(),
            )
        };
        if visible(&app.scene, group) {
            let matrix = world.as_dmat4();
            for vertex in &mesh.vertices {
                absorb(matrix.transform_point3(qt(vertex.position)), lo, hi, seen);
            }
        }
        for child in children {
            walk(app, child, world, lo, hi, seen);
        }
    }
    for group in &root.groups {
        if visible(&app.scene, group) {
            walk(app, group, Mat4::IDENTITY, &mut lo, &mut hi, &mut seen);
        }
    }
    if seen {
        json!({"min":qt(lo).to_array(),"max":qt(hi).to_array()})
    } else {
        Value::Null
    }
}
impl IngeTrazoApp {
    pub fn ai_query_model(&mut self) -> Value {
        let key = (
            self.model_revision,
            self.scene.version,
            self.scene.view_version,
        );
        if self.ai_query_cache.key != Some(key) {
            self.ai_query_cache.bounds = bounds(self);
            self.ai_query_cache.builds += 1;
            self.ai_query_cache.key = Some(key);
        }
        let root = self
            .component_edit_stack
            .first()
            .map(|s| &s.original_scene)
            .unwrap_or(&self.scene);
        let instances = root
            .groups
            .iter()
            .filter(|g| {
                if let Some(session) = self
                    .component_edit_stack
                    .iter()
                    .find(|s| s.group_id == g.id)
                {
                    !session
                        .original_scene
                        .groups
                        .iter()
                        .find(|p| p.id == g.id)
                        .is_some_and(|p| p.children.is_empty())
                } else {
                    placement(g)
                }
            })
            .count();
        let mut materials: Vec<_> = self.scene.materials.keys().collect();
        materials.sort();
        materials.truncate(100);
        json!({"faces":self.scene.mesh.faces.len(),"edges":self.scene.mesh.edges.len(),"groups":root.groups.len()-instances,"component_instances":instances,"group_names":root.groups.iter().take(100).map(|g|&g.name).collect::<Vec<_>>(),"materials":materials,"layers":self.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"dimensions":self.scene.dimensions.len(),"section_planes":self.scene.section_planes.len(),"selection":self.scene.selection.len(),"bounds":self.ai_query_cache.bounds})
    }
    pub fn ai_query_bounds_builds(&self) -> u64 {
        self.ai_query_cache.builds
    }
    pub fn start_ai_bridge(&mut self, ctx: &egui::Context, port: u16) -> Result<u16, String> {
        if self.ai_bridge.is_some() {
            return Err("Bridge is already running".into());
        }
        let ctx = ctx.clone();
        let bridge =
            ingetrazo_core::ai_bridge::Bridge::start(port, Default::default(), move || {
                ctx.request_repaint()
            })
            .map_err(|e| e.to_string())?;
        let port = bridge.address().port();
        self.ai_bridge = Some(bridge);
        Ok(port)
    }
    pub fn stop_ai_bridge(&mut self) {
        self.ai_bridge = None;
    }
    pub(crate) fn poll_ai_bridge(&mut self) {
        if let Some(mut bridge) = self.ai_bridge.take() {
            match bridge.poll(|request| self.ai_bridge_request(request)) {
                Ok(_) => self.ai_bridge = Some(bridge),
                Err(error) => self.status_message = format!("Bridge stopped: {error}"),
            }
        }
    }
    fn ai_bridge_run_python(&mut self, args: Option<&Value>) -> Result<Value, String> {
        let code = match args.filter(|v| truth(v)) {
            None => None,
            Some(Value::Object(args)) => {
                if args.keys().any(|k| k != "code") { return Err("TypeError: run_python accepts only code".into()); }
                args.get("code")
            }
            Some(_) => return Err("TypeError: args must be an object".into()),
        };
        #[cfg(feature="engineering-python")]
        {
            if !self.engineering_python_enabled { return Err("Native engineering Python is disabled; explicit enable required".into()); }
            if self.navigation_suspended() || self.scene.active_sheet_index.is_some()
                || self.active_tool.tool_type() != ingetrazo_tools::base::ToolType::Select
                || self.navigation.mode.is_some()
            { return Err("Bridge Python while a modal, composer, navigation or transform tool is active is not supported".into()); }
            use ingetrazo_ai_interpreter_proof::vm_mesh::CodeInput;
            let input=match code {None=>CodeInput::Text(""),Some(Value::String(source))=>CodeInput::Text(source),Some(value)=>CodeInput::Json(value)};
            let receipt = self.execute_engineering_python_input(input)?;
            Ok(json!({"stdout":receipt.stdout,"stderr":receipt.stderr,"error":receipt.error,"changed":receipt.changed}))
        }
        #[cfg(not(feature="engineering-python"))]
        { let _ = code; Err("Native engineering Python support is not compiled".into()) }
    }
    pub fn ai_bridge_request(&mut self, request: &Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let tool = request["tool"].as_str().unwrap_or("");
        if matches!(tool, "query_model" | "undo" | "redo") {
            if let Some(args) = request.get("args").filter(|a| truth(a)) {
                if !args.as_object().is_some_and(|a| a.is_empty()) {
                    return json!({"id":id,"ok":false,"error":"TypeError: this tool accepts no arguments"});
                }
            }
        }
        let result=match tool{
   "query_model"=>Ok(self.ai_query_model()),
   "run_python"=>self.ai_bridge_run_python(request.get("args")),
   "screenshot"=>Err("Native captured-image transport is not implemented".to_owned()),
   "undo"|"redo"=>{
    if self.scene.active_sheet_index.is_some()||self.section_name_prompt.is_some()||self.active_tool.tool_type()!=ingetrazo_tools::base::ToolType::Select||self.navigation.mode.is_some(){Err("Bridge history while a modal, composer, navigation or transform tool is active is not yet supported".to_owned())}
    else {let changed=if tool=="undo"{self.undo_outcome()}else{self.redo_outcome()};Ok(json!({"ok":changed}))}
   },
   _=>Err(format!("ValueError: unknown tool '{tool}'")),
  };
        match result {
            Ok(result) => json!({"id":id,"ok":true,"result":result}),
            Err(error) => json!({"id":id,"ok":false,"error":error}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpStream,
        time::{Duration, Instant},
    };
    #[test]
    fn real_loopback_jobs_execute_inside_production_update_ui() {
        let mut app = IngeTrazoApp::new();
        let ctx = egui::Context::default();
        let port = app.start_ai_bridge(&ctx, 0).unwrap();
        let mut expected = app.ai_query_model();
        assert_eq!(app.ai_query_bounds_builds(), 1);
        assert_eq!(app.ai_query_model(), expected);
        assert_eq!(app.ai_query_bounds_builds(), 1);
        app.save_undo_state();
        let a = app.scene.mesh.add_vertex(DVec3::ZERO);
        let b = app.scene.mesh.add_vertex(DVec3::X);
        app.scene.mesh.add_edge(a, b).unwrap();
        app.scene.bump_version();
        expected["edges"] = 1.into();
        expected["bounds"] = json!({"min":[0.,0.,0.],"max":[1.,0.,0.]});
        let peer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut replies = Vec::new();
            for (id, tool) in [
                (1, "query_model"),
                (2, "undo"),
                (3, "redo"),
                (4, "run_python"),
                (5, "screenshot"),
            ] {
                let mut request = serde_json::to_vec(&json!({"id":id,"tool":tool})).unwrap();
                request.push(b'\n');
                stream.write_all(&request).unwrap();
                let mut line = Vec::new();
                let mut byte = [0];
                loop {
                    stream.read_exact(&mut byte).unwrap();
                    if byte[0] == b'\n' {
                        break;
                    }
                    line.push(byte[0]);
                }
                replies.push(serde_json::from_slice::<Value>(&line).unwrap());
            }
            replies
        });
        let until = Instant::now() + Duration::from_secs(5);
        while !peer.is_finished() {
            assert!(Instant::now() < until);
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    ..Default::default()
                },
                |ctx| app.update_ui(ctx),
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let replies = peer.join().unwrap();
        assert_eq!(replies[0]["result"], expected);
        assert_eq!(replies[1]["result"], json!({"ok":true}));
        assert_eq!(replies[2]["result"], json!({"ok":true}));
        assert_eq!(replies[3]["ok"], false);
        assert_eq!(replies[4]["ok"], false);
        assert_eq!(app.scene.mesh.edges.len(), 1);
        app.stop_ai_bridge();
    }
    #[test]
    fn refusal_and_nonargument_errors_leave_document_history_unchanged() {
        let mut app = IngeTrazoApp::new();
        let before = app.scene.clone();
        for request in [
            json!({"tool":"run_python","args":{"code":"scene.materials.clear()"}}),
            json!({"tool":"query_model","args":{"unexpected":true}}),
            json!({"tool":"missing"}),
        ] {
            assert_eq!(app.ai_bridge_request(&request)["ok"], false);
            assert_eq!(app.scene, before);
            assert!(app.undo_stack.is_empty());
        }
        assert_eq!(
            app.ai_bridge_request(&json!({"tool":"undo"}))["result"],
            json!({"ok":false})
        );
    }
}
