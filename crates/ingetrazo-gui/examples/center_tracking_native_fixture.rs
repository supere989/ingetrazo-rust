// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual native Vulkan application/input path; writes observational state only.
use glam::DVec3;
use ingetrazo_gui::{
    native_vulkan::{App, System},
    IngeTrazoApp,
};
use ingetrazo_tools::base::ToolType;
struct Fixture {
    app: IngeTrazoApp,
    dir: std::path::PathBuf,
    mode: String,
}
impl App for Fixture {
    fn title() -> &'static str {
        "IngeTrazo native center tracking acceptance"
    }
    fn new(_: &mut System, path: Option<std::path::PathBuf>) -> Self {
        let mut app = IngeTrazoApp::new();
        app.show_tray = false;
        app.native_gpu_viewport_enabled = true;
        let mode = std::env::var("INGETRAZO_CENTER_MODE").unwrap_or("curve".into());
        let radius = if mode == "tiny" { 0.01 } else { 1.2 };
        let center = DVec3::new(2., 3., 1.);
        let ids = (0..24)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 24.;
                app.scene
                    .mesh
                    .add_vertex(center + DVec3::new(radius * a.cos(), radius * a.sin(), 0.))
            })
            .collect::<Vec<_>>();
        for i in 0..24 {
            let id = app.scene.mesh.add_edge(ids[i], ids[(i + 1) % 24]).unwrap();
            app.scene.mesh.edges[id].curve_id = Some(71);
        }
        if mode == "malformed" {
            let v = app.scene.mesh.add_vertex(center + DVec3::X * 3.);
            let id = app.scene.mesh.add_edge(ids[0], v).unwrap();
            app.scene.mesh.edges[id].curve_id = Some(71);
        }
        app.camera.target = DVec3::new(2., 3., 0.);
        app.camera.distance = 10.;
        app.camera.pitch = std::f64::consts::PI / 6.;
        app.camera.yaw = -std::f64::consts::FRAC_PI_2;
        app.camera.perspective = true;
        app.set_tool(ToolType::Line);
        app.mark_document_clean();
        Self {
            app,
            dir: path.unwrap(),
            mode,
        }
    }
    fn build_ui(&mut self, ctx: &egui::Context) {
        self.app.update_ui(ctx);
        if let Some(rect) = self.app.native_gpu_viewport_rect {
            let matrix = self.app.camera.projection_matrix() * self.app.camera.view_matrix();
            let project = |p: DVec3| {
                let c = matrix * p.extend(1.);
                let n = c.truncate() / c.w;
                [
                    rect.left() as f64 + (n.x + 1.) * 0.5 * rect.width() as f64,
                    rect.top() as f64 + (1. - n.y) * 0.5 * rect.height() as f64,
                ]
            };
            let center = if self.app.scene.mesh.vertices.len() >= 24 {
                self.app.scene.mesh.vertices[..24]
                    .iter()
                    .map(|v| v.position)
                    .sum::<DVec3>()
                    / 24.
            } else {
                DVec3::new(2., 3., 1.)
            };
            let radius = if self.mode == "tiny" { 0.01 } else { 1.2 };
            let rim = center + DVec3::X * radius;
            let state = serde_json::json!({"mode":self.mode,"time":ctx.input(|i|i.time),"center":center.to_array(),"rim":project(rim),"center_pixel":project(center),"move_target":project(rim+DVec3::X*std::env::var("INGETRAZO_MOVE_DISTANCE").ok().and_then(|v|v.parse::<f64>().ok()).unwrap_or(1.)),"kind":self.app.snap_engine.current_inference().map(|s|format!("{:?}",s.kind)),"inference_point":self.app.snap_engine.current_inference().map(|s|s.point.to_array()),"memory":self.app.snap_engine.encouraged_points.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"edges":self.app.scene.mesh.edges.len(),"faces":self.app.scene.mesh.faces.len(),"undo":self.app.undo_stack.len(),"anchor":self.app.active_tool.snap_anchor().map(|p|p.to_array()),"tool":format!("{:?}",self.app.active_tool_type),"revision":self.app.model_revision});
            std::fs::write(
                self.dir.join("state.tmp"),
                serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
            std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    fn prepare_gpu(&mut self, system: &mut System, resize: bool) {
        App::prepare_gpu(&mut self.app, system, resize)
    }
    fn gpu_error(&mut self, error: &str) {
        panic!("Native fixture GPU error: {error}");
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap();
    ingetrazo_gui::native_vulkan::run_app::<Fixture>(Some(dir.into()))
}
