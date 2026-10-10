#[cfg(not(target_os = "macos"))]
mod native {
    use ingetrazo_core::camera::StandardView;
    use ingetrazo_gui::{
        native_vulkan::{App, System},
        navigation::Mode,
        IngeTrazoApp,
    };
    struct Fixture {
        app: IngeTrazoApp,
        dir: std::path::PathBuf,
        frames: u64,
    }
    impl App for Fixture {
        fn title() -> &'static str {
            "IngeTrazo Position navigation proof"
        }
        fn new(system: &mut System, _: Option<std::path::PathBuf>) -> Self {
            let mut app = IngeTrazoApp::new();
            app.system_clipboard = None;
            app.set_language("en");
            app.show_tray = false;
            app.show_toolbar = false;
            app.native_composer.factory = Some(system.enable_cad2d().unwrap().factory().into());
            app.native_gpu_viewport_enabled = true;
            let a = app.scene.mesh.add_vertex(glam::DVec3::new(2., 2., 0.));
            let b = app.scene.mesh.add_vertex(glam::DVec3::new(2.5, 2., 0.));
            app.scene.mesh.add_edge(a, b);
            app.camera.distance = 10.;
            app.camera.target = glam::DVec3::ZERO;
            app.camera.perspective = false;
            app.camera.set_standard_view(StandardView::Top);
            app.activate_navigation(Mode::Position);
            Self {
                app,
                dir: std::env::args().nth(1).unwrap().into(),
                frames: 0,
            }
        }
        fn prepare_gpu(&mut self, system: &mut System, resize: bool) {
            <IngeTrazoApp as App>::prepare_gpu(&mut self.app, system, resize);
        }
        fn build_ui(&mut self, ctx: &egui::Context) {
            self.app.update_ui(ctx);
            self.frames += 1;
            let rect = self.app.native_gpu_viewport_rect;
            let preview = self
                .app
                .navigation
                .preview_line()
                .map(|(a, b)| [a.to_array(), b.to_array()]);
            let projected = rect.and_then(|r| {
                preview.map(|line| {
                    line.map(|p| {
                        ingetrazo_render::pipeline::project_point(
                            glam::DVec3::from_array(p),
                            &(self.app.camera.projection_matrix() * self.app.camera.view_matrix()),
                            r,
                        )
                        .map(|p| [p.0.x, p.0.y])
                    })
                })
            });
            let endpoint = rect.and_then(|r| {
                ingetrazo_render::pipeline::project_point(
                    glam::DVec3::new(2., 2., 0.),
                    &(self.app.camera.projection_matrix() * self.app.camera.view_matrix()),
                    r,
                )
                .map(|p| [p.0.x, p.0.y])
            });
            let state = serde_json::json!({"endpoint_pixel":endpoint,"snap_kind":self.app.snap_engine.current_inference().map(|r|format!("{:?}",r.kind)),"frames":self.frames,"eye":self.app.camera.eye().to_array(),"forward":self.app.camera.forward().to_array(),"mode":format!("{:?}",self.app.navigation.mode),"preview":preview,"preview_pixels":projected,"viewport":rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"undo":self.app.undo_stack.len(),"version":self.app.scene.version,"faces":self.app.scene.mesh.faces.len(),"focused":ctx.input(|i|i.focused),"pointer":ctx.input(|i|i.pointer.hover_pos()).map(|p|[p.x,p.y])});
            std::fs::write(self.dir.join("state.tmp"), state.to_string()).unwrap();
            std::fs::rename(self.dir.join("state.tmp"), self.dir.join("state.json")).unwrap();
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)
    }
}
#[cfg(not(target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::run()
}
#[cfg(target_os = "macos")]
fn main() {
    eprintln!("This diagnostic exercises native Vulkan on Linux/Windows; actual Metal viewport acceptance is separate.");
    std::process::exit(2);
}
