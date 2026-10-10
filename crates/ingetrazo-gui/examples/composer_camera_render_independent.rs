#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
use glam::DVec3;
use ingetrazo_core::{
    composition::{SheetItem, ViewFrame},
    groups::Group,
    mesh::Mesh,

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
        "IngeTrazo independent live-camera composer"
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
        app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
        app.camera.target=DVec3::new(0.5,0.5,0.);app.camera.distance=4.;
        app.scene.camera=app.camera.clone();
        app.scene.camera.set_standard_view(ingetrazo_core::camera::StandardView::Front);
        app.scene.saved_views.push(ingetrazo_core::saved_views::SavedView::capture_scene("StoredFront",&app.scene,&app.scene.camera));
        app.scene.sheets.push(Default::default());
        app.scene.sheets[0].items.clear();
        let mut frame = ViewFrame::default();
        frame.style = "vectorial".into();
        frame.show_title=false;
        frame.view_key = String::new();
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
            let frame=match &mut self.app.scene.sheets[0].items[0]{SheetItem::ViewFrame(f)=>f,_=>panic!()};
            frame.view_key=String::new();
            match requested.as_str(){
                "navigated"=>self.app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Front),
                "restored"|"missing"|"bound"|"bound-live-change"=>self.app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top),
                _=>{}
            }
            if requested=="missing"{frame.view_key="scene:Missing".into();}
            if requested=="bound"||requested=="bound-live-change"{frame.view_key="scene:StoredFront".into();}
            if requested=="bound-live-change"{self.app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Iso);}
            if requested.starts_with("order-"){
                self.app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
                self.app.scene.sheets[0].items.truncate(1);
                let mut blank=match &self.app.scene.sheets[0].items[0]{SheetItem::ViewFrame(f)=>f.clone(),_=>panic!()};
                blank.id=ingetrazo_core::composition::ViewFrame::default().id;blank.view_key="std:front".into();
                let z=if requested=="order-behind"{-1.}else if requested=="order-cover"{1.}else{0.};
                self.app.scene.sheets[0].native_items.insert(blank.id,serde_json::json!({"z":z,"hidden":requested=="order-hidden"}));
                self.app.scene.sheets[0].items.push(SheetItem::ViewFrame(blank));
            }
            if requested=="border-on"{self.app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);self.app.scene.sheets[0].items.truncate(1);if let SheetItem::ViewFrame(f)=&mut self.app.scene.sheets[0].items[0]{f.border=true;}}
            self.phase = requested;
        }
        self.app.update_ui(ctx);

        let pids = self.app.composer_compute.worker_ids();
        if self.pids.is_empty() {
            self.pids = pids.clone();
        }
        assert_eq!(self.pids, pids);
        let state = serde_json::json!({"phase":self.phase,"jobs":self.app.composer_compute.completed_jobs,"pids":pids,"live_camera":self.app.camera,"stored_camera":self.app.scene.camera,"model_revision":self.app.model_revision});
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
