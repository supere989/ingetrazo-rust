// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Headless Blender Cycles/EEVEE Rendering Bridge (Track 5).
//!
//! Provides background photorealistic rendering through external Blender (`blender -b`).
//! Handles Blender binary auto-discovery, GLB scene export, lighting, physical sky,
//! camera matching, environment cleanup, and live progress reporting.

use crate::camera::OrbitCamera;
use crate::scene::Scene;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlenderLight {
    pub kind: LightKind,
    pub pos: [f64; 3],
    pub dir: [f64; 3],
    pub kelvin: Option<u32>,
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
            kelvin: Some(DEFAULT_KELVIN),
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
    clean_env_values(std::env::vars().collect(), false)
}
/// Deterministic source environment law. Native builds are not PyInstaller;
/// packaging callers may explicitly select its frozen-library fallback.
pub fn clean_env_values(mut env: Vec<(String, String)>, frozen: bool) -> Vec<(String, String)> {
    for name in ["LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"] {
        let original = format!("{name}_ORIG");
        if let Some(i) = env.iter().position(|(k, _)| k == &original) {
            let (_, value) = env.remove(i);
            env.retain(|(k, _)| k != name);
            env.push((name.into(), value));
        } else if frozen {
            env.retain(|(k, _)| k != name);
        }
    }
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
fn qt_vec(v: glam::DVec3) -> glam::Vec3 {
    v.as_vec3()
}
fn qt_normalized(v: glam::Vec3) -> glam::Vec3 {
    let m = v.abs().max_element();
    let length = if m == 0. {
        0.
    } else {
        let s = v / m;
        m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
    };
    if (length - 1.).abs() <= 1e-5 {
        v
    } else if length.abs() <= 1e-5 {
        glam::Vec3::ZERO
    } else {
        v / length
    }
}
fn qt_eye(camera: &OrbitCamera) -> glam::Vec3 {
    let cp = camera.pitch.cos();
    let sp = camera.pitch.sin();
    let cy = camera.yaw.cos();
    let sy = camera.yaw.sin();
    qt_vec(camera.target)
        + glam::Vec3::new(
            (camera.distance * cp * cy) as f32,
            (camera.distance * cp * sy) as f32,
            (camera.distance * sp) as f32,
        )
}
/// Admit finite poses through the original QVector3D float32 conversion.
/// Rendering does not rewrite the double-precision document camera. Reject
/// nonfinite or overflowing poses before cold export and persistent requests.
pub fn validate_camera(camera: &OrbitCamera) -> Result<(), String> {
    for v in [camera.target, camera.up] {
        if !v.is_finite() || !qt_vec(v).is_finite() {
            return Err("Blender camera requires finite QVector3D target/up".into());
        }
    }
    if ![
        camera.distance,
        camera.yaw,
        camera.pitch,
        camera.fov_deg,
        camera.znear,
        camera.zfar,
    ]
    .iter()
    .all(|v| v.is_finite())
        || !qt_eye(camera).is_finite()
    {
        return Err("Blender camera derived pose must be finite".into());
    }
    Ok(())
}
pub fn camera_dict(camera: &OrbitCamera) -> serde_json::Value {
    let eye = qt_eye(camera);
    let target = qt_vec(camera.target);
    let raw = target - eye;
    let squared = raw.x * raw.x + raw.y * raw.y + raw.z * raw.z;
    let forward = if squared > 1e-18 {
        qt_normalized(raw)
    } else {
        glam::Vec3::Y
    };
    let stored_up = qt_vec(camera.up);
    let dot = forward.x * stored_up.x + forward.y * stored_up.y + forward.z * stored_up.z;
    let up = if dot.abs() < 0.9999 {
        stored_up
    } else {
        let sign = if camera.pitch > 0. { -1. } else { 1. };
        glam::Vec3::new(
            (sign * camera.yaw.cos()) as f32,
            (sign * camera.yaw.sin()) as f32,
            0.,
        )
    };
    let half_h = camera.distance * (camera.fov_deg.to_radians() / 2.0).tan();
    serde_json::json!({"eye":eye.as_dvec3().to_array(),"target":target.as_dvec3().to_array(),"up":up.as_dvec3().to_array(),"perspective":camera.perspective,"fov_deg":camera.fov_deg,"half_height":half_h,"near":camera.znear.min((camera.distance*0.02).max(1e-4)),"far":camera.zfar})
}

/// Builds ground catcher dictionary for the Blender script.
pub fn ground_dict(scene: &Scene) -> Option<serde_json::Value> {
    fn truth(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(v) => *v,
            serde_json::Value::Number(v) => v.as_f64() != Some(0.),
            serde_json::Value::String(v) => !v.is_empty(),
            serde_json::Value::Array(v) => !v.is_empty(),
            serde_json::Value::Object(v) => !v.is_empty(),
        }
    }
    let mut bb = crate::math::BoundingBox::empty();
    // Original bounds ignores orphan loose vertices and Edge.hidden; it asks
    // entity_visible(edge), which is the layer, not incident-face visibility.
    for edge in &scene.mesh.edges {
        if scene.is_layer_visible(edge.layer.as_deref()) {
            for id in [edge.v0, edge.v1] {
                if let Some(v) = scene.mesh.vertices.get(id) {
                    bb.extend(qt_vec(v.position).as_dvec3());
                }
            }
        }
    }
    for face in &scene.mesh.faces {
        if !face.attrs.get("hidden").is_some_and(truth)
            && scene.is_layer_visible(
                face.layer
                    .as_deref()
                    .or_else(|| face.attrs.get("layer").and_then(|v| v.as_str())),
            )
        {
            for id in &face.vertices {
                if let Some(v) = scene.mesh.vertices.get(*id) {
                    bb.extend(qt_vec(v.position).as_dvec3());
                }
            }
        }
    }
    fn placement(
        scene: &Scene,
        g: &crate::groups::Group,
        parent: glam::Mat4,
        bb: &mut crate::math::BoundingBox,
    ) {
        let local = glam::Mat4::from_cols_array(&g.transform.to_cols_array().map(|v| v as f32));
        let matrix = parent * local;
        if !g.hidden && !g.billboard && scene.is_layer_visible(g.layer.as_deref()) {
            // Source NumPy applies the composed Qt float matrix in double;
            // rounding happens at the final bounds QVector3D, not per vertex.
            let matrix = glam::DMat4::from_cols_array(&matrix.to_cols_array().map(f64::from));
            for v in &g.mesh.vertices {
                bb.extend(matrix.transform_point3(qt_vec(v.position).as_dvec3()));
            }
        }
        // Original placements filters each nested item, rather than pruning
        // descendants of an invisible intermediate placement.
        for child in &g.children {
            placement(scene, child, matrix, bb);
        }
    }
    for g in &scene.groups {
        if !g.hidden && !g.billboard && scene.is_layer_visible(g.layer.as_deref()) {
            placement(scene, g, glam::Mat4::IDENTITY, &mut bb);
        }
    }
    if !bb.is_valid() {
        return None;
    }
    let lo = qt_vec(bb.min).as_dvec3();
    let hi = qt_vec(bb.max).as_dvec3();
    let size = (hi.x - lo.x).max(hi.y - lo.y).max(1.);
    Some(
        serde_json::json!({"z":lo.z.min(0.)-0.002,"size":(size*200.).max(4000.),"color":[0.42,0.42,0.40]}),
    )
}

