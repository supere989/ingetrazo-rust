//! Software regression reuses independent e2110fe contour geometry protocol and production World.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let input = std::path::Path::new(&args[0]);
    let output = std::path::Path::new(&args[1]);
    let context=VulkanContext::new_headless(&DeviceRequest::software_fixture())?;
    let budget = Arc::new(VulkanBudget::new(
        &context.instance,
        context.physical_device,
    )?);
    budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
    let mut scene = ingetrazo_formats::igz::load_igz(input)?;
    scene.show_section_fill = true;
    scene.show_section_cuts = true;
    let name = input.file_stem().unwrap().to_string_lossy();
    let mut camera = ingetrazo_core::camera::OrbitCamera::default();
    camera.set_standard_view(if name.ends_with("iso") {
        ingetrazo_core::camera::StandardView::Iso
    } else if name.ends_with("front") {
        ingetrazo_core::camera::StandardView::Front
    } else {
        ingetrazo_core::camera::StandardView::Top
    });
    camera.target = DVec3::new(0.5, 0.5, 1.5);
    camera.distance = 5.;
    camera.perspective = false;
    camera.aspect = 800. / 600.;
    let mut style = ingetrazo_render::style::StyleConfig::default();
    style.show_edges = false;
    style.show_profiles = false;
    style.show_sky = false;
    style.show_grid = false;
    style.section_fill_color = [1., 0., 1., 1.];
    let mut world = World::new(&context, budget.clone())?;
    world.resize_offscreen(
        &context,
        vk::Extent2D {
            width: 800,
            height: 600,
        },
    )?;
    let device = &context.device;
    let fence = unsafe {
        device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )
    }?;
    world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
    world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
    assert_eq!(world.uploads, 1);
    let commands = unsafe {
        device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(context.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )
    }?;
    unsafe {
        device.begin_command_buffer(commands[0], &vk::CommandBufferBeginInfo::default())?;
    }
    world.record(commands[0], 1)?;
    unsafe {
        device.end_command_buffer(commands[0])?;
        device.reset_fences(&[fence])?;
        device.queue_submit(
            context.graphics_queue,
            &[vk::SubmitInfo::default().command_buffers(&commands)],
            fence,
        )?;
    }
    context.wait_transfer(fence)?;
    let image = world.readback(&context)?;
    assert_eq!(image.len(), 800 * 600 * 4);
    image::save_buffer(output, &image, 800, 600, image::ColorType::Rgba8)?;
    unsafe {
        device.free_command_buffers(context.command_pool, &commands);
        device.destroy_fence(fence, None);
    }
    drop(world);
    assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    println!(
        "PASS softwareVulkan contour originalIGZ={} GPUreadback={} uploads1/2calls ownerledger0",
        input.display(),
        output.display()
    );
    Ok(())
}
