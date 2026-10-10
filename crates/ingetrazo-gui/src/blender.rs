// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Headless Blender Cycles/EEVEE Rendering Bridge GUI (Track 5).
//!
//! Provides the UI dialog, tray panel, asynchronous background process manager,
//! and live progress reporting for photorealistic rendering with Blender.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::SystemTime;

use egui::{Color32, Vec2};
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_core::i18n::tr;
use ingetrazo_core::render_blender::{
    build_job, clean_env, command, default_work_base, find_blender, kelvin_to_rgb, progress_of,
    Ambience, BlenderEngine, BlenderFound, BlenderLight, LightKind, RenderQuality,
    BLENDER_RENDER_SCRIPT, MAX_KELVIN, MIN_KELVIN,
};
use ingetrazo_core::scene::Scene;
use ingetrazo_formats::gltf::export_glb;

use crate::app::IngeTrazoApp;

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
            lights: vec![BlenderLight::default()],
            blender_found: found,
            is_rendering: false,
            progress: 0.0,
            status_line: tr("Ready to render"),
            last_rendered_image_path: None,
            last_rendered_texture: None,
            rx: None,
        }
    }
}

impl BlenderRenderState {
    pub fn poll_events(&mut self, ctx: &egui::Context) {
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    BlenderRenderMsg::Progress(p, line) => {
                        self.progress = p;
                        self.status_line = line;
                        ctx.request_repaint();
                    }
                    BlenderRenderMsg::Done(path) => {
                        self.progress = 1.0;
                        self.is_rendering = false;
                        self.status_line = tr("Render complete");
                        self.last_rendered_image_path = Some(path.clone());
                        // Load image to texture
                        if let Ok(img_data) = fs::read(&path) {
                            if let Ok(img) = image::load_from_memory(&img_data) {
                                let rgba = img.to_rgba8();
                                let size = [rgba.width() as usize, rgba.height() as usize];
                                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                    size,
                                    rgba.as_flat_samples().as_slice(),
                                );
                                self.last_rendered_texture = Some(ctx.load_texture(
                                    "blender_render_preview",
                                    color_image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                        }
                        ctx.request_repaint();
                    }
                    BlenderRenderMsg::Error(err) => {
                        self.is_rendering = false;
                        self.status_line = format!("{}: {}", tr("Error"), err);
                        ctx.request_repaint();
                    }
                }
            }
        }
    }

    pub fn start_render(&mut self, scene: &Scene, camera: &OrbitCamera) {
        if self.is_rendering {
            return;
        }

        let found = match &self.blender_found {
            Some(f) => f.clone(),
            None => {
                if let Some(f) = find_blender(None) {
                    self.blender_found = Some(f.clone());
                    f
                } else {
                    self.status_line = tr("Blender not found on system");
                    return;
                }
            }
        };

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let work_dir = default_work_base().join(format!("job_{now}"));
        if let Err(e) = fs::create_dir_all(&work_dir) {
            self.status_line = format!("Failed to create work directory: {e}");
            return;
        }

        let glb_path = work_dir.join("model.glb");
        let out_png = work_dir.join("render.png");
        let blend_path = if self.keep_blend {
            Some(work_dir.join("render.blend"))
        } else {
            None
        };
        let script_path = work_dir.join("render_scene.py");
        let job_path = work_dir.join("job.json");

        // 1. Export GLB
        match fs::File::create(&glb_path) {
            Ok(mut file) => {
                if let Err(e) = export_glb(scene, &mut file) {
                    self.status_line = format!("Failed to export GLB: {e}");
                    return;
                }
            }
            Err(e) => {
                self.status_line = format!("Failed to open GLB file: {e}");
                return;
            }
        }

        // 2. Write embedded Blender python script
        if let Err(e) = fs::write(&script_path, BLENDER_RENDER_SCRIPT) {
            self.status_line = format!("Failed to write Blender script: {e}");
            return;
        }

        // 3. Build and write job.json
        let (w, h) = if self.res_preset == ResolutionPreset::Custom {
            (self.width, self.height)
        } else {
            self.res_preset.dimensions()
        };

        let job_json = build_job(
            scene,
            camera,
            &glb_path,
            &out_png,
            blend_path.as_deref(),
            self.engine,
            self.quality,
            w,
            h,
            self.ambience,
            self.sun_scale,
            self.ground,
            &self.lights,
        );

        if let Err(e) = fs::write(&job_path, job_json.to_string()) {
            self.status_line = format!("Failed to write job.json: {e}");
            return;
        }

        // 4. Spawn background rendering thread
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.is_rendering = true;
        self.progress = 0.05;
        self.status_line = tr("Starting Blender...");

        let cmd_args = command(&found, &script_path, &job_path);
        let env_vars = clean_env();

        thread::spawn(move || {
            let mut cmd = std::process::Command::new(&cmd_args[0]);
            for arg in &cmd_args[1..] {
                cmd.arg(arg);
            }
            cmd.envs(env_vars);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(BlenderRenderMsg::Error(e.to_string()));
                    return;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line_res in reader.lines() {
                    if let Ok(line) = line_res {
                        if let Some(p) = progress_of(&line) {
                            let _ = tx.send(BlenderRenderMsg::Progress(p, line.clone()));
                        } else if line.starts_with("INGETRAZO rendering") {
                            let _ = tx.send(BlenderRenderMsg::Progress(0.1, tr("Raytracing scene...")));
                        } else if line.starts_with("INGETRAZO imported") {
                            let _ = tx.send(BlenderRenderMsg::Progress(0.08, tr("Loaded GLB mesh geometry")));
                        }
                    }
                }
            }

            match child.wait() {
                Ok(status) => {
                    if status.success() && out_png.is_file() {
                        let _ = tx.send(BlenderRenderMsg::Done(out_png));
                    } else {
                        let _ = tx.send(BlenderRenderMsg::Error(format!(
                            "Blender exited with status: {}",
                            status
                        )));
                    }
                }
                Err(e) => {
                    let _ = tx.send(BlenderRenderMsg::Error(e.to_string()));
                }
            }
        });
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
    // Blender discovery banner
    let is_blender_available = app.blender_state.blender_found.is_some();
    if let Some(ref found) = app.blender_state.blender_found {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("✓").color(Color32::from_rgb(50, 190, 70)).strong());
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
                ui.radio_value(&mut app.blender_state.engine, BlenderEngine::Cycles, tr("Cycles (Photorealistic Path Tracer)"));
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut app.blender_state.engine, BlenderEngine::Eevee, tr("EEVEE (Real-time Rasterizer)"));
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
                    ui.selectable_value(&mut app.blender_state.quality, RenderQuality::Draft, tr("Draft (Fast Preview)"));
                    ui.selectable_value(&mut app.blender_state.quality, RenderQuality::Medium, tr("Medium (Balanced)"));
                    ui.selectable_value(&mut app.blender_state.quality, RenderQuality::High, tr("High (Production Quality)"));
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
                    ui.selectable_value(&mut app.blender_state.ambience, Ambience::Day, tr("Day (Physical Nishita Sky & Sun)"));
                    ui.selectable_value(&mut app.blender_state.ambience, Ambience::Night, tr("Night (Dark Sky & Artificial Lamps)"));
                    ui.selectable_value(&mut app.blender_state.ambience, Ambience::Overcast, tr("Overcast (Diffuse Neutral Sky)"));
                });

            if app.blender_state.ambience == Ambience::Day {
                ui.horizontal(|ui| {
                    ui.label(tr("Sun Strength:"));
                    ui.add(egui::Slider::new(&mut app.blender_state.sun_scale, 0.1..=3.0).text("x"));
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
                    ui.selectable_value(&mut app.blender_state.res_preset, ResolutionPreset::Hd720p, "1280 × 720 (720p)");
                    ui.selectable_value(&mut app.blender_state.res_preset, ResolutionPreset::Fhd1080p, "1920 × 1080 (1080p FHD)");
                    ui.selectable_value(&mut app.blender_state.res_preset, ResolutionPreset::Qhd1440p, "2560 × 1440 (1440p 2K)");
                    ui.selectable_value(&mut app.blender_state.res_preset, ResolutionPreset::Uhd4k, "3840 × 2160 (2160p 4K)");
                    ui.selectable_value(&mut app.blender_state.res_preset, ResolutionPreset::Custom, "Custom");
                });

            ui.add_space(6.0);
            ui.checkbox(&mut app.blender_state.ground, tr("Catch shadows on ground plane"));
            ui.checkbox(&mut app.blender_state.keep_blend, tr("Save .blend scene file"));
        });

        // Right column: Lights List, Preview Image
        cols[1].vertical(|ui| {
            ui.strong(tr("Artificial Lights:"));
            let mut remove_light_idx = None;

            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                for (idx, lt) in app.blender_state.lights.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut lt.on, &lt.name);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕").clicked() {
                                    remove_light_idx = Some(idx);
                                }
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.radio_value(&mut lt.kind, LightKind::Point, tr("Point"));
                            ui.radio_value(&mut lt.kind, LightKind::Spot, tr("Spot"));
                            ui.label(tr("Power:"));
                            ui.add(egui::DragValue::new(&mut lt.power).range(1.0..=10000.0).suffix(" W"));
                        });

                        ui.horizontal(|ui| {
                            ui.label(tr("Temp:"));
                            let mut kelvin_val = lt.kelvin as f64;
                            if ui.add(egui::Slider::new(&mut kelvin_val, MIN_KELVIN..=MAX_KELVIN).text("K")).changed() {
                                lt.kelvin = kelvin_val as u32;
                                lt.color = kelvin_to_rgb(kelvin_val);
                            }
                            // Color swatch
                            let c = Color32::from_rgb(
                                (lt.color[0] * 255.0) as u8,
                                (lt.color[1] * 255.0) as u8,
                                (lt.color[2] * 255.0) as u8,
                            );
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 2.0, c);
                        });
                    });
                }
            });

            if let Some(del) = remove_light_idx {
                if app.blender_state.lights.len() > del {
                    app.blender_state.lights.remove(del);
                }
            }

            if ui.button(tr("+ Add Light")).clicked() {
                let n = app.blender_state.lights.len() + 1;
                let mut new_lt = BlenderLight::default();
                new_lt.name = format!("Light {n}");
                app.blender_state.lights.push(new_lt);
            }

            ui.separator();

            // Render Preview thumbnail
            if let Some(ref tex) = app.blender_state.last_rendered_texture {
                ui.strong(tr("Last Rendered Image:"));
                let max_preview_size = Vec2::new(300.0, 170.0);
                ui.image((tex.id(), max_preview_size));

                if ui.button(tr("Save Image As…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "render_export.png".to_string();
                }
            }
        });
    });

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

        if ui.add_enabled(is_blender_available && !is_rendering, render_btn).clicked() {
            let camera = app.camera.clone();
            let scene = app.scene.clone();
            app.blender_state.start_render(&scene, &camera);
        }

        if is_rendering {
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
        assert_eq!(state.lights.len(), 1);
        assert_eq!(state.lights[0].kind, LightKind::Point);
    }

    #[test]
    fn test_resolution_presets() {
        assert_eq!(ResolutionPreset::Hd720p.dimensions(), (1280, 720));
        assert_eq!(ResolutionPreset::Fhd1080p.dimensions(), (1920, 1080));
        assert_eq!(ResolutionPreset::Qhd1440p.dimensions(), (2560, 1440));
        assert_eq!(ResolutionPreset::Uhd4k.dimensions(), (3840, 2160));
    }
}
