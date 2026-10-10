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
    pub readback_submissions: u64,
    pub plan_uploads: u64,
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
    layout: Vec<(u32, u32)>,
    references: Option<Arc<Buffer>>,
    tile_count: u32,
    id: u64,
    vertices: Arc<Buffer>,
    indices: Arc<Buffer>,
    bounds: [[f32; 2]; 2],
}
struct Frame {
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    serial: u64,
    _vertices: Arc<Buffer>,
    _indices: Arc<Buffer>,
    _target: Arc<Texture>,
    _readback: Option<Arc<Buffer>>,
    _references: Option<Arc<Buffer>>,
    _palette: Option<Arc<Buffer>>,
}
struct Inner {
    pipeline: Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    compute: Option<Retained<ProtocolObject<dyn MTLComputePipelineState>>>,
    gpu_palette: Option<Arc<Buffer>>,
    target: Arc<Texture>,
    readback: Option<Arc<Buffer>>,
    width: usize,
    height: usize,
    row: usize,
    cached: Option<Cached>,
    frames: std::collections::VecDeque<Frame>,
    completed_serial: u64,
    stats: MetalCadStats,
    palette: Vec<Batch>,
    colors: Vec<[f32; 4]>,
    // Device/queue outlive all resource owners on verified normal retirement.
    metal: NativeMetal,
}
/// Completion-unknown Drop retains the entire inner owner, including pipeline,
/// target/geometry/readback, command, queue/device and ledger. No implicit wait.
pub struct MetalCad2d {
    inner: ManuallyDrop<Inner>,
    unknown: bool,
}
impl Drop for MetalCad2d {
    fn drop(&mut self) {
        if !self.inner.frames.is_empty() {
            self.inner.metal.poisoned.store(true, Ordering::Release);
        }
        if !self.unknown && self.inner.frames.is_empty() {
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
        objc2::rc::autoreleasepool(|_| Self::new_inner(metal, width, height, false))
    }
    /// Direct ordered-span compute path for IMR Intel/AMD and Apple devices.
    /// It admits source pixel spans only, not arbitrary triangle/affine parity.
    pub fn new_ordered_spans(
        metal: NativeMetal,
        width: usize,
        height: usize,
    ) -> Result<Self, String> {
        objc2::rc::autoreleasepool(|_| Self::new_inner(metal, width, height, true))
    }
    fn new_inner(
        metal: NativeMetal,
        width: usize,
        height: usize,
        compute: bool,
    ) -> Result<Self, String> {
        if !compute && !Self::supported(&metal) {
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
        let shader = if compute {
            include_str!("shaders/cad2d_spans.metal")
        } else {
            include_str!("shaders/cad2d.metal")
        };
        let library = metal
            .device
            .newLibraryWithSource_options_error(&NSString::from_str(shader), Some(&options))
            .map_err(|e| format!("Metal shader compile: {e:?}"))?;
        let (pipeline, compute_pipeline) = if compute {
            let function = library
                .newFunctionWithName(&NSString::from_str("span_compute"))
                .ok_or("Metal compute function missing")?;
            let pipeline = metal
                .device
                .newComputePipelineStateWithFunction_error(&function)
                .map_err(|e| format!("Metal compute pipeline: {e:?}"))?;
            if pipeline.maxTotalThreadsPerThreadgroup() < 64 {
                return Err("Metal compute cannot admit8x8 tile group".into());
            }
            (None, Some(pipeline))
        } else {
            let vertex = library
                .newFunctionWithName(&NSString::from_str("cad_vertex"))
                .ok_or("Metal CAD vertex missing")?;
            let fragment = library
                .newFunctionWithName(&NSString::from_str("cad_fragment"))
                .ok_or("Metal CAD fragment missing")?;
            let descriptor = MTLRenderPipelineDescriptor::new();
            descriptor.setVertexFunction(Some(&vertex));
            descriptor.setFragmentFunction(Some(&fragment));
            let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
            attachment.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
            attachment.setBlendingEnabled(false);
            (
                Some(
                    metal
                        .device
                        .newRenderPipelineStateWithDescriptor_error(&descriptor)
                        .map_err(|e| format!("Metal CAD pipeline: {e:?}"))?,
                ),
                None,
            )
        };
        let row = (width.checked_mul(4).ok_or("Target byte overflow")? + 255) & !255;
        let target = Arc::new(Self::target(&metal, width, height)?);
        let stats = MetalCadStats {
            target_bytes: MTLResource::allocatedSize(&**target.raw) as u64,
            ..Default::default()
        };
        Ok(Self {
            inner: ManuallyDrop::new(Inner {
                metal,
                pipeline,
                compute: compute_pipeline,
                gpu_palette: None,
                target,
                readback: None,
                width,
                height,
                row,
                cached: None,
                frames: std::collections::VecDeque::new(),
                completed_serial: 0,
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
        descriptor.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderWrite);
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
        if self.inner.compute.is_some() {
            let color = ingetrazo_render::cad2d::EncodedRgba::from_bytes([255; 4], 1.)?;
            return self.prepare_spans(
                geometry,
                &[Batch {
                    first: 0,
                    count: geometry.indices().len() as u32,
                    color,
                }],
            );
        }

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
            layout: Vec::new(),
            references: None,
            tile_count: 0,
            id: geometry.id(),
            vertices: Arc::new(vertices),
            indices: Arc::new(indices),
            bounds,
        });
        self.inner.stats.geometry_uploads += 1;
        self.inner.stats.geometry_bytes = geometry.logical_bytes();
        Ok(())
    }
    fn upload_bytes(&self, bytes: &[u8]) -> Result<Arc<Buffer>, String> {
        let mut buffer = self.inner.metal.buffer(bytes.len(), true)?;
        buffer.write(bytes)?;
        Ok(Arc::new(buffer))
    }
    fn prepare_spans(&mut self, geometry: &Arc<Geometry>, batches: &[Batch]) -> Result<(), String> {
        if self.unknown || self.inner.metal.poisoned.load(Ordering::Acquire) {
            return Err("Metal compute completion unknown".into());
        }
        let layout: Vec<_> = batches.iter().map(|b| (b.first, b.count)).collect();
        if self
            .inner
            .cached
            .as_ref()
            .is_some_and(|c| c.id == geometry.id() && c.layout == layout)
        {
            self.inner.stats.warm_hits += 1;
            return Ok(());
        }
        let plan = crate::span_plan::compile(geometry, batches)?;
        let bounds = self.geometry_bounds(geometry)?;
        let vertices = self.upload_bytes(bytemuck::cast_slice(&plan.spans))?;
        let indices = self.upload_bytes(bytemuck::cast_slice(&plan.tiles))?;
        let references = self.upload_bytes(bytemuck::cast_slice(&plan.references))?;
        let bytes = (plan.spans.len() * std::mem::size_of::<crate::span_plan::Span>()
            + plan.tiles.len() * std::mem::size_of::<crate::span_plan::Tile>()
            + plan.references.len() * 4) as u64;
        self.inner.cached = Some(Cached {
            id: geometry.id(),
            bounds,
            layout,
            vertices,
            indices,
            references: Some(references),
            tile_count: plan.tiles.len() as u32,
        });
        self.inner.stats.plan_uploads += 1;
        self.inner.stats.geometry_uploads += 1;
        self.inner.stats.geometry_bytes = bytes;
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
    /// Nonblocking actual completion collection. Error retains all owners.
    pub fn poll_completed(&mut self) -> Result<u64, String> {
        if self.unknown {
            return Err("Metal CAD completion unknown; refused".into());
        }
        while let Some(frame) = self.inner.frames.front() {
            let status = frame.command.status();
            if status == MTLCommandBufferStatus::Completed {
                self.inner.completed_serial = frame.serial;
                self.inner.frames.pop_front();
            } else if status == MTLCommandBufferStatus::Error {
                self.unknown = true;
                self.inner.metal.poisoned.store(true, Ordering::Release);
                return Err("Metal CAD command error; all owners retained".into());
            } else {
                break;
            }
        }
        Ok(self.inner.completed_serial)
    }
    /// Explicit engineering wait only. Production submission never invokes it.
    pub fn wait_diagnostic(&mut self, serial: u64, timeout: Duration) -> Result<(), String> {
        if timeout > Duration::from_secs(5) || serial > self.inner.stats.submissions {
            return Err("Metal CAD diagnostic wait domain invalid".into());
        }
        let start = Instant::now();
        loop {
            if self.poll_completed()? >= serial {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                self.unknown = true;
                self.inner.metal.poisoned.store(true, Ordering::Release);
                return Err("Metal CAD deadline expired; all owners/ledger retained".into());
            }
            std::thread::sleep(
                Duration::from_millis(1).min(timeout.saturating_sub(start.elapsed())),
            );
        }
    }
    pub fn in_flight(&self) -> usize {
        self.inner.frames.len()
    }
    /// One render encoder with coherent raster-order framebuffer fetch, followed
    /// only by a diagnostic BLIT readback. Uniform changes do not rebuild geometry.
    /// Production offscreen draw: no image copy/readback or CPU output.
    pub fn render(
        &mut self,
        geometry: &Arc<Geometry>,
        batches: &[Batch],
        pose: Pose,
        clear: [u8; 4],
    ) -> Result<u64, String> {
        objc2::rc::autoreleasepool(|_| {
            self.render_inner(geometry, batches, pose, clear, false)?;
            Ok(self.inner.stats.submissions)
        })
    }
    /// Explicit engineering readback; storage is admitted/mapped only when
    /// requested and reused. Not used by the production draw entry point.
    pub fn render_readback(
        &mut self,
        geometry: &Arc<Geometry>,
        batches: &[Batch],
        pose: Pose,
        clear: [u8; 4],
    ) -> Result<Vec<u8>, String> {
        objc2::rc::autoreleasepool(|_| {
            self.wait_diagnostic(self.inner.stats.submissions, Duration::from_secs(5))?;
            self.render_inner(geometry, batches, pose, clear, true)
                .map(|v| v.unwrap())
        })
    }
    fn render_inner(
        &mut self,
        geometry: &Arc<Geometry>,
        batches: &[Batch],
        pose: Pose,
        clear: [u8; 4],
        readback: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        self.poll_completed()?;
        if self.inner.frames.len() >= 3 {
            return Err(
                "Metal CAD in-flight capacity3; explicit backpressure, no wait/allocation".into(),
            );
        }
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
        if readback && self.inner.readback.is_none() {
            self.inner.readback = Some(Arc::new(
                self.inner
                    .metal
                    .buffer(self.inner.row * self.inner.height, true)?,
            ));
        }
        if self.inner.compute.is_some() {
            if pose.scale != [1.; 2]
                || pose
                    .translation
                    .iter()
                    .any(|v| v.fract() != 0. || v.abs() > 1e6)
            {
                return Err("Metal span compute requires integral translation and unit scale; rerasterize phase geometry otherwise".into());
            }
        }
        let gpu_palette = if self.inner.compute.is_some() {
            if let Some(ref colors) = colors {
                let values: Vec<[u32; 4]> = colors
                    .iter()
                    .map(|c| c.map(|v| (v * 255.).round() as u32))
                    .collect();
                Some(self.upload_bytes(bytemuck::cast_slice(&values))?)
            } else {
                None
            }
        } else {
            None
        };
        if self.inner.compute.is_some() {
            self.prepare_spans(geometry, batches)?;
        } else {
            self.prepare(geometry)?;
        }
        if let Some(palette) = gpu_palette {
            self.inner.gpu_palette = Some(palette);
        }
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
        if let Some(ref pipeline) = self.inner.compute {
            #[repr(C)]
            #[derive(Clone, Copy)]
            struct ComputePush {
                translation: [i32; 2],
                size: [u32; 2],
                clear: [u32; 4],
                tile_count: u32,
                pad: [u32; 3],
            }
            let cached = self.inner.cached.as_ref().unwrap();
            let push = ComputePush {
                translation: pose.translation.map(|v| v as i32),
                size: [self.inner.width as u32, self.inner.height as u32],
                clear: clear.map(|v| v as u32),
                tile_count: cached.tile_count,
                pad: [0; 3],
            };
            let encoder = command
                .computeCommandEncoder()
                .ok_or("Metal span compute encoder failed")?;
            encoder.setComputePipelineState(pipeline);
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&cached.vertices.raw), 0, 0);
                encoder.setBuffer_offset_atIndex(Some(&cached.indices.raw), 0, 1);
                encoder.setBuffer_offset_atIndex(
                    Some(&cached.references.as_ref().unwrap().raw),
                    0,
                    2,
                );
                encoder.setBuffer_offset_atIndex(
                    Some(&self.inner.gpu_palette.as_ref().unwrap().raw),
                    0,
                    3,
                );
                encoder.setBytes_length_atIndex(
                    std::ptr::NonNull::from(&push).cast(),
                    std::mem::size_of::<ComputePush>(),
                    4,
                );
                encoder.setTexture_atIndex(Some(&self.inner.target.raw), 0);
                encoder.dispatchThreadgroups_threadsPerThreadgroup(
                    MTLSize {
                        width: self.inner.width.div_ceil(8),
                        height: self.inner.height.div_ceil(8),
                        depth: 1,
                    },
                    MTLSize {
                        width: 8,
                        height: 8,
                        depth: 1,
                    },
                );
            }
            encoder.endEncoding();
        } else {
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
            encoder.setRenderPipelineState(self.inner.pipeline.as_ref().unwrap());
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
                    encoder
                        .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                            MTLPrimitiveType::Triangle,
                            batch.count as usize,
                            MTLIndexType::UInt32,
                            &cached.indices.raw,
                            batch.first as usize * 4,
                        );
                }
            }
            encoder.endEncoding();
        }
        if readback {
            let blit = command
                .blitCommandEncoder()
                .ok_or("Metal CAD readback encoder failed")?;
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(&self.inner.target.raw,0,0,MTLOrigin{x:0,y:0,z:0},MTLSize{width:self.inner.width,height:self.inner.height,depth:1},&self.inner.readback.as_ref().unwrap().raw,0,self.inner.row,self.inner.row*self.inner.height);
            }
            blit.endEncoding();
            self.inner.stats.readback_submissions += 1;
        }
        let cached = self.inner.cached.as_ref().unwrap();
        let serial = self.inner.stats.submissions + 1;
        let frame = Frame {
            command,
            serial,
            _references: cached.references.clone(),
            _palette: self.inner.gpu_palette.clone(),
            _vertices: cached.vertices.clone(),
            _indices: cached.indices.clone(),
            _target: self.inner.target.clone(),
            _readback: if readback {
                self.inner.readback.clone()
            } else {
                None
            },
        };
        self.inner.frames.push_back(frame);
        self.inner.frames.back().unwrap().command.commit();
        self.inner.stats.submissions = serial;
        if !readback {
            return Ok(None);
        }
        self.wait_diagnostic(serial, Duration::from_secs(5))?;
        let raw = unsafe {
            std::slice::from_raw_parts(
                self.inner
                    .readback
                    .as_ref()
                    .unwrap()
                    .raw
                    .contents()
                    .as_ptr()
                    .cast::<u8>(),
                self.inner.row * self.inner.height,
            )
        };
        let mut output = Vec::with_capacity(self.inner.width * self.inner.height * 4);
        for row in raw.chunks_exact(self.inner.row) {
            output.extend_from_slice(&row[..self.inner.width * 4]);
        }
        Ok(Some(output))
    }
}
