//! Actual native ordered generic/CAD encoder and persistent canvas diagnostic.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{compute_canvas::Pool, egui_painter::EguiPainter, NativeMetal};
    use ingetrazo_render::{
        cad2d::{Batch, EncodedRgba, Geometry, Pose, SourceSpan, Vertex},
        cad_callback::Factory,
    };
    use objc2_metal::MTLTexture;
    use std::{path::PathBuf, sync::Arc, time::Duration};
    let root = std::env::args()
        .find_map(|s| s.strip_prefix("--output=").map(PathBuf::from))
        .ok_or("--output required")?;
    std::fs::create_dir_all(&root)?;
    if std::fs::read_dir(&root)?.next().is_some() {
        return Err("Fresh probe output required".into());
    }
    let metal = NativeMetal::new()?;
    if !ingetrazo_metal::cad_compute_painter::Painter::supported(&metal) {
        std::fs::write(
            root.join("result.json"),
            format!(
                "{{\"passed\":false,\"unsupported\":true,\"device\":{:?}}}",
                metal.info.name
            ),
        )?;
        return Err("Exact native RGBA read/write tier2 unsupported".into());
    }
    let geometry = || {
        let mut rows = Vec::new();
        let mut v = Vec::new();
        let mut i = Vec::new();
        let mut ranges = Vec::new();
        let mut batches = Vec::new();
        for (a, b) in [(8, 56), (16, 48)] {
            let first = i.len() as u32;
            for y in a..b {
                let row = SourceSpan {
                    x: a,
                    y,
                    len: b - a,
                    coverage: 255,
                };
                let base = v.len() as u32;
                for p in [[a, y], [b, y], [b, y + 1], [a, y + 1]] {
                    v.push(Vertex {
                        position: [
                            ((f64::from(p[0]) - 0.25) / 3.) as f32,
                            ((f64::from(p[1]) - 0.75) / 3.) as f32,
                        ],
                        coverage: 1.,
                    });
                }
                i.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                rows.push(row);
            }
            let end = i.len() as u32;
            ranges.push(first..end);
            batches.push((first, end - first));
        }
        Ok::<_, String>((
            Geometry::new_with_source_spans(v, i, ranges, rows, 1.5, 2., [0.25, 0.75])?,
            batches,
        ))
    };
    let (g, layout) = geometry()?;
    let pose = Pose {
        scale: [1.5; 2],
        translation: [0.125, 0.375],
    };
    let factory = Factory::default();
    let handle = factory.handle(g.clone(), Arc::<[Batch]>::from([]), pose)?;
    let scene = || {
        vec![egui::ClippedPrimitive {
            clip_rect: egui::Rect::EVERYTHING,
            primitive: egui::epaint::Primitive::Callback(egui::epaint::PaintCallback {
                rect: egui::Rect::EVERYTHING,
                callback: handle.clone(),
            }),
        }]
    };
    let mut painter = EguiPainter::new_compute(&metal)?;
    let mut pool = Pool::default();
    let render = |painter: &EguiPainter, paint, pool: &mut Pool| {
        let mut owner = painter.submit_diagnostic_with_pool(&metal, paint, 64, 64, false, pool)?;
        owner.wait_readback(Duration::from_secs(5))
    };
    let mut frozen = None;
    let mut saved_palette = None;
    for alpha in [0, 1, 127, 128, 254, 255] {
        let palette: Arc<[Batch]> = vec![
            Batch {
                first: layout[0].0,
                count: layout[0].1,
                color: EncodedRgba::from_bytes([37, 93, 157, alpha], 1.)?,
            },
            Batch {
                first: layout[1].0,
                count: layout[1].1,
                color: EncodedRgba::from_bytes([1, 7, 19, 128], 1.)?,
            },
        ]
        .into();
        handle.update(g.clone(), palette.clone(), pose)?;
        let prepared = painter.prepare(&metal, &scene(), 2.)?;
        let actual = render(&painter, prepared.clone(), &mut pool)?;
        let reference = match alpha {
            0 => include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-0.rgba"),
            1 => include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-1.rgba"),
            127 => {
                include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-127.rgba")
            }
            128 => {
                include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-128.rgba")
            }
            254 => {
                include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-254.rgba")
            }
            _ => {
                include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-255.rgba")
            }
        };
        std::fs::write(root.join(format!("alpha-{alpha}.rgba")), &actual)?;
        assert_eq!(
            actual.as_slice(),
            reference,
            "Actual QPainter complete image alpha{alpha}"
        );
        let baseline = painter.compute_stats().unwrap();
        for _ in 0..5 {
            let warm = painter.prepare(&metal, &scene(), 2.)?;
            assert_eq!(render(&painter, warm, &mut pool)?, actual);
        }
        let warm = painter.compute_stats().unwrap();
        assert_eq!(warm.plan_uploads, baseline.plan_uploads);
        assert_eq!(warm.palette_uploads, baseline.palette_uploads);
        if alpha == 128 {
            frozen = Some((prepared, actual));
            saved_palette = Some(palette);
        }
    }
    assert_eq!(
        pool.stats.allocations, 1,
        "Persistent diagnostic canvas warm reuse"
    );
    assert!(pool.stats.reused >= 30);
    let (frozen, expected) = frozen.unwrap();
    let palette = saved_palette.unwrap();
    handle.update(g.clone(), palette.clone(), pose)?;
    let texture = egui::TextureId::Managed(900);
    painter.update_textures(
        &metal,
        &egui::TexturesDelta {
            set: vec![(
                texture,
                egui::epaint::ImageDelta::full(
                    egui::ColorImage::new([1, 1], egui::Color32::WHITE),
                    egui::TextureOptions::NEAREST,
                ),
            )],
            free: vec![],
        },
    )?;
    let primitive = |rect: egui::Rect, color: egui::Color32| {
        let mut mesh = egui::Mesh::with_texture(texture);
        mesh.add_colored_rect(rect, color);
        egui::ClippedPrimitive {
            clip_rect: egui::Rect::EVERYTHING,
            primitive: egui::epaint::Primitive::Mesh(mesh),
        }
    };
    let mut mixed = vec![primitive(
        egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(32., 32.)),
        egui::Color32::BLUE,
    )];
    mixed.extend(scene());
    mixed.push(primitive(
        egui::Rect::from_min_max(egui::pos2(16., 16.), egui::pos2(20., 20.)),
        egui::Color32::GREEN,
    ));
    let prepared = painter.prepare(&metal, &mixed, 2.)?;
    let bytes = render(&painter, prepared, &mut pool)?;
    std::fs::write(root.join("mixed-ui-cad.rgba"), &bytes)?;
    assert_eq!(
        bytes.as_slice(),
        include_bytes!("../../../docs/audits/metal-compute-callback/source/mixed-ui-cad.rgba"),
        "Actual QPainter generic-before/CAD/generic-after order"
    );
    let missing = painter.prepare(&metal, &mixed[..2], 2.)?;
    let missing_bytes=render(&painter,missing,&mut pool)?;
    std::fs::write(root.join("missing-final-ui.rgba"),&missing_bytes)?;
    assert_ne!(missing_bytes.as_slice(),include_bytes!("../../../docs/audits/metal-compute-callback/source/mixed-ui-cad.rgba"),"Missing final UI negative");

    // Actual GPU reversed draw order must fail the literal original reference.
    handle.update(
        g.clone(),
        Arc::<[Batch]>::from([palette[1], palette[0]]),
        pose,
    )?;
    let reversed = painter.prepare(&metal, &scene(), 2.)?;
    let reversed_bytes=render(&painter,reversed,&mut pool)?;std::fs::write(root.join("reversed-order.rgba"),&reversed_bytes)?;assert_ne!(reversed_bytes,expected);
    assert_eq!(
        render(&painter, frozen.clone(), &mut pool)?,
        expected,
        "Earlier snapshot remains immutable"
    );
    handle.update(g.clone(), palette.clone(), pose)?;
    let restored = painter.prepare(&metal, &scene(), 2.)?;
    assert_eq!(render(&painter, restored, &mut pool)?, expected);
    let pose_baseline = painter.compute_stats().unwrap();
    handle.update(
        g.clone(),
        palette.clone(),
        Pose {
            translation: [4.125, 0.375],
            ..pose
        },
    )?;
    let shifted = painter.prepare(&metal, &scene(), 2.)?;
    let actual = render(&painter, shifted, &mut pool)?;
    for y in 0..64 {
        for x in 8..64 {
            assert_eq!(
                &actual[(y * 64 + x) * 4..(y * 64 + x + 1) * 4],
                &expected[(y * 64 + x - 8) * 4..(y * 64 + x - 7) * 4]
            );
        }
    }
    assert_eq!(
        painter.compute_stats().unwrap().plan_uploads,
        pose_baseline.plan_uploads
    );
    assert_eq!(
        painter.compute_stats().unwrap().palette_uploads,
        pose_baseline.palette_uploads
    );
    let before = painter.compute_stats().unwrap();
    handle.update(
        g.clone(),
        palette.clone(),
        Pose {
            translation: [4.125001, 0.375],
            ..pose
        },
    )?;
    assert!(painter.prepare(&metal, &scene(), 2.).is_err());
    assert_eq!(
        painter.compute_stats().unwrap().plan_uploads,
        before.plan_uploads
    );
    let (g2, _) = geometry()?;
    let admitted = metal.buffer(80 * 24, true)?.allocation_bytes();
    let charged = metal.budget.lock().unwrap().total_committed();
    metal.budget.lock().unwrap().total_limit = Some(charged + admitted);
    handle.update(g2, palette, pose)?;
    assert!(painter.prepare(&metal, &scene(), 2.).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), charged);
    metal.budget.lock().unwrap().total_limit = None;
    assert_eq!(render(&painter, frozen.clone(), &mut pool)?, expected);
    let a = pool.acquire(&metal, (64, 64))?.unwrap();
    let b = pool.acquire(&metal, (64, 64))?.unwrap();
    let c = pool.acquire(&metal, (64, 64))?.unwrap();
    assert!(pool.acquire(&metal, (64, 64))?.is_none());
    assert!(pool.acquire(&metal, (80, 80))?.is_none());
    drop((a, b, c));
    let resized = pool.acquire(&metal, (80, 80))?.unwrap();
    assert_eq!(resized.texture().width(), 80);
    drop(resized);
    let stats = painter.compute_stats().unwrap();
    let canvas_stats = pool.stats;
    drop((frozen, painter, pool, handle, g));
    assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
    std::fs::write(root.join("result.json"),format!("{{\"passed\":true,\"device\":{:?},\"read_write_tier2\":true,\"actual_qt_whole_images\":7,\"missing_ui_negative\":true,\"source_order_negative\":true,\"phase_negative\":true,\"partial_plan_budget_rollback\":true,\"plan_uploads\":{},\"palette_uploads\":{},\"warm_hits\":{},\"uploaded_bytes\":{},\"canvas_allocations\":{},\"canvas_allocated_bytes_cumulative\":{},\"canvas_warm_reuses\":{},\"final_ledger\":0,\"presentation_claim\":false}}",metal.info.name,stats.plan_uploads,stats.palette_uploads,stats.warm_hits,stats.uploaded_bytes,canvas_stats.allocations,canvas_stats.allocation_bytes,canvas_stats.reused))?;
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Actual native Metal required");
    std::process::exit(2);
}
