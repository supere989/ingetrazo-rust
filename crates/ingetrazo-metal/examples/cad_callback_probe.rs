//! Actual same-pass native CAD callback consumption; no presentation claim.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{MetalCad2d, NativeMetal, egui_painter::EguiPainter};
    use ingetrazo_render::{
        cad_callback::Factory,
        cad2d::{Batch, EncodedRgba, Geometry, Pose, Vertex},
    };
    use std::{path::PathBuf, sync::Arc, time::Duration};
    let root = std::env::args()
        .find_map(|a| a.strip_prefix("--output=").map(PathBuf::from))
        .ok_or("--output required")?;
    std::fs::create_dir_all(&root)?;
    if std::fs::read_dir(&root)?.next().is_some() {
        return Err("Fresh proof directory required".into());
    }
    let metal = NativeMetal::new()?;
    if !MetalCad2d::supported(&metal) {
        std::fs::write(
            root.join("result.json"),
            format!(
                "{{\"passed\":false,\"unsupported\":true,\"device\":{:?},\"intel_exact_compute_presentation_open\":true}}",
                metal.info.name
            ),
        )?;
        if std::env::args().any(|a| a == "--require-supported") {
            return Err("Exact Apple callback capability unsupported".into());
        }
        return Ok(());
    }
    let geometry = |offset: f32| {
        Geometry::new(
            vec![
                Vertex {
                    position: [16. + offset, 16.],
                    coverage: 1.,
                },
                Vertex {
                    position: [48. + offset, 16.],
                    coverage: 1.,
                },
                Vertex {
                    position: [48. + offset, 48.],
                    coverage: 1.,
                },
                Vertex {
                    position: [16. + offset, 48.],
                    coverage: 1.,
                },
            ],
            vec![0, 1, 2, 0, 2, 3],
        )
    };
    let g = geometry(0.)?;
    let palette: Arc<[Batch]> = vec![Batch {
        first: 0,
        count: 6,
        color: EncodedRgba::from_bytes([37, 93, 157, 128], 1.)?,
    }]
    .into();
    let factory = Factory::default();
    let handle = factory.handle(g.clone(), palette.clone(), Pose::default())?;
    let scene = || {
        vec![egui::ClippedPrimitive {
            clip_rect: egui::Rect::EVERYTHING,
            primitive: egui::epaint::Primitive::Callback(egui::epaint::PaintCallback {
                rect: egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(56., 56.)),
                callback: handle.clone(),
            }),
        }]
    };
    let mut painter = EguiPainter::new(&metal)?;
    let initial = painter.prepare(&metal, &scene(), 1.)?;
    let render = |painter: &EguiPainter, paint| {
        let mut owner = painter.submit_diagnostic(&metal, paint, 64, 64, false)?;
        owner.wait_readback(Duration::from_secs(5))
    };
    let expected = |offset: usize, color: [u8; 4]| {
        let mut bytes = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                bytes.extend(
                    if (16 + offset..48 + offset).contains(&x) && (16..48).contains(&y) {
                        color
                    } else {
                        [255; 4]
                    },
                );
            }
        }
        bytes
    };
    let bytes = render(&painter, initial.clone())?;
    let original = expected(0, [146, 174, 206, 255]);
    std::fs::write(root.join("initial.rgba"), &bytes)?;
    assert_eq!(bytes, original, "Exact authored QColor over white");
    let baseline = painter.cad_stats();
    for _ in 0..10 {
        let prepared = painter.prepare(&metal, &scene(), 1.)?;
        assert_eq!(render(&painter, prepared)?, bytes);
    }
    assert_eq!(painter.cad_stats().uploads, baseline.uploads);
    assert_eq!(
        painter.cad_stats().palette_preparations,
        baseline.palette_preparations
    );
    handle.update(
        g.clone(),
        palette.clone(),
        Pose {
            translation: [4., 0.],
            ..Default::default()
        },
    )?;
    let moved = painter.prepare(&metal, &scene(), 1.)?;
    assert_eq!(render(&painter, moved)?, expected(4, [146, 174, 206, 255]));
    assert_eq!(
        render(&painter, initial.clone())?,
        original,
        "Frozen submitted palette/pose ownership"
    );
    let charged = metal.budget.lock().unwrap().total_committed();
    let admitted_vertex_bytes = metal
        .buffer(g.vertices().len() * std::mem::size_of::<Vertex>(), true)?
        .allocation_bytes();
    metal.budget.lock().unwrap().total_limit = Some(charged + admitted_vertex_bytes);
    handle.update(geometry(1.)?, palette.clone(), Pose::default())?;
    assert!(painter.prepare(&metal, &scene(), 1.).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), charged);
    assert_eq!(painter.cad_stats().uploads, baseline.uploads);
    metal.budget.lock().unwrap().total_limit = None;
    assert_eq!(
        render(&painter, initial.clone())?,
        original,
        "Admission failure preserves prior callback paint"
    );
    handle.update(g, palette, Pose::default())?;
    let retried = painter.prepare(&metal, &scene(), 1.)?;
    assert_eq!(render(&painter, retried)?, original);
    let stats = painter.cad_stats();
    drop(initial);
    drop(painter);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
    std::fs::write(
        root.join("result.json"),
        format!(
            "{{\"passed\":true,\"device\":{:?},\"geometry_uploads\":{},\"palette_preparations\":{},\"warm_hits\":{},\"final_ledger\":0,\"presentation_claim\":false}}",
            metal.info.name, stats.uploads, stats.palette_preparations, stats.warm_hits
        ),
    )?;
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal callback execution requires macOS");
    std::process::exit(2);
}
