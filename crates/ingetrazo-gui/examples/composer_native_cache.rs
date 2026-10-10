#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
mod oracle {
    use glam::DVec3;
    use ingetrazo_core::composition::{SheetItem, ViewFrame};
    use ingetrazo_gui::{app::IngeTrazoApp, composer_compute::ComposerCompute};
    pub fn paint(a: &mut IngeTrazoApp, c: &egui::Context) {
        let _ = c.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 900.),
                )),
                ..Default::default()
            },
            |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    ingetrazo_gui::composer::render_sheet_composer(a, ui, 0)
                });
            },
        );
    }
    pub fn verify(a: &mut IngeTrazoApp) -> Vec<usize> {
        let frames: Vec<_> = a.scene.sheets[0]
            .items
            .iter()
            .filter_map(|x| {
                if let SheetItem::ViewFrame(f) = x {
                    Some(f.clone())
                } else {
                    None
                }
            })
            .collect();
        let jobs = a.composer_compute.completed_jobs;
        let mut counts = vec![];
        for f in frames {
            let mut s = a.scene.clone();
            if let Some(name) = f.view_key.strip_prefix("scene:") {
                let v = s
                    .saved_views
                    .iter()
                    .find(|v| v.name == name)
                    .unwrap()
                    .clone();
                let mut c = s.camera.clone();
                v.apply_scene(&mut s, &mut c);
            }
            let camera = ingetrazo_gui::composer::camera_for_frame(&a.scene, &f);
            let actual = a
                .composer_compute
                .segments(
                    &s,
                    &camera,
                    a.clipboard_document,
                    a.model_revision,
                    &f.view_key,
                    f.profiles,
                    f.hidden_lines,
                )
                .unwrap();
            let expected = ingetrazo_render::vector_hlr::parallel_segments(
                &s,
                &camera,
                f.profiles,
                f.hidden_lines,
            );
            assert_eq!(actual.len(), expected.len());
            for (x, y) in actual.iter().zip(&expected) {
                assert_eq!(x.kind, y.kind);
                assert!(x.a.distance(y.a) < 1e-10 && x.b.distance(y.b) < 1e-10);
            }
            counts.push(actual.len());
        }
        assert_eq!(
            a.composer_compute.completed_jobs, jobs,
            "panel failed to populate exact frame cache"
        );
        counts
    }
}

use glam::DVec3;
use ingetrazo_core::{
    composition::{SheetItem, ViewFrame},
    groups::Group,
    mesh::Mesh,
    scene::SelectionItem,
};
use ingetrazo_gui::{
    composer_compute::ComposerCompute,
    native_vulkan::{App, System},
    IngeTrazoApp,
};
struct Fixture {
    app: IngeTrazoApp,
    out: std::path::PathBuf,
    phase: String,
    pids: Vec<u32>,
}
impl App for Fixture {
    fn title() -> &'static str {
        "IngeTrazo native composer cache"
    }
    fn new(system: &mut System, _: Option<std::path::PathBuf>) -> Self {
        let mut app = <IngeTrazoApp as App>::new(system, None);
        let out = std::path::PathBuf::from(std::env::var("INGETRAZO_ACCEPTANCE_OUT").unwrap());
        app.composer_compute = ComposerCompute::with_worker(&std::path::PathBuf::from(
            std::env::var("INGETRAZO_HLR_WORKER_EXE").unwrap(),
        ));
        let mut mesh = Mesh::new();
        let ids =
            [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(), vec![]);
        app.scene.groups.push(Group::with_mesh("Panel", mesh));
        app.scene.sheets.push(Default::default());
        app.scene.sheets[0].items.clear();
        let mut frame = ViewFrame::default();
        frame.style = "vectorial".into();
        frame.view_key = "std:top".into();
        frame.scale_n = 10.0;
        app.scene.sheets[0].items.push(SheetItem::ViewFrame(frame));
        app.scene.active_sheet_index = Some(0);
        app.mark_document_clean();
        Self {
            app,
            out,
            phase: String::new(),
            pids: vec![],
        }
    }
    fn build_ui(&mut self, ctx: &egui::Context) {
        let requested =
            std::fs::read_to_string(self.out.join("phase")).unwrap_or_else(|_| "baseline".into());
        if requested != self.phase {
            match requested.as_str() {
                "edited" => {
                    self.app.scene.active_sheet_index = None;
                    self.app.save_undo_state();
                    self.app.scene.groups[0].transform =
                        glam::DMat4::from_scale(DVec3::new(1.5, 1., 1.));
                    self.app.scene.bump_version();
                    self.app.scene.active_sheet_index = Some(0);
                }
                "undo" => {
                    self.app.scene.active_sheet_index = None;
                    self.app.undo();
                    self.app.scene.active_sheet_index = Some(0);
                }
                "context-cancel" => {
                    self.app.scene.active_sheet_index = None;
                    self.app.scene.select(SelectionItem::Group(0));
                    self.app.begin_component_edit();
                    assert!(!self.app.scene.mesh.faces.is_empty());
                    self.app.scene.mesh.vertices[0].position += DVec3::X * 0.1;
                    self.app.scene.bump_version();
                    self.app.cancel_component_edit();
                    self.app.scene.active_sheet_index = Some(0);
                }
                "recovered" => {
                    let store = ingetrazo_gui::session_state::SessionStore::isolated(
                        &self.out.join("writer"),
                    );
                    let mut scene = self.app.scene.clone();
                    scene.groups[0].transform = glam::DMat4::from_scale(DVec3::new(0.5, 1., 1.));
                    let record = store.autosave(&scene, None).unwrap();
                    self.app.mark_document_clean();
                    self.app.request_document_action(
                        ingetrazo_gui::document_session::DocumentAction::Recover(record),
                    );
                    self.app.scene.active_sheet_index = Some(0);
                }
                _ => {}
            }
            self.phase = requested;
        }
        self.app.update_ui(ctx);
        let counts = oracle::verify(&mut self.app);
        let pids = self.app.composer_compute.worker_ids();
        if self.pids.is_empty() {
            self.pids = pids.clone();
        }
        assert_eq!(self.pids, pids);
        let state = serde_json::json!({"phase":self.phase,"jobs":self.app.composer_compute.completed_jobs,"pids":pids,"counts":counts,"model_revision":self.app.model_revision});
        std::fs::write(self.out.join("state.tmp"), state.to_string()).unwrap();
        std::fs::rename(self.out.join("state.tmp"), self.out.join("state.json")).unwrap();
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)
}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
