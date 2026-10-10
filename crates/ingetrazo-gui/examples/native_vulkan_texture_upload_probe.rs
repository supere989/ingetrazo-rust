#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Actual production optimal image upload/readback, bounded staging and reuse.
use ash::vk;
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget,
    headless::DeviceRequest,
    resources::{Resources, TextureImage},
    VulkanContext,
};
use std::{sync::Arc, time::Instant};
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = if std::env::args().any(|s| s == "--software-fixture") {
        DeviceRequest::software_fixture()
    } else {
        DeviceRequest::nvidia_hardware()
    };
    let context = VulkanContext::new_headless(&request)?;
    let budget = Arc::new(VulkanBudget::new(
        &context.instance,
        context.physical_device,
    )?);
    budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
    let mut resources = Resources::new(&context, budget.clone());
    let extent = vk::Extent2D {
        width: 2048,
        height: 2176,
    };
    let bytes = (0..17 * 1024 * 1024)
        .map(|i| (i % 251) as u8)
        .collect::<Vec<_>>();
    let start = Instant::now();
    let image = TextureImage::rgba8(&context, &mut resources, extent, &bytes, false, true)?;
    let upload_ms = start.elapsed().as_secs_f64() * 1000.;
    let result = resources.readback_texture_mip(&context, &image, 0)?;
    assert!(result == bytes, "Actual17MiB GPU image readback differs");
    assert_eq!(resources.stats.image_bytes, 17 * 1024 * 1024);
    assert!(resources.image_staging_bytes() <= 8 * 1024 * 1024);
    drop(image);
    assert_eq!(resources.stats.image_staging_maps,1,"Large upload mapped staging per chunk");
    assert_eq!(resources.stats.image_staging_unmaps,0,"Staging prematurely unmapped");
    let allocations = resources.stats.staging_allocations;
    let commands = resources.stats.transfer_command_allocations;
    let fences = resources.stats.transfer_fence_creations;
    let extent = vk::Extent2D {
        width: 7,
        height: 5,
    };
    let color = [37, 93, 157, 128];
    let bytes = color.repeat(35);
    let mip = TextureImage::rgba8(&context, &mut resources, extent, &bytes, true, true)?;
    for level in 0..mip.mip_levels {
        let actual = resources.readback_texture_mip(&context, &mip, level)?;
        assert!(
            actual.chunks_exact(4).all(|p| p == color),
            "GPU generated constant mip changed encoded RGBA"
        );
    }
    assert_eq!(
        resources.stats.staging_allocations, allocations,
        "Repeated images allocated new staging"
    );
    assert_eq!(resources.stats.image_staging_maps,1,"Repeated image remapped retained staging");
    assert_eq!(resources.stats.transfer_command_allocations, commands);
    assert_eq!(resources.stats.transfer_fence_creations, fences);
    assert!(resources
        .readback_texture_mip(&context, &mip, mip.mip_levels)
        .is_err());
    assert!(TextureImage::rgba8(
        &context,
        &mut resources,
        extent,
        &bytes[..bytes.len() - 1],
        true,
        true
    )
    .is_err());
    let before = budget.ledger.lock().unwrap().total_committed();
    {
        budget.ledger.lock().unwrap().total_limit = Some(before);
    }
    assert!(
        TextureImage::rgba8(&context, &mut resources, extent, &bytes, true, true).is_err(),
        "Actual budget denial did not reject new image"
    );
    {
        let mut ledger = budget.ledger.lock().unwrap();
        assert_eq!(ledger.total_committed(), before);
        assert!(ledger.heaps.iter().all(|h| h.reserved == 0));
        ledger.total_limit = Some(128 * 1024 * 1024);
    }
    assert!(
        resources.readback_texture_mip(&context, &mip, 0)? == bytes,
        "Failed replacement changed old GPU image"
    );
    // Actual image admission succeeds, but a newly needed staging buffer is
    // denied. Query requirements rather than guessing allocator granularity.
    resources.release_image_staging();
    let admitted = budget.ledger.lock().unwrap().total_committed();
    let temp = unsafe {context.device.create_image(&vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D).format(vk::Format::R8G8B8A8_UNORM)
        .extent(vk::Extent3D{width:7,height:5,depth:1}).mip_levels(3).array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1).tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST|vk::ImageUsageFlags::TRANSFER_SRC|vk::ImageUsageFlags::SAMPLED),None)}?;
    let requirements = unsafe {context.device.get_image_memory_requirements(temp)};
    unsafe {context.device.destroy_image(temp,None)};
    budget.ledger.lock().unwrap().total_limit=Some(admitted+requirements.size);
    assert!(TextureImage::rgba8(&context,&mut resources,extent,&bytes,true,true).is_err(),"Image admitted without staging denial");
    {let mut ledger=budget.ledger.lock().unwrap();assert_eq!(ledger.total_committed(),admitted);assert!(ledger.heaps.iter().all(|h|h.reserved==0));ledger.total_limit=Some(128*1024*1024);}
    assert!(resources.readback_texture_mip(&context,&mip,0)?==bytes,"Staging denial changed old GPU image");
    let retry=TextureImage::rgba8(&context,&mut resources,extent,&bytes,true,true)?;
    assert!(resources.readback_texture_mip(&context,&retry,0)?==bytes);drop(retry);
    drop(mip);
    resources.release_image_staging();
    assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    assert_eq!(resources.stats.image_staging_maps,resources.stats.image_staging_unmaps,"Retained staging mapping leaked");
    eprintln!("PASS17MiB optimal GPU texture/readback upload{upload_ms:.3}ms; constant7x5 mip chain; persistentstaging/command/fence; actualbudgetdenial oldimage preserved; ledgerzero; {:?}",resources.stats);
    Ok(())
}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
