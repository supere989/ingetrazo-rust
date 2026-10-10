//! Production cache identity, actual descriptors and unpublished rollback.
use ash::vk;
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget, headless::DeviceRequest, resources::Resources,
    texture_cache::TextureCache, VulkanContext,
};
use std::{collections::HashSet, io::Cursor, sync::Arc};
fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let mut cache = TextureCache::diagnostic_readback();
    let pixels = vec![
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 0,
    ];
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(2, 2, pixels.clone()).unwrap())
        .write_to(&mut bytes, image::ImageFormat::Png)?;
    let asset = cache.identify(bytes.get_ref());
    let id = asset.id();
    let before = budget.ledger.lock().unwrap().total_committed();
    let candidate = cache.prepare(&context, &mut resources, &asset, 1024)?;
    assert!(!cache.contains(id));
    let retained_staging = resources.image_staging_bytes();
    drop(candidate);
    assert_eq!(
        budget.ledger.lock().unwrap().total_committed(),
        before + retained_staging,
        "Unpublished image leaked"
    );
    let candidate = cache.prepare(&context, &mut resources, &asset, 1024)?;
    let (view, sampler, hard) = candidate.sampled();
    assert!(!hard);
    // Referencing descriptors are constructed BEFORE image publication.
    let bindings = [vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
    let layout = unsafe {
        context.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
            None,
        )
    }?;
    let sizes = [vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
    }];
    let pool = unsafe {
        context.device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default()
                .max_sets(1)
                .pool_sizes(&sizes),
            None,
        )
    }?;
    let layouts = [layout];
    let sets = unsafe {
        context.device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(pool)
                .set_layouts(&layouts),
        )
    }?;
    let images = [vk::DescriptorImageInfo::default()
        .image_view(view)
        .sampler(sampler)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    unsafe {
        context.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(sets[0])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&images)],
            &[],
        )
    };
    cache.publish(candidate)?;
    assert!(
        cache.readback_for_probe(&context, &mut resources, id)? == pixels,
        "Published sampled GPU image differs"
    );
    let uploads = cache.stats.uploads;
    let decodes = cache.stats.decodes;
    let hashes = cache.stats.hashes;
    let candidate = cache.prepare(&context, &mut resources, &asset, 1024)?;
    assert_eq!(candidate.sampled().0, view);
    cache.publish(candidate)?;
    assert_eq!(cache.stats.uploads, uploads);
    assert_eq!(cache.stats.decodes, decodes);
    assert_eq!(cache.stats.hashes, hashes);
    // A different encoded identity cannot alias the first image even same name.
    let mut other = bytes.get_ref().clone();
    other.push(0);
    let other_asset = cache.identify(&other);
    assert_ne!(other_asset.id(), id);
    let charge = budget.ledger.lock().unwrap().total_committed();
    budget.ledger.lock().unwrap().total_limit = Some(charge);
    assert!(cache
        .prepare(&context, &mut resources, &other_asset, 1024)
        .is_err());
    assert!(!cache.contains(other_asset.id()));
    assert_eq!(cache.sampled(id)?.0, view);
    assert_eq!(budget.ledger.lock().unwrap().total_committed(), charge);
    budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
    let fence = unsafe {
        context
            .device
            .create_fence(&vk::FenceCreateInfo::default(), None)
    }?;
    cache.mark_used(id, 9)?;
    assert_eq!(
        cache.collect_unused(&context, &HashSet::new(), fence, 9)?,
        0
    );
    unsafe {
        context
            .device
            .queue_submit(context.graphics_queue, &[vk::SubmitInfo::default()], fence)?
    };
    context.wait_transfer(fence)?;
    // Descriptor references retired on same completed fence before image owner.
    unsafe {
        context.device.destroy_descriptor_pool(pool, None);
        context.device.destroy_descriptor_set_layout(layout, None)
    };
    assert_eq!(
        cache.collect_unused(&context, &HashSet::new(), fence, 8)?,
        0
    );
    assert_eq!(
        cache.collect_unused(&context, &HashSet::new(), fence, 9)?,
        1
    );
    unsafe { context.device.destroy_fence(fence, None) };
    resources.release_image_staging();
    assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    eprintln!("PASS bounded texture cache/actualdescriptor/GPUreadback/unpublishedrollback/cachehits/identitydenial/serialgate/ledgerzero {:?}",cache.stats);
    Ok(())
}
