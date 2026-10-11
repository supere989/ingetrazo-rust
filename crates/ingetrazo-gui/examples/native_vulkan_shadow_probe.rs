// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ash::vk;
    use ingetrazo_gui::native_vulkan::{
        budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
    };
    use std::sync::Arc;
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        return Err("Usage: native_vulkan_shadow_probe INPUT.igz OUT [--software-fixture]".into());
    }
    let reference = if let Some(i) = args.iter().position(|s| s == "--reference") {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&args[i + 1])?)?;
        let name = args.get(i + 2).ok_or("Missing reference case")?;
        Some(
            v["cases"]
                .as_array()
                .ok_or("Missing cases")?
                .iter()
                .find(|c| c["name"].as_str() == Some(name))
                .ok_or("Unknown reference case")?
                .clone(),
        )
    } else {
        None
    };
    let scene = ingetrazo_formats::igz::load_igz(std::path::Path::new(&args[1]))?;
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out)?;
    let request = if args.iter().any(|s| s == "--software-fixture") {
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
        )
    }?;
    let command = unsafe {
        context.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(context.command_pool)
                .command_buffer_count(1),
        )
    }?[0];
    let mut world = if reference.is_some() {
        World::new_with_sample_count_for_probe(&context, budget.clone(), 1)?
    } else {
        World::new(&context, budget.clone())?
    };
    world.set_shadow_readback_for_probe()?;
    world.resize_offscreen(
        &context,
        vk::Extent2D {
            width: 800,
            height: 600,
        },
    )?;
    let mut camera = scene.camera.clone();
    camera.aspect = 800. / 600.;
    let mut style = ingetrazo_render::style::StyleConfig::default();
    if args.iter().any(|s| s == "--hidden-line") {
        style.visual_style = ingetrazo_render::style::VisualStyle::HiddenLine;
    }
    if args.iter().any(|s| s == "--monochrome") {
        style.visual_style = ingetrazo_render::style::VisualStyle::Monochrome;
    }
    style.show_edges = false;
    style.show_profiles = false;
    style.background_color = [1.; 4];
    let mut sun = [0.6, 0., 0.8];
    if let Some(r) = &reference {
        assert_eq!(r["samples"].as_u64(), Some(0));
        let c = &r["camera"];
        camera.target = glam::DVec3::new(
            c["target"][0].as_f64().ok_or("target")?,
            c["target"][1].as_f64().ok_or("target")?,
            c["target"][2].as_f64().ok_or("target")?,
        );
        camera.distance = c["distance"].as_f64().ok_or("distance")?;
        camera.yaw = c["yaw"].as_f64().ok_or("yaw")?;
        camera.pitch = c["pitch"].as_f64().ok_or("pitch")?;
        camera.fov_deg = c["fov_deg"].as_f64().ok_or("fov")?;
        camera.perspective = c["perspective"].as_bool().ok_or("perspective")?;
        assert_eq!(c["width"].as_u64(), Some(800));
        assert_eq!(c["height"].as_u64(), Some(600));
        let st = &r["style"];
        style.visual_style = match st["face_mode"].as_str().ok_or("face mode")? {
            "textures" => ingetrazo_render::style::VisualStyle::ShadedWithTextures,
            "hidden_line" => ingetrazo_render::style::VisualStyle::HiddenLine,
            _ => return Err("Unadmitted reference style".into()),
        };
        style.show_sky = false;
        style.show_edges = false;
        style.show_profiles = false;
        for i in 0..3 {
            sun[i] = r["sun"][i].as_f64().ok_or("sun")?;
            style.background_color[i] = st["background"][i].as_f64().ok_or("background")? as f32;
            style.front_color[i] = st["front_color"][i].as_f64().ok_or("front")? as f32;
        }
    }
    let mut serial = 0;
    let render = |world: &mut World, serial: &mut u64| -> Result<Vec<u8>, String> {
        unsafe {
            context
                .device
                .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
                .map_err(|e| e.to_string())?;
            context
                .device
                .begin_command_buffer(command, &vk::CommandBufferBeginInfo::default())
                .map_err(|e| e.to_string())?;
        }
        world.record(command, *serial + 1)?;
        unsafe {
            context
                .device
                .end_command_buffer(command)
                .map_err(|e| e.to_string())?;
            context
                .device
                .reset_fences(&[fence])
                .map_err(|e| e.to_string())?;
            context
                .device
                .queue_submit(
                    context.graphics_queue,
                    &[vk::SubmitInfo::default().command_buffers(&[command])],
                    fence,
                )
                .map_err(|e| e.to_string())?;
        }
        context.wait_transfer(fence).map_err(|e| e.to_string())?;
        *serial += 1;
        world.readback(&context)
    };
    let ground = World::solar_ground_receiver_for_scene(&scene);
    world.set_scene_with_sun(
        &context,
        &scene,
        &camera,
        &style,
        fence,
        serial,
        1,
        Some(sun),
        0.55,
        ground,
    )?;
    let on = render(&mut world, &mut serial)?;
    image::save_buffer(out.join("on.png"), &on, 800, 600, image::ColorType::Rgba8)?;
    let receiver_matrix = world.shadow_receiver_matrix_for_probe();
    let light_matrix = world
        .shadow_map_matrix_for_probe()
        .ok_or("Missing light matrix")?;
    if let Some(r) = &reference {
        for i in 0..16 {
            assert_eq!(
                receiver_matrix[i].to_bits(),
                (r["camera_vp_column_major"][i]
                    .as_f64()
                    .ok_or("camera matrix")? as f32)
                    .to_bits(),
                "camera matrix {i}"
            );
            assert_eq!(
                light_matrix[i].to_bits(),
                (r["light_vp_column_major"][i]
                    .as_f64()
                    .ok_or("light matrix")? as f32)
                    .to_bits(),
                "light matrix {i}"
            );
        }
    }
    let depth = world.shadow_readback_for_probe(&context, fence, serial)?;
    image::save_buffer(
        out.join("depth.png"),
        &depth,
        2048,
        2048,
        image::ColorType::Rgba8,
    )?;
    let cold = world.shadow_stats_for_probe();
    let uploads = world.uploads;
    for _ in 0..3 {
        world.set_scene_with_sun(
            &context,
            &scene,
            &camera,
            &style,
            fence,
            serial,
            1,
            Some(sun),
            0.55,
            ground,
        )?;
        assert_eq!(render(&mut world, &mut serial)?, on);
        assert_eq!(world.shadow_stats_for_probe(), cold);
        assert_eq!(world.uploads, uploads);
    }
    let committed = budget.ledger.lock().unwrap().total_committed();
    budget.ledger.lock().unwrap().total_limit = Some(committed);
    let refusal = world
        .set_scene_with_sun(
            &context,
            &scene,
            &camera,
            &style,
            fence,
            serial,
            1,
            Some(sun),
            0.25,
            ground,
        )
        .unwrap_err();
    assert_eq!(world.shadow_stats_for_probe(), cold);
    assert_eq!(budget.ledger.lock().unwrap().total_committed(), committed);
    budget.ledger.lock().unwrap().total_limit = Some(256 * 1024 * 1024);
    assert_eq!(render(&mut world, &mut serial)?, on);
    world.set_scene_with_sun(
        &context, &scene, &camera, &style, fence, serial, 1, None, 0.55, ground,
    )?;
    let off = render(&mut world, &mut serial)?;
    image::save_buffer(out.join("off.png"), &off, 800, 600, image::ColorType::Rgba8)?;
    world.set_scene_with_sun(
        &context,
        &scene,
        &camera,
        &style,
        fence,
        serial,
        1,
        Some(sun),
        0.55,
        ground,
    )?;
    let restored = render(&mut world, &mut serial)?;
    assert_eq!(restored, on);
    image::save_buffer(
        out.join("restored.png"),
        &restored,
        800,
        600,
        image::ColorType::Rgba8,
    )?;
    let mut bitmap_direction_control = false;
    if reference.as_ref().is_some_and(|r|r["name"]=="billboard-bitmap") {
        let (_, assets)=world.texture_stats();
        let asset_work=(assets.hashes,assets.decodes,assets.uploads);
        let old_uploads=world.uploads;
        let old_renders=world.shadow_stats_for_probe().0;
        let changed=[-sun[1],sun[0],sun[2]];
        world.set_scene_with_sun(&context,&scene,&camera,&style,fence,serial,1,Some(changed),0.55,ground)?;
        let turned=render(&mut world,&mut serial)?;
        assert_ne!(turned,on,"sun turn must move the actual bitmap shadow");
        assert_eq!(world.uploads,old_uploads,"sun-only change reuploaded ordinary scene");
        let (_,assets)=world.texture_stats();
        assert_eq!((assets.hashes,assets.decodes,assets.uploads),asset_work,"sun-only change rebuilt images");
        assert_eq!(world.shadow_stats_for_probe().0,old_renders+1);
        image::save_buffer(out.join("sun-turned.png"),&turned,800,600,image::ColorType::Rgba8)?;
        world.set_scene_with_sun(&context,&scene,&camera,&style,fence,serial,1,Some(sun),0.55,ground)?;
        assert_eq!(render(&mut world,&mut serial)?,on,"sun restore must restore full frame");
        let map_renders=world.shadow_stats_for_probe().0;
        let mut moved=camera.clone();moved.yaw+=0.125;
        world.set_scene_with_sun(&context,&scene,&moved,&style,fence,serial,1,Some(sun),0.55,ground)?;
        let _=render(&mut world,&mut serial)?;
        assert_eq!(world.shadow_stats_for_probe().0,map_renders,"camera-only change rerendered map");
        assert_eq!(world.uploads,old_uploads);
        world.set_scene_with_sun(&context,&scene,&camera,&style,fence,serial,1,Some(sun),0.55,ground)?;
        assert_eq!(render(&mut world,&mut serial)?,on);
        bitmap_direction_control=true;
    }
    drop(world);
    let final_bytes = budget.ledger.lock().unwrap().total_committed();
    assert_eq!(final_bytes, 0);
    unsafe {
        context
            .device
            .free_command_buffers(context.command_pool, &[command]);
        context.device.destroy_fence(fence, None);
    }
    std::fs::write(
        out.join("result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"source_commit":option_env!("INGETRAZO_BUILD_SOURCE"),"bitmap_direction_control":bitmap_direction_control,"ground_receiver":ground,"cold":cold,"uploads":uploads,"refusal":refusal,"ledger_zero":final_bytes==0,"map_size":2048,"source_reference_pending":reference.is_none(),"reference_case":reference.as_ref().map(|r|r["name"].clone()),"receiver_matrix":receiver_matrix,"light_matrix":light_matrix,"samples":if reference.is_some(){1}else{4}}),
        )?,
    )?;
    Ok(())
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!("Vulkan shadow probe requires Linux/Windows; macOS uses native Metal.");
    std::process::exit(2);
}
