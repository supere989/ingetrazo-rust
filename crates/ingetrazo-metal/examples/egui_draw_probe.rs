//! Actual direct native Metal textured/font/scissor draws plus bounded ownership.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{
        NativeMetal,
        egui_painter::{EguiPainter, PreparedPaint},
    };
    use objc2_metal::MTLSharedEvent;
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };
    struct ReleaseGate(objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn MTLSharedEvent>>);
    impl Drop for ReleaseGate { fn drop(&mut self) { self.0.setSignaledValue(1); } }
    let output = std::env::args()
        .find_map(|arg| arg.strip_prefix("--output=").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("metal-egui-draw-proof"));
    std::fs::create_dir_all(&output)?;
    for name in [
        "initial.rgba",
        "partial.rgba",
        "expected-initial.rgba",
        "expected-partial.rgba",
        "result.json",
    ] {
        if output.join(name).exists() {
            return Err(format!("Refusing stale diagnostic artifact: {name}").into());
        }
    }

    let metal = NativeMetal::new()?;
    eprintln!(
        "Native Metal egui sampled draw: device={}; unified={}",
        metal.info.name, metal.info.unified_memory
    );
    let mut painter = EguiPainter::new(&metal)?;
    let initial = egui::ColorImage {
        size: [2, 2],
        pixels: vec![
            egui::Color32::RED,
            egui::Color32::GREEN,
            egui::Color32::BLUE,
            egui::Color32::WHITE,
        ],
    };
    let mut font = egui::FontImage::new([1, 1]);
    font.pixels[0] = 1.;
    let textures = egui::TexturesDelta {
        set: vec![
            (
                egui::TextureId::Managed(42),
                egui::epaint::ImageDelta::full(initial, egui::TextureOptions::NEAREST),
            ),
            (
                egui::TextureId::Managed(43),
                egui::epaint::ImageDelta::full(font, egui::TextureOptions::NEAREST),
            ),
        ],
        free: vec![],
    };
    painter.update_textures(&metal, &textures)?;
    let quad = |rect: egui::Rect, color: egui::Color32, id, clip| {
        let positions = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ];
        let uvs = [
            egui::pos2(0., 0.),
            egui::pos2(1., 0.),
            egui::pos2(1., 1.),
            egui::pos2(0., 1.),
        ];
        egui::ClippedPrimitive {
            clip_rect: clip,
            primitive: egui::epaint::Primitive::Mesh(egui::epaint::Mesh {
                vertices: positions
                    .into_iter()
                    .zip(uvs)
                    .map(|(pos, uv)| egui::epaint::Vertex { pos, uv, color })
                    .collect(),
                indices: vec![0, 1, 2, 0, 2, 3],
                texture_id: id,
            }),
        }
    };
    let scene = |offset| {
        vec![
            quad(
                egui::Rect::from_min_size(egui::pos2(offset, 0.), egui::vec2(64., 64.)),
                egui::Color32::WHITE,
                egui::TextureId::Managed(42),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(48., 48.)),
            ),
            quad(
                egui::Rect::from_min_size(egui::pos2(8., 8.), egui::vec2(8., 8.)),
                egui::Color32::BLACK,
                egui::TextureId::Managed(43),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(64., 64.)),
            ),
        ]
    };
    let expected = |partial: bool| {
        let mut bytes = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                let c = if (8..16).contains(&x) && (8..16).contains(&y) {
                    [0, 0, 0, 255]
                } else if x >= 48 || y >= 48 {
                    [255, 255, 255, 255]
                } else if x < 32 && y < 32 {
                    [255, 0, 0, 255]
                } else if x >= 32 && y < 32 {
                    if partial {
                        [255, 255, 0, 255]
                    } else {
                        [0, 255, 0, 255]
                    }
                } else if x < 32 {
                    [0, 0, 255, 255]
                } else {
                    [255, 255, 255, 255]
                };
                bytes.extend(c);
            }
        }
        bytes
    };
    let render = |painter: &EguiPainter, paint: Arc<PreparedPaint>| -> Result<Vec<u8>, String> {
        let mut submitted = painter.submit_diagnostic(&metal, paint, 64, 64, false)?;
        submitted.wait_readback(Duration::from_secs(5))
    };
    let paint = painter.prepare(&metal, &scene(0.), 1.)?;
    let initial_pixels = render(&painter, paint.clone())?;
    std::fs::write(output.join("initial.rgba"), &initial_pixels)?;
    std::fs::write(output.join("expected-initial.rgba"), expected(false))?;
    assert_eq!(
        initial_pixels,
        expected(false),
        "Whole textured/font/scissor GPU image"
    );
    let stats = painter.stats();
    for _ in 0..10 {
        let warm = painter.prepare(&metal, &scene(0.), 1.)?;
        assert_eq!(render(&painter, warm)?, initial_pixels);
    }
    assert_eq!(painter.stats().geometry_uploads, stats.geometry_uploads);
    assert_eq!(painter.stats().texture_uploads, stats.texture_uploads);
    let baseline = metal.budget.lock().unwrap().total_committed();
    let vb_bytes = metal.buffer(8 * 20, true)?.allocation_bytes();
    metal.budget.lock().unwrap().total_limit = Some(baseline + vb_bytes);
    assert!(painter.prepare(&metal, &scene(1.), 1.).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
    assert_eq!(painter.stats().geometry_uploads, stats.geometry_uploads);
    metal.budget.lock().unwrap().total_limit = None;
    assert_eq!(render(&painter, paint.clone())?, initial_pixels);
    let replacement = egui::TexturesDelta {
        set: vec![(
            egui::TextureId::Managed(42),
            egui::epaint::ImageDelta::full(
                egui::ColorImage::new([2, 2], egui::Color32::BLUE),
                egui::TextureOptions::NEAREST,
            ),
        )],
        free: vec![],
    };
    metal.budget.lock().unwrap().total_limit = Some(baseline);
    assert!(painter.update_textures(&metal, &replacement).is_err());
    metal.budget.lock().unwrap().total_limit = None;
    assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
    let restored = painter.prepare(&metal, &scene(0.), 1.)?;
    assert_eq!(render(&painter, restored)?, initial_pixels);
    let invalid = egui::TexturesDelta {
        set: vec![(
            egui::TextureId::Managed(42),
            egui::epaint::ImageDelta::partial(
                [2, 0],
                egui::ColorImage::new([1, 1], egui::Color32::YELLOW),
                egui::TextureOptions::NEAREST,
            ),
        )],
        free: vec![],
    };
    assert!(painter.update_textures(&metal, &invalid).is_err());
    let partial = egui::TexturesDelta {
        set: vec![(
            egui::TextureId::Managed(42),
            egui::epaint::ImageDelta::partial(
                [1, 0],
                egui::ColorImage::new([1, 1], egui::Color32::YELLOW),
                egui::TextureOptions::NEAREST,
            ),
        )],
        free: vec![],
    };
    painter.update_textures(&metal, &partial)?;
    let revised = painter.prepare(&metal, &scene(0.), 1.)?;
    let partial_pixels = render(&painter, revised.clone())?;
    std::fs::write(output.join("partial.rgba"), &partial_pixels)?;
    std::fs::write(output.join("expected-partial.rgba"), expected(true))?;
    assert_eq!(partial_pixels, expected(true));
    assert_ne!(
        initial_pixels,
        vec![255; 64 * 64 * 4],
        "Missing draw negative"
    );
    let mut wrong_clip = expected(false);
    wrong_clip[(24 * 64 + 60) * 4..(24 * 64 + 60) * 4 + 4].copy_from_slice(&[0, 255, 0, 255]);
    assert_ne!(initial_pixels, wrong_clip);
    let known_baseline = metal.budget.lock().unwrap().total_committed();
    let completed = painter.submit_diagnostic(&metal, revised.clone(), 64, 64, false)?;
    let start = Instant::now();
    while !completed.completed() {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(completed);
    assert_eq!(
        metal.budget.lock().unwrap().total_committed(),
        known_baseline,
        "Completed Drop without explicit collection must free owners"
    );
    let mut blocked = painter.submit_diagnostic(&metal, revised.clone(), 64, 64, true)?;
    let _gate_cleanup = ReleaseGate(blocked.probe_gate().unwrap());
    let charged = metal.budget.lock().unwrap().total_committed();
    assert!(charged > known_baseline);
    assert!(blocked.wait_readback(Duration::from_millis(50)).is_err());
    assert!(!blocked.completed());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), charged);
    assert!(metal.buffer(16, true).is_err());
    blocked.release_probe_gate();
    assert_eq!(
        blocked.wait_readback(Duration::from_secs(5))?,
        partial_pixels
    );
    drop(blocked);
    assert_eq!(
        metal.budget.lock().unwrap().total_committed(),
        known_baseline
    );
    drop(paint);
    drop(revised);
    drop(painter);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
    // Separate actual pending-Drop control: no guessed completion or implicit wait.
    let pending_metal = NativeMetal::new()?;
    let mut pending_painter = EguiPainter::new(&pending_metal)?;
    pending_painter.update_textures(&pending_metal, &textures)?;
    let p = pending_painter.prepare(&pending_metal, &scene(0.), 1.)?;
    let blocked = pending_painter.submit_diagnostic(&pending_metal, p, 64, 64, true)?;
    let gate = blocked.probe_gate().unwrap();
    let _pending_gate_cleanup = ReleaseGate(gate.clone());
    assert!(!blocked.completed());
    let held = pending_metal.budget.lock().unwrap().total_committed();
    drop(blocked);
    assert_eq!(pending_metal.budget.lock().unwrap().total_committed(), held);
    assert!(pending_metal.buffer(16, true).is_err());
    gate.setSignaledValue(1);
    drop(pending_painter);
    assert!(
        pending_metal.budget.lock().unwrap().total_committed() > 0,
        "Pending Drop must retain submitted owners for process lifetime"
    );
    std::fs::write(
        output.join("result.json"),
        "{\"passed\":true,\"whole_images\":2,\"warm_identical_gpu_images\":10,\"completed_drop\":true,\"actual_shared_event_timeout\":true,\"pending_drop_retained\":true,\"physical_mac_claim\":false}",
    )?;
    eprintln!(
        "PASS actual native textured/font/clip images, sparse partial, warm uploads0, real admission rollback, completed vs pending Drop and actual GPU-gated timeout. Physical Mac/WSI still open."
    );
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal requires macOS");
    std::process::exit(2);
}
