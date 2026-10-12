//! DAE/glTF actual App import: no invented options or camera fit.
use crate::IngeTrazoApp;
use ingetrazo_formats::FileImportPlan;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dae,
    Gltf,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Dae => "DAE",
            Self::Gltf => "glTF/GLB",
        }
    }
}
struct Job {
    path: PathBuf,
    kind: Kind,
}
struct Service {
    send: Option<mpsc::SyncSender<Job>>,
    done: mpsc::Receiver<Result<FileImportPlan, String>>,
    thread: Option<thread::JoinHandle<()>>,
    parses: Arc<AtomicU64>,
}
impl Service {
    fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<Job>(1);
        let (out, done) = mpsc::sync_channel(1);
        let parses = Arc::new(AtomicU64::new(0));
        let count = parses.clone();
        let thread = thread::Builder::new()
            .name("exchange-import".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    count.fetch_add(1, Ordering::Relaxed);
                    let result = match job.kind {
                        Kind::Dae => ingetrazo_formats::parse_dae_import(&job.path)
                            .map_err(|e| e.to_string()),
                        Kind::Gltf => ingetrazo_formats::gltf_import::parse_gltf_import(&job.path)
                            .map_err(|e| e.to_string()),
                    };
                    if out.send(result).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            send: Some(tx),
            done,
            thread: Some(thread),
            parses,
        })
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.send.take();
        let (_, empty) = mpsc::channel();
        self.done = empty;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
#[derive(Default)]
pub struct State {
    service: Option<Service>,
    running: Option<(PathBuf, Kind, uuid::Uuid, u64)>,
    pub error: Option<String>,
    pub completed: u64,
    pub label: Option<(uuid::Uuid, String)>,
}
impl State {
    pub fn owns_input(&self) -> bool {
        self.running.is_some() || self.error.is_some()
    }
}
impl IngeTrazoApp {
    pub fn choose_dae_import(&mut self) {
        self.choose_exchange(Kind::Dae);
    }
    pub fn choose_glb_import(&mut self) {
        self.choose_exchange(Kind::Gltf);
    }
    fn choose_exchange(&mut self, kind: Kind) {
        let filter = match kind {
            Kind::Dae => "*.dae|COLLADA (*.dae)",
            Kind::Gltf => "*.glb *.gltf|glTF/GLB (*.glb *.gltf)",
        };
        if let Some(path) =
            crate::dialogs::pick_open_file(&format!("Import {}", kind.label()), filter)
        {
            if let Err(e) = self.begin_exchange_import(&path, kind) {
                self.status_message = e;
            }
        }
    }
    pub fn begin_exchange_import(&mut self, path: &Path, kind: Kind) -> Result<(), String> {
        if self.exchange_import.owns_input() {
            return Err("An exchange import already owns input".into());
        }
        if self.exchange_import.service.is_none() {
            self.exchange_import.service = Some(Service::new()?);
        }
        self.exchange_import
            .service
            .as_ref()
            .unwrap()
            .send
            .as_ref()
            .ok_or("Import worker stopped")?
            .try_send(Job {
                path: path.to_owned(),
                kind,
            })
            .map_err(|_| "Import worker queue unavailable")?;
        self.exchange_import.running = Some((
            path.to_owned(),
            kind,
            self.clipboard_document,
            self.model_revision,
        ));
        self.status_message = format!("Importing {}", path.display());
        Ok(())
    }
    pub fn exchange_import_stats(&self) -> (u64, u64) {
        (
            self.exchange_import
                .service
                .as_ref()
                .map_or(0, |s| s.parses.load(Ordering::Relaxed)),
            self.exchange_import.completed,
        )
    }
    pub fn exchange_import_running(&self) -> bool {
        self.exchange_import.running.is_some()
    }
    pub fn dismiss_exchange_import_error(&mut self) {
        self.exchange_import.error = None;
    }
    pub fn poll_exchange_import(&mut self) {
        if self.exchange_import.running.is_none() {
            return;
        }
        let Some(result) = self
            .exchange_import
            .service
            .as_ref()
            .and_then(|s| s.done.try_recv().ok())
        else {
            return;
        };
        let (path, kind, document, revision) = self.exchange_import.running.take().unwrap();
        if (document, revision) != (self.clipboard_document, self.model_revision) {
            self.status_message = "Import result retired after document/model change".into();
            return;
        }
        match result.and_then(|plan| self.publish_exchange_import(plan)) {
            Ok(()) => {
                self.exchange_import.completed += 1;
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.exchange_import.label = Some((document, name.clone()));
                self.status_message = format!("Imported {name}");
            }
            Err(e) => {
                self.status_message = format!("Import {} failed: {e}", kind.label());
                self.exchange_import.error = Some(e);
            }
        }
    }
    fn publish_exchange_import(&mut self, plan: FileImportPlan) -> Result<(), String> {
        let active = plan.edits_active_mesh();
        let parked = plan.appends_root_groups() && !self.component_edit_stack.is_empty();
        let owner = if parked {
            &self.component_edit_stack[0].original_scene
        } else {
            &self.scene
        };
        let mut candidate = owner.clone();
        candidate.layers = self.scene.layers.clone();
        candidate.embedded_textures = self.scene.embedded_textures.clone();
        let active_mesh = if active && parked {
            let mut mesh = self.scene.mesh.clone();
            plan.emit_active_mesh(&mut mesh)?;
            Some(mesh)
        } else {
            if active {
                plan.emit_active_mesh(&mut candidate.mesh)?;
            }
            None
        };
        candidate
            .groups
            .try_reserve(plan.groups.len())
            .map_err(|_| "Import group allocation refused")?;
        candidate
            .embedded_textures
            .try_reserve(plan.assets.len())
            .map_err(|_| "Import resource allocation refused")?;
        candidate
            .component_definitions
            .try_reserve(plan.definitions.len())
            .map_err(|_| "Import definition allocation refused")?;
        candidate
            .mesh_shares
            .try_reserve(plan.shares.len())
            .map_err(|_| "Import mesh registry allocation refused")?;
        candidate.groups.extend(plan.groups);
        candidate.embedded_textures.extend(plan.assets);
        candidate.component_definitions.extend(plan.definitions);
        candidate.mesh_shares.extend(plan.shares);
        if candidate.geo_datum.is_none() {
            candidate.geo_datum = plan.datum;
        }
        if parked {
            self.publish_root_import_with_mesh(candidate, active_mesh)?;
        } else {
            self.undo_stack
                .try_reserve(1)
                .map_err(|_| "Import history allocation refused")?;
            // Source datum adoption is deliberately outside SnapshotImport Undo.
            self.scene.geo_datum = candidate.geo_datum.clone();
            self.save_undo_state();
            self.scene = candidate;
            if active {
                if let Some(s) = self.component_edit_stack.last_mut() {
                    s.note_source_mesh_import();
                }
            }
            self.bump_model_revision();
        }
        Ok(())
    }
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if app.exchange_import.running.is_some() {
        egui::Modal::new(egui::Id::new("exchange-import-progress")).show(ctx, |ui| {
            ui.label("Reading and preparing model…");
            ui.spinner();
        });
        ctx.request_repaint();
    } else if let Some(error) = app.exchange_import.error.clone() {
        let mut close = false;
        egui::Modal::new(egui::Id::new("exchange-import-error")).show(ctx, |ui| {
            ui.label(error);
            let response = ui.button("OK");
            if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::new("exchange-import-error-ok"), response.rect)
                });
            }
            close = response.clicked();
        });
        if close {
            app.dismiss_exchange_import_error();
        }
    }
}
