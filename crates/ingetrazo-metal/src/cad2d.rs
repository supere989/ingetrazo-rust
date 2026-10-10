//! Opt-in direct native Metal CAD2D. Exact framebuffer blending is admitted
//! only on Apple tiled families with raster-order groups; IMR remains OPEN.
use crate::{Buffer, NativeMetal};
use ingetrazo_core::gpu_memory::MemoryBudget;
use ingetrazo_render::cad2d::{Batch, Geometry, Pose};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{
    mem::ManuallyDrop,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct MetalCadStats {
    pub geometry_uploads: u64,
    pub warm_hits: u64,
    pub submissions: u64,
    pub geometry_bytes: u64,
    pub target_bytes: u64,
    pub palette_preparations: u64,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Push {
    scale: [f32; 2],
    translation: [f32; 2],
    screen: [f32; 2],
    pad: [f32; 2],
    rgba: [f32; 4],
}
struct Texture {
    raw: ManuallyDrop<Retained<ProtocolObject<dyn MTLTexture>>>,
    budget: Arc<Mutex<MemoryBudget>>,
    handle: u64,
}
impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.raw) };
        if let Ok(mut b) = self.budget.lock() {
            let _ = b.freed(self.handle);
        }
    }
}
struct Cached {
    id: u64,
    vertices: Buffer,
    indices: Buffer,
    bounds: [[f32; 2]; 2],
}
struct Inner {
    metal: NativeMetal,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    target: Texture,
    readback: Buffer,
    width: usize,
    height: usize,
    row: usize,
    cached: Option<Cached>,
    pending: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    stats: MetalCadStats,
    palette: Vec<Batch>,
    colors: Vec<[f32; 4]>,
}
/// Completion-unknown Drop retains the entire inner owner, including pipeline,
/// target/geometry/readback, command, queue/device and ledger. No implicit wait.
pub struct MetalCad2d {
    inner: ManuallyDrop<Inner>,
    unknown: bool,
}
impl Drop for MetalCad2d {
    fn drop(&mut self) {
        if !self.unknown {
            unsafe { ManuallyDrop::drop(&mut self.inner) }
        }
    }
}
impl MetalCad2d {
    pub fn supported(metal: &NativeMetal) -> bool {
        metal.device.supportsFamily(MTLGPUFamily::Apple1)
            && metal.device.areRasterOrderGroupsSupported()
    }
    pub fn new(metal: NativeMetal, width: usize, height: usize) -> Result<Self, String> {
        if !Self::supported(&metal) {
            return Err(format!(
                "Exact Metal CAD framebuffer fetch unsupported: device={}, Apple1={}, ROG={}; Intel/AMD exact native path remains OPEN",
                metal.info.name,
                metal.device.supportsFamily(MTLGPUFamily::Apple1),
                metal.device.areRasterOrderGroupsSupported()
            ));
        }
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Metal CAD target outside admitted1..4096 extent".into());
        }
        let options = MTLCompileOptions::new();
        options.setFastMathEnabled(false);
        let library = metal
            .device
            .newLibraryWithSource_options_error(
                &NSString::from_str(include_str!("shaders/cad2d.metal")),
                Some(&options),
            )
            .map_err(|e| format!("Metal shader compile: {e:?}"))?;
        let vertex = library
            .newFunctionWithName(&NSString::from_str("cad_vertex"))
            .ok_or("Metal CAD vertex missing")?;
        let fragment = library
            .newFunctionWithName(&NSString::from_str("cad_fragment"))
            .ok_or("Metal CAD fragment missing")?;
        let descriptor = MTLRenderPipelineDescriptor::new();
        descriptor.setVertexFunction(Some(&vertex));
        descriptor.setFragmentFunction(Some(&fragment));
        // Index zero is statically bounded; one encoded UNORM attachment, no sRGB.
        let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
        attachment.setBlendingEnabled(false);
        let pipeline = metal
            .device
            .newRenderPipelineStateWithDescriptor_error(&descriptor)
            .map_err(|e| format!("Metal CAD pipeline: {e:?}"))?;
        let row = (width.checked_mul(4).ok_or("Target byte overflow")? + 255) & !255;
        let target = Self::target(&metal, width, height)?;
        let readback = metal.buffer(row.checked_mul(height).ok_or("Readback overflow")?, true)?;
        let stats = MetalCadStats {
            target_bytes: MTLResource::allocatedSize(&**target.raw) as u64,
            ..Default::default()
        };
        Ok(Self {
            inner: ManuallyDrop::new(Inner {
                metal,
                pipeline,
                target,
                readback,
                width,
                height,
                row,
                cached: None,
                pending: None,
                stats,
                palette: Vec::new(),
                colors: Vec::new(),
            }),
            unknown: false,
        })
    }
    fn target(metal: &NativeMetal, width: usize, height: usize) -> Result<Texture, String> {
        let request = (width * height * 4) as u64;
        let ticket = metal
            .budget
            .lock()
            .map_err(|_| "Metal budget poisoned")?
            .reserve(0, request)?;
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::RGBA8Unorm,
                width,
                height,
                false,
            )
        };
        descriptor.setStorageMode(MTLStorageMode::Private);
        descriptor.setUsage(MTLTextureUsage::RenderTarget);
        let Some(raw) = metal.device.newTextureWithDescriptor(&descriptor) else {
            metal.budget.lock().unwrap().cancel(ticket)?;
            return Err("Metal target allocation failed".into());
        };
        let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
        let mut budget = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
        let allocated = MTLResource::allocatedSize(&*raw) as u64;
        if allocated != request {
            budget.cancel(ticket)?;
            let actual = budget.reserve(0, allocated)?;
            budget.commit(actual, handle)?;
        } else {
            budget.commit(ticket, handle)?;
        }
        drop(budget);
        Ok(Texture {
            raw: ManuallyDrop::new(raw),
            budget: metal.budget.clone(),
            handle,
        })
    }
    pub fn stats(&self) -> MetalCadStats {
        self.inner.stats
    }
    pub fn budget(&self) -> Arc<Mutex<MemoryBudget>> {
        self.inner.metal.budget.clone()
    }
    /// Cold immutable geometry uploaded once. Prepared replacement is published
    /// only when both allocations and writes succeed; failed admission keeps old.
    pub fn prepare(&mut self, geometry: &Arc<Geometry>) -> Result<(), String> {
        if self.unknown || self.inner.metal.poisoned.load(Ordering::Acquire) {
            return Err("Metal CAD completion unknown; admission refused".into());
        }
        if self
            .inner
            .cached
            .as_ref()
            .is_some_and(|c| c.id == geometry.id())
        {
            self.inner.stats.warm_hits += 1;
            return Ok(());
        }
        if geometry.vertices().is_empty()
            || geometry.indices().is_empty()
            || geometry.logical_bytes() > 64 * 1024 * 1024
        {
            return Err("Metal CAD empty/over64MiB geometry refused".into());
        }
        let bounds = self.geometry_bounds(geometry)?;
        let mut vertices = self
            .inner
            .metal
            .buffer(geometry.vertices().len() * 12, true)?;
        vertices.write(bytemuck::cast_slice(geometry.vertices()))?;
        let mut indices = self
            .inner
            .metal
            .buffer(geometry.indices().len() * 4, true)?;
        indices.write(bytemuck::cast_slice(geometry.indices()))?;
        self.inner.cached = Some(Cached {
            id: geometry.id(),
            vertices,
            indices,
            bounds,
        });
        self.inner.stats.geometry_uploads += 1;
        self.inner.stats.geometry_bytes = geometry.logical_bytes();
        Ok(())
    }
    fn geometry_bounds(&self, geometry: &Arc<Geometry>) -> Result<[[f32; 2]; 2], String> {
        if let Some(c) = self.inner.cached.as_ref().filter(|c| c.id == geometry.id()) {
            return Ok(c.bounds);
        }
        if geometry.vertices().is_empty() || geometry.logical_bytes() > 64 * 1024 * 1024 {
            return Err("Metal CAD geometry admission refused".into());
        }
        let mut bounds = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
        for vertex in geometry.vertices() {
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(vertex.position[axis]);
                bounds[axis][1] = bounds[axis][1].max(vertex.position[axis]);
            }
        }
        Ok(bounds)
    }
    /// One render encoder with coherent raster-order framebuffer fetch, followed
    /// only by a diagnostic BLIT readback. Uniform changes do not rebuild geometry.
    pub fn render(
        &mut self,
        geometry: &Arc<Geometry>,
        batches: &[Batch],
        pose: Pose,
        clear: [u8; 4],
    ) -> Result<Vec<u8>, String> {
        if batches.len() > 4096 {
            return Err("Metal CAD batch admission exceeds4096".into());
        }
        geometry.validate_batches(batches)?;
        pose.validate()?;
        // Bound actual shader f32 products and transformed viewport domain before
        // allocation/encoding, including huge-product cancellation falsepositives.
        if clear[..3].iter().any(|v| *v > clear[3]) {
            return Err("Metal CAD clear must be encoded premultiplied RGBA".into());
        }
        let bounds = self.geometry_bounds(geometry)?;
        for axis in 0..2 {
            for v in bounds[axis] {
                let product = v * pose.scale[axis];
                let sum = product + pose.translation[axis];
                if !product.is_finite()
                    || !sum.is_finite()
                    || sum.abs() > 1e6
                    || !(sum * 2.).is_finite()
                {
                    return Err("Metal CAD pose arithmetic/domain refused".into());
                }
            }
        }
        let palette_changed = self.inner.palette != batches;
        let colors = if palette_changed {
            Some(
                batches
                    .iter()
                    .map(|b| b.color.qt_premultiplied8())
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            None
        };
        self.prepare(geometry)?;
        if let Some(colors) = colors {
            self.inner.palette = batches.to_vec();
            self.inner.colors = colors;
            self.inner.stats.palette_preparations += 1;
        }
        let command = self
            .inner
            .metal
            .queue
            .commandBuffer()
            .ok_or("Metal CAD command creation failed")?;
        let pass = MTLRenderPassDescriptor::new();
        let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(&self.inner.target.raw));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        attachment.setClearColor(MTLClearColor {
            red: clear[0] as f64 / 255.,
            green: clear[1] as f64 / 255.,
            blue: clear[2] as f64 / 255.,
            alpha: clear[3] as f64 / 255.,
        });
        let encoder = command
            .renderCommandEncoderWithDescriptor(&pass)
            .ok_or("Metal CAD render encoder failed")?;
        encoder.setRenderPipelineState(&self.inner.pipeline);
        let cached = self.inner.cached.as_ref().unwrap();
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&cached.vertices.raw), 0, 0);
        }
        for (index, batch) in batches.iter().enumerate() {
            let push = Push {
                scale: pose.scale,
                translation: pose.translation,
                screen: [self.inner.width as f32, self.inner.height as f32],
                pad: [0.; 2],
                rgba: self.inner.colors[index],
            };
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    std::ptr::NonNull::from(&push).cast(),
                    std::mem::size_of::<Push>(),
                    1,
                );
                encoder.setFragmentBytes_length_atIndex(
                    std::ptr::NonNull::from(&push).cast(),
                    std::mem::size_of::<Push>(),
                    1,
                );
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    batch.count as usize,
                    MTLIndexType::UInt32,
                    &cached.indices.raw,
                    batch.first as usize * 4,
                );
            }
        }
        encoder.endEncoding();
        let blit = command
            .blitCommandEncoder()
            .ok_or("Metal CAD readback encoder failed")?;
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(&self.inner.target.raw,0,0,MTLOrigin{x:0,y:0,z:0},MTLSize{width:self.inner.width,height:self.inner.height,depth:1},&self.inner.readback.raw,0,self.inner.row,self.inner.row*self.inner.height);
        }
        blit.endEncoding();
        self.inner.pending = Some(command);
        self.inner.pending.as_ref().unwrap().commit();
        self.inner.stats.submissions += 1;
        let start = Instant::now();
        loop {
            let status = self.inner.pending.as_ref().unwrap().status();
            if status == MTLCommandBufferStatus::Completed {
                break;
            }
            if status == MTLCommandBufferStatus::Error || start.elapsed() >= Duration::from_secs(5)
            {
                self.unknown = true;
                self.inner.metal.poisoned.store(true, Ordering::Release);
                return Err(format!(
                    "Metal CAD completion unverified; entire owner retained: {status:?}"
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.inner.pending.take();
        let raw = unsafe {
            std::slice::from_raw_parts(
                self.inner.readback.raw.contents().as_ptr().cast::<u8>(),
                self.inner.row * self.inner.height,
            )
        };
        let mut output = Vec::with_capacity(self.inner.width * self.inner.height * 4);
        for row in raw.chunks_exact(self.inner.row) {
            output.extend_from_slice(&row[..self.inner.width * 4]);
        }
        Ok(output)
    }
}
