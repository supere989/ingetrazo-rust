//! STL dialog and one lazy mesh-only worker. Accepted options persist before parsing.
use crate::IngeTrazoApp;
use ingetrazo_core::i18n::tr;
use ingetrazo_formats::stl::{self, Simplification, StlPlan};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
};
struct Job {
    path: PathBuf,
    scale: f64,
    mode: Simplification,
}
struct Service {
    sender: Option<mpsc::SyncSender<Job>>,
    receiver: mpsc::Receiver<Result<StlPlan, String>>,
    worker: Option<thread::JoinHandle<()>>,
    parses: Arc<AtomicU64>,
    parse_ns: Arc<AtomicU64>,
}
impl Service {
    fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<Job>(1);
        let (done, result) = mpsc::sync_channel(1);
        let parses = Arc::new(AtomicU64::new(0));
        let count = parses.clone();
        let parse_ns = Arc::new(AtomicU64::new(0));
        let elapsed = parse_ns.clone();
        let worker = thread::Builder::new()
            .name("stl-import".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    count.fetch_add(1, Ordering::Relaxed);
                    let started = std::time::Instant::now();
                    let answer = stl::parse_stl_path(&job.path, job.scale, job.mode)
                        .map_err(|e| e.to_string());
                    elapsed.store(
                        started.elapsed().as_nanos().min(u64::MAX as u128) as u64,
                        Ordering::Release,
                    );
                    if done.send(answer).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender: Some(tx),
            receiver: result,
            worker: Some(worker),
            parses,
            parse_ns,
        })
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.sender.take();
        let (_, empty) = mpsc::channel();
        self.receiver = empty;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub struct Dialog {
    pub path: PathBuf,
    pub unit: String,
    pub simplify: String,
}
#[derive(Default)]
pub struct State {
    pub dialog: Option<Dialog>,
    service: Option<Service>,
    running: Option<(PathBuf, uuid::Uuid, u64)>,
    pub error: Option<String>,
    pub completed: u64,
}
impl State {
    pub fn owns_input(&self) -> bool {
        self.dialog.is_some() || self.running.is_some() || self.error.is_some()
    }
}
impl IngeTrazoApp {
    pub fn choose_stl_import(&mut self) {
        if let Some(path) = crate::dialogs::pick_open_file(&tr("Import STL"), "*.stl|STL mesh") {
            if let Err(e) = self.begin_stl_import(&path) {
                self.status_message = format!("{}: {e}", tr("Import STL failed"));
            }
        }
    }
    pub fn begin_stl_import(&mut self, path: &Path) -> Result<(), String> {
        if self.stl_import.owns_input() {
            return Err("An STL import already owns the dialog".into());
        }
        let prefs = &self.document_session.preferences.extra;
        let unit = prefs
            .get("import/stl_unit")
            .and_then(|v| v.as_str())
            .filter(|v| stl::unit_factor(v).is_some())
            .unwrap_or("mm")
            .to_string();
        let simplify = prefs
            .get("import/stl_simplify_mode")
            .and_then(|v| v.as_str())
            .filter(|v| Simplification::parse(v).is_ok())
            .unwrap_or("principal")
            .to_string();
        self.stl_import.dialog = Some(Dialog {
            path: path.to_path_buf(),
            unit,
            simplify,
        });
        Ok(())
    }
    pub fn cancel_stl_import(&mut self) {
        self.stl_import.dialog = None;
    }
    pub fn accept_stl_import(&mut self) -> Result<(), String> {
        let dialog = self
            .stl_import
            .dialog
            .as_ref()
            .ok_or("No STL dialog is pending")?;
        let scale = stl::unit_factor(&dialog.unit).ok_or("Invalid STL unit")?;
        let mode = Simplification::parse(&dialog.simplify).map_err(|e| e.to_string())?;
        if self.stl_import.service.is_none() {
            self.stl_import.service = Some(Service::new()?);
        }
        let job = Job {
            path: dialog.path.clone(),
            scale,
            mode,
        };
        self.stl_import
            .service
            .as_ref()
            .unwrap()
            .sender
            .as_ref()
            .ok_or("STL worker stopped")?
            .try_send(job)
            .map_err(|_| "STL worker queue unavailable")?;
        let dialog = self.stl_import.dialog.take().unwrap();
        self.document_session
            .preferences
            .extra
            .insert("import/stl_unit".into(), dialog.unit.into());
        self.document_session
            .preferences
            .extra
            .insert("import/stl_simplify_mode".into(), dialog.simplify.into());
        self.persist_preferences();
        self.status_message = format!("{} {}", tr("Importing"), dialog.path.display());
        self.stl_import.running = Some((dialog.path, self.clipboard_document, self.model_revision));
        Ok(())
    }
    pub fn stl_import_stats(&self) -> (u64, u64) {
        (
            self.stl_import
                .service
                .as_ref()
                .map_or(0, |s| s.parses.load(Ordering::Relaxed)),
            self.stl_import.completed,
        )
    }
    /// Cold diagnostic for the accepted worker job; no geometry/timing policy change.
    pub fn stl_worker_seconds(&self) -> f64 {
        self.stl_import
            .service
            .as_ref()
            .map_or(0., |s| s.parse_ns.load(Ordering::Acquire) as f64 / 1e9)
    }
    pub fn poll_stl_import(&mut self) {
        if self.stl_import.running.is_none() {
            return;
        }
        let Some(result) = self
            .stl_import
            .service
            .as_ref()
            .and_then(|s| s.receiver.try_recv().ok())
        else {
            return;
        };
        let (path, document, revision) = self.stl_import.running.take().unwrap();
        if (document, revision) != (self.clipboard_document, self.model_revision) {
            self.status_message = "STL result retired after document/model change".into();
            return;
        }
        let answer = result.and_then(|plan| self.publish_stl_import(plan, &path));
        if let Err(error) = answer {
            self.status_message = format!("{}: {error}", tr("Import STL failed"));
            self.stl_import.error = Some(error);
        } else {
            self.stl_import.completed += 1;
        }
    }
    fn publish_stl_import(&mut self, plan: StlPlan, path: &Path) -> Result<(), String> {
        // Small meshes target the active editing context; large reference groups
        // belong to the document root, matching the original Scene.groups owner.
        let grouped = plan.grouped();
        let parked = grouped && !self.component_edit_stack.is_empty();
        let owner = if parked {
            &self.component_edit_stack[0].original_scene
        } else {
            &self.scene
        };
        let mut candidate = owner.clone();
        candidate.layers = self.scene.layers.clone();
        if parked {
            candidate.embedded_textures = self.scene.embedded_textures.clone();
        }
        plan.emit(
            &mut candidate,
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("STL"),
        )
        .map_err(|e| e.to_string())?;
        if parked {
            self.publish_root_import(candidate)?;
        } else {
            self.undo_stack
                .try_reserve(1)
                .map_err(|_| "STL history allocation refused")?;
            self.save_undo_state();
            self.scene = candidate;
            if !grouped {
                if let Some(session) = self.component_edit_stack.last_mut() {
                    session.note_source_mesh_import();
                }
            }
            self.bump_model_revision();
        }
        self.status_message = format!(
            "{} {}",
            tr("Imported"),
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        Ok(())
    }
}
fn observe(ui: &egui::Ui, id: &str, r: egui::Rect) {
    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(id), r));
    }
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if app.stl_import.running.is_some() {
        egui::Modal::new(egui::Id::new("stl-progress")).show(ctx, |ui| {
            ui.label(tr("Reading and preparing mesh…"));
            ui.spinner();
        });
        ctx.request_repaint();
        return;
    }
    if let Some(error) = app.stl_import.error.clone() {
        let mut close = false;
        egui::Modal::new(egui::Id::new("stl-error")).show(ctx, |ui| {
            ui.label(error);
            close = ui.button(tr("OK")).clicked();
        });
        if close {
            app.stl_import.error = None;
        }
        return;
    }
    let Some(dialog) = app.stl_import.dialog.as_mut() else {
        return;
    };
    let (mut accept, mut cancel) = (false, false);
    egui::Modal::new(egui::Id::new("stl-options")).show(ctx, |ui| {
        ui.label(
            dialog
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
        );
        let unit_response = egui::ComboBox::from_id_salt("stl-unit")
            .selected_text(crate::import_units::unit_label(&dialog.unit))
            .show_ui(ui, |ui| {
                for key in ["m", "cm", "mm", "in", "ft"] {
                    let r = ui.selectable_value(
                        &mut dialog.unit,
                        key.into(),
                        crate::import_units::unit_label(key),
                    );
                    observe(ui, &format!("stl-ui-unit-{key}"), r.rect);
                }
            });
        observe(ui, "stl-ui-unit-combo", unit_response.response.rect);
        let mode_response = egui::ComboBox::from_id_salt("stl-simplify")
            .selected_text(&dialog.simplify)
            .show_ui(ui, |ui| {
                for (key, label) in [
                    ("none", "No mesh simplification"),
                    (
                        "principal",
                        "Merge coplanar triangles on XY, XZ and YZ planes",
                    ),
                    ("all", "Advanced: merge all coplanar surfaces"),
                ] {
                    let r = ui.selectable_value(&mut dialog.simplify, key.into(), tr(label));
                    observe(ui, &format!("stl-ui-mode-{key}"), r.rect);
                }
            });
        observe(ui, "stl-ui-mode-combo", mode_response.response.rect);
        ui.horizontal(|ui| {
            let r = ui.button(tr("Import"));
            observe(ui, "stl-ui-import", r.rect);
            accept = r.clicked();
            let r = ui.button(tr("Cancel"));
            observe(ui, "stl-ui-cancel", r.rect);
            cancel = r.clicked();
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if cancel {
        app.cancel_stl_import();
    } else if accept {
        if let Err(error) = app.accept_stl_import() {
            app.stl_import.error = Some(error);
        }
    }
}
