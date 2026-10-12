// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Headless Blender Cycles/EEVEE Rendering Bridge GUI (Track 5).
//!
//! Provides the UI dialog, tray panel, asynchronous background process manager,
//! and live progress reporting for photorealistic rendering with Blender.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use egui::{Color32, Vec2};
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_core::i18n::tr;
use ingetrazo_core::render_blender::{
    build_job, command, default_work_base, find_blender, kelvin_to_rgb, Ambience, BlenderEngine,
    BlenderFound, BlenderLight, LightKind, RenderQuality, BLENDER_RENDER_SCRIPT, MAX_KELVIN,
    MIN_KELVIN,
};
use ingetrazo_core::scene::Scene;
use ingetrazo_formats::gltf::export_glb_with_camera;

use crate::app::IngeTrazoApp;

mod document;
mod follow;
mod projection;
pub use follow::camera_key as follow_camera_key;
mod worker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPreset {
    Hd720p,
    Fhd1080p,
    Qhd1440p,
    Uhd4k,
    Custom,
}

impl ResolutionPreset {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ResolutionPreset::Hd720p => (1280, 720),
            ResolutionPreset::Fhd1080p => (1920, 1080),
            ResolutionPreset::Qhd1440p => (2560, 1440),
            ResolutionPreset::Uhd4k => (3840, 2160),
            ResolutionPreset::Custom => (1920, 1080),
        }
    }
}

pub enum BlenderRenderMsg {
    Progress(f32, String),
    Done(PathBuf),
    Error(String),
    Cancelled,
    Started(u32),
    Stopped { completed: bool },
}

pub struct BlenderRenderState {
    pub engine: BlenderEngine,
    pub quality: RenderQuality,
    pub ambience: Ambience,
    pub res_preset: ResolutionPreset,
    pub width: u32,
    pub height: u32,
    pub ground: bool,
    pub keep_blend: bool,
    pub sun_scale: f64,
    pub lights: Vec<BlenderLight>,
    pub blender_found: Option<BlenderFound>,
    pub is_rendering: bool,
    pub progress: f32,
    pub status_line: String,
    pub last_rendered_image_path: Option<PathBuf>,
    pub last_rendered_texture: Option<egui::TextureHandle>,
    pub rx: Option<Receiver<BlenderRenderMsg>>,
    pub process_id: Option<u32>,
    pub model_exports: u64,
    pub completed_renders: u64,
    worker: Option<worker::Worker>,
    reuse_key: Option<String>,
    work_dir: Option<PathBuf>,
    running_size: (u32, u32),
    running_follow: bool,
    draft_images: std::collections::VecDeque<PathBuf>,
    repaint: Option<egui::Context>,
    cached_document: Option<serde_json::Value>,
    cached_document_id: Option<uuid::Uuid>,
    pending_document_controls: bool,
    pub follow: follow::FollowState,
    projection: projection::ProjectionCache,
    pub selected_light: Option<usize>,
    pick: Option<document::Pick>,
    picked_light: Option<(usize, serde_json::Value)>,
}

impl Default for BlenderRenderState {
    fn default() -> Self {
        let found = find_blender(None);
        Self {
            engine: BlenderEngine::Cycles,
            quality: RenderQuality::Medium,
            ambience: Ambience::Day,
            res_preset: ResolutionPreset::Fhd1080p,
            width: 1920,
            height: 1080,
            ground: true,
            keep_blend: false,
            sun_scale: 1.0,
            lights: vec![],
            blender_found: found,
            is_rendering: false,
            progress: 0.0,
            status_line: tr("Ready to render"),
            last_rendered_image_path: None,
            last_rendered_texture: None,
            rx: None,
            process_id: None,
            model_exports: 0,
            completed_renders: 0,
            worker: None,
            reuse_key: None,
            work_dir: None,
            running_size: (0, 0),
            running_follow: false,
            draft_images: Default::default(),
            repaint: None,
            cached_document: None,
            cached_document_id: None,
            pending_document_controls: false,
            follow: Default::default(),
            projection: Default::default(),
            selected_light: None,
            pick: None,
            picked_light: None,
        }
    }
}

