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
    // Unpublished page range writes preserve neighboring bytes and reject invalid ranges.
    let mut page_expected=vec![11u8;20*1024*1024];
    let page=resources.allocate_buffer(page_expected.len()as u64,vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC)?;
    // SAFETY: this fresh page is exclusively owned by this probe and never published.
    unsafe{resources.upload_fresh_range(&context,page,0,&page_expected)?;}
    let patch=vec![73u8;17*1024*1024];
    unsafe{resources.upload_fresh_range(&context,page,16,&patch)?;}
    page_expected[16..16+patch.len()].copy_from_slice(&patch);
    assert_eq!(resources.readback_buffer(&context,page)?,page_expected);
    let before_calls=resources.stats.upload_calls;
    let before_allocations=resources.stats.buffer_allocations;
    for (offset,data) in [(1u64,&[0u8;4][..]),(u64::MAX-3,&[0u8;8][..]),(20*1024*1024,&[0u8;4][..]),(0,&[0u8;3][..])]{
      assert!(unsafe{resources.upload_fresh_range(&context,page,offset,data)}.is_err());
    }
    assert_eq!(resources.stats.upload_calls,before_calls);
    assert_eq!(resources.stats.buffer_allocations,before_allocations);
    resources.retire(page)?;
    assert!(unsafe{resources.upload_fresh_range(&context,page,0,&[1u8;4])}.is_err());
    let page_done=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
    assert_eq!(resources.collect(page_done,0)?,1);
    unsafe{context.device.destroy_fence(page_done,None)};
    // Interleave buffer and batch writes with the retained image staging owner.
    let allocations_before = resources.stats.staging_allocations;
    let maps_before = resources.stats.upload_staging_maps;
    let buffer_data = (0..17*1024*1024).map(|i| (i%197) as u8).collect::<Vec<_>>();
    let usage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC;
    let large = resources.upload(&context,&buffer_data,usage,true)?;
    assert_eq!(resources.readback_buffer(&context,large)?,buffer_data);
    let small = [93u8;1024];
    let batch = resources.upload_many(&context,&[(&small,usage),(&small[..512],usage)])?;
    assert_eq!(resources.readback_buffer(&context,batch[0])?,small);
    assert_eq!(resources.readback_buffer(&context,batch[1])?,small[..512]);
    assert_eq!(resources.readback_texture_mip(&context,&image,0)?,bytes,"Buffer staging reuse corrupted original image");
    assert_eq!(resources.stats.staging_allocations,allocations_before);
    assert_eq!(resources.stats.upload_staging_maps,maps_before);
    assert_eq!(resources.stats.transfer_command_allocations,1);
    assert_eq!(resources.stats.transfer_fence_creations,1);
    let done=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
    resources.retire(large)?;
    for id in batch {resources.retire(id)?;}
    assert_eq!(resources.collect(done,0)?,3);
    unsafe{context.device.destroy_fence(done,None)};
    drop(image);
    assert_eq!(resources.stats.upload_staging_maps,1,"Large upload mapped staging per chunk");
    assert_eq!(resources.stats.upload_staging_unmaps,0,"Staging prematurely unmapped");
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
    assert_eq!(resources.stats.upload_staging_maps,1,"Repeated image remapped retained staging");
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
    assert_eq!(resources.stats.upload_staging_maps,resources.stats.upload_staging_unmaps,"Retained staging mapping leaked");
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
