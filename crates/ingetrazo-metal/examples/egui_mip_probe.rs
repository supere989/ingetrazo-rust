//! Actual queue-ordered mip GPU generation/readback; no production CPU waits.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{NativeMetal, egui_painter::EguiPainter};
    use std::{path::PathBuf, time::Duration};
    let out = std::env::args()
        .find_map(|a| a.strip_prefix("--output=").map(PathBuf::from))
        .ok_or("--output=FRESH_DIR required")?;
    std::fs::create_dir_all(&out)?;
    if out.join("checker-minified.rgba").exists()
        || out.join("result.json").exists()
        || (0..4).any(|i| out.join(format!("batch-mip-{i}.rgba")).exists())
        || (0..3).any(|i| out.join(format!("mip-{i}.rgba")).exists())
    {
        return Err("stale mip artifacts refused".into());
    }
    let metal = NativeMetal::new()?;
    eprintln!(
        "Native Metal mipGPU device={}; unified={}",
        metal.info.name, metal.info.unified_memory
    );
    let mut painter = EguiPainter::new(&metal)?;
    let mut options = egui::TextureOptions::LINEAR;
    options.mipmap_mode = Some(egui::TextureFilter::Linear);
    let image = egui::ColorImage::new([7, 5], egui::Color32::from_rgb(37, 93, 157));
    let delta = egui::TexturesDelta {
        set: vec![
            (
                egui::TextureId::Managed(1),
                egui::epaint::ImageDelta::full(image.clone(), options),
            ),
            (
                egui::TextureId::Managed(5),
                egui::epaint::ImageDelta::full(
                    egui::ColorImage::new([8, 4], egui::Color32::from_rgb(37, 93, 157)),
                    options,
                ),
            ),
        ],
        free: vec![],
    };
    painter.update_textures(&metal, &delta)?;
    let mips = painter.read_texture_mips_for_probe(
        &metal,
        egui::TextureId::Managed(1),
        Duration::from_secs(5),
    )?;
    assert_eq!(mips.len(), 3);
    for (level, (bytes, size)) in mips.iter().zip([(7, 5), (3, 2), (1, 1)]).enumerate() {
        std::fs::write(out.join(format!("mip-{level}.rgba")), bytes)?;
        assert_eq!(*bytes, [37, 93, 157, 255].repeat(size.0 * size.1));
    }
    let batch = painter.read_texture_mips_for_probe(
        &metal,
        egui::TextureId::Managed(5),
        Duration::from_secs(5),
    )?;
    assert_eq!(batch.len(), 4);
    for (level, (bytes, size)) in batch
        .iter()
        .zip([(8, 4), (4, 2), (2, 1), (1, 1)])
        .enumerate()
    {
        std::fs::write(out.join(format!("batch-mip-{level}.rgba")), bytes)?;
        assert_eq!(*bytes, [37, 93, 157, 255].repeat(size.0 * size.1));
    }
    let baseline = metal.budget.lock().unwrap().total_committed();
    let stats = painter.stats();
    assert_eq!(stats.mip_generation_submissions, 1);
    assert_eq!(stats.generated_mip_levels, 5);
    for _ in 0..5 {
        painter.update_textures(&metal, &egui::TexturesDelta::default())?;
        assert_eq!(
            painter.read_texture_mips_for_probe(
                &metal,
                egui::TextureId::Managed(1),
                Duration::from_secs(5)
            )?,
            mips
        );
    }
    assert_eq!(painter.stats().texture_uploads, stats.texture_uploads);
    assert_eq!(painter.stats().mip_generation_submissions, 1);
    let limit = metal.budget.lock().unwrap().total_limit;
    metal.budget.lock().unwrap().total_limit = Some(baseline);
    assert!(painter.update_textures(&metal, &delta).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
    metal.budget.lock().unwrap().total_limit = limit;
    assert_eq!(
        painter.read_texture_mips_for_probe(
            &metal,
            egui::TextureId::Managed(1),
            Duration::from_secs(5)
        )?,
        mips
    );
    // Earlier admitted replacement must not submit GPU work if later input is invalid.
    let broken = egui::TexturesDelta {
        set: vec![
            (
                egui::TextureId::Managed(2),
                egui::epaint::ImageDelta::full(image, options),
            ),
            (
                egui::TextureId::Managed(3),
                egui::epaint::ImageDelta::partial(
                    [0, 0],
                    egui::ColorImage::new([1, 1], egui::Color32::RED),
                    options,
                ),
            ),
        ],
        free: vec![],
    };
    assert!(painter.update_textures(&metal, &broken).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), baseline);
    assert_eq!(painter.stats().mip_generation_submissions, 1);
    // Actual sampler LOD must select the GPU-generated 1x1 checker average,
    // not the black base-level nearest texel at uv=(.5,.5).
    let mut checker_options = egui::TextureOptions::NEAREST;
    checker_options.mipmap_mode = Some(egui::TextureFilter::Nearest);
    let checker = egui::ColorImage {
        size: [8, 8],
        pixels: (0..64)
            .map(|i| {
                if (i % 8 + i / 8) % 2 == 0 {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::WHITE
                }
            })
            .collect(),
    };
    painter.update_textures(
        &metal,
        &egui::TexturesDelta {
            set: vec![(
                egui::TextureId::Managed(4),
                egui::epaint::ImageDelta::full(checker, checker_options),
            )],
            free: vec![],
        },
    )?;
    let quad = egui::ClippedPrimitive {
        clip_rect: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
        primitive: egui::epaint::Primitive::Mesh(egui::epaint::Mesh {
            vertices: [
                (egui::pos2(0., 0.), egui::pos2(0., 0.)),
                (egui::pos2(1., 0.), egui::pos2(1., 0.)),
                (egui::pos2(1., 1.), egui::pos2(1., 1.)),
                (egui::pos2(0., 1.), egui::pos2(0., 1.)),
            ]
            .into_iter()
            .map(|(pos, uv)| egui::epaint::Vertex {
                pos,
                uv,
                color: egui::Color32::WHITE,
            })
            .collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
            texture_id: egui::TextureId::Managed(4),
        }),
    };
    let paint = painter.prepare(&metal, &[quad], 1.)?;
    let mut sampled = painter.submit_diagnostic(&metal, paint, 1, 1, false)?;
    let sample = sampled.wait_readback(Duration::from_secs(5))?;
    std::fs::write(out.join("checker-minified.rgba"), &sample)?;
    assert_eq!(sample, [128, 128, 128, 255]);
    drop(sampled);
    painter.poll_mip_jobs()?;
    drop(painter);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
    // Block the persistent queue with a real unsignaled GPU event, then drop a
    // pending mip painter: complete command/image/device owners must be retained.
    use objc2_metal::{MTLCommandBuffer, MTLCommandQueue, MTLDevice, MTLSharedEvent};
    let blocked_metal = NativeMetal::new()?;
    let event = blocked_metal
        .device
        .newSharedEvent()
        .ok_or("shared event unavailable")?;
    struct Release(objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn MTLSharedEvent>>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.setSignaledValue(1)
        }
    }
    let _release = Release(event.clone());
    let block = blocked_metal
        .queue
        .commandBuffer()
        .ok_or("command unavailable")?;
    block.encodeWaitForEvent_value(objc2::runtime::ProtocolObject::from_ref(&*event), 1);
    block.commit();
    let mut pending = EguiPainter::new(&blocked_metal)?;
    pending.update_textures(&blocked_metal, &delta)?;
    let held = blocked_metal.budget.lock().unwrap().total_committed();
    assert!(held > 0);
    drop(pending);
    assert_eq!(blocked_metal.budget.lock().unwrap().total_committed(), held);
    assert!(blocked_metal.buffer(16, true).is_err());
    event.setSignaledValue(1);
    let drain = blocked_metal
        .queue
        .commandBuffer()
        .ok_or("drain command unavailable")?;
    drain.commit();
    let start = std::time::Instant::now();
    while drain.status() != objc2_metal::MTLCommandBufferStatus::Completed {
        if start.elapsed() > Duration::from_secs(5) {
            return Err("diagnostic drain bounded timeout".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(blocked_metal.budget.lock().unwrap().total_committed(), held);
    std::fs::write(
        out.join("result.json"),
        "{\"passed\":true,\"mips\":3,\"batch_images\":2,\"batch_commands\":1,\"warm_generation_jobs\":0,\"transactional_refusal\":true,\"pending_drop_retained\":true,\"physical_mac_claim\":false}",
    )?;
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    std::process::exit(2)
}
