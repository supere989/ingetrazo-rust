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
    modal_submission: Option<u64>,
    deadline_presentation_diagnostics: Option<String>,
    deadline_window_visible: Option<bool>,
    current_submission_floor: u64,
    current_output_has_modal: bool,
    modal_candidates: std::collections::VecDeque<(u64, [f64; 2])>,
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
            modal_submission: None,
            deadline_presentation_diagnostics: None,
            deadline_window_visible: None,
            current_submission_floor: 0,
            current_output_has_modal: false,
            modal_candidates: std::collections::VecDeque::with_capacity(32),
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
    pub fn observe(
        &mut self,
        app: &IngeTrazoApp,
        shapes: &[ClippedShape],
        submitted_before_ui: u64,
    ) {
        self.ui_turns += 1;
        self.current_submission_floor = submitted_before_ui;
        self.current_output_has_modal = false;
        self.center = None;
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
            self.center = Some(center);
            self.current_output_has_modal = true;
        }
    }
    /// Called only after present_paint reports an actual submitted command for
    /// this UI output containing the modal, not merely after a redraw attempt.
    pub fn submitted(&mut self, app: &IngeTrazoApp, submission: u64) {
        if self.phase == 1
            && app.document_session.confirm_unsaved
            && self.current_output_has_modal
            && submission > self.current_submission_floor
        {
            if let Some(center) = self.center {
                if self.modal_candidates.len() == 32 {
                    self.modal_candidates.pop_front();
                }
                self.modal_candidates.push_back((submission, center));
            }
        }
    }
    fn needs_cancel_verification(&self, app: &IngeTrazoApp) -> bool {
        self.choice == "cancel"
            && !self.cancel_verified
            && self.releases > self.event_baseline.2
            && !app.document_session.confirm_unsaved
            && app.document_session.pending.is_none()
    }
    pub fn tick(
        &mut self,
        app: &IngeTrazoApp,
        gpu: &MetalPresentation,
        window: &Window,
    ) -> Result<(), String> {
        if self.started.elapsed() > Duration::from_secs(15) {
            // Snapshot before event-loop teardown can order the native window out.
            // This is error-only observation, never proof of display/retirement.
            self.deadline_window_visible = window.is_visible();
            self.deadline_presentation_diagnostics = Some(gpu.diagnostics());
            eprintln!("Metal lifecycle BEFORE event-loop exit: {}",self.deadline_presentation_diagnostics.as_deref().unwrap());
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
                && self
                    .modal_candidates
                    .iter()
                    .any(|(id, _)| gpu.was_displayed(*id)) =>
            {
                let (id, center) = self
                    .modal_candidates
                    .iter()
                    .copied()
                    .find(|(id, _)| gpu.was_displayed(*id))
                    .ok_or("Missing exact presented modal command")?;
                self.modal_submission = Some(id);
                self.center = Some(center);
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
            4 if self.needs_cancel_verification(app) => {
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
                self.modal_submission = None;
                self.modal_candidates.clear();
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
        let report = serde_json::json!({"passed":check.is_ok(),"choice":self.choice,"phase":self.phase,"close_events":self.close_events,"moves":self.moves,"presses":self.presses,"releases":self.releases,"cancel_verified":self.cancel_verified,"modal_submission":self.modal_submission,"deadline_window_visible":self.deadline_window_visible,"deadline_presentation_diagnostics":self.deadline_presentation_diagnostics,"modal_candidates":self.modal_candidates,"modal_label":self.label,"last_modal_center":self.center,"current_output_has_modal":self.current_output_has_modal,"current_submission_floor":self.current_submission_floor,"ui_turns":self.ui_turns,"confirm_unsaved":app.document_session.confirm_unsaved,"presentation_diagnostics":gpu.diagnostics(),"displayed":gpu.presentation_counters().0,"skipped":gpu.presentation_counters().1,"pending":gpu.counters().2,"error":check.as_ref().err(),"physical_input_claim":false,"physical_mac_claim":false});
        std::fs::write(
            self.root.join("result.json"),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        check
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_session::DocumentAction;
    #[test]
    fn previous_submission_or_current_output_without_modal_cannot_label_a_frame() {
        let root =
            std::env::temp_dir().join(format!("metal-modal-identity-{}", uuid::Uuid::new_v4()));
        let mut probe = Probe::new(root.clone(), "discard").unwrap();
        let mut app = IngeTrazoApp::new();
        app.document_session.confirm_unsaved = true;
        probe.phase = 1;
        // A stale center and a prior successful GPU submission are not evidence
        // for this ctx.run output, even if the native modal state is stilltrue.
        probe.center = Some([10., 10.]);
        probe.observe(&app, &[], 7);
        assert!(probe.center.is_none());
        probe.submitted(&app, 7);
        probe.submitted(&app, 8);
        assert!(probe.modal_candidates.is_empty());
        // The per-output callback must advance beyond the recorded preparation
        // floor; same-ID hook at can_prepare reproduced the rejected regression.
        probe.current_output_has_modal = true;
        probe.center = Some([10., 10.]);
        probe.submitted(&app, 7);
        assert!(probe.modal_candidates.is_empty());
        probe.submitted(&app, 8);
        assert_eq!(probe.modal_candidates.front(), Some(&(8, [10., 10.])));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn actual_document_cancel_then_discard_does_not_repeat_cancel_assertion() {
        let root =
            std::env::temp_dir().join(format!("metal-cancel-guard-{}", uuid::Uuid::new_v4()));
        let mut probe = Probe::new(root.clone(), "cancel").unwrap();
        let mut app = IngeTrazoApp::new();
        app.attach_session_store(SessionStore::isolated(&root.join("session")));
        app.open_document(&probe.document);
        probe.setup(&mut app).unwrap();
        app.request_document_action(DocumentAction::Close);
        assert!(!probe.needs_cancel_verification(&app));
        app.cancel_document_action();
        probe.phase = 4;
        probe.releases = 1;
        assert!(probe.needs_cancel_verification(&app));
        assert!(app.document_dirty());
        assert!(!app.document_session.close_allowed);
        probe.cancel_verified = true;
        app.request_document_action(DocumentAction::Close);
        app.discard_document_action();
        assert!(app.document_session.close_allowed);
        assert!(app.document_dirty());
        // Phase remains4 after second button release, exactly the former race.
        assert_eq!(probe.phase, 4);
        assert!(!probe.needs_cancel_verification(&app));
        assert_eq!(std::fs::read(&probe.document).unwrap(), probe.initial_bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}