/// Estimates sun unit vector toward the sun based on scene datum.
pub fn sun_toward(scene: &Scene, year: i32) -> Option<[f64; 3]> {
    let mut settings = scene.shadows.clone();
    // Rendering follows the date/site even when viewport shadows are disabled.
    settings.enabled = true;
    crate::sun::scene_direction(&settings, scene.geo_datum.as_ref(), year)
        .ok()
        .flatten()
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
    year: i32,
) -> serde_json::Value {
    let samples = samples_for(engine, quality);
    let exposure = exposure_for(ambience);

    let active_lights: Vec<serde_json::Value> = lights
        .iter()
        .filter(|lt| lt.on && lt.pos.iter().chain(lt.dir.iter()).all(|v| v.is_finite()))
        .map(|lt| {
            let kelvin = lt
                .kelvin
                .map(|k| k.clamp(MIN_KELVIN as u32, MAX_KELVIN as u32));
            let dir = if lt.dir.iter().map(|v| v * v).sum::<f64>().sqrt() < 1e-9 {
                [0., 0., -1.]
            } else {
                lt.dir
            };
            serde_json::json!({
                "kind": lt.kind.as_str(),
                "pos": lt.pos,
                "dir": dir,
                "kelvin": kelvin,
                "color": kelvin.map_or(lt.color,|k|kelvin_to_rgb(f64::from(k))),
                "power": if lt.power.is_nan(){0.}else{lt.power.clamp(0.,1e6)},
                "angle": if lt.angle.is_nan(){1.}else{lt.angle.clamp(1.,179.)},
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
        "sun": if ambience == Ambience::Day { sun_toward(scene, year) } else { None },
        "sun_strength": 3.0 * if sun_scale.is_nan(){0.}else{sun_scale.clamp(0.0, 4.0)},
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
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    work_base_values(
        &home,
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .as_deref(),
        Path::new("/.flatpak-info").exists(),
    )
}
/// Linux QStandardPaths GenericCacheLocation and host-visible Flatpak rule.
/// Native macOS/Windows platform cache-location acceptance remains separate.
pub fn work_base_values(home: &Path, xdg: Option<&Path>, flatpak: bool) -> PathBuf {
    let base = if flatpak {
        home.join(".cache")
    } else {
        xdg.filter(|p| p.is_absolute())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".cache"))
    };
    base.join("IngeTrazo").join("render")
}

/// Embedded Python script that executes inside Blender.
pub const BLENDER_RENDER_SCRIPT: &str =
    include_str!("../../../ingetrazo/resources/blender/render_scene.py");

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

mod document;
pub use document::{clean_lights, BlenderDocument};
