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
// A small real native non-aliasable heap, shared by immutable cached assets.
// This is a requested backing capacity, not an estimated child allocation cap.
const HEAP_CHUNK: usize = 4 * 1024 * 1024;
const MAX_HEAP: usize = 256 * 1024 * 1024;
struct HeapOwner {
    raw: ManuallyDrop<Retained<ProtocolObject<dyn MTLHeap>>>,
    capacity: usize,
    budget: Arc<Mutex<MemoryBudget>>,
    handle: u64,
}
impl Drop for HeapOwner {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.raw) };
        if let Ok(mut budget) = self.budget.lock() {
            let _ = budget.freed(self.handle);
        }
    }
}
pub struct Asset {
    pub id: Identity,
    pub image: ManuallyDrop<Retained<ProtocolObject<dyn MTLTexture>>>,
    pub sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    pub cutout: bool,
    pub mean: [f32; 3],
    backing: Backing,
}
enum Backing {
    Heap(Arc<HeapOwner>),
    Standalone {
        budget: Arc<Mutex<MemoryBudget>>,
        handle: u64,
        bytes: u64,
    },
}
impl Drop for Backing {
    fn drop(&mut self) {
        if let Self::Standalone { budget, handle, .. } = self {
            if let Ok(mut b) = budget.lock() {
                let _ = b.freed(*handle);
            }
        }
    }
}
impl Drop for Asset {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.image) };
    }
}
impl Asset {
    pub(crate) fn backing_kind(&self)->&'static str {match self.backing {Backing::Heap(_)=>"heap",Backing::Standalone{..}=>"standalone"}}
    pub(crate) fn heap_observation(&self) -> (usize, [u64; 3]) {
        match &self.backing {
            Backing::Heap(heap) => (
                Arc::as_ptr(heap) as usize,
                [
                    heap.capacity as u64,
                    heap.raw.currentAllocatedSize() as u64,
                    heap.raw.usedSize() as u64,
                ],
            ),
            Backing::Standalone { handle, bytes, .. } => {
                (*handle as usize, [*bytes, *bytes, *bytes])
            }
        }
    }
}
fn new_heap(metal: &NativeMetal, required: usize) -> Result<Arc<HeapOwner>, String> {
    let capacity = required
        .checked_add(HEAP_CHUNK - 1)
        .ok_or("Metal sampled heap capacity overflow")?
        / HEAP_CHUNK
        * HEAP_CHUNK;
    if capacity == 0 || capacity > MAX_HEAP {
        return Err("Metal sampled heap capacity exceeds256MiB".into());
    }
    let ticket = metal
        .budget
        .lock()
        .map_err(|_| "Metal sampled heap budget poisoned")?
        .reserve(0, capacity as u64)?;
    let descriptor = MTLHeapDescriptor::new();
    descriptor.setStorageMode(MTLStorageMode::Private);
    descriptor.setHazardTrackingMode(MTLHazardTrackingMode::Tracked);
    descriptor.setSize(capacity);
    let Some(raw) = metal.device.newHeapWithDescriptor(&descriptor) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal heap budget poisoned")?
            .cancel(ticket)?;
        return Err("Metal sampled backing heap allocation unavailable".into());
    };
    let size = raw.size();
    let allocated = raw.currentAllocatedSize();
    if size > capacity || allocated > capacity || size < required {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal heap budget poisoned")?
            .cancel(ticket)?;
        return Err(format!("Metal sampled backing heap refused: requested_capacity={capacity}, heap_size={size}, allocated_size={allocated}, resource_required={required}"));
    }
    let handle = (&*raw as *const ProtocolObject<dyn MTLHeap>).cast::<()>() as usize as u64;
    let mut budget = metal
        .budget
        .lock()
        .map_err(|_| "Metal heap budget poisoned")?;
    if let Err(error) = budget.commit(ticket, handle) {
        let _ = budget.cancel(ticket);
        return Err(error);
    }
    drop(budget);
    Ok(Arc::new(HeapOwner {
        raw: ManuallyDrop::new(raw),
        capacity,
        budget: metal.budget.clone(),
        handle,
    }))
}
pub fn identity(encoded: &[u8]) -> Result<Identity, String> {
    if encoded.is_empty() || encoded.len() > 16 * 1024 * 1024 {
        return Err("Metal texture encoded admission exceeds16MiB".into());
    }
    Ok(Identity(*blake3::hash(encoded).as_bytes()))
}
pub fn upload(
    metal: &NativeMetal,
    id: Identity,
    encoded: &[u8],
    published: &[Arc<Asset>],
    candidates: &[Arc<Asset>],
) -> Result<Arc<Asset>, String> {
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
    let sampler = metal
        .device
        .newSamplerStateWithDescriptor(&sd)
        .ok_or("Metal sampled sampler allocation failed")?;
    let (raw, backing) = if native.size == 0 && native.align == 0 {
        standalone(metal, &d, minimum as u64)?
    } else {
        if native.size == 0 || native.align == 0 {
            return Err(format!("Metal sampled native query unavailable: requested_bytes={minimum}, queried_size={}, queried_align={}, extent={w}x{h}, levels={levels}, storage={:?}",native.size,native.align,d.storageMode()));
        }
        if !native.align.is_power_of_two() {
            return Err("Metal sampled native heap alignment invalid".into());
        }
        let mut heap = None;
        // Cold bounded first fit over live cache owners. No new allocator registry,
        // no aliasable resources, and no lookup on warm unchanged frames.
        for asset in candidates.iter().cloned().chain(published.iter().cloned()) {
            if let Backing::Heap(h) = &asset.backing {
                if Arc::ptr_eq(&h.budget, &metal.budget)
                    && h.raw.maxAvailableSizeWithAlignment(native.align) >= native.size
                {
                    heap = Some(h.clone());
                    break;
                }
            }
        }
        let heap = match heap {
            Some(heap) => heap,
            None => new_heap(metal, native.size)?,
        };
        let available = heap.raw.maxAvailableSizeWithAlignment(native.align);
        if available < native.size {
            return Err(format!("Metal sampled heap placement refused: queried_size={}, queried_align={}, available={available}",native.size,native.align));
        }
        let used_before = heap.raw.usedSize();
        let Some(raw) = heap.raw.newTextureWithDescriptor(&d) else {
            return Err("Metal sampled heap texture allocation failed".into());
        };
        let used_after = heap.raw.usedSize();
        let allocated = heap.raw.currentAllocatedSize();
        let placement = used_after
            .checked_sub(used_before)
            .ok_or("Metal sampled heap placement accounting regressed")?;
        if placement > native.size || used_after > heap.capacity || allocated > heap.capacity {
            return Err(format!("Metal sampled heap placement exceeded admission: queried_size={}, queried_align={}, used_before={used_before}, used_after={used_after}, placement_size={placement}, heap_capacity={}, heap_allocated={allocated}, child_allocated={}",native.size,native.align,heap.capacity,MTLResource::allocatedSize(&*raw)));
        }
        (raw, Backing::Heap(heap))
    };
    let asset = Arc::new(Asset {
        id,
        image: ManuallyDrop::new(raw),
        sampler,
        cutout,
        mean,
        backing,
    });
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

// Zero heap-layout queries are handled only with a standalone owner. Heap-backed
// cache entries retain the existing native placement and shared capacity accounting.
fn standalone(
    metal: &NativeMetal,
    d: &MTLTextureDescriptor,
    requested: u64,
) -> Result<(Retained<ProtocolObject<dyn MTLTexture>>, Backing), String> {
    let (admitted, ticket) = {
        let mut b = metal
            .budget
            .lock()
            .map_err(|_| "Metal sampled budget poisoned")?;
        let n = crate::world_admission::standalone_capacity(&b)?;
        if n < requested || n == 0 {
            return Err("Metal sampled standalone admission refused".into());
        }
        (n, b.reserve(0, n)?)
    };
    let Some(raw) = metal.device.newTextureWithDescriptor(d) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal sampled budget poisoned")?
            .cancel(ticket)?;
        return Err("Metal sampled standalone allocation failed".into());
    };
    let actual = MTLResource::allocatedSize(&*raw) as u64;
    let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
    let mut b = metal
        .budget
        .lock()
        .map_err(|_| "Metal sampled budget poisoned")?;
    if actual == 0 || actual > admitted {
        b.cancel(ticket)?;
        return Err("Metal sampled standalone size exceeds finite admission or is zero".into());
    }
    let ticket = if actual != admitted {
        b.cancel(ticket)?;
        b.reserve(0, actual)?
    } else {
        ticket
    };
    if let Err(e) = b.commit(ticket, handle) {
        let _ = b.cancel(ticket);
        return Err(e);
    }
    drop(b);
    Ok((
        raw,
        Backing::Standalone {
            budget: metal.budget.clone(),
            handle,
            bytes: actual,
        },
    ))
}