fn read_render_image(
    path: &std::path::Path,
    expected: (u32, u32),
) -> Result<image::DynamicImage, String> {
    let bytes = u64::from(expected.0)
        .checked_mul(u64::from(expected.1))
        .and_then(|n| n.checked_mul(4))
        .ok_or("Rendered image size overflow")?;
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > bytes.saturating_mul(2).saturating_add(1024 * 1024) {
        return Err("Rendered file exceeds its expected image budget".into());
    }
    let dimensions = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    if dimensions != expected {
        return Err("Blender returned the wrong image dimensions".into());
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(expected.0);
    limits.max_image_height = Some(expected.1);
    limits.max_alloc = Some(bytes.saturating_mul(2).saturating_add(1024 * 1024));
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())
}

impl BlenderRenderState {
    pub fn poll_events(&mut self, ctx: &egui::Context) {
        self.repaint = Some(ctx.clone());
        if let Some((progress, line)) = self.worker.as_ref().and_then(|worker| worker.progress()) {
            if self.is_rendering {
                self.progress = progress;
                self.status_line = line;
            }
        }
        let events: Vec<_> = self
            .rx
            .as_ref()
            .map(|rx| rx.try_iter().take(64).collect())
            .unwrap_or_default();
        for event in events {
            match event {
                BlenderRenderMsg::Started(pid) => self.process_id = Some(pid),
                BlenderRenderMsg::Stopped { .. } => {
                    if self.follow.enabled { self.follow.retry_after_failure(); }
                    self.process_id = None;
                    self.worker = None;
                    self.reuse_key = None;
                }
                BlenderRenderMsg::Progress(progress, line) => {
                    if self.is_rendering {
                        self.progress = progress;
                        self.status_line = line;
                    }
                }
                BlenderRenderMsg::Done(path) => {
                    self.is_rendering = false;
                    let decoded = read_render_image(&path, self.running_size);
                    match decoded {
                        Ok(img) if (img.width(), img.height()) == self.running_size => {
                            let rgba = img.to_rgba8();
                            let size = [rgba.width() as usize, rgba.height() as usize];
                            self.last_rendered_texture = Some(ctx.load_texture(
                                "blender_render_preview",
                                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                                egui::TextureOptions::LINEAR,
                            ));
                            if self.running_follow {
                                self.draft_images.push_back(path.clone());
                                while self.draft_images.len() > 3 {
                                    if let Some(old) = self.draft_images.pop_front() {
                                        let _ = fs::remove_file(old);
                                    }
                                }
                            }
                            self.last_rendered_image_path = Some(path);
                            self.completed_renders += 1;
                            self.progress = 1.;
                            self.status_line = tr("Render complete");
                        }
                        other => {
                            self.status_line = match other {
                                Err(e) => format!("Invalid rendered image: {e}"),
                                _ => "Blender returned the wrong image dimensions".into(),
                            };
                            self.worker = None;
                            self.reuse_key = None;
                        }
                    }
                }
                BlenderRenderMsg::Cancelled => {
                    self.is_rendering = false;
                    self.status_line = tr("Render cancelled");
                }
                BlenderRenderMsg::Error(error) => {
                    if self.follow.enabled { self.follow.retry_after_failure(); }
                    self.is_rendering = false;
                    self.status_line = format!("{}: {error}", tr("Error"));
                }
            }
            ctx.request_repaint();
        }
        if self.is_rendering {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    pub fn cancel_render(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel();
            self.status_line = tr("Stopping Blender…");
        }
    }

    pub fn save_image(&self, path: &std::path::Path) -> Result<(), String> {
        let source = self
            .last_rendered_image_path
            .as_ref()
            .ok_or("No rendered image to save")?;
        if source == path {
            return Ok(());
        }
        use std::io::{Seek, SeekFrom};
        let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
        let reader = image::ImageReader::new(std::io::BufReader::new(
            input.try_clone().map_err(|e| e.to_string())?,
        ));
        reader
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .into_dimensions()
            .map_err(|e| e.to_string())?;
        input.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let temporary = parent.join(format!(".ingetrazo-render-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
            drop(output);
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e: std::io::Error| e.to_string())
    }

    pub fn start_render(&mut self, scene: &Scene, camera: &OrbitCamera) {
        // Callers without the application's monotonic geometry identity cannot reuse a model safely.
        self.start_render_cached(scene, camera, None);
    }

    pub fn start_render_cached(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        geometry_revision: Option<u64>,
    ) {
        if self.follow.enabled {
            self.set_follow(false);
        }
        if self.is_rendering {
            return;
        }
        if let Err(error) = self.prepare_render(scene, camera, geometry_revision, None) {
            self.status_line = format!("{}: {error}", tr("Error"));
        }
    }

    fn prepare_render(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        geometry_revision: Option<u64>,
        draft: Option<(u32, u32, u32)>,
    ) -> Result<(), String> {
        ingetrazo_core::render_blender::validate_camera(camera)?;
        self.running_follow = draft.is_some();
        let (w, h) = if let Some((w, h, _)) = draft {
            (w, h)
        } else if self.res_preset == ResolutionPreset::Custom {
            (self.width, self.height)
        } else {
            self.res_preset.dimensions()
        };
        if !(2..=16384).contains(&w) || !(2..=16384).contains(&h) {
            return Err("Render dimensions must be between 2 and 16384 pixels".into());
        }
        let year = crate::composer_text_fields::sample_calendar()?.year();
        let key = geometry_revision.map(|revision| {
            serde_json::json!([
                revision,
                scene.version,
                scene.shadows,
                scene.geo_datum,
                scene.display_style,
                self.engine,
                self.ambience,
                self.ground,
                self.sun_scale,
                self.lights,
                year
            ])
            .to_string()
        });
        let keep_blend = self.keep_blend && draft.is_none();
        let samples = draft.map_or_else(
            || ingetrazo_core::render_blender::samples_for(self.engine, self.quality),
            |(_, _, s)| s,
        );
        let can_reuse = !keep_blend
            && key.is_some()
            && self.reuse_key == key
            && self.worker.is_some()
            && self.process_id.is_some();
        if can_reuse {
            let output = self
                .work_dir
                .as_ref()
                .unwrap()
                .join(format!("view-{}.png", uuid::Uuid::new_v4()));
            let request = serde_json::json!({"camera":ingetrazo_core::render_blender::camera_dict(camera),"output":output,"width":w,"height":h,"samples":samples});
            // Clear prior request's coalesced diagnostics before associating the next job.
            let _ = self.worker.as_ref().unwrap().progress();
            self.worker.as_ref().unwrap().request(request)?;
            self.running_size = (w, h);
            self.is_rendering = true;
            self.progress = 0.;
            self.status_line = tr("Rendering view…");
            return Ok(());
        }
        // Retire the previous owner; UUID work directories prevent stale-output success and collisions.
        self.worker = None;
        self.rx = None;
        self.process_id = None;
        self.reuse_key = None;
        let found = self
            .blender_found
            .clone()
            .or_else(|| find_blender(None))
            .ok_or("Blender not found on system")?;
        self.blender_found = Some(found.clone());
        let work_dir = default_work_base().join(format!("job_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
        let glb = work_dir.join("model.glb");
        let output = work_dir.join("render.png");
        let script = work_dir.join("render_scene.py");
        let job_path = work_dir.join("job.json");
        let blend = keep_blend.then(|| work_dir.join("render.blend"));
        let mut file = fs::File::create(&glb).map_err(|e| e.to_string())?;
        export_glb_with_camera(scene, camera, &mut file).map_err(|e| e.to_string())?;
        drop(file);
        self.model_exports += 1;
        fs::write(&script, BLENDER_RENDER_SCRIPT).map_err(|e| e.to_string())?;
        let mut job = build_job(
            scene,
            camera,
            &glb,
            &output,
            blend.as_deref(),
            self.engine,
            self.quality,
            w,
            h,
            self.ambience,
            self.sun_scale,
            self.ground,
            &self.lights,
            year,
        );
        job["samples"] = samples.into();
        let serving = !keep_blend;
        if serving {
            job["serve"] = true.into();
        }
        fs::write(&job_path, job.to_string()).map_err(|e| e.to_string())?;
        let request = serde_json::json!({"camera":job["camera"],"output":output,"width":w,"height":h,"samples":job["samples"]});
        let (worker, rx) = worker::Worker::launch(
            command(&found, &script, &job_path),
            output,
            serving,
            request,
            self.repaint.clone(),
        );
        self.worker = Some(worker);
        self.rx = Some(rx);
        self.work_dir = Some(work_dir);
        self.reuse_key = key;
        self.running_size = (w, h);
        self.is_rendering = true;
        self.progress = 0.;
        self.status_line = tr("Starting Blender…");
        Ok(())
    }
}

/// Renders the full modal dialog for "Render with Blender".
pub fn render_blender_dialog(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if !app.show_render_dialog {
        return;
    }

    let mut open = app.show_render_dialog;
    egui::Window::new(tr("Render with Blender (Cycles & EEVEE)"))
        .open(&mut open)
        .default_width(720.0)
        .default_height(580.0)
        .resizable(true)
        .show(ctx, |ui| {
            render_blender_ui_contents(app, ui);
        });

    app.show_render_dialog = open;
}

/// Contents of the Render Controls, shared between dialog and side tray.
pub fn render_blender_ui_contents(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    app.sync_blender_document();
    let before_document = app.blender_document_controls();
    let mut begin_pick = None;
    let mut editing_document_text = false;
    // Blender discovery banner
    let is_blender_available = app.blender_state.blender_found.is_some();
    if let Some(ref found) = app.blender_state.blender_found {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("✓")
                    .color(Color32::from_rgb(50, 190, 70))
                    .strong(),
            );
            ui.label(format!("{}: {}", tr("Blender Engine"), found.where_desc));
        });
    } else {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("⚠").color(Color32::from_rgb(230, 160, 30)).strong());
            ui.label(tr("Blender not detected on host system. Install Blender via package manager or Flatpak."));
        });
    }

    ui.separator();

    ui.columns(2, |cols| {
        // Left column: Engine, Quality, Ambience, Resolution, Ground
        cols[0].vertical(|ui| {
            ui.strong(tr("Rendering Engine:"));
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut app.blender_state.engine,
                    BlenderEngine::Cycles,
                    tr("Cycles (Photorealistic Path Tracer)"),
                );
            });
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut app.blender_state.engine,
                    BlenderEngine::Eevee,
                    tr("EEVEE (Real-time Rasterizer)"),
                );
            });

            ui.add_space(6.0);
            ui.strong(tr("Quality & Samples:"));
            egui::ComboBox::from_id_salt("blender_quality_combo")
                .selected_text(match app.blender_state.quality {
                    RenderQuality::Draft => tr("Draft (Fast Preview)"),
                    RenderQuality::Medium => tr("Medium (Balanced)"),
                    RenderQuality::High => tr("High (Production Quality)"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.blender_state.quality,
                        RenderQuality::Draft,
                        tr("Draft (Fast Preview)"),
                    );
                    ui.selectable_value(
                        &mut app.blender_state.quality,
                        RenderQuality::Medium,
                        tr("Medium (Balanced)"),
                    );
                    ui.selectable_value(
                        &mut app.blender_state.quality,
                        RenderQuality::High,
                        tr("High (Production Quality)"),
                    );
                });

            ui.add_space(6.0);
            ui.strong(tr("Ambience & Sky:"));
            egui::ComboBox::from_id_salt("blender_ambience_combo")
                .selected_text(match app.blender_state.ambience {
                    Ambience::Day => tr("Day (Physical Nishita Sky & Sun)"),
                    Ambience::Night => tr("Night (Dark Sky & Artificial Lamps)"),
                    Ambience::Overcast => tr("Overcast (Diffuse Neutral Sky)"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.blender_state.ambience,
                        Ambience::Day,
                        tr("Day (Physical Nishita Sky & Sun)"),
                    );
                    ui.selectable_value(
                        &mut app.blender_state.ambience,
                        Ambience::Night,
                        tr("Night (Dark Sky & Artificial Lamps)"),
                    );
                    ui.selectable_value(
                        &mut app.blender_state.ambience,
                        Ambience::Overcast,
                        tr("Overcast (Diffuse Neutral Sky)"),
                    );
                });

            if app.blender_state.ambience == Ambience::Day {
                ui.horizontal(|ui| {
                    ui.label(tr("Sun Strength:"));
                    ui.add(
                        egui::Slider::new(&mut app.blender_state.sun_scale, 0.1..=2.0).text("x"),
                    );
                });
            }

            ui.add_space(6.0);
            ui.strong(tr("Output Resolution:"));
            egui::ComboBox::from_id_salt("blender_res_combo")
                .selected_text(match app.blender_state.res_preset {
                    ResolutionPreset::Hd720p => "1280 × 720 (720p)",
                    ResolutionPreset::Fhd1080p => "1920 × 1080 (1080p FHD)",
                    ResolutionPreset::Qhd1440p => "2560 × 1440 (1440p 2K)",
                    ResolutionPreset::Uhd4k => "3840 × 2160 (2160p 4K)",
                    ResolutionPreset::Custom => "Custom",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.blender_state.res_preset,
                        ResolutionPreset::Hd720p,
                        "1280 × 720 (720p)",
                    );
                    ui.selectable_value(
                        &mut app.blender_state.res_preset,
                        ResolutionPreset::Fhd1080p,
                        "1920 × 1080 (1080p FHD)",
                    );
                    ui.selectable_value(
                        &mut app.blender_state.res_preset,
                        ResolutionPreset::Qhd1440p,
                        "2560 × 1440 (1440p 2K)",
                    );
                    ui.selectable_value(
                        &mut app.blender_state.res_preset,
                        ResolutionPreset::Uhd4k,
                        "3840 × 2160 (2160p 4K)",
                    );
                    ui.selectable_value(
                        &mut app.blender_state.res_preset,
                        ResolutionPreset::Custom,
                        "Custom",
                    );
                });

            if app.blender_state.res_preset == ResolutionPreset::Custom {
                ui.horizontal(|ui| {
                    ui.label(tr("Width"));
                    ui.add(egui::DragValue::new(&mut app.blender_state.width).range(2..=16384));
                    ui.label(tr("Height"));
                    ui.add(egui::DragValue::new(&mut app.blender_state.height).range(2..=16384));
                });
            }
            ui.add_space(6.0);
            ui.checkbox(
                &mut app.blender_state.ground,
                tr("Catch shadows on ground plane"),
            );
            ui.checkbox(
                &mut app.blender_state.keep_blend,
                tr("Save .blend scene file"),
            );
        });

        // Right column: Lights List, Preview Image
        cols[1].vertical(|ui| {
            ui.strong(tr("Artificial Lights:"));
            let mut remove_light_idx = None;

            egui::ScrollArea::vertical()
                .max_height(140.0)
                .show(ui, |ui| {
                    for (idx, lt) in app.blender_state.lights.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut lt.on, "");
                                let response=ui.text_edit_singleline(&mut lt.name);
                                editing_document_text |= response.has_focus() && !ui.input(|i|i.key_pressed(egui::Key::Enter));
                                if ui.small_button(tr("Move")).clicked() {
                                    begin_pick = Some(document::Pick::Move(idx));
                                }
                                if lt.kind == LightKind::Spot
                                    && ui.small_button(tr("Aim")).clicked()
                                {
                                    begin_pick = Some(document::Pick::Aim(idx));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("✕").clicked() {
                                            remove_light_idx = Some(idx);
                                        }
                                    },
                                );
                            });

                            ui.horizontal(|ui| {
                                ui.radio_value(&mut lt.kind, LightKind::Point, tr("Point"));
                                ui.radio_value(&mut lt.kind, LightKind::Spot, tr("Spot"));
                                ui.label(tr("Power:"));
                                let response=ui.add(
                                    egui::DragValue::new(&mut lt.power)
                                        .range(1.0..=100000.0)
                                        .suffix(" W"),
                                );
                                editing_document_text |= response.has_focus() && !response.dragged() && !response.drag_stopped() && !ui.input(|i|i.key_pressed(egui::Key::Enter));
                            });

                            ui.horizontal(|ui| {
                                ui.label(tr("Temp:"));
                                let mut kelvin_val = lt
                                    .kelvin
                                    .unwrap_or(ingetrazo_core::render_blender::DEFAULT_KELVIN)
                                    as f64;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut kelvin_val, MIN_KELVIN..=MAX_KELVIN)
                                            .text("K"),
                                    )
                                    .changed()
                                {
                                    lt.kelvin = Some(kelvin_val as u32);
                                    lt.color = kelvin_to_rgb(kelvin_val);
                                }
                                // Color swatch
                                let c = Color32::from_rgb(
                                    (lt.color[0] * 255.0) as u8,
                                    (lt.color[1] * 255.0) as u8,
                                    (lt.color[2] * 255.0) as u8,
                                );
                                ui.painter()
                                    .rect_filled(ui.available_rect_before_wrap(), 2.0, c);
                            });
                        });
                    }
                });

            if let Some(del) = remove_light_idx {
                if app.blender_state.lights.len() > del {
                    app.blender_state.lights.remove(del);
                }
            }

            ui.horizontal(|ui| {
                if ui.button(tr("+ Point light")).clicked() {
                    begin_pick = Some(document::Pick::New(LightKind::Point));
                }
                if ui.button(tr("+ Spot light")).clicked() {
                    begin_pick = Some(document::Pick::New(LightKind::Spot));
                }
            });

            ui.separator();

            // Render Preview thumbnail
            if let Some(ref tex) = app.blender_state.last_rendered_texture {
                ui.strong(tr("Last Rendered Image:"));
                let max_preview_size = Vec2::new(300.0, 170.0);
                ui.image((tex.id(), max_preview_size));

                if ui.button(tr("Save Image As…")).clicked() {
                    if let Some(path) = crate::dialogs::pick_save_file(
                        &tr("Save rendered image"),
                        "render.png",
                        "*.png|PNG image",
                    ) {
                        if let Err(error) = app.blender_state.save_image(&path) {
                            app.blender_state.status_line = error;
                        }
                    }
                }
            }
        });
    });

    if app.blender_document_controls() != before_document {
        app.blender_state.pending_document_controls = true;
    }
    if app.blender_state.pending_document_controls && !editing_document_text && !ui.input(|i|i.pointer.any_down()) {
        app.commit_blender_document_controls();
    }
    if let Some(pick) = begin_pick {
        app.begin_blender_pick(pick);
    }
    let mut follow = app.blender_state.follow.enabled;
    if ui.checkbox(&mut follow, tr("Sync with the view")).changed() {
        app.blender_state.set_follow(follow);
    }

    ui.add_space(10.0);
    ui.separator();

    // Bottom execution strip
    ui.horizontal(|ui| {
        let is_rendering = app.blender_state.is_rendering;

        let render_btn = egui::Button::new(
            egui::RichText::new(if is_rendering {
                tr("Rendering in progress…")
            } else {
                tr("Start Render")
            })
            .strong(),
        );

        if ui
            .add_enabled(is_blender_available && !is_rendering, render_btn)
            .clicked()
        {
            app.start_blender_render();
        }

        if is_rendering {
            if ui.button(tr("Cancel render")).clicked() {
                app.blender_state.cancel_render();
            }
            ui.add(egui::ProgressBar::new(app.blender_state.progress).show_percentage());
        }

        ui.label(&app.blender_state.status_line);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blender_state_initialization() {
        let state = BlenderRenderState::default();
        assert_eq!(state.engine, BlenderEngine::Cycles);
        assert_eq!(state.quality, RenderQuality::Medium);
        assert_eq!(state.ambience, Ambience::Day);
        assert_eq!(state.width, 1920);
        assert_eq!(state.height, 1080);
        assert!(state.lights.is_empty());
    }

    #[test]
    fn test_resolution_presets() {
        assert_eq!(ResolutionPreset::Hd720p.dimensions(), (1280, 720));
        assert_eq!(ResolutionPreset::Fhd1080p.dimensions(), (1920, 1080));
        assert_eq!(ResolutionPreset::Qhd1440p.dimensions(), (2560, 1440));
        assert_eq!(ResolutionPreset::Uhd4k.dimensions(), (3840, 2160));
    }
}

#[cfg(test)]
mod output_tests {
    use super::*;
    #[test]
    fn blender_image_admission_and_save_failure_preserve_previous_output() {
        let dir = std::env::temp_dir().join(format!(
            "ingetrazo-blender-control-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("render.png");
        image::RgbaImage::new(3, 2).save(&source).unwrap();
        assert!(read_render_image(&source, (2, 2)).is_err());
        assert!(read_render_image(&source, (3, 2)).is_ok());
        let dest = dir.join("existing.png");
        fs::write(&dest, b"original destination").unwrap();
        let mut state = BlenderRenderState::default();
        state.last_rendered_image_path = Some(source.clone());
        fs::write(&source, b"invalid png").unwrap();
        assert!(state.save_image(&dest).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"original destination");
        image::RgbaImage::new(3, 2).save(&source).unwrap();
        let directory = dir.join("destination-directory");
        fs::create_dir(&directory).unwrap();
        assert!(state.save_image(&directory).is_err());
        assert!(directory.is_dir());
        assert!(!fs::read_dir(&dir).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        state.save_image(&dest).unwrap();
        assert_eq!(fs::read(&source).unwrap(), fs::read(&dest).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
}
