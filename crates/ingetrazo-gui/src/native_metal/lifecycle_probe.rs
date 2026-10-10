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
    canvas_uploads: Option<([u64; 5], u64)>,
    canvas_warm_frames: u64,
    canvas_last_raw: Option<[u64; 5]>,
    canvas_submission_observations: u64,
    canvas_cancelled: u64,
    canvas_raw_swatch: Option<egui::TextureHandle>,
}
impl Probe {
    pub fn new(root: PathBuf, choice: &str) -> Result<Self, String> {
        if !matches!(choice, "save" | "discard" | "cancel" | "canvas") {
            return Err("choice must be save|discard|cancel|canvas".into());
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
            canvas_uploads: None,
            canvas_warm_frames: 0,
            canvas_last_raw: None,
            canvas_submission_observations: 0,
            canvas_cancelled: 0,
            canvas_raw_swatch: None,
        })
    }
    pub fn setup(&mut self, app: &mut IngeTrazoApp) -> Result<(), String> {
        if app.current_file.as_ref() != Some(&self.document) || app.document_dirty() {
            return Err("Native probe did not reopen clean IGZ".into());
        }
        if self.choice == "canvas" {
            use ingetrazo_core::composition::{Sheet, SheetItem, TextItem};
            let mut sheet = Sheet::default();
            sheet.items.clear();
            sheet.items.push(SheetItem::Text(TextItem {
                id: uuid::Uuid::new_v4(), pos: (30., 35.),
                text: "Native Metal Canvas 0123456789".into(),
                font_size_pt: 24., bold: false,
            }));
            app.scene.sheets = vec![sheet];
            app.scene.active_sheet_index = Some(0);
            // Save through the application so the authored fixture becomes the
            // clean baseline before guarded Open. A raw writer leaves it dirty
            // and Open correctly queues an unsaved-document confirmation.
            app.save_to_path(&self.document);
            if app.document_dirty() { return Err("Canvas fixture save did not establish clean document".into()); }
            self.initial_bytes = std::fs::read(&self.document).map_err(|e| e.to_string())?;
            app.open_document(&self.document);
            app.scene.active_sheet_index = Some(0);
            if app.document_dirty() || app.document_session.pending.is_some() || app.document_session.confirm_unsaved {
                return Err("Canvas fixture did not reopen cleanly".into());
            }
            return Ok(());
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
    // Retain an independent raw-texture owner fixture when Text uses ordered CAD.
    // This diagnostic does not alter the strict source/native font corpus.
    pub fn setup_raw_swatch(&mut self, ctx: &egui::Context) -> Result<(), String> {
        if self.choice != "canvas" { return Ok(()); }
        self.canvas_raw_swatch = Some(crate::paper_raw_texture::create(ctx, [2, 2],
            vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255])?);
        let cancelled = crate::paper_raw_texture::create(ctx, [1, 1], vec![17, 31, 63, 255])?;
        drop(cancelled);
        Ok(())
    }
    pub fn paint_raw_swatch(&self, ctx: &egui::Context) {
        if let Some(texture) = &self.canvas_raw_swatch {
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground,
                egui::Id::new("metal-lifecycle-raw-swatch"))).image(texture.id(),
                egui::Rect::from_min_size(egui::pos2(8., 8.), egui::vec2(16., 16.)),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)), egui::Color32::WHITE);
        }
    }
    /// Observe only successfully submitted application frames. No queue work is
    /// added to ordinary host rendering when the diagnostic is absent.
    pub fn canvas_submitted(&mut self, ctx: &egui::Context, painter: &EguiPainter) {
        if self.choice != "canvas" { return; }
        self.canvas_submission_observations += 1;
        self.canvas_last_raw = crate::paper_raw_texture::stats(ctx);
        let Some(raw) = self.canvas_last_raw else { return; };
        self.canvas_cancelled = crate::paper_raw_texture::cancelled(ctx).unwrap_or(0);
        let uploads = painter.stats().texture_uploads;
        if self.canvas_raw_swatch.is_none() || self.canvas_cancelled == 0 || raw[0] != 0 || raw[1] != 0 || raw[4] == 0 || raw[2].checked_sub(self.canvas_cancelled) != Some(raw[4]) {
            self.canvas_warm_frames = 0;
            self.canvas_uploads = None;
            return;
        }
        if self.canvas_uploads == Some((raw, uploads)) {
            self.canvas_warm_frames += 1;
        } else {
            self.canvas_warm_frames = 0;
        }
        self.canvas_uploads = Some((raw, uploads));
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
            // Capture live AppKit visibility before event-loop teardown changes it.
            std::fs::write(self.root.join("presentation-before-exit.txt"), gpu.diagnostics())
                .map_err(|e| e.to_string())?;
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
        if self.choice == "canvas" {
            // Diagnostic identity only: never substitutes for presentation proof.
            if self.started.elapsed() > Duration::from_secs(2) && !self.root.join("diagnostic-window.json").exists() {
                let snapshot = serde_json::json!({"window_id":gpu.native_window_id()?,"displayed":gpu.presentation_counters().0,"diagnostics":gpu.diagnostics(),"acceptance_proof":false});
                let pending=self.root.join("diagnostic-window.pending");
                std::fs::write(&pending,serde_json::to_vec_pretty(&snapshot).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                std::fs::rename(pending,self.root.join("diagnostic-window.json")).map_err(|e|e.to_string())?;
            }
            if gpu.presentation_counters().0 > 0 && self.phase == 0 && self.canvas_warm_frames >= 3 {
                let snapshot = serde_json::json!({
                    "window_id": gpu.native_window_id()?,
                    "submitted": gpu.counters().0,
                    "displayed": gpu.presentation_counters().0,
                    "completed_inflight": gpu.completed_inflight_for_probe(),
                    "physical_mac_claim": false,
                    "raw_uploads": self.canvas_uploads.map(|v|v.0),
                    "texture_uploads": self.canvas_uploads.map(|v|v.1),
                    "warm_frames": self.canvas_warm_frames,
                    "cancelled_uploads": self.canvas_cancelled,
                    "raw_owner_fixture": "live-2x2-swatch-and-cancelled-1x1"
                });
                let pending = self.root.join("own-window.pending");
                std::fs::write(&pending, serde_json::to_vec_pretty(&snapshot).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                std::fs::rename(pending, self.root.join("own-window.json")).map_err(|e| e.to_string())?;
                self.phase = 1;
            }
            if self.phase == 1 && self.root.join("capture-finished").is_file() {
                if app.document_dirty() { return Err("Canvas display changed document".into()); }
                request_close_for_probe(window)?;
                self.phase = 4;
            }
            return Ok(());
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
            if self.choice == "canvas" {
                if self.phase != 4 || self.close_events == 0 || gpu.presentation_counters().0 == 0 || self.canvas_warm_frames < 3 {
                    return Err("Missing completed Canvas presentation and native closure".into());
                }
                if app.document_dirty() || std::fs::read(&self.document).map_err(|e| e.to_string())? != self.initial_bytes {
                    return Err("Canvas display modified document".into());
                }
                return Ok(());
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
        std::fs::write(self.root.join("presentation-diagnostics.txt"), gpu.diagnostics()).map_err(|e| e.to_string())?;
        let report = serde_json::json!({"passed":check.is_ok(),"choice":self.choice,"phase":self.phase,"close_events":self.close_events,"moves":self.moves,"presses":self.presses,"releases":self.releases,"cancel_verified":self.cancel_verified,"modal_submission":self.modal_submission,"displayed":gpu.presentation_counters().0,"skipped":gpu.presentation_counters().1,"pending":gpu.counters().2,"error":check.as_ref().err(),"physical_input_claim":false,"physical_mac_claim":false,"canvas_cancelled":self.canvas_cancelled,"canvas_last_raw":self.canvas_last_raw,"canvas_submission_observations":self.canvas_submission_observations,"canvas_warm_frames":self.canvas_warm_frames,"canvas_last_stable_uploads":self.canvas_uploads,"application_status":app.status_message,"active_sheet_index":app.scene.active_sheet_index,"sheet_count":app.scene.sheets.len()});
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
