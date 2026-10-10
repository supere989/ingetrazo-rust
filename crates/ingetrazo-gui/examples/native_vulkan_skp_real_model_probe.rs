//! Diagnostic production World rendering of complete native SKP imports.
//! Annotation UI overlays are not emitted by World and remain a required gap.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
};
use std::sync::Arc;
fn capture(
    context: &VulkanContext,
    world: &mut World,
    command: vk::CommandBuffer,
    fence: vk::Fence,
    serial: u64,
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
    assert_eq!(pixels.len(), 800 * 600 * 4);
    Ok(pixels)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let input = std::path::PathBuf::from(args.get(1).ok_or("input directory required")?);
    let output = std::path::PathBuf::from(args.get(2).ok_or("output directory required")?);
    std::fs::create_dir_all(&output)?;
    let protocol: serde_json::Value =
        serde_json::from_slice(&std::fs::read(input.join("protocol.json"))?)?;
    let cases = protocol["cases"].as_array().ok_or("missing cases")?;
    let names = cases
        .iter()
        .map(|c| c["name"].as_str().ok_or("case name"))
        .collect::<Result<Vec<_>, _>>()?;
    if names
        != [
            "gondola_v20-iso",
            "gondola_v20-asymmetric",
            "capilla_quiroz_v17-iso",
            "capilla_quiroz_v17-asymmetric",
        ]
    {
        return Err("mandatory four-case corpus required".into());
    }
    let request = if args.iter().any(|a| a == "--software-fixture") {
        DeviceRequest::software_fixture()
    } else {
        DeviceRequest::nvidia_hardware()
    };
    let context = VulkanContext::new_headless(&request)?;
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
    let commands = unsafe {
        context.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(context.command_pool)
                .command_buffer_count(1),
        )?
    };
    let mut rows = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let model = case["model"].as_str().ok_or("model name")?;
        let mut scene = ingetrazo_formats::igz::load_igz(&input.join(format!("{model}.igz")))?;
        let c = &case["camera"];
        let number = |k: &str| c[k].as_f64().ok_or("missing numeric camera field");
        let mut camera = ingetrazo_core::camera::OrbitCamera::default();
        camera.target = DVec3::new(
            c["target"][0].as_f64().ok_or("target")?,
            c["target"][1].as_f64().ok_or("target")?,
            c["target"][2].as_f64().ok_or("target")?,
        );
        camera.distance = number("distance")?;
        camera.yaw = number("yaw")?;
        camera.pitch = number("pitch")?;
        camera.fov_deg = number("fov_deg")?;
        camera.perspective = false;
        camera.aspect = 800. / 600.;
        let mut style = ingetrazo_render::style::StyleConfig::default();
        style.background_color = [1.; 4];
        style.show_sky = false;
        style.show_edges = true;
        style.show_profiles = false;
        style.show_textures = false;
        let mut world = World::new(&context, budget.clone())?;
        world.resize_offscreen(
            &context,
            vk::Extent2D {
                width: 800,
                height: 600,
            },
        )?;
        world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
        let cold = world.texture_stats();
        let cold = (
            cold.0.uploads,
            cold.1.hashes,
            cold.1.decodes,
            cold.1.uploads,
            cold.0.average_decodes,
            cold.0.average_hits,
            cold.0.average_entries,
        );
        if name.starts_with("capilla_quiroz_v17") {
            assert!(
                cold.4 > 0 && cold.6 > 0,
                "real chapel textures must exercise cold source-average decoding"
            );
        }
        if scene.image_planes.len() == 0 {
            assert_eq!(
                (cold.2, cold.3),
                (0, 0),
                "shaded surface means must not decode/upload sampled GPU images"
            );
        }
        for _ in 0..3 {
            world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
            assert_eq!(world.uploads, 1);
            let warm = world.texture_stats();
            assert_eq!(
                cold,
                (
                    warm.0.uploads,
                    warm.1.hashes,
                    warm.1.decodes,
                    warm.1.uploads,
                    warm.0.average_decodes,
                    warm.0.average_hits,
                    warm.0.average_entries
                ),
                "warm scene must avoid hashes, decoding, uploads and mean lookups"
            );
        }
        let pixels = capture(&context, &mut world, commands[0], fence, 1)?;
        image::save_buffer(
            output.join(format!("{name}-native-vulkan.png")),
            &pixels,
            800,
            600,
            image::ColorType::Rgba8,
        )?;
        let mut replacement_control = None;
        if name.starts_with("capilla_quiroz_v17") {
            let uploads = world.uploads;
            let entries = world.texture_stats().0.average_entries;
            let originals = std::mem::take(&mut scene.embedded_textures);
            assert!(!originals.is_empty());
            scene.embedded_textures = originals
                .keys()
                .map(|key| (key.clone(), vec![0u8]))
                .collect();
            let refused = world.set_scene(&context, &scene, &camera, &style, fence, 1, 2);
            scene.embedded_textures = originals;
            let error =
                refused.expect_err("invalid replacement texture must refuse before publication");
            assert_eq!(
                world.uploads, uploads,
                "failed replacement must not publish scene revision"
            );
            assert_eq!(
                world.texture_stats().0.average_entries,
                entries,
                "failed replacement must preserve old mean entries"
            );
            let after_failure = capture(&context, &mut world, commands[0], fence, 2)?;
            assert_eq!(
                after_failure, pixels,
                "failed replacement must preserve complete previous GPU image"
            );
            world.set_scene(&context, &scene, &camera, &style, fence, 2, 2)?;
            let retry = capture(&context, &mut world, commands[0], fence, 3)?;
            assert_eq!(
                retry, pixels,
                "retry after refusal must restore exact full image"
            );
            let average_decodes = world.texture_stats().0.average_decodes;
            style.show_textures = true;
            world.set_scene(&context, &scene, &camera, &style, fence, 3, 2)?;
            let sampled = capture(&context, &mut world, commands[0], fence, 4)?;
            assert_ne!(
                sampled, pixels,
                "sampled real textures must differ from their constant means"
            );
            let sampled_stats = world.texture_stats();
            assert!(sampled_stats.1.decodes > 0 && sampled_stats.1.uploads > 0);
            assert_eq!(
                sampled_stats.0.average_decodes, average_decodes,
                "sampled switch must not compute means"
            );
            let sampled_counts = (
                sampled_stats.1.hashes,
                sampled_stats.1.decodes,
                sampled_stats.1.uploads,
                sampled_stats.0.average_decodes,
            );
            style.visual_style = ingetrazo_render::style::VisualStyle::Monochrome;
            style.show_textures = false;
            world.set_scene(&context, &scene, &camera, &style, fence, 4, 2)?;
            let monochrome = capture(&context, &mut world, commands[0], fence, 5)?;
            assert_ne!(
                monochrome, sampled,
                "monochrome must not reuse sampled texture drawing"
            );
            let mono_stats = world.texture_stats();
            assert_eq!(
                sampled_counts,
                (
                    mono_stats.1.hashes,
                    mono_stats.1.decodes,
                    mono_stats.1.uploads,
                    mono_stats.0.average_decodes
                ),
                "monochrome must not hash, decode or upload surface textures"
            );
            style.visual_style = ingetrazo_render::style::VisualStyle::ShadedWithTextures;
            world.set_scene(&context, &scene, &camera, &style, fence, 5, 2)?;
            let returned = capture(&context, &mut world, commands[0], fence, 6)?;
            assert_eq!(
                returned, pixels,
                "returning to average mode must reproduce initial full frame"
            );
            replacement_control = Some(
                serde_json::json!({"error":error,"old_frame_exact":true,"old_entries_preserved":true,"retry_exact":true,"sampled_mode_exercised":true,"monochrome_no_surface_texture_work":true,"style_roundtrip_exact":true}),
            );
        }
        rows.push(serde_json::json!({"name":name,"geometry_uploads":world.uploads,"warm_uploads":0,"warm_frames":3,"replacement_control":replacement_control,"cold_texture_stats":{"geometry_uploads":cold.0,"hashes":cold.1,"sampled_decodes":cold.2,"sampled_uploads":cold.3,"average_decodes":cold.4,"average_hits":cold.5,"average_entries":cold.6},"hard_edge_bytes":world.edge_geometry_bytes(),"dimensions":scene.dimensions.len(),"text_labels":scene.text_labels.len(),"scope":"production native World; annotation overlays not rendered"}));
        drop(world);
        assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    }
    unsafe {
        context
            .device
            .free_command_buffers(context.command_pool, &commands);
        context.device.destroy_fence(fence, None);
    }
    std::fs::write(
        output.join("native-world-results.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"cases":rows,"ledger_final":0,"scope":"complete imported scene input, World-only drawing; strict pixels checked separately"}),
        )?,
    )?;
    Ok(())
}
