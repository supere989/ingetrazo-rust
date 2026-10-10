//! Render actual original alpha IGZ inputs through production Vulkan world commands.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let root = std::path::PathBuf::from(&args[1]);
    let out = std::path::PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out)?;
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("original-alpha-results.json"))?)?;
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
    budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
    let fence = unsafe {
        context.device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )
    }?;
    let commands = unsafe {
        context.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(context.command_pool)
                .command_buffer_count(1),
        )
    }?;
    let mut rows = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let scene = ingetrazo_formats::igz::load_igz(&root.join(format!("{name}.igz")))?;
        let mut camera = ingetrazo_core::camera::OrbitCamera::default();
        camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
        camera.target = DVec3::ZERO;
        camera.distance = 4.;
        camera.perspective = false;
        camera.aspect = 800. / 600.;
        let mut style = ingetrazo_render::style::StyleConfig::default();
        for i in 0..3 {
            style.background_color[i] = case["background"][i].as_f64().unwrap() as f32;
        }
        if let Some(mode)=case["style_mode"].as_str() {style.visual_style=match mode {"wireframe"=>ingetrazo_render::style::VisualStyle::Wireframe,"hidden_line"=>ingetrazo_render::style::VisualStyle::HiddenLine,"monochrome"=>ingetrazo_render::style::VisualStyle::Monochrome,"xray"=>ingetrazo_render::style::VisualStyle::XRay,_=>ingetrazo_render::style::VisualStyle::ShadedWithTextures};}
        if name.starts_with("style-") {for i in 0..3 {style.front_color[i]=case["style_front_color"][i].as_f64().unwrap()as f32;style.back_color[i]=case["style_back_color"][i].as_f64().unwrap()as f32;}}
        style.show_sky = false;
        style.show_edges = case["show_edges"].as_bool().unwrap_or(false);
        style.show_profiles = false;
        let mut world = World::new(&context, budget.clone())?;
        world.resize_offscreen(
            &context,
            vk::Extent2D {
                width: 800,
                height: 600,
            },
        )?;
        world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
        world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
        assert_eq!(world.uploads, 1);
        if name.starts_with("style-") {
            let committed=budget.ledger.lock().unwrap().total_committed();
            for mode in [ingetrazo_render::style::VisualStyle::Wireframe,ingetrazo_render::style::VisualStyle::XRay,ingetrazo_render::style::VisualStyle::HiddenLine,ingetrazo_render::style::VisualStyle::Monochrome] {
                let mut changed=style.clone();changed.visual_style=mode;changed.front_color=[0.3,0.2,0.1,1.];changed.back_color=[0.1,0.2,0.3,1.];
                world.set_scene(&context,&scene,&camera,&changed,fence,0,1)?;
                assert_eq!(world.uploads,1,"style-only change must reuse cached geometry");if !scene.show_section_cuts {assert_eq!(budget.ledger.lock().unwrap().total_committed(),committed);} // Cap visibility has separate legitimate allocation.
            }
            world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
        }
        unsafe {
            context
                .device
                .reset_command_buffer(commands[0], vk::CommandBufferResetFlags::empty())?;
            context
                .device
                .begin_command_buffer(commands[0], &vk::CommandBufferBeginInfo::default())?;
        }
        world.record(commands[0], 1)?;
        unsafe {
            context.device.end_command_buffer(commands[0])?;
            context.device.reset_fences(&[fence])?;
            context.device.queue_submit(
                context.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&commands)],
                fence,
            )?;
        }
        context.wait_transfer(fence)?;
        let image = world.readback(&context)?;
        image::save_buffer(
            out.join(format!("{name}.png")),
            &image,
            800,
            600,
            image::ColorType::Rgba8,
        )?;
        let offset = (300 * 800 + 400) * 4;
        rows.push(serde_json::json!({"case":name,"center_rgba":&image[offset..offset+4],"geometry_uploads":world.uploads,"hard_edge_bytes":world.edge_geometry_bytes(),"hard_edge_endpoint_count":world.edge_endpoint_count(),"opacity_runs":world.transparent_runs(),"paint_runs":world.paint_runs(),"ordinary_runs":world.ordinary_paint_runs(),"consolidated_opaque_count":world.consolidated_opaque_count(),"surface_bytes":world.transparent_surface_bytes(),"contour_metrics":format!("{:?}",world.contour_metrics())}));
        drop(world);
        assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    }
    unsafe {
        context
            .device
            .free_command_buffers(context.command_pool, &commands);
        context.device.destroy_fence(fence, None);
    }
    let result = serde_json::json!({"cases":rows,"scope":"Production World native GPU input/reference corpus render; no parity assertion in renderer fixture itself"});
    std::fs::write(
        out.join("native-alpha-results.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
