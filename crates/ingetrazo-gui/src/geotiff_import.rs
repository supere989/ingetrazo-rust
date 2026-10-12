//! Cold orthomosaic import. One lazy worker; no frame file I/O or worker Scene copy.
//! Publication retains the existing required Add history snapshot. The configured
//! CPU byte limit admits TIFF decoding only; libwebp picture/work is separate.
use crate::IngeTrazoApp;
use glam::DVec3;
use ingetrazo_core::{
    camera::StandardView,
    georef::{SceneDatum, TileCapture},
    i18n::tr,
    image_plane::ImagePlane,
};
use ingetrazo_formats::geotiff::{self, Info, Limits};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
};
struct Job {
    path: PathBuf,
    max_px: usize,
    limits: Limits,
    cancel: Arc<AtomicBool>,
}
struct Prepared {
    info: Info,
    width: u32,
    height: u32,
    factor: usize,
    encoded: Vec<u8>,
    stats: geotiff::DecodeStats,
}
struct Service {
    sender: Option<mpsc::SyncSender<Job>>,
    receiver: mpsc::Receiver<Result<Prepared, String>>,
    worker: Option<thread::JoinHandle<()>>,
    busy: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
    cancel: Option<Arc<AtomicBool>>,
}
impl Service {
    fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<Job>(1);
        let (done, result) = mpsc::sync_channel(1);
        let busy = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0f64.to_bits()));
        let running = busy.clone();
        let fraction = progress.clone();
        let worker = thread::Builder::new()
            .name("geotiff-import".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    let answer = (|| {
                        let mut file = std::fs::File::open(&job.path).map_err(|e| e.to_string())?;
                        let info = geotiff::read_header(&mut file, &job.path, job.limits)?;
                        if !info.georeferenced() {
                            return Err("The orthomosaic carries no supported georeference".into());
                        }
                        if let Some(reason) = info.unsupported_reason() {
                            return Err(format!("{reason} is not supported by this reader"));
                        }
                        info.corners_geodetic()?;
                        let decoded =
                            geotiff::decode(&mut file, &info, job.max_px, job.limits, |v| {
                                fraction.store((v * 0.9).to_bits(), Ordering::Release);
                                !job.cancel.load(Ordering::Acquire)
                            })?;
                        if job.cancel.load(Ordering::Acquire) {
                            return Err("cancelled".into());
                        }
                        fraction.store(0.92f64.to_bits(), Ordering::Release);
                        let encoded = ingetrazo_formats::webp::encode_rgba92(
                            &decoded.rgba,
                            decoded.width as u32,
                            decoded.height as u32,
                        )?;
                        if job.cancel.load(Ordering::Acquire) {
                            return Err("cancelled".into());
                        }
                        Ok(Prepared {
                            info,
                            width: decoded.width as u32,
                            height: decoded.height as u32,
                            factor: decoded.factor,
                            encoded,
                            stats: decoded.stats,
                        })
                    })();
                    running.store(false, Ordering::Release);
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
            busy,
            progress,
            cancel: None,
        })
    }
    fn begin(&mut self, path: PathBuf, max_px: usize, limits: Limits) -> Result<(), String> {
        if self.busy.swap(true, Ordering::AcqRel) {
            return Err("The previous orthomosaic operation is still completing".into());
        }
        self.progress.store(0f64.to_bits(), Ordering::Release);
        let cancel = Arc::new(AtomicBool::new(false));
        let job = Job {
            path,
            max_px,
            limits,
            cancel: cancel.clone(),
        };
        if self
            .sender
            .as_ref()
            .ok_or("Orthomosaic worker has stopped")?
            .try_send(job)
            .is_err()
        {
            self.busy.store(false, Ordering::Release);
            return Err("Orthomosaic queue is unavailable".into());
        }
        self.cancel = Some(cancel);
        Ok(())
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
        self.sender.take();
        let (_, empty) = mpsc::channel();
        self.receiver = empty;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[derive(serde::Serialize)]
pub struct Summary {
    pub input: PathBuf,
    pub size: [u32; 2],
    pub factor: usize,
    pub resource: String,
    pub encoded_bytes: usize,
    pub stats: geotiff::DecodeStats,
}
#[derive(Default)]
pub struct State {
    device_ceiling: Option<usize>,
    service: Option<Service>,
    pending: Option<(uuid::Uuid, u64)>,
    pub title: String,
    pub error: Option<String>,
    pub last: Option<Summary>,
    pub starts: u64,
    pub completions: u64,
}
impl State {
    pub fn owns_input(&self) -> bool {
        self.pending.is_some() || self.error.is_some()
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn progress(&self) -> f64 {
        self.service
            .as_ref()
            .map_or(0., |s| f64::from_bits(s.progress.load(Ordering::Acquire)))
    }
}
fn memory_limit() -> Result<usize, String> {
    std::env::var("INGETRAZO_GEOTIFF_CPU_BYTES").ok().map_or(
        Ok(Limits::default().memory_bytes),
        |v| {
            v.parse::<usize>()
                .map_err(|_| "Invalid orthomosaic decoder byte budget".into())
        },
    )
}
fn qt_length(v: glam::Vec3) -> f32 {
    let v = v.abs();
    let scale = v.max_element();
    if scale == 0. {
        0.
    } else {
        let q = v / scale;
        scale * ((q.x * q.x + q.y * q.y) + q.z * q.z).sqrt()
    }
}
fn source_sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut hi, mut lo) = (0f64, 0f64);
    for x in values {
        let total = hi + x;
        lo += if hi.abs() >= x.abs() {
            (hi - total) + x
        } else {
            (x - total) + hi
        };
        hi = total;
    }
    hi + lo
}
impl IngeTrazoApp {
    pub fn begin_geotiff_import_dialog(&mut self) {
        if let Some(path) =
            crate::dialogs::pick_open_file(&tr("Import orthomosaic"), "*.tif *.tiff|GeoTIFF")
        {
            if let Err(error) = self.begin_geotiff_import(&path) {
                self.status_message = error.clone();
                self.geotiff_import.error = Some(error);
            }
        }
    }
    /// Cold renderer capability publication; no device query on import/repaint.
    pub fn set_geotiff_device_ceiling(&mut self, ceiling: Option<usize>) {
        self.geotiff_import.device_ceiling = ceiling.filter(|&value| value > 0);
    }
    pub fn begin_geotiff_import(&mut self, path: &Path) -> Result<(), String> {
        self.begin_geotiff_import_with_ceiling(path, self.geotiff_import.device_ceiling.unwrap_or(8192))
    }
    /// Device capability intake, clamped exactly as original max_texture_size.
    pub fn begin_geotiff_import_with_ceiling(
        &mut self,
        path: &Path,
        device_ceiling: usize,
    ) -> Result<(), String> {
        if self.geotiff_import.owns_input() {
            return Err("An orthomosaic dialog already owns input".into());
        }
        let budget = memory_limit()?;
        if budget == 0 {
            return Err("Orthomosaic decoder byte budget is zero".into());
        }
        let state = &mut self.geotiff_import;
        if state.service.is_none() {
            state.service = Some(Service::new()?);
        }
        state.service.as_mut().unwrap().begin(
            path.to_owned(),
            device_ceiling.clamp(1024, 8192),
            Limits {
                memory_bytes: budget,
            },
        )?;
        state.pending = Some((self.clipboard_document, self.model_revision));
        state.title = tr("Importing {name}…").replace(
            "{name}",
            &path.file_name().unwrap_or_default().to_string_lossy(),
        );
        state.starts += 1;
        Ok(())
    }
    /// Decoder cancellation is an API control, not a source GUI Cancel button.
    pub fn cancel_geotiff_work(&mut self) {
        if let Some(service) = &self.geotiff_import.service {
            if let Some(flag) = &service.cancel {
                flag.store(true, Ordering::Release);
            }
        }
    }
    pub fn poll_geotiff_import(&mut self) {
        let result = self
            .geotiff_import
            .service
            .as_ref()
            .and_then(|s| s.receiver.try_recv().ok());
        let Some(result) = result else {
            return;
        };
        let owner = self.geotiff_import.pending.take();
        self.geotiff_import.completions += 1;
        if owner != Some((self.clipboard_document, self.model_revision)) {
            self.status_message = tr("Orthomosaic import target changed");
            return;
        }
        let result = result.and_then(|prepared| self.publish_geotiff(prepared));
        if let Err(error) = result {
            self.status_message = error.clone();
            self.geotiff_import.error = Some(error);
        }
    }
    fn publish_geotiff(&mut self, prepared: Prepared) -> Result<(), String> {
        let corners = prepared.info.corners_geodetic()?;
        let fresh = self.scene.geo_datum.is_none();
        let datum = self.scene.geo_datum.clone().unwrap_or_else(|| {
            SceneDatum::new(
                source_sum(corners.iter().map(|c| c.0)) / 4.,
                source_sum(corners.iter().map(|c| c.1)) / 4.,
                0.,
                0.,
            )
        });
        let [bl, br, _, tl] =
            corners.map(|(lat, lon)| datum.geodetic_to_local(lat, lon, None).as_vec3());
        let u = br - bl;
        let v = tl - bl;
        if !bl.is_finite() || !u.is_finite() || !v.is_finite() {
            return Err("Orthomosaic local frame is not finite".into());
        }
        let width = qt_length(u);
        let height = qt_length(v);
        let aspect = if width > 1e-9 {
            f64::from(height) / f64::from(width)
        } else {
            1.
        };
        let name = prepared
            .info
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let digest = blake3::hash(&prepared.encoded).to_hex();
        let resource = format!("textures/{}-{name}.webp", &digest[..16]);
        let mut plane = ImagePlane::new(&resource, bl.as_dvec3(), u.as_dvec3(), v.as_dvec3());
        plane.aspect = aspect;
        plane.name = name;
        plane.locked = true;
        let mut camera = self.camera.clone();
        let mut capture = None;
        if fresh {
            let mut low = bl;
            let mut high = bl;
            for p in plane.corners() {
                low = low.min(p.as_vec3());
                high = high.max(p.as_vec3());
            }
            low.z = 0.;
            high.z = 0.;
            camera.set_standard_view(StandardView::Top);
            camera.target = ((low.as_dvec3() + high.as_dvec3()) * 0.5)
                .as_vec3()
                .as_dvec3();
            let diagonal = f64::from(qt_length(high - low)).max(0.02);
            camera.distance =
                (diagonal * 1.3 / (2. * (camera.fov_deg.to_radians() * 0.5).tan())).max(0.02);
            if !camera.target.is_finite() || !camera.distance.is_finite() {
                return Err("Orthomosaic camera extent is not finite".into());
            }
            if !(1..=21).contains(&self.base_map_requested_zoom) {
                return Err("Base-map zoom is outside the supported UI domain".into());
            }
            capture = Some(TileCapture::import_reference(
                self.scene.tile_layer.as_ref(),
                &datum,
                low.as_dvec3(),
                high.as_dvec3(),
                self.base_map_requested_zoom,
                self.base_map_requested_visible,
            ));
        }
        self.scene
            .embedded_textures
            .try_reserve(1)
            .map_err(|_| "Could not reserve orthomosaic asset")?;
        self.scene.image_planes.try_reserve(1)?;
        self.scene
            .layers
            .try_reserve(1)
            .map_err(|_| "Could not reserve image layer")?;
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Could not reserve image history")?;
        // Fresh datum is direct state outside AddImagePlane; failures above remain atomic.
        if fresh {
            self.scene.geo_datum = Some(datum.clone());
            for session in &mut self.component_edit_stack {
                session.original_scene.geo_datum = Some(datum.clone());
                session.cancel_scene.geo_datum = Some(datum.clone());
            }
        }
        let encoded_bytes = prepared.encoded.len();
        self.add_prepared_image_plane(plane, prepared.encoded);
        if fresh {
            self.scene.tile_layer = capture;
            self.camera = camera.clone();
            self.scene.camera = camera;
            for session in &mut self.component_edit_stack {
                session.note_survey_camera();
            }
        }
        self.bump_image_presentation_revision();
        self.status_message = tr("Imported {name}").replace(
            "{name}",
            &prepared
                .info
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
        );
        self.geotiff_import.last = Some(Summary {
            input: prepared.info.path,
            size: [prepared.width, prepared.height],
            factor: prepared.factor,
            resource,
            encoded_bytes,
            stats: prepared.stats,
        });
        Ok(())
    }
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if app.geotiff_import.pending() {
        let title = app.geotiff_import.title.clone();
        let fraction = app.geotiff_import.progress();
        egui::Modal::new(egui::Id::new("orthomosaic-progress")).show(ctx, |ui| {
            ui.heading(title);
            ui.label(tr(if fraction < 0.92 {
                "Decoding the orthomosaic…"
            } else {
                "Storing the picture…"
            }));
            ui.add(egui::ProgressBar::new(fraction as f32));
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(40));
    }
    if let Some(error) = app.geotiff_import.error.clone() {
        let response = egui::Modal::new(egui::Id::new("orthomosaic-error")).show(ctx, |ui| {
            ui.heading(tr("Import orthomosaic"));
            ui.label(error);
            let button = ui.button(tr("OK"));
            if std::env::var_os("INGETRAZO_UI_OBSERVE").is_some() {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("geotiff-error-ok"), button.rect));
            }
            button.clicked()
        });
        if response.inner || response.should_close() {
            app.geotiff_import.error = None;
        }
    }
}
