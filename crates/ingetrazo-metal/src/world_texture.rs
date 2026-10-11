//! Cold immutable sampled assets. Publication owners retain image/sampler until
//! every Prepared reader completes; unknown upload completion is quarantined.
use crate::NativeMetal;
use ingetrazo_core::gpu_memory::MemoryBudget;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::*;
use std::{
    mem::ManuallyDrop,
    sync::{Arc, Mutex},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Identity(pub [u8; 32]);
pub struct Asset {
    pub id: Identity,
    pub image: ManuallyDrop<Retained<ProtocolObject<dyn MTLTexture>>>,
    pub sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    pub cutout: bool,
    pub mean: [f32; 3],
    budget: Arc<Mutex<MemoryBudget>>,
    handle: u64,
}
impl Drop for Asset {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.image) };
        if let Ok(mut b) = self.budget.lock() {
            let _ = b.freed(self.handle);
        }
    }
}
pub fn identity(encoded: &[u8]) -> Result<Identity, String> {
    if encoded.is_empty() || encoded.len() > 16 * 1024 * 1024 {
        return Err("Metal texture encoded admission exceeds16MiB".into());
    }
    Ok(Identity(*blake3::hash(encoded).as_bytes()))
}
pub fn upload(metal: &NativeMetal, id: Identity, encoded: &[u8]) -> Result<Arc<Asset>, String> {
    if identity(encoded)? != id {
        return Err("Metal texture immutable identity changed".into());
    }
    let mut decoded = ingetrazo_render::source_texture_decode::decode(
        encoded,
        ingetrazo_render::source_texture_decode::Limits::default(),
    )?;
    let pixels = ingetrazo_render::source_texture_average::source_average_pixels(
        &decoded.rgba,
        decoded.width,
        decoded.height,
        decoded.alpha,
    )?;
    let mut sums = [0u32; 3];
    for pixel in pixels {
        for channel in 0..3 {
            sums[channel] += pixel[channel] as u32;
        }
    }
    let mean = sums.map(|sum| (sum as f64 / (255. * 16.)) as f32);
    let (w, h) = (decoded.width as usize, decoded.height as usize);
    if w > 16384 || h > 16384 {
        return Err("Metal sampled texture dimensions exceed16384".into());
    }
    let mut occupied = 0usize;
    if decoded.alpha {
        for y in 0..64usize {
            for x in 0..64usize {
                let sy = ((2 * y + 1) * h / 128).min(h - 1);
                let sx = ((2 * x + 1) * w / 128).min(w - 1);
                occupied += usize::from(decoded.rgba[(sy * w + sx) * 4 + 3] > 128);
            }
        }
    }
    let cutout = decoded.alpha && (occupied as f64 / 4096.) < 0.6;
    // Literal source QOpenGLTexture(QImage.mirrored()): in-place row swap,
    // no second full decoded image or per-frame orientation work.
    let row = w.checked_mul(4).ok_or("Metal texture row overflow")?;
    for y in 0..h / 2 {
        let (split_a, split_b) = decoded.rgba.split_at_mut((h - 1 - y) * row);
        split_a[y * row..(y + 1) * row].swap_with_slice(&mut split_b[..row]);
    }
    let levels = if cutout {
        1
    } else {
        usize::BITS - (w.max(h) as usize).leading_zeros()
    };
    // Existing native texture upload path uses256-byte row alignment for Mac
    // buffer->texture blits. Reserve the full padded staging domain before copy.
    let stride = row
        .checked_add(255)
        .ok_or("Metal sampled row alignment overflow")?
        & !255;
    let staging_bytes = stride
        .checked_mul(h)
        .ok_or("Metal sampled staging extent overflow")?;
    if staging_bytes > 64 * 1024 * 1024 {
        return Err("Metal sampled staging admission exceeds64MiB".into());
    }
    let d = MTLTextureDescriptor::new();
    d.setTextureType(MTLTextureType::Type2D);
    unsafe {
        d.setWidth(w);
        d.setHeight(h);
        d.setMipmapLevelCount(levels as usize);
    }
    d.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    d.setUsage(MTLTextureUsage::ShaderRead);
    d.setHazardTrackingMode(MTLHazardTrackingMode::Tracked);
    // Native GPU-private layout, matching the admitted world target policy.
    // Shared standalone textures are not sized by the heap-layout query.
    // One cold staging blit uploads the immutable image; warm frames reuse it.
    d.setStorageMode(MTLStorageMode::Private);
    let native = metal.device.heapTextureSizeAndAlignWithDescriptor(&d);
    let mut minimum = 0usize;
    for level in 0..levels as usize {
        minimum = minimum
            .checked_add(
                (w >> level)
                    .max(1)
                    .checked_mul((h >> level).max(1))
                    .and_then(|n| n.checked_mul(4))
                    .ok_or("Metal texture mip extent overflow")?,
            )
            .ok_or("Metal texture mip bytes overflow")?;
    }
    if native.size < minimum || native.align == 0 {
        return Err(format!("Metal sampled native query unavailable: requested_bytes={minimum}, queried_size={}, queried_align={}, extent={w}x{h}, levels={levels}, storage={:?}",native.size,native.align,d.storageMode()));
    }
    let ticket = metal
        .budget
        .lock()
        .map_err(|_| "Metal texture budget poisoned")?
        .reserve(0, native.size as u64)?;
    let Some(raw) = metal.device.newTextureWithDescriptor(&d) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal texture budget poisoned")?
            .cancel(ticket)?;
        return Err("Metal sampled allocation failed".into());
    };
    let actual = MTLResource::allocatedSize(&*raw) as u64;
    let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
    if actual > native.size as u64 {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal texture budget poisoned")?
            .cancel(ticket)?;
        return Err(format!("Metal sampled allocation exceeded native preallocation admission: requested_bytes={minimum}, queried_size={}, queried_align={}, allocated_size={actual}, extent={w}x{h}, levels={levels}, format={:?}, storage={:?}, usage={:?}, device={}",native.size,native.align,d.pixelFormat(),d.storageMode(),d.usage(),metal.device.name()));
    }
    {
        let mut b = metal
            .budget
            .lock()
            .map_err(|_| "Metal texture budget poisoned")?;
        if let Err(error) = b.commit(ticket, handle) {
            let _ = b.cancel(ticket);
            return Err(error);
        }
    }
    let sd = MTLSamplerDescriptor::new();
    sd.setMinFilter(MTLSamplerMinMagFilter::Linear);
    sd.setMagFilter(MTLSamplerMinMagFilter::Linear);
    sd.setMipFilter(if cutout {
        MTLSamplerMipFilter::NotMipmapped
    } else {
        MTLSamplerMipFilter::Linear
    });
    sd.setSAddressMode(MTLSamplerAddressMode::Repeat);
    sd.setTAddressMode(MTLSamplerAddressMode::Repeat);
    // Create guarded image owner before sampler/upload fallibility.
    struct Guard {
        raw: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
        budget: Arc<Mutex<MemoryBudget>>,
        handle: u64,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(raw) = self.raw.take() {
                drop(raw);
                if let Ok(mut b) = self.budget.lock() {
                    let _ = b.freed(self.handle);
                }
            }
        }
    }
    let mut guard = Guard {
        raw: Some(raw),
        budget: metal.budget.clone(),
        handle,
    };
    let sampler = metal
        .device
        .newSamplerStateWithDescriptor(&sd)
        .ok_or("Metal sampled sampler allocation failed")?;
    let asset = Arc::new(Asset {
        id,
        image: ManuallyDrop::new(guard.raw.take().unwrap()),
        sampler,
        cutout,
        mean,
        budget: guard.budget.clone(),
        handle,
    });
    drop(guard);
    let staging = metal.buffer(staging_bytes, true)?;
    unsafe {
        let dst = std::slice::from_raw_parts_mut(
            staging.raw.contents().as_ptr().cast::<u8>(),
            staging_bytes,
        );
        dst.fill(0);
        for y in 0..h {
            dst[y * stride..y * stride + row]
                .copy_from_slice(&decoded.rgba[y * row..(y + 1) * row]);
        }
    }
    let command = metal
        .queue
        .commandBuffer()
        .ok_or("Metal sampled upload command unavailable")?;
    let blit = command
        .blitCommandEncoder()
        .ok_or("Metal sampled upload encoder unavailable")?;
    unsafe {
        blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(&staging.raw,0,stride,staging_bytes,MTLSize{width:w,height:h,depth:1},&asset.image,0,0,MTLOrigin{x:0,y:0,z:0});
    }
    if levels > 1 {
        blit.generateMipmapsForTexture(&asset.image);
    }
    blit.endEncoding();
    command.commit();
    let start = std::time::Instant::now();
    while command.status() != MTLCommandBufferStatus::Completed {
        if command.status() == MTLCommandBufferStatus::Error
            || start.elapsed() > std::time::Duration::from_secs(5)
        {
            metal
                .poisoned
                .store(true, std::sync::atomic::Ordering::Release);
            std::mem::forget(asset);
            std::mem::forget(command);
            std::mem::forget(staging);
            return Err(
                "Metal sampled upload completion unknown; image/staging/command owners quarantined"
                    .into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(asset)
}
