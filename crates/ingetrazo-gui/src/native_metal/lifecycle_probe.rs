//! Own-window Cocoa diagnostic: actual native event delivery plus shared app.
use super::*;
use egui::epaint::{ClippedShape, Shape};
use ingetrazo_metal::native_input::{MousePhase, mouse_for_probe, request_close_for_probe};

pub(super) struct Probe {
    pub root: PathBuf,
    pub document: PathBuf,
    choice: String,
    initial_bytes: Vec<u8>,
    started: Instant,
    phase: u8,
    label: String,
    center: Option<[f64; 2]>,
    modal_presented: u64,
    ui_turns: u64,
    observed_turn: u64,
    close_events: u64,
    moves: u64,
    presses: u64,
    releases: u64,
    cancel_verified: bool,
    event_baseline: (u64, u64, u64),
}
impl Probe {
    pub fn new(root: PathBuf, choice: &str) -> Result<Self, String> {
        if !matches!(choice, "save" | "discard" | "cancel") {
            return Err("choice must be save|discard|cancel".into());
        }
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        if std::fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
        {
            return Err("Lifecycle probe requires fresh output directory".into());
        }
        let document = root.join("document.igz");
        ingetrazo_formats::igz::save_igz(&ingetrazo_core::scene::Scene::default(), &document)
            .map_err(|e| e.to_string())?;
        let initial_bytes = std::fs::read(&document).map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            document,
            choice: choice.into(),
            initial_bytes,
            started: Instant::now(),
            phase: 0,
            label: choice.to_ascii_lowercase(),
            center: None,
            modal_presented: 0,
            ui_turns: 0,
            observed_turn: 0,
            close_events: 0,
            moves: 0,
            presses: 0,
            releases: 0,
            cancel_verified: false,
            event_baseline: (0, 0, 0),
        })
    }
    pub fn setup(&mut self, app: &mut IngeTrazoApp) -> Result<(), String> {
        if app.current_file.as_ref() != Some(&self.document) || app.document_dirty() {
            return Err("Native probe did not reopen clean IGZ".into());
        }
        let ids = [
            glam::DVec3::new(123., 234., 345.),
            glam::DVec3::new(124., 234., 345.),
            glam::DVec3::new(123., 235., 345.),
        ]
        .map(|p| app.scene.mesh.add_vertex(p));
        app.scene
            .mesh
            .add_face(ids.to_vec(), vec![])
            .ok_or("Could not author dirty triangle")?;
        if !app.document_dirty() {
            return Err("Authored triangle did not dirty document".into());
        }
        Ok(())
    }
    pub fn native_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_events += 1,
            WindowEvent::CursorMoved { .. } => self.moves += 1,
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => self.presses += 1,
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Released,
                button: winit::event::MouseButton::Left,
                ..
            } => self.releases += 1,
            _ => {}
        }
    }
    pub fn observe(&mut self, app: &IngeTrazoApp, shapes: &[ClippedShape], displayed: u64) {
        self.ui_turns += 1;
        if !app.document_session.confirm_unsaved {
            return;
        }
        fn find(shape: &Shape, label: &str) -> Option<[f64; 2]> {
            match shape {
                Shape::Text(text) if text.galley.text().eq_ignore_ascii_case(label) => {
                    let p = text.pos + text.galley.rect.center().to_vec2();
                    Some([p.x as f64, p.y as f64])
                }
                Shape::Vec(children) => children.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        if let Some(center) = shapes.iter().find_map(|s| find(&s.shape, &self.label)) {
            if self.center.is_none() {
                self.modal_presented = displayed;
            }
            self.center = Some(center);
        }
    }
    pub fn tick(
        &mut self,
        app: &IngeTrazoApp,
        gpu: &MetalPresentation,
        window: &Window,
    ) -> Result<(), String> {
        if self.started.elapsed() > Duration::from_secs(15) {
            return Err(format!(
                "Bounded Cocoa lifecycle deadline phase={} close={} move={} press={} release={} displayed={}",
                self.phase,
                self.close_events,
                self.moves,
                self.presses,
                self.releases,
                gpu.presentation_counters().0
            ));
        }
        match self.phase {
            0 if gpu.presentation_counters().0 > 0 => {
                request_close_for_probe(window)?;
                self.phase = 1;
            }
            1 if self.close_events > 0
                && app.document_session.confirm_unsaved
                && self.center.is_some()
                && gpu.presentation_counters().0 > self.modal_presented =>
            {
                self.event_baseline = (self.moves, self.presses, self.releases);
                mouse_for_probe(
                    window,
                    self.center.unwrap(),
                    MousePhase::Move,
                    self.started.elapsed().as_secs_f64(),
                )?;
                self.observed_turn = self.ui_turns;
                self.phase = 2;
            }
            2 if self.moves > self.event_baseline.0 && self.ui_turns > self.observed_turn => {
                self.event_baseline.1 = self.presses;
                mouse_for_probe(
                    window,
                    self.center.unwrap(),
                    MousePhase::Press,
                    self.started.elapsed().as_secs_f64(),
                )?;
                self.observed_turn = self.ui_turns;
                self.phase = 3;
            }
            3 if self.presses > self.event_baseline.1 && self.ui_turns > self.observed_turn => {
                self.event_baseline.2 = self.releases;
                mouse_for_probe(
                    window,
                    self.center.unwrap(),
                    MousePhase::Release,
                    self.started.elapsed().as_secs_f64(),
                )?;
                self.phase = 4;
            }
            4 if self.choice == "cancel"
                && self.releases > self.event_baseline.2
                && !app.document_session.confirm_unsaved
                && app.document_session.pending.is_none() =>
            {
                if !app.document_dirty()
                    || app.document_session.close_allowed
                    || std::fs::read(&self.document).map_err(|e| e.to_string())?
                        != self.initial_bytes
                {
                    return Err("Cancel changed dirty document/file/close permission".into());
                }
                self.cancel_verified = true;
                self.label = "discard".into();
                self.center = None;
                self.moves = 0;
                self.presses = 0;
                self.releases = 0;
                request_close_for_probe(window)?;
                self.phase = 1;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn finish(
        &mut self,
        app: &IngeTrazoApp,
        gpu: &MetalPresentation,
        error: Option<&str>,
    ) -> Result<(), String> {
        let check = (|| -> Result<(), String> {
            if let Some(error) = error {
                return Err(error.into());
            }
            if self.phase != 4
                || self.close_events == 0
                || self.presses == 0
                || self.releases <= self.event_baseline.2
                || !app.document_session.close_allowed
            {
                return Err("Missing actual Cocoa input or authorized native closure".into());
            }
            if gpu.presentation_counters().0 == 0 {
                return Err("No positive native presentation".into());
            }
            let bytes = std::fs::read(&self.document).map_err(|e| e.to_string())?;
            if self.choice == "save" {
                if app.document_dirty() || bytes == self.initial_bytes {
                    return Err("Save did not persist authored dirty model".into());
                }
                let reopened =
                    ingetrazo_formats::igz::load_igz(&self.document).map_err(|e| e.to_string())?;
                let marker = glam::DVec3::new(123., 234., 345.);
                if !reopened.mesh.faces.iter().any(|face| {
                    face.vertices
                        .iter()
                        .any(|&id| reopened.mesh.vertices[id].position == marker)
                }) {
                    return Err("Saved native IGZ lacks authored triangle".into());
                }
            } else if bytes != self.initial_bytes {
                return Err("Discard/Cancel modified original document bytes".into());
            }
            if self.choice == "cancel" && (!self.cancel_verified || self.close_events < 2) {
                return Err("Cancel state not observed before subsequent Discard".into());
            }
            Ok(())
        })();
        let report = serde_json::json!({"passed":check.is_ok(),"choice":self.choice,"phase":self.phase,"close_events":self.close_events,"moves":self.moves,"presses":self.presses,"releases":self.releases,"cancel_verified":self.cancel_verified,"displayed":gpu.presentation_counters().0,"skipped":gpu.presentation_counters().1,"pending":gpu.counters().2,"error":check.as_ref().err(),"physical_input_claim":false,"physical_mac_claim":false});
        std::fs::write(
            self.root.join("result.json"),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        check
    }
}
