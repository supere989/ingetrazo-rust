use glam::DVec3;
use ingetrazo_core::camera::StandardView;
use ingetrazo_gui::{app::TrayTab, IngeTrazoApp};
struct Fixture {
    app: IngeTrazoApp,
    out: std::path::PathBuf,
}
impl eframe::App for Fixture {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::update(&mut self.app, ctx, frame);
        let rect = self
            .app
            .native_gpu_viewport_rect
            .unwrap_or(egui::Rect::NOTHING);
        let vp = ingetrazo_core::screen_pick::PixelViewport::new(
            &self.app.camera,
            rect.width() as f64,
            rect.height() as f64,
        );
        let points = [
            DVec3::new(-2., -1., 0.),
            DVec3::new(0., 0., 0.),
            DVec3::new(1., 1., 0.),
            DVec3::new(3., 2., 0.),
        ]
        .map(|p| {
            vp.project(p)
                .map(|v| [v.x + rect.left() as f64, v.y + rect.top() as f64])
        });
        let state = serde_json::json!({"active":self.app.bim_active_tag_mode,"tag":self.app.scene.active_ifc,"faces":self.app.scene.mesh.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices.iter().map(|&i|self.app.scene.mesh.vertices[i].position.to_array()).collect::<Vec<_>>(),"attrs":f.attrs})).collect::<Vec<_>>(),"undo":self.app.undo_stack.len(),"redo":self.app.redo_stack.len(),"tool":format!("{:?}",self.app.active_tool_type),"viewport":[rect.left(),rect.top(),rect.width(),rect.height()],"points":points,"status":self.app.status_message});
        std::fs::write(
            self.out.join("state.tmp"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        std::fs::rename(self.out.join("state.tmp"), self.out.join("state.json")).unwrap();
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }
}
fn main() -> eframe::Result {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut app = IngeTrazoApp::new();
    app.set_language("en");
    app.active_tray_tab = TrayTab::Bim;
    app.show_tray = true;
    app.camera.set_standard_view(StandardView::Top);
    app.camera.perspective = false;
    app.camera.target = DVec3::ZERO;
    app.camera.distance = 10.;
    eframe::run_native(
        "BIM actual draw",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200., 800.]),
            ..Default::default()
        },
        Box::new(move |_| Ok(Box::new(Fixture { app, out }))),
    )
}
