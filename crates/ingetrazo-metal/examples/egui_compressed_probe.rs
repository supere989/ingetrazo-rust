//! Direct compressed block GPU transfer/decode and lifetime/admission controls.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{
        egui_painter::{CompressedFormat, EguiPainter},
        NativeMetal,
    };
    use std::{path::PathBuf, time::Duration};
    let out = std::env::args()
        .find_map(|a| a.strip_prefix("--output=").map(PathBuf::from))
        .ok_or("--output=FRESH_DIR required")?;
    std::fs::create_dir_all(&out)?;
    if std::fs::read_dir(&out)?.next().is_some() {
        return Err("nonempty compression output refused".into());
    }
    let metal = NativeMetal::new()?;
    eprintln!(
        "Native Metal compression device={}; unified={}",
        metal.info.name, metal.info.unified_memory
    );
    let cases = [
        (
            "bc1",
            CompressedFormat::Bc1Rgba,
            vec![0, 248, 0, 0, 0, 0, 0, 0],
        ),
        (
            "astc",
            CompressedFormat::Astc4x4Ldr,
            vec![
                252, 253, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 255, 255,
            ],
        ),
    ];
    let mut advertised = Vec::new();
    for (name, format, blocks) in cases {
        let supported = format.supported(&metal);
        advertised.push(supported);
        let mut painter = EguiPainter::new(&metal)?;
        let before = metal.budget.lock().unwrap().total_committed();
        if !supported {
            assert!(painter
                .upload_compressed_texture(
                    &metal,
                    egui::TextureId::Managed(1),
                    format,
                    [4, 4],
                    &blocks,
                    egui::TextureOptions::NEAREST
                )
                .is_err());
            assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
            eprintln!("EXPLICIT_UNSUPPORTED {name}");
            continue;
        }
        painter.upload_compressed_texture(
            &metal,
            egui::TextureId::Managed(1),
            format,
            [4, 4],
            &blocks,
            egui::TextureOptions::NEAREST,
        )?;
        let gpu_blocks = painter.read_compressed_blocks_for_probe(
            &metal,
            egui::TextureId::Managed(1),
            Duration::from_secs(5),
        )?;
        std::fs::write(out.join(format!("{name}.blocks")), &gpu_blocks)?;
        assert_eq!(gpu_blocks, blocks);
        let image_bytes = metal.budget.lock().unwrap().total_committed() - before;
        assert!(image_bytes >= blocks.len() as u64);
        let quad = egui::ClippedPrimitive {
            clip_rect: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(4., 4.)),
            primitive: egui::epaint::Primitive::Mesh(egui::epaint::Mesh {
                vertices: [
                    (egui::pos2(0., 0.), egui::pos2(0., 0.)),
                    (egui::pos2(4., 0.), egui::pos2(1., 0.)),
                    (egui::pos2(4., 4.), egui::pos2(1., 1.)),
                    (egui::pos2(0., 4.), egui::pos2(0., 1.)),
                ]
                .into_iter()
                .map(|(pos, uv)| egui::epaint::Vertex {
                    pos,
                    uv,
                    color: egui::Color32::WHITE,
                })
                .collect(),
                indices: vec![0, 1, 2, 0, 2, 3],
                texture_id: egui::TextureId::Managed(1),
            }),
        };
        let paint = painter.prepare(&metal, &[quad], 1.)?;
        let render = |painter: &EguiPainter| -> Result<Vec<u8>, String> {
            let mut frame = painter.submit_diagnostic(&metal, paint.clone(), 4, 4, false)?;
            frame.wait_readback(Duration::from_secs(5))
        };
        let sampled = render(&painter)?;
        std::fs::write(out.join(format!("{name}.rgba")), &sampled)?;
        assert_eq!(sampled, [255, 0, 0, 255].repeat(16));
        let stats = painter.stats();
        for _ in 0..5 {
            assert_eq!(render(&painter)?, sampled)
        }
        assert_eq!(painter.stats().texture_uploads, stats.texture_uploads);
        let baseline = metal.budget.lock().unwrap().total_committed();
        let limit = metal.budget.lock().unwrap().total_limit;
        metal.budget.lock().unwrap().total_limit = Some(baseline);
        assert!(painter
            .upload_compressed_texture(
                &metal,
                egui::TextureId::Managed(1),
                format,
                [4, 4],
                &blocks,
                egui::TextureOptions::NEAREST
            )
            .is_err());
        assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
        metal.budget.lock().unwrap().total_limit = Some(baseline + image_bytes);
        assert!(painter
            .upload_compressed_texture(
                &metal,
                egui::TextureId::Managed(1),
                format,
                [4, 4],
                &blocks,
                egui::TextureOptions::NEAREST
            )
            .is_err());
        assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
        metal.budget.lock().unwrap().total_limit = limit;
        assert_eq!(render(&painter)?, sampled);
        assert!(painter
            .upload_compressed_texture(
                &metal,
                egui::TextureId::Managed(1),
                format,
                [4, 4],
                &blocks[..blocks.len() - 1],
                egui::TextureOptions::NEAREST
            )
            .is_err());
        let patch = egui::TexturesDelta {
            set: vec![(
                egui::TextureId::Managed(1),
                egui::epaint::ImageDelta::partial(
                    [0, 0],
                    egui::ColorImage::new([1, 1], egui::Color32::GREEN),
                    egui::TextureOptions::NEAREST,
                ),
            )],
            free: vec![],
        };
        assert!(painter.update_textures(&metal, &patch).is_err());
        let mut mips = egui::TextureOptions::NEAREST;
        mips.mipmap_mode = Some(egui::TextureFilter::Nearest);
        assert!(painter
            .upload_compressed_texture(
                &metal,
                egui::TextureId::Managed(1),
                format,
                [4, 4],
                &blocks,
                mips
            )
            .is_err());
        assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
        assert_eq!(painter.stats().compressed_upload_submissions, 1);
        drop(paint);
        drop(painter);
        assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
        // A real queue stall must retain BOTH the private compressed image and
        // upload staging after the painter disappears, not just cached pixels.
        use objc2_metal::{MTLCommandBuffer, MTLCommandQueue, MTLDevice, MTLSharedEvent};
        let stalled = NativeMetal::new()?;
        let event = stalled
            .device
            .newSharedEvent()
            .ok_or("shared event unavailable")?;
        struct Release(objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn MTLSharedEvent>>);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.setSignaledValue(1);
            }
        }
        let _release = Release(event.clone());
        let gate = stalled
            .queue
            .commandBuffer()
            .ok_or("gate command unavailable")?;
        gate.encodeWaitForEvent_value(objc2::runtime::ProtocolObject::from_ref(&*event), 1);
        gate.commit();
        let mut pending = EguiPainter::new(&stalled)?;
        pending.upload_compressed_texture(
            &stalled,
            egui::TextureId::Managed(1),
            format,
            [4, 4],
            &blocks,
            egui::TextureOptions::NEAREST,
        )?;
        let retained = stalled.budget.lock().unwrap().total_committed();
        assert!(
            retained > image_bytes,
            "pending staging must also be charged"
        );
        drop(pending);
        assert_eq!(stalled.budget.lock().unwrap().total_committed(), retained);
        assert!(
            stalled.buffer(16, true).is_err(),
            "unknown completion must poison admission"
        );
        event.setSignaledValue(1);
        let drain = stalled
            .queue
            .commandBuffer()
            .ok_or("drain command unavailable")?;
        drain.commit();
        let started = std::time::Instant::now();
        while drain.status() != objc2_metal::MTLCommandBufferStatus::Completed {
            if started.elapsed() > Duration::from_secs(5) {
                return Err("compressed diagnostic drain bounded timeout".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            stalled.budget.lock().unwrap().total_committed(),
            retained,
            "quarantined owners cannot be reclaimed by a guessed completion"
        );
        eprintln!("PASS {name}: stalled compressed image+staging retained_bytes={retained}; admission poisoned; real later completion observed");
        eprintln!(
            "PASS {name}: actual allocated_image_bytes={image_bytes}; block_bytes={}; warm_uploads0; pressure+rollback; finalledger0",
            blocks.len()
        );
    }
    std::fs::write(
        out.join("result.json"),
        format!(
            "{{\"passed\":{},\"bc1_supported\":{},\"astc_supported\":{},\"physical_mac_claim\":false}}",
            advertised.iter().any(|v| *v),
            advertised[0],
            advertised[1]
        ),
    )?;
    if !advertised.iter().any(|v| *v) {
        return Err("Neither supported compressed format; no GPU compression success claim".into());
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    std::process::exit(2)
}
