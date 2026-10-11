#[cfg(any(target_os = "linux", target_os = "windows"))]
mod platform {
    use ash::vk;
    use glam::DVec3;
    use ingetrazo_gui::native_vulkan::{
        budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
    };
    use ingetrazo_tools::base::FaceSweepPreview;
    use std::{path::PathBuf, sync::Arc};
    fn vec(v: &serde_json::Value) -> DVec3 {
        DVec3::new(
            v[0].as_f64().unwrap(),
            v[1].as_f64().unwrap(),
            v[2].as_f64().unwrap(),
        )
    }
    fn capture(
        context: &VulkanContext,
        world: &mut World,
        command: vk::CommandBuffer,
        fence: vk::Fence,
        serial: u64,
        path: &std::path::Path,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        unsafe {
            context
                .device
                .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())?;
            context
                .device
                .begin_command_buffer(command, &vk::CommandBufferBeginInfo::default())?;
        }
        world.record(command, serial)?;
        unsafe {
            context.device.end_command_buffer(command)?;
            context.device.reset_fences(&[fence])?;
            context.device.queue_submit(
                context.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&[command])],
                fence,
            )?;
        }
        context.wait_transfer(fence)?;
        let pixels = world.readback(context)?;
        image::save_buffer(path, &pixels, 800, 600, image::ColorType::Rgba8)?;
        Ok(pixels)
    }
    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let args = std::env::args().collect::<Vec<_>>();
        let root = PathBuf::from(&args[1]);
        let out = PathBuf::from(&args[2]);
        std::fs::create_dir_all(&out)?;
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("original.json"))?)?;
        let context =
            VulkanContext::new_headless(&if args.iter().any(|a| a == "--software-fixture") {
                DeviceRequest::software_fixture()
            } else {
                DeviceRequest::nvidia_hardware()
            })?;
        let budget = Arc::new(VulkanBudget::new(
            &context.instance,
            context.physical_device,
        )?);
        budget.ledger.lock().unwrap().total_limit = Some(256 * 1024 * 1024);
        let fence = unsafe {
            context.device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?
        };
        let command = unsafe {
            context.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(context.command_pool)
                    .command_buffer_count(1),
            )?
        }[0];
        let mut rows = Vec::new();
        let mut serial = 0u64;
        for case in original["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let scene =
                ingetrazo_formats::igz::load_igz(&root.join(case["igz"].as_str().unwrap()))?;
            let c = &case["camera"];
            let mut camera = ingetrazo_core::camera::OrbitCamera::default();
            camera.target = vec(&c["target"]);
            camera.distance = c["distance"].as_f64().unwrap();
            camera.yaw = c["yaw"].as_f64().unwrap();
            camera.pitch = c["pitch"].as_f64().unwrap();
            camera.fov_deg = c["fov_deg"].as_f64().unwrap();
            camera.perspective = false;
            camera.aspect = 800. / 600.;
            let mut style = ingetrazo_render::style::StyleConfig::default();
            style.background_color = [1.; 4];
            style.show_sky = false;
            style.show_edges = true;
            style.show_profiles = false;
            let face = case["face"].as_u64().unwrap() as usize;
            let mut descriptor = FaceSweepPreview {
                face,
                face_identity: scene.mesh.faces[face].history_id,
                mesh_revision: scene.version,
                anchor: vec(&case["anchor"]),
                normal: vec(&case["normal"]),
                offset: DVec3::ZERO,
                attached: case["attached"].as_bool().unwrap(),
                prism_cap: case["prism_cap"].as_bool().unwrap(),
                keep_base: case["keep_base"].as_bool().unwrap(),
            };
            let mut world = World::new_with_sample_count_for_probe(&context, budget.clone(), 4)?;
            world.resize_offscreen(
                &context,
                vk::Extent2D {
                    width: 800,
                    height: 600,
                },
            )?;
            let mut frames = Vec::new();
            for frame in case["frames"].as_array().unwrap() {
                let phase = frame["phase"].as_str().unwrap();
                descriptor.offset = descriptor.normal * frame["distance"].as_f64().unwrap();
                world.set_face_sweep_scene_for_probe(
                    &context,
                    &scene,
                    &camera,
                    &style,
                    Some(descriptor),
                    1,
                    fence,
                    serial,
                )?;
                let before = (world.uploads, world.face_sweep_stats_for_probe());
                let charge = budget.ledger.lock().unwrap().total_committed();
                world.set_face_sweep_scene_for_probe(
                    &context,
                    &scene,
                    &camera,
                    &style,
                    Some(descriptor),
                    1,
                    fence,
                    serial,
                )?;
                assert_eq!(before.0, world.uploads);
                let after = world.face_sweep_stats_for_probe();
                assert_eq!(before.1 .0, after.0);
                assert_eq!(before.1 .2.builds, after.2.builds);
                assert_eq!(before.1 .2.uploads, after.2.uploads);
                assert_eq!(charge, budget.ledger.lock().unwrap().total_committed());
                serial += 1;
                let pixels = capture(
                    &context,
                    &mut world,
                    command,
                    fence,
                    serial,
                    &out.join(format!("{name}-{phase}.png")),
                )?;
                if name == "reversed-door" && phase == "preview" {
                    let mut bad = descriptor;
                    bad.face_identity = bad.face_identity.wrapping_add(1);
                    assert!(world
                        .set_face_sweep_scene_for_probe(
                            &context,
                            &scene,
                            &camera,
                            &style,
                            Some(bad),
                            1,
                            fence,
                            serial
                        )
                        .is_err());
                    budget.ledger.lock().unwrap().total_limit = Some(charge);
                    bad = descriptor;
                    bad.offset *= 1.25;
                    assert!(world
                        .set_face_sweep_scene_for_probe(
                            &context,
                            &scene,
                            &camera,
                            &style,
                            Some(bad),
                            1,
                            fence,
                            serial
                        )
                        .is_err());
                    budget.ledger.lock().unwrap().total_limit = Some(256 * 1024 * 1024);
                    assert_eq!(charge, budget.ledger.lock().unwrap().total_committed());
                    serial += 1;
                    assert_eq!(
                        pixels,
                        capture(
                            &context,
                            &mut world,
                            command,
                            fence,
                            serial,
                            &out.join("refused.png")
                        )?
                    );
                }
                frames.push(serde_json::json!({"phase":phase,"world_uploads":world.uploads,"preview_uploads":after.0,"preview_bytes":after.1,"texture_builds":after.2.builds,"texture_uploads":after.2.uploads,"sampled_entries":after.2.sampled_entries,"committed":charge,"warm_zero_upload":true}));
            }
            rows.push(serde_json::json!({"name":name,"frames":frames}));
            drop(world);
            assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
        }
        unsafe {
            context
                .device
                .free_command_buffers(context.command_pool, &[command]);
            context.device.destroy_fence(fence, None);
        }
        std::fs::write(
            out.join("native.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"cases":rows,"final_ledger":0,"scope":"actual source-sweep production command consumer; app lifecycle separate"}),
            )?,
        )?;
        Ok(())
    }
}
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    platform::main()
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!("Native Vulkan fixture; macOS uses Metal.");
    std::process::exit(2);
}
