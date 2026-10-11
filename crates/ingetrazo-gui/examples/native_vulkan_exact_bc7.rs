//! Independent hardware decode, actual cache ownership and production draws.
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod platform {
    use ash::vk;
    use glam::DVec3;
    use ingetrazo_gui::native_vulkan::{
        budget::VulkanBudget,
        headless::DeviceRequest,
        resources::{Resources, TextureImage},
        texture_cache::TextureCache,
        world::World,
        VulkanContext,
    };
    use std::{io::Cursor, path::Path, sync::Arc};
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, rgba.to_vec()).unwrap())
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    fn draw(
        ctx: &VulkanContext,
        world: &mut World,
        command: vk::CommandBuffer,
        fence: vk::Fence,
        serial: u64,
    ) -> Result<Vec<u8>> {
        unsafe {
            ctx.device
                .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())?;
            ctx.device
                .begin_command_buffer(command, &vk::CommandBufferBeginInfo::default())?;
        }
        world.record(command, serial)?;
        unsafe {
            ctx.device.end_command_buffer(command)?;
            ctx.device.reset_fences(&[fence])?;
            ctx.device.queue_submit(
                ctx.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&[command])],
                fence,
            )?;
        }
        ctx.wait_transfer(fence)?;
        Ok(world.readback(ctx)?)
    }
    fn fixture(w: u32, h: u32, rotation: usize) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| {
                let mut p = [[0, 84, 171, 255][(i % w % 4) as usize]; 4];
                p[3] = [23, 92, 165, 234][(i / w % 4) as usize];
                if rotation > 0 {
                    p.swap(3, rotation - 1);
                }
                p
            })
            .collect()
    }
    fn cache_cases(
        ctx: &VulkanContext,
        budget: &Arc<VulkanBudget>,
        out: &Path,
    ) -> Result<serde_json::Value> {
        let mut resources = Resources::new(ctx, budget.clone());
        let mut rows = vec![];
        for (name, w, h, rgba, compressed) in [
            ("rotation0", 8, 8, fixture(8, 8, 0), true),
            ("rotation1", 8, 8, fixture(8, 8, 1), true),
            ("rotation2", 8, 8, fixture(8, 8, 2), true),
            ("rotation3", 8, 8, fixture(8, 8, 3), true),
            (
                "odd-edge",
                7,
                5,
                (0..35)
                    .flat_map(|i| [255, 255, 255, if i % 3 == 0 { 255 } else { 0 }])
                    .collect(),
                true,
            ),
            (
                "large-mask",
                512,
                512,
                (0..512 * 512)
                    .flat_map(|i| [255, 255, 255, if i % 5 == 0 { 255 } else { 0 }])
                    .collect(),
                true,
            ),
            (
                "inexact",
                8,
                8,
                (0..64)
                    .flat_map(|i| [1, 3, 5, if i % 4 == 0 { 255 } else { 0 }])
                    .collect(),
                false,
            ),
            (
                "opaque-mipped",
                8,
                8,
                [255, 255, 255, 255].repeat(64),
                false,
            ),
            ("tiny", 1, 1, vec![255, 255, 255, 0], false),
        ] {
            let encoded = png(w, h, &rgba);
            std::fs::write(out.join(format!("{name}.png")), &encoded)?;
            let mut cache = TextureCache::diagnostic_readback();
            let asset = cache.identify(&encoded);
            let id = asset.id();
            let candidate = cache.prepare(ctx, &mut resources, &asset, 2 * 1024 * 1024)?;
            let view = candidate.sampled().0;
            cache.publish(candidate)?;
            let wanted = u64::from(compressed && ctx.texture_bc_enabled);
            assert_eq!(cache.stats.bc7_uploads, wanted, "{name}");
            let mut expected = rgba.clone();
            for y in 0..h as usize / 2 {
                for x in 0..w as usize * 4 {
                    expected.swap(
                        y * w as usize * 4 + x,
                        (h as usize - 1 - y) * w as usize * 4 + x,
                    );
                }
            }
            assert_eq!(
                cache.readback_for_probe(ctx, &mut resources, id)?,
                expected,
                "Hardware-decoded source texels: {name}"
            );
            let attempts = cache.stats.compression_attempts;
            let saved = cache.stats.exact_saved_bytes;
            let allocation = cache.allocation_bytes();
            let warm = std::time::Instant::now();
            for _ in 0..100 {
                let candidate = cache.prepare(ctx, &mut resources, &asset, 2 * 1024 * 1024)?;
                assert_eq!(candidate.sampled().0, view);
                cache.publish(candidate)?;
            }
            let warm_ms = warm.elapsed().as_secs_f64() * 1000.;
            assert_eq!(
                (
                    cache.stats.decodes,
                    cache.stats.uploads,
                    cache.stats.compression_attempts
                ),
                (1, 1, attempts)
            );
            // Actual image allocation denial cannot replace the published resource.
            let mut changed = encoded.clone();
            changed.push(0);
            let other = cache.identify(&changed);
            let old = budget.ledger.lock().unwrap().total_committed();
            budget.ledger.lock().unwrap().total_limit = Some(old);
            let error = match cache.prepare(ctx, &mut resources, &other, 2 * 1024 * 1024) {
                Ok(_) => panic!("Expected allocation refusal"),
                Err(e) => e,
            };
            assert!(!cache.contains(other.id()));
            assert_eq!(cache.sampled(id)?.0, view);
            assert_eq!(budget.ledger.lock().unwrap().total_committed(), old);
            budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
            assert_eq!(
                cache.readback_for_probe(ctx, &mut resources, id)?,
                expected,
                "Retained after failed replacement"
            );
            rows.push(serde_json::json!({"case":name,"width":w,"height":h,"bc7_uploads":wanted,"uploaded_bytes":cache.stats.uploaded_bytes,"saved_logical_bytes":saved,"actual_image_allocation":allocation,"cold_compression_ms":cache.stats.compression_ms,"warm100_ms":warm_ms,"hardware_texels_exact":true,"allocation_refusal":error}));
        }
        // Mutation control goes through hardware decoder, not the encoder decoder.
        if ctx.texture_bc_enabled {
            let rgba = [255, 255, 255, 0].repeat(16);
            let mut blocks = ingetrazo_render::block_compression::exact_bc7(4, 4, &rgba)?.unwrap();
            blocks[1] ^= 1;
            let image = TextureImage::bc7(
                ctx,
                &mut resources,
                vk::Extent2D {
                    width: 4,
                    height: 4,
                },
                &blocks,
                true,
            )?;
            assert_ne!(
                resources.readback_texture_decoded_for_probe(ctx, &image)?,
                rgba,
                "Mutated endpoint must be detected independently"
            );
        }
        if ctx.texture_bc_enabled {
            // More than8MiB of actual BC7 blocks, ending with partial edge blocks:
            // exercise block-row chunk offsets independently of image encoding.
            let extent = vk::Extent2D {
                width: 4097,
                height: 2049,
            };
            let count = ingetrazo_render::texture_layout::Format::Bc7
                .mip_bytes(extent.width, extent.height)? as usize
                / 16;
            let block = ingetrazo_render::block_compression::exact_bc7(
                4,
                4,
                &[255, 255, 255, 0].repeat(16),
            )?
            .unwrap();
            let blocks = block.repeat(count);
            let before = resources.stats.chunks;
            let texture = TextureImage::bc7(ctx, &mut resources, extent, &blocks, true)?;
            let chunks = resources.stats.chunks - before;
            assert_eq!(chunks, 2);
            assert_eq!(
                resources.readback_texture_mip(ctx, &texture, 0)?,
                blocks,
                "Compressed edge/block-row transfer bytes"
            );
            let decoded = resources.readback_texture_decoded_for_probe(ctx, &texture)?;
            assert_eq!(
                decoded.len(),
                extent.width as usize * extent.height as usize * 4
            );
            assert!(
                decoded.chunks_exact(4).all(|p| p == [255, 255, 255, 0]),
                "GPU decoded texels across chunk/partial-block boundaries"
            );
            assert!(resources.image_staging_bytes() <= 8 * 1024 * 1024);
            rows.push(serde_json::json!({"case":"compressed-multi-chunk","width":extent.width,"height":extent.height,"uploaded_bytes":blocks.len(),"chunks":chunks,"hardware_texels_exact":true,"actual_image_allocation":texture.allocation_bytes}));
        }
        drop(resources);
        assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
        Ok(serde_json::json!(rows))
    }
    fn world_case(
        ctx: &VulkanContext,
        budget: &Arc<VulkanBudget>,
        out: &Path,
    ) -> Result<Vec<Vec<u8>>> {
        let rgba = fixture(64, 64, 0);
        let encoded = png(64, 64, &rgba);
        let path = "exact-bc7.png";
        let mut scene = ingetrazo_core::scene::Scene::new();
        scene.embedded_textures.insert(path.into(), encoded);
        let ids = [
            DVec3::new(-1., -1., 0.1),
            DVec3::new(1., -1., 0.1),
            DVec3::new(1., 1., 0.1),
            DVec3::new(-1., 1., 0.1),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        let face = scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        scene.mesh.faces[face].attrs.insert(
            "texture".into(),
            serde_json::json!({"path":path,"uvw":[0.5,0.,0.,0.5,0.,0.5,0.,0.5]}),
        );
        scene
            .image_planes
            .push(ingetrazo_core::image_plane::ImagePlane::new(
                path,
                DVec3::new(-1.5, -1.5, 0.),
                DVec3::new(3., 0., 0.),
                DVec3::new(0., 3., 0.),
            ));
        let mut camera = ingetrazo_core::camera::OrbitCamera::default();
        camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
        camera.target = DVec3::ZERO;
        camera.distance = 4.;
        camera.perspective = false;
        camera.aspect = 1.;
        let mut style = ingetrazo_render::style::StyleConfig::default();
        style.show_textures = true;
        style.show_edges = false;
        style.show_profiles = false;
        style.show_sky = false;
        let fence = unsafe {
            ctx.device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?
        };
        let command = unsafe {
            ctx.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(ctx.command_pool)
                    .command_buffer_count(1),
            )?
        }[0];
        let mut world = World::new(ctx, budget.clone())?;
        world.resize_offscreen(
            ctx,
            vk::Extent2D {
                width: 256,
                height: 256,
            },
        )?;
        let mut frames = vec![];
        for step in 0..4u64 {
            if step == 1 {
                camera.target = DVec3::new(0.13, -0.17, 0.);
                camera.distance = 3.14;
            }
            if step == 2 {
                scene.image_planes.clear();
                scene.version += 1;
            }
            if step == 3 {
                style.visual_style = ingetrazo_render::style::VisualStyle::XRay;
            }
            world.set_scene(ctx, &scene, &camera, &style, fence, step, step + 1)?;
            let before = (
                world.texture_stats().1.hashes,
                world.texture_stats().1.decodes,
                world.texture_stats().1.uploads,
                world.texture_stats().1.compression_attempts,
            );
            world.set_scene(ctx, &scene, &camera, &style, fence, step, step + 1)?;
            let stats = &world.texture_stats().1;
            assert_eq!(
                before,
                (
                    stats.hashes,
                    stats.decodes,
                    stats.uploads,
                    stats.compression_attempts
                )
            );
            assert_eq!(stats.uploads, 1, "Face/plane share one cache image");
            assert_eq!(stats.bc7_uploads, u64::from(ctx.texture_bc_enabled));
            let frame = draw(ctx, &mut world, command, fence, step + 1)?;
            assert!(frame
                .chunks_exact(4)
                .any(|p| p[0] != p[1] || p[1] != p[2] || p[0] < 200));
            image::save_buffer(
                out.join(format!("world-{step}.png")),
                &frame,
                256,
                256,
                image::ColorType::Rgba8,
            )?;
            frames.push(frame);
        }
        drop(world);
        assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
        unsafe {
            ctx.device
                .free_command_buffers(ctx.command_pool, &[command]);
            ctx.device.destroy_fence(fence, None);
        }
        Ok(frames)
    }
    pub fn main() -> Result<()> {
        let args = std::env::args().collect::<Vec<_>>();
        let out = Path::new(args.get(1).ok_or("Output directory required")?);
        let request = if args.iter().any(|s| s == "--software-fixture") {
            DeviceRequest::software_fixture()
        } else {
            DeviceRequest::nvidia_hardware()
        };
        let mut records = vec![];
        let mut reference = None;
        for bc in [false, true] {
            let ctx = if bc {
                VulkanContext::new_headless(&request)?
            } else {
                VulkanContext::new_headless_rgba_fixture(&request)?
            };
            if bc {
                assert!(ctx.texture_bc_enabled, "Strict requested BC device");
            }
            let properties = unsafe {
                ctx.instance
                    .get_physical_device_properties(ctx.physical_device)
            };
            let name = unsafe { std::ffi::CStr::from_ptr(properties.device_name.as_ptr()) }
                .to_string_lossy();
            let budget = Arc::new(VulkanBudget::new(&ctx.instance, ctx.physical_device)?);
            budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
            let directory = out.join(if bc { "bc7" } else { "rgba" });
            std::fs::create_dir_all(&directory)?;
            let cases = cache_cases(&ctx, &budget, &directory)?;
            let frames = world_case(&ctx, &budget, &directory)?;
            if let Some(old) = reference.as_ref() {
                assert_eq!(
                    &frames, old,
                    "Every pixel of all actual production draws must match RGBA"
                );
            } else {
                reference = Some(frames);
            }
            records.push(serde_json::json!({"bc_enabled":ctx.texture_bc_enabled,"device":name,"vendor_id":properties.vendor_id,"device_id":properties.device_id,"driver":properties.driver_version,"cases":cases,"world_frames":4,"world_pixels_exact":true,"ledger_final":budget.ledger.lock().unwrap().total_committed()}));
        }
        let report = serde_json::json!({"runs":records,"scope":"Exact compression optimization against original RGBA upload; hardware decode and actual shared face/reference production draws, not a claim of all renderer parity"});
        std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }
}
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    platform::main()
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!("Native Vulkan proof requires Linux/Windows");
    std::process::exit(2);
}
