// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Headless Blender Cycles/EEVEE Rendering Bridge (Track 5).
//!
//! Provides background photorealistic rendering through external Blender (`blender -b`).
//! Handles Blender binary auto-discovery, GLB scene export, lighting, physical sky,
//! camera matching, environment cleanup, and live progress reporting.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use glam::DVec3;
use crate::camera::OrbitCamera;
use crate::scene::Scene;

pub const MIN_KELVIN: f64 = 1800.0;
pub const MAX_KELVIN: f64 = 10000.0;
pub const DEFAULT_KELVIN: u32 = 2700;

pub const BLENDER_FLATPAK_ID: &str = "org.blender.Blender";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlenderEngine {
    Eevee,
    Cycles,
}

impl Default for BlenderEngine {
    fn default() -> Self {
        BlenderEngine::Cycles
    }
}

impl BlenderEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlenderEngine::Eevee => "eevee",
            BlenderEngine::Cycles => "cycles",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderQuality {
    Draft,
    Medium,
    High,
}

impl Default for RenderQuality {
    fn default() -> Self {
        RenderQuality::Medium
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ambience {
    Day,
    Night,
    Overcast,
}

impl Default for Ambience {
    fn default() -> Self {
        Ambience::Day
    }
}

impl Ambience {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ambience::Day => "day",
            Ambience::Night => "night",
            Ambience::Overcast => "overcast",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightKind {
    Point,
    Spot,
}

impl Default for LightKind {
    fn default() -> Self {
        LightKind::Point
    }
}

impl LightKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LightKind::Point => "point",
            LightKind::Spot => "spot",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlenderLight {
    pub kind: LightKind,
    pub pos: [f64; 3],
    pub dir: [f64; 3],
    pub kelvin: u32,
    pub color: [f64; 3],
    pub power: f64, // Watts
    pub angle: f64, // Beam angle in degrees
    pub on: bool,
    pub name: String,
}

impl Default for BlenderLight {
    fn default() -> Self {
        Self {
            kind: LightKind::Point,
            pos: [0.0, 0.0, 3.0],
            dir: [0.0, 0.0, -1.0],
            kelvin: DEFAULT_KELVIN,
            color: kelvin_to_rgb(DEFAULT_KELVIN as f64),
            power: 400.0,
            angle: 60.0,
            on: true,
            name: "Light".to_string(),
        }
    }
}

/// Black-body radiation color fit (Tanner Helland's formula).
pub fn kelvin_to_rgb(kelvin: f64) -> [f64; 3] {
    let t = (kelvin.clamp(1000.0, 40000.0)) / 100.0;
    let r;
    let g;
    let b;

    if t <= 66.0 {
        r = 255.0;
        g = 99.4708025861 * t.ln() - 161.1195681661;
        b = if t <= 19.0 {
            0.0
        } else {
            138.5177312231 * (t - 10.0).ln() - 305.0447927307
        };
    } else {
        r = 329.698727446 * (t - 60.0).powf(-0.1332047592);
        g = 288.1221695283 * (t - 60.0).powf(-0.0755148492);
        b = 255.0;
    }

    let r_norm = (r.clamp(0.0, 255.0) / 255.0).max(0.0);
    let g_norm = (g.clamp(0.0, 255.0) / 255.0).max(0.0);
    let b_norm = (b.clamp(0.0, 255.0) / 255.0).max(0.0);

    let max_val = r_norm.max(g_norm).max(b_norm).max(1e-4);
    [
        (r_norm / max_val * 10000.0).round() / 10000.0,
        (g_norm / max_val * 10000.0).round() / 10000.0,
        (b_norm / max_val * 10000.0).round() / 10000.0,
    ]
}

pub fn samples_for(engine: BlenderEngine, quality: RenderQuality) -> u32 {
    match (engine, quality) {
        (BlenderEngine::Eevee, RenderQuality::Draft) => 16,
        (BlenderEngine::Eevee, RenderQuality::Medium) => 64,
        (BlenderEngine::Eevee, RenderQuality::High) => 128,
        (BlenderEngine::Cycles, RenderQuality::Draft) => 32,
        (BlenderEngine::Cycles, RenderQuality::Medium) => 128,
        (BlenderEngine::Cycles, RenderQuality::High) => 512,
    }
}

pub fn exposure_for(ambience: Ambience) -> f64 {
    match ambience {
        Ambience::Day => -0.6,
        Ambience::Night => 0.0,
        Ambience::Overcast => -0.3,
    }
}

#[derive(Debug, Clone)]
pub struct BlenderFound {
    pub command: Vec<String>,
    pub where_desc: String,
}

/// Locates an executable in the system PATH.
fn which_bin(bin: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Locates Blender executable on host or flatpak sandbox.
pub fn find_blender(saved: Option<&str>) -> Option<BlenderFound> {
    if let Some(custom) = saved {
        let p = Path::new(custom);
        if p.is_file() {
            return Some(BlenderFound {
                command: vec![custom.to_string()],
                where_desc: custom.to_string(),
            });
        }
    }

    // Check system PATH
    if let Some(path) = which_bin("blender") {
        let p_str = path.to_string_lossy().to_string();
        return Some(BlenderFound {
            command: vec![p_str.clone()],
            where_desc: p_str,
        });
    }

    // Standard UNIX / Linux candidate paths
    let linux_candidates = [
        "/usr/bin/blender",
        "/usr/local/bin/blender",
        "/snap/bin/blender",
        "/opt/blender/blender",
    ];
    for &cand in &linux_candidates {
        if Path::new(cand).is_file() {
            return Some(BlenderFound {
                command: vec![cand.to_string()],
                where_desc: cand.to_string(),
            });
        }
    }

    // macOS candidate paths
    #[cfg(target_os = "macos")]
    {
        let mac_candidates = [
            "/Applications/Blender.app/Contents/MacOS/Blender",
            "/Applications/Blender 4.2.app/Contents/MacOS/Blender",
            "/Applications/Blender 4.0.app/Contents/MacOS/Blender",
        ];
        for &cand in &mac_candidates {
            if Path::new(cand).is_file() {
                return Some(BlenderFound {
                    command: vec![cand.to_string()],
                    where_desc: cand.to_string(),
                });
            }
        }
    }

    // Windows candidate paths
    #[cfg(target_os = "windows")]
    {
        let win_candidates = [
            r"C:\Program Files\Blender Foundation\Blender 4.2\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender 4.1\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender 4.0\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender\blender.exe",
        ];
        for &cand in &win_candidates {
            if Path::new(cand).is_file() {
                return Some(BlenderFound {
                    command: vec![cand.to_string()],
                    where_desc: cand.to_string(),
                });
            }
        }
    }

    // Flatpak host check
    if let Some(flatpak_bin) = which_bin("flatpak") {
        let output = std::process::Command::new(&flatpak_bin)
            .args(["info", BLENDER_FLATPAK_ID])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                return Some(BlenderFound {
                    command: vec![
                        flatpak_bin.to_string_lossy().to_string(),
                        "run".to_string(),
                        "--filesystem=home".to_string(),
                        BLENDER_FLATPAK_ID.to_string(),
                    ],
                    where_desc: format!("Flatpak {BLENDER_FLATPAK_ID}"),
                });
            }
        }
    }

    None
}

/// Strips conflicting environment variables (PyInstaller, Qt, etc.)
pub fn clean_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    env.retain(|(k, _)| {
        !matches!(
            k.as_str(),
            "PYTHONHOME"
                | "PYTHONPATH"
                | "QT_PLUGIN_PATH"
                | "QT_QPA_PLATFORM_PLUGIN_PATH"
                | "QML2_IMPORT_PATH"
                | "QT_QPA_PLATFORM"
        )
    });
    // Restore or remove LD_LIBRARY_PATH if overridden
    if let Ok(orig) = std::env::var("LD_LIBRARY_PATH_ORIG") {
        if let Some(pos) = env.iter().position(|(k, _)| k == "LD_LIBRARY_PATH") {
            env[pos] = ("LD_LIBRARY_PATH".to_string(), orig);
        } else {
            env.push(("LD_LIBRARY_PATH".to_string(), orig));
        }
    }
    env
}

/// Parses progress 0.0..1.0 from Blender stdout line.
pub fn progress_of(line: &str) -> Option<f32> {
    // Cycles: "Sample 16/64" or "Sample 16 / 64"
    if let Some(idx) = line.find("Sample ") {
        let rest = &line[idx + 7..];
        if let Some(slash) = rest.find('/') {
            let done_str = rest[..slash].trim();
            let after = rest[slash + 1..].trim();
            let total_str = after.split_whitespace().next().unwrap_or("");
            if let (Ok(done), Ok(total)) = (done_str.parse::<f32>(), total_str.parse::<f32>()) {
                if total > 0.0 {
                    return Some((done / total).clamp(0.0, 1.0));
                }
            }
        }
    }

    // EEVEE: "Rendering 32 / 64 samples"
    if let Some(idx) = line.find("Rendering ") {
        let rest = &line[idx + 10..];
        if let Some(slash) = rest.find('/') {
            let done_str = rest[..slash].trim();
            let after = rest[slash + 1..].trim();
            let total_str = after.split_whitespace().next().unwrap_or("");
            if let (Ok(done), Ok(total)) = (done_str.parse::<f32>(), total_str.parse::<f32>()) {
                if total > 0.0 {
                    return Some((done / total).clamp(0.0, 1.0));
                }
            }
        }
    }

    None
}

/// Builds camera dictionary for the Blender script.
pub fn camera_dict(camera: &OrbitCamera) -> serde_json::Value {
    let eye = camera.eye();
    let target = camera.target;
    let up = camera.up;
    let half_h = camera.distance * (camera.fov_deg.to_radians() / 2.0).tan();

    serde_json::json!({
        "eye": [eye.x, eye.y, eye.z],
        "target": [target.x, target.y, target.z],
        "up": [up.x, up.y, up.z],
        "perspective": camera.perspective,
        "fov_deg": camera.fov_deg,
        "half_height": half_h,
        "near": (camera.distance * 0.02).max(1e-4),
        "far": 100000.0,
    })
}

/// Builds ground catcher dictionary for the Blender script.
pub fn ground_dict(scene: &Scene) -> Option<serde_json::Value> {
    let mut bb = crate::math::BoundingBox::empty();
    for v in &scene.mesh.vertices {
        bb.extend(v.position);
    }
    for g in &scene.groups {
        if !g.hidden {
            for v in &g.mesh.vertices {
                bb.extend(g.transform.transform_point3(v.position));
            }
        }
    }
    if !bb.is_valid() {
        return None;
    }
    let size = (bb.max.x - bb.min.x)
        .max(bb.max.y - bb.min.y)
        .max(1.0);
    Some(serde_json::json!({
        "z": bb.min.z.min(0.0) - 0.002,
        "size": (size * 200.0).max(4000.0),
        "color": [0.42, 0.42, 0.40],
    }))
}

/// Estimates sun unit vector toward the sun based on scene datum.
pub fn sun_toward(scene: &Scene) -> Option<[f64; 3]> {
    // Standard afternoon architectural lighting (45 deg azimuth, 35 deg altitude)
    let alt_rad = 38.0_f64.to_radians();
    let az_rad = 135.0_f64.to_radians(); // From southeast

    let north_rot = scene.geo_datum.as_ref().map(|d| d.north.to_radians()).unwrap_or(0.0);
    let effective_az = az_rad + north_rot;

    let x = alt_rad.cos() * effective_az.sin();
    let y = alt_rad.cos() * effective_az.cos();
    let z = alt_rad.sin();

    let v = DVec3::new(x, y, z).normalize();
    Some([v.x, v.y, v.z])
}

/// Builds complete job json configuration for Blender.
pub fn build_job(
    scene: &Scene,
    camera: &OrbitCamera,
    glb_path: &Path,
    out_png: &Path,
    blend_path: Option<&Path>,
    engine: BlenderEngine,
    quality: RenderQuality,
    width: u32,
    height: u32,
    ambience: Ambience,
    sun_scale: f64,
    ground: bool,
    lights: &[BlenderLight],
) -> serde_json::Value {
    let samples = samples_for(engine, quality);
    let exposure = exposure_for(ambience);

    let active_lights: Vec<serde_json::Value> = lights
        .iter()
        .filter(|lt| lt.on)
        .map(|lt| {
            serde_json::json!({
                "kind": lt.kind.as_str(),
                "pos": lt.pos,
                "dir": lt.dir,
                "kelvin": lt.kelvin,
                "color": lt.color,
                "power": lt.power,
                "angle": lt.angle,
                "on": lt.on,
                "name": lt.name,
            })
        })
        .collect();

    serde_json::json!({
        "glb": glb_path.to_string_lossy().to_string(),
        "output": out_png.to_string_lossy().to_string(),
        "blend": blend_path.map(|p| p.to_string_lossy().to_string()),
        "width": width,
        "height": height,
        "engine": engine.as_str(),
        "samples": samples,
        "camera": camera_dict(camera),
        "ambience": ambience.as_str(),
        "exposure": exposure,
        "sun": if ambience == Ambience::Day { sun_toward(scene) } else { None },
        "sun_strength": 3.0 * sun_scale.clamp(0.0, 4.0),
        "lights": active_lights,
        "ground": if ground { ground_dict(scene) } else { None },
    })
}

/// Assembles command arguments to invoke Blender headlessly.
pub fn command(found: &BlenderFound, script_path: &Path, job_path: &Path) -> Vec<String> {
    let mut args = found.command.clone();
    args.push("-b".to_string());
    args.push("--factory-startup".to_string());
    args.push("--python".to_string());
    args.push(script_path.to_string_lossy().to_string());
    args.push("--".to_string());
    args.push(job_path.to_string_lossy().to_string());
    args
}

/// Returns the cache work base directory for rendering jobs.
pub fn default_work_base() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".cache").join("IngeTrazo").join("render")
}

/// Embedded Python script that executes inside Blender.
pub const BLENDER_RENDER_SCRIPT: &str = include_str!("../../../ingetrazo/resources/blender/render_scene.py");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelvin_to_rgb_values() {
        let warm = kelvin_to_rgb(2700.0);
        assert!(warm[0] >= 0.99); // Warm should be high red
        assert!(warm[2] < warm[0]); // Less blue than red

        let cool = kelvin_to_rgb(6500.0);
        assert!(cool[0] > 0.8);
        assert!(cool[2] > 0.8); // High blue for cool daylight
    }

    #[test]
    fn test_progress_parsing() {
        assert_eq!(
            progress_of("INGETRAZO stats Remaining: 00:03.05 | Mem: 77M | Sample 16/64"),
            Some(0.25)
        );
        assert_eq!(
            progress_of("INGETRAZO stats Rendering 32 / 64 samples"),
            Some(0.5)
        );
        assert_eq!(
            progress_of("INGETRAZO stats Mem: 0M | Synchronizing object"),
            None
        );
    }

    #[test]
    fn test_command_building() {
        let found = BlenderFound {
            command: vec!["/usr/bin/blender".to_string()],
            where_desc: "/usr/bin/blender".to_string(),
        };
        let script = Path::new("/tmp/render_scene.py");
        let job = Path::new("/tmp/job.json");
        let cmd = command(&found, script, job);
        assert_eq!(cmd[0], "/usr/bin/blender");
        assert_eq!(cmd[1], "-b");
        assert_eq!(cmd[2], "--factory-startup");
        assert_eq!(cmd[3], "--python");
        assert_eq!(cmd[4], "/tmp/render_scene.py");
        assert_eq!(cmd[5], "--");
        assert_eq!(cmd[6], "/tmp/job.json");
    }
}
