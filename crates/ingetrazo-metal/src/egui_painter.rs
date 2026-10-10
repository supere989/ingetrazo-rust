//! Direct native Metal generic egui painter. Precise authored CAD uses a separate
//! pipeline; this generic Color32 path cannot recover its lost input precision.
use crate::{Buffer, NativeMetal};
use egui::{ClippedPrimitive, TextureFilter, TextureId, TextureOptions, TextureWrapMode};
use ingetrazo_core::gpu_memory::MemoryBudget;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{
    collections::HashMap,
    mem::ManuallyDrop,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy)]
pub enum CompressedFormat {
    Bc1Rgba,
    Astc4x4Ldr,
}
impl CompressedFormat {
    pub fn supported(self, metal: &NativeMetal) -> bool {
        match self {
            Self::Bc1Rgba => metal.device.supportsBCTextureCompression(),
            Self::Astc4x4Ldr => metal.device.supportsFamily(MTLGPUFamily::Apple2),
        }
    }
    fn pixel_format(self) -> MTLPixelFormat {
        match self {
            Self::Bc1Rgba => MTLPixelFormat::BC1_RGBA,
            Self::Astc4x4Ldr => MTLPixelFormat::ASTC_4x4_LDR,
        }
    }
    fn block_bytes(self) -> usize {
        match self {
            Self::Bc1Rgba => 8,
            Self::Astc4x4Ldr => 16,
        }
    }
}
struct Image {
    raw: ManuallyDrop<Retained<ProtocolObject<dyn MTLTexture>>>,
    budget: Arc<Mutex<MemoryBudget>>,
    handle: u64,
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.raw) };
        if let Ok(mut b) = self.budget.lock() {
            let _ = b.freed(self.handle);
        }
    }
}
const CPU_TEXTURE_LIMIT: usize = 256 * 1024 * 1024;
/// Counts every retained texture snapshot, including frames removed from cache.
struct CachedPixels {
    bytes: Vec<u8>,
    charged: usize,
    retained: Arc<std::sync::atomic::AtomicUsize>,
}
impl CachedPixels {
    fn new(count: usize, retained: &Arc<std::sync::atomic::AtomicUsize>) -> Result<Self, String> {
        use std::sync::atomic::Ordering;
        retained
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(count).filter(|n| *n <= CPU_TEXTURE_LIMIT)
            })
            .map_err(|_| "Metal retained CPU texture snapshots exceed256MiB")?;
        let mut result = Self {
            bytes: Vec::new(),
            charged: count,
            retained: retained.clone(),
        };
        result
            .bytes
            .try_reserve_exact(count)
            .map_err(|e| format!("Metal CPU texture allocation: {e}"))?;
        let extra = result.bytes.capacity() - count;
        if extra > 0 {
            retained
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                    used.checked_add(extra).filter(|n| *n <= CPU_TEXTURE_LIMIT)
                })
                .map_err(|_| "Metal CPU texture capacity exceeds admission")?;
            result.charged += extra;
        }
        result.bytes.resize(count, 0);
        Ok(result)
    }
}
impl std::ops::Deref for CachedPixels {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}
impl Drop for CachedPixels {
    fn drop(&mut self) {
        self.retained
            .fetch_sub(self.charged, std::sync::atomic::Ordering::AcqRel);
    }
}
struct Texture {
    image: Image,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    size: [usize; 2],
    pixels: CachedPixels,
}
struct MipOwners {
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    _textures: Vec<Arc<Texture>>,
    _staging: Vec<Buffer>,
    poisoned: Arc<std::sync::atomic::AtomicBool>,
    _queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    _device: Retained<ProtocolObject<dyn MTLDevice>>,
}
struct MipJob {
    owners: ManuallyDrop<MipOwners>,
}
impl Drop for MipJob {
    fn drop(&mut self) {
        if self.owners.command.status() == MTLCommandBufferStatus::Completed {
            unsafe { ManuallyDrop::drop(&mut self.owners) }
        } else {
            self.owners
                .poisoned
                .store(true, std::sync::atomic::Ordering::Release);
            eprintln!("Metal texture/mip upload completion unknown: complete ownership retained");
        }
    }
}
struct MipReadbackOwners {
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    _texture: Arc<Texture>,
    readback: Buffer,
    poisoned: Arc<std::sync::atomic::AtomicBool>,
    _queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    _device: Retained<ProtocolObject<dyn MTLDevice>>,
}
struct MipReadback {
    owners: ManuallyDrop<MipReadbackOwners>,
}
impl Drop for MipReadback {
    fn drop(&mut self) {
        if self.owners.command.status() == MTLCommandBufferStatus::Completed {
            unsafe { ManuallyDrop::drop(&mut self.owners) }
        } else {
            self.owners
                .poisoned
                .store(true, std::sync::atomic::Ordering::Release);
            eprintln!("Metal mip diagnostic readback completion unknown: complete owners retained");
        }
    }
}
struct Draw {
    first: usize,
    count: usize,
    clip: egui::Rect,
    texture: Arc<Texture>,
}
struct Mesh {
    vertices: Buffer,
    indices: Buffer,
    key: blake3::Hash,
    bounds: [[f32; 2]; 2],
}
enum PaintOrder { Mesh(usize), Cad(Arc<crate::cad_callback_painter::Prepared>, egui::Rect), Compute(Arc<crate::cad_compute_painter::Prepared>,egui::Rect) }
pub struct PreparedPaint {
    pipeline: Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    mesh: Option<Arc<Mesh>>,
    draws: Vec<Draw>,
    order: Vec<PaintOrder>,
    density: f32,
    canvas: bool,
}
impl PreparedPaint {
    pub(crate) fn encode(
        &self,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
        extent: (u32, u32),
    ) -> Result<(), String> {
        if self.canvas{return Err("Metal compute canvas requires ordered command encoding".into());}
        let logical = self.mesh.as_ref().map(|mesh|shader_logical_size(mesh.bounds,self.density,extent)).transpose()?;
        for order in &self.order {
            if let PaintOrder::Cad(prepared,clip)=order {
                let [x,y,right,bottom]=clipped_scissor(*clip,self.density,extent)?;
                if right>x&&bottom>y {
                    encoder.setScissorRect(MTLScissorRect{x,y,width:right-x,height:bottom-y});
                    prepared.encode(encoder,extent,self.density)?;
                }
                continue;
            }
            let PaintOrder::Mesh(index)=order else{unreachable!()};
            let draw=&self.draws[*index];
            let mesh=self.mesh.as_ref().ok_or("Metal prepared mesh owner missing")?;
            let logical=logical.as_ref().ok_or("Metal prepared logical domain missing")?;
            encoder.setRenderPipelineState(&self.pipeline);
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.raw),0,0);
                encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(logical).cast(),8,1);
            }
            let [x, y, right, bottom] = clipped_scissor(draw.clip, self.density, extent)?;
            if right == x || bottom == y {
                continue;
            }
            encoder.setScissorRect(MTLScissorRect {
                x,
                y,
                width: right - x,
                height: bottom - y,
            });
            unsafe {
                encoder.setFragmentTexture_atIndex(Some(&draw.texture.image.raw), 0);
                encoder.setFragmentSamplerState_atIndex(Some(&draw.texture.sampler), 0);
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    draw.count,
                    MTLIndexType::UInt32,
                    &mesh.indices.raw,
                    draw.first * 4,
                );
            }
        }
        Ok(())
    }
    pub fn requires_canvas(&self)->bool{self.canvas}
    fn encode_mesh(&self,index:usize,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,extent:(u32,u32))->Result<(),String>{
        let logical=self.mesh.as_ref().map(|mesh|shader_logical_size(mesh.bounds,self.density,extent)).transpose()?;
            let draw=&self.draws[index];
            let mesh=self.mesh.as_ref().ok_or("Metal prepared mesh owner missing")?;
            let logical=logical.as_ref().ok_or("Metal prepared logical domain missing")?;
            encoder.setRenderPipelineState(&self.pipeline);
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.raw),0,0);
                encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(logical).cast(),8,1);
            }
            let [x, y, right, bottom] = clipped_scissor(draw.clip, self.density, extent)?;
            if right == x || bottom == y {
                return Ok(());
            }
            encoder.setScissorRect(MTLScissorRect {
                x,
                y,
                width: right - x,
                height: bottom - y,
            });
            unsafe {
                encoder.setFragmentTexture_atIndex(Some(&draw.texture.image.raw), 0);
                encoder.setFragmentSamplerState_atIndex(Some(&draw.texture.sampler), 0);
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    draw.count,
                    MTLIndexType::UInt32,
                    &mesh.indices.raw,
                    draw.first * 4,
                );
            }
        Ok(())
    }
    pub fn encode_canvas(&self,command:&ProtocolObject<dyn MTLCommandBuffer>,lease:&crate::compute_canvas::Lease,clear:[f64;4])->Result<(),String>{
        let canvas=lease.texture();
        if !self.canvas||canvas.pixelFormat()!=MTLPixelFormat::RGBA8Unorm||canvas.width()==0||canvas.height()==0||canvas.width()>16384||canvas.height()>16384||canvas.textureType()!=MTLTextureType::Type2D||canvas.sampleCount()!=1||!canvas.usage().contains(MTLTextureUsage::RenderTarget|MTLTextureUsage::ShaderRead|MTLTextureUsage::ShaderWrite)||clear.iter().any(|v|!v.is_finite()||!(0. ..=1.).contains(v)){return Err("Metal canvas mode/format/domain mismatch".into());}
        let extent=(canvas.width()as u32,canvas.height()as u32);
        let open=|clear_pass:bool|->Result<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>,String>{
            let pass=MTLRenderPassDescriptor::new();let color=unsafe{pass.colorAttachments().objectAtIndexedSubscript(0)};
            color.setTexture(Some(canvas));color.setLoadAction(if clear_pass{MTLLoadAction::Clear}else{MTLLoadAction::Load});color.setStoreAction(MTLStoreAction::Store);
            color.setClearColor(MTLClearColor{red:clear[0],green:clear[1],blue:clear[2],alpha:clear[3]});
            command.renderCommandEncoderWithDescriptor(&pass).ok_or_else(||"Metal canvas render encoder unavailable".into())
        };
        let mut encoder=Some(open(true)?);
        for order in &self.order{
            match order {
                PaintOrder::Mesh(index)=>{if encoder.is_none(){encoder=Some(open(false)?);}if let Err(error)=self.encode_mesh(*index,encoder.as_ref().unwrap(),extent){encoder.take().unwrap().endEncoding();return Err(error);}},
                PaintOrder::Compute(prepared,clip)=>{
                    if let Some(e)=encoder.take(){e.endEncoding();}
                    let c=clipped_scissor(*clip,self.density,extent)?;
                    if c[2]>c[0]&&c[3]>c[1]{prepared.encode(command,lease.integer_view(),c.map(|v|v as u32))?;}
                },
                PaintOrder::Cad(_,_)=>{if let Some(e)=encoder.take(){e.endEncoding();}return Err("Apple inline CAD cannot bind RGBA compute canvas".into());}
            }
        }
        if let Some(e)=encoder{e.endEncoding();}Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PainterStats {
    pub geometry_uploads: u64,
    pub texture_uploads: u64,
    pub gpu_geometry_bytes: u64,
    pub cpu_texture_bytes: usize,
    pub retained_cpu_texture_bytes: usize,
    pub encoded_source_pixels: u64,
    pub copied_snapshot_bytes: u64,
    pub texture_uploaded_bytes: u64,
    pub mip_generation_submissions: u64,
    pub generated_mip_levels: u64,
    pub compressed_upload_submissions: u64,
    pub compressed_uploaded_bytes: u64,
}
pub struct EguiPainter {
    pipeline: Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    cad: crate::cad_callback_painter::Painter,
    compute: Option<crate::cad_compute_painter::Painter>,
    textures: HashMap<TextureId, Arc<Texture>>,
    mesh: Option<Arc<Mesh>>,
    stats: PainterStats,
    retained_cpu: Arc<std::sync::atomic::AtomicUsize>,
    mip_jobs: std::collections::VecDeque<MipJob>,
}
impl EguiPainter {
    pub fn new(metal: &NativeMetal) -> Result<Self,String>{Self::new_mode(metal,false)}
    pub fn new_compute(metal:&NativeMetal)->Result<Self,String>{
        if !crate::cad_compute_painter::Painter::supported(metal){return Err("Exact Metal canvas read/write unsupported".into());}
        Self::new_mode(metal,true)
    }
    fn new_mode(metal:&NativeMetal,compute:bool)->Result<Self,String>{
        objc2::rc::autoreleasepool(|_| {
            let library = metal
                .device
                .newLibraryWithSource_options_error(
                    &NSString::from_str(include_str!("../shaders/egui.metal")),
                    None,
                )
                .map_err(|e| format!("Metal egui MSL: {e:?}"))?;
            let vertex = library
                .newFunctionWithName(&NSString::from_str("egui_vertex"))
                .ok_or("Missing egui vertex function")?;
            let fragment = library
                .newFunctionWithName(&NSString::from_str("egui_fragment"))
                .ok_or("Missing egui fragment function")?;
            let descriptor = MTLRenderPipelineDescriptor::new();
            descriptor.setVertexFunction(Some(&vertex));
            descriptor.setFragmentFunction(Some(&fragment));
            let color = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
            color.setPixelFormat(if compute{MTLPixelFormat::RGBA8Unorm}else{MTLPixelFormat::BGRA8Unorm});
            color.setBlendingEnabled(true);
            color.setSourceRGBBlendFactor(MTLBlendFactor::One);
            color.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
            color.setSourceAlphaBlendFactor(MTLBlendFactor::One);
            color.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
            let pipeline = metal
                .device
                .newRenderPipelineStateWithDescriptor_error(&descriptor)
                .map_err(|e| format!("Metal egui pipeline: {e:?}"))?;
            Ok(Self {
                pipeline: Arc::new(pipeline),
                cad: crate::cad_callback_painter::Painter::default(),
                compute:compute.then(crate::cad_compute_painter::Painter::default),
                textures: HashMap::new(),
                mesh: None,
                stats: PainterStats::default(),
                retained_cpu: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                mip_jobs: std::collections::VecDeque::new(),
            })
        })
    }
    pub fn cad_stats(&self)->crate::cad_callback_painter::Stats{self.cad.stats}
    pub fn compute_stats(&self)->Option<crate::cad_compute_painter::Stats>{self.compute.as_ref().map(|p|p.stats)}
    pub fn stats(&self) -> PainterStats {
        PainterStats {
            retained_cpu_texture_bytes: self
                .retained_cpu
                .load(std::sync::atomic::Ordering::Acquire),
            ..self.stats
        }
    }
    /// Changed textures are immutable replacements. Earlier submissions retain
    /// their old image/sampler owners; partial updates cannot mutate in-flight GPU reads.
    pub fn update_textures(
        &mut self,
        metal: &NativeMetal,
        delta: &egui::TexturesDelta,
    ) -> Result<(), String> {
        objc2::rc::autoreleasepool(|_| self.update_textures_inner(metal, delta))
    }
    fn update_textures_inner(
        &mut self,
        metal: &NativeMetal,
        delta: &egui::TexturesDelta,
    ) -> Result<(), String> {
        self.poll_mip_jobs()?;
        if !delta.set.is_empty() && self.mip_jobs.len() >= 3 {
            return Err("Metal mip submission capacity reached; delta preserved for retry".into());
        }
        let mut staged = HashMap::new();
        let mut cpu_bytes = self.stats.cpu_texture_bytes;
        let mut copied_bytes = 0u64;
        let mut uploaded_bytes = 0u64;
        for (id, change) in &delta.set {
            let size = change.image.size();
            if size.contains(&0) || size.iter().any(|x| *x > 4096) {
                return Err("Metal egui texture extent exceeds4096".into());
            }
            let font = matches!(&change.image, egui::ImageData::Font(_));
            let source_count = match &change.image {
                egui::ImageData::Color(c) => c.pixels.len(),
                egui::ImageData::Font(f) => f.pixels.len(),
            };
            if source_count != size[0] * size[1] {
                return Err("Malformed egui texture pixel count".into());
            }
            let old = staged.get(id).or_else(|| self.textures.get(id));
            let target_size = if let Some(pos) = change.pos {
                let old: &Arc<Texture> =
                    old.ok_or("Metal partial update has no existing texture")?;
                if old.image.raw.pixelFormat() != MTLPixelFormat::RGBA8Unorm {
                    return Err("RGBA partial update cannot patch compressed blocks".into());
                }
                if pos[0].checked_add(size[0]).is_none_or(|n| n > old.size[0])
                    || pos[1].checked_add(size[1]).is_none_or(|n| n > old.size[1])
                {
                    return Err("Metal partial texture update out of bounds".into());
                }
                old.size
            } else {
                size
            };
            // Reject unsupported sampling before reserving/copying CPU or GPU data.
            let sampler = make_sampler(metal, change.options)?;
            let mut pixels =
                CachedPixels::new(target_size[0] * target_size[1] * 4, &self.retained_cpu)?;
            if change.pos.is_some() {
                pixels.bytes.copy_from_slice(&old.unwrap().pixels);
                copied_bytes += pixels.len() as u64;
            }
            let pos = change.pos.unwrap_or([0, 0]);
            let mut write = |index: usize, color: egui::Color32| {
                let x = index % size[0];
                let y = index / size[0];
                let offset = ((pos[1] + y) * target_size[0] + pos[0] + x) * 4;
                pixels.bytes[offset..offset + 4].copy_from_slice(&encoded_pixel(color, font));
            };
            match &change.image {
                egui::ImageData::Color(c) => {
                    for (i, color) in c.pixels.iter().copied().enumerate() {
                        write(i, color)
                    }
                }
                egui::ImageData::Font(f) => {
                    for (i, color) in f.srgba_pixels(None).enumerate() {
                        write(i, color)
                    }
                }
            }
            if let Some(old) = old {
                cpu_bytes = cpu_bytes
                    .checked_sub(old.pixels.len())
                    .ok_or("CPU texture accounting underflow")?;
            }
            cpu_bytes = cpu_bytes
                .checked_add(pixels.len())
                .ok_or("CPU texture size overflow")?;
            if cpu_bytes > 256 * 1024 * 1024 {
                return Err("Metal egui CPU texture cache exceeds256MiB".into());
            }
            let image = allocate_image(
                metal,
                target_size,
                &pixels,
                change.options.mipmap_mode.is_some(),
            )?;
            uploaded_bytes += pixels.len() as u64;

            staged.insert(
                *id,
                Arc::new(Texture {
                    image,
                    sampler,
                    size: target_size,
                    pixels,
                }),
            );
        }
        // Encode only after the whole replacement transaction has passed CPU,
        // sampler and actual GPU-allocation admission. No CPU completion wait.
        let mipped: Vec<_> = staged
            .values()
            .filter(|t| t.image.raw.mipmapLevelCount() > 1)
            .cloned()
            .collect();
        let generation = if mipped.is_empty() {
            None
        } else {
            let command = metal
                .queue
                .commandBuffer()
                .ok_or("Metal mip command allocation failed")?;
            let encoder = command
                .blitCommandEncoder()
                .ok_or("Metal mip encoder allocation failed")?;
            for texture in &mipped {
                encoder.generateMipmapsForTexture(&texture.image.raw);
            }
            encoder.endEncoding();
            Some((command, mipped))
        };
        self.stats.texture_uploads += staged.len() as u64;
        self.stats.encoded_source_pixels += delta
            .set
            .iter()
            .map(|(_, d)| d.image.size()[0] as u64 * d.image.size()[1] as u64)
            .sum::<u64>();
        self.stats.copied_snapshot_bytes += copied_bytes;
        self.stats.texture_uploaded_bytes += uploaded_bytes;
        self.stats.cpu_texture_bytes = cpu_bytes;
        self.textures.extend(staged);
        if let Some((command, textures)) = generation {
            self.stats.generated_mip_levels += textures
                .iter()
                .map(|t| t.image.raw.mipmapLevelCount() as u64 - 1)
                .sum::<u64>();
            self.stats.mip_generation_submissions += 1;
            // Publish the complete persistent owner before GPU submission.
            self.mip_jobs.push_back(MipJob {
                owners: ManuallyDrop::new(MipOwners {
                    command: command.clone(),
                    _textures: textures,
                    _staging: Vec::new(),
                    poisoned: metal.poisoned.clone(),
                    _queue: metal.queue.clone(),
                    _device: metal.device.clone(),
                }),
            });
            command.commit();
        }
        // Previous frame owns any texture that is still referenced by its draws.
        Ok(())
    }
    /// Immutable native compressed block upload. Container decoding and authored
    /// compressed mip chains are not part of this bounded primitive.
    pub fn upload_compressed_texture(
        &mut self,
        metal: &NativeMetal,
        id: TextureId,
        format: CompressedFormat,
        size: [usize; 2],
        blocks: &[u8],
        options: TextureOptions,
    ) -> Result<(), String> {
        self.poll_mip_jobs()?;
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        if !format.supported(metal) {
            return Err(format!(
                "Native compressed format {format:?} unsupported on {}",
                metal.info.name
            ));
        }
        if self.mip_jobs.len() >= 3 {
            return Err("Metal texture submission capacity reached".into());
        }
        if size.iter().any(|n| *n == 0 || *n > 4096 || n % 4 != 0) {
            return Err("Compressed extent requires whole4x4 blocks and <=4096".into());
        }
        if options.mipmap_mode.is_some() {
            return Err("Compressed authored mip chains unsupported; GPU mip generation cannot write compressed format".into());
        }
        let packed_row = size[0] / 4 * format.block_bytes();
        let rows = size[1] / 4;
        if blocks.len() != packed_row * rows {
            return Err("Compressed block byte count mismatch".into());
        }
        let stride = (packed_row + 255) & !255;
        let staging_bytes = stride * rows;
        if staging_bytes > 8 * 1024 * 1024 {
            return Err("Compressed upload staging exceeds8MiB bounded primitive".into());
        }
        let sampler = make_sampler(metal, options)?;
        let mut pixels = CachedPixels::new(blocks.len(), &self.retained_cpu)?;
        pixels.bytes.copy_from_slice(blocks);
        let image = allocate_compressed_image(metal, size, format, blocks.len())?;
        let mut staging = metal.buffer(staging_bytes, true)?;
        // One directly mapped shared staging owner, no intermediate RGBA decode.
        unsafe {
            let dst = std::slice::from_raw_parts_mut(
                staging.raw.contents().as_ptr().cast::<u8>(),
                staging_bytes,
            );
            dst.fill(0);
            for y in 0..rows {
                dst[y * stride..y * stride + packed_row]
                    .copy_from_slice(&blocks[y * packed_row..(y + 1) * packed_row]);
            }
        }
        let texture = Arc::new(Texture {
            image,
            sampler,
            size,
            pixels,
        });
        let command = metal
            .queue
            .commandBuffer()
            .ok_or("Compressed upload command allocation failed")?;
        let blit = command
            .blitCommandEncoder()
            .ok_or("Compressed upload encoder allocation failed")?;
        unsafe {
            blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(&staging.raw,0,stride,staging_bytes,MTLSize{width:size[0],height:size[1],depth:1},&texture.image.raw,0,0,MTLOrigin{x:0,y:0,z:0});
        }
        blit.endEncoding();
        let old_bytes = self.textures.get(&id).map_or(0, |t| t.pixels.len());
        self.stats.cpu_texture_bytes = self.stats.cpu_texture_bytes - old_bytes + blocks.len();
        self.stats.texture_uploads += 1;
        self.stats.compressed_upload_submissions += 1;
        self.stats.compressed_uploaded_bytes += blocks.len() as u64;
        self.mip_jobs.push_back(MipJob {
            owners: ManuallyDrop::new(MipOwners {
                command: command.clone(),
                _textures: vec![texture.clone()],
                _staging: vec![staging],
                poisoned: metal.poisoned.clone(),
                _queue: metal.queue.clone(),
                _device: metal.device.clone(),
            }),
        });
        self.textures.insert(id, texture);
        command.commit();
        Ok(())
    }
    /// Diagnostic GPU block readback, not a production sampling shortcut.
    pub fn read_compressed_blocks_for_probe(
        &mut self,
        metal: &NativeMetal,
        id: TextureId,
        timeout: std::time::Duration,
    ) -> Result<Vec<u8>, String> {
        if timeout.is_zero() || timeout > std::time::Duration::from_secs(5) {
            return Err("Compressed diagnostic deadline outside bounded domain".into());
        }
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        let texture = self
            .textures
            .get(&id)
            .cloned()
            .ok_or("Missing compressed texture")?;
        let block = match texture.image.raw.pixelFormat() {
            MTLPixelFormat::BC1_RGBA => 8,
            MTLPixelFormat::ASTC_4x4_LDR => 16,
            _ => return Err("Diagnostic texture is not supported compressed format".into()),
        };
        let packed_row = texture.size[0] / 4 * block;
        let rows = texture.size[1] / 4;
        let stride = (packed_row + 255) & !255;
        let bytes = stride * rows;
        let readback = metal.buffer(bytes, true)?;
        let command = metal
            .queue
            .commandBuffer()
            .ok_or("Compressed readback command missing")?;
        let blit = command
            .blitCommandEncoder()
            .ok_or("Compressed readback encoder missing")?;
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(&texture.image.raw,0,0,MTLOrigin{x:0,y:0,z:0},MTLSize{width:texture.size[0],height:texture.size[1],depth:1},&readback.raw,0,stride,bytes);
        }
        blit.endEncoding();
        let retained = MipReadback {
            owners: ManuallyDrop::new(MipReadbackOwners {
                command: command.clone(),
                _texture: texture,
                readback,
                poisoned: metal.poisoned.clone(),
                _queue: metal.queue.clone(),
                _device: metal.device.clone(),
            }),
        };
        command.commit();
        let start = std::time::Instant::now();
        while command.status() != MTLCommandBufferStatus::Completed {
            if command.status() == MTLCommandBufferStatus::Error || start.elapsed() >= timeout {
                return Err("Compressed GPU readback failed/unknown; ownership retained".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let raw = unsafe {
            std::slice::from_raw_parts(
                retained
                    .owners
                    .readback
                    .raw
                    .contents()
                    .as_ptr()
                    .cast::<u8>(),
                bytes,
            )
        };
        let result = (0..rows)
            .flat_map(|y| raw[y * stride..y * stride + packed_row].iter().copied())
            .collect();
        self.poll_mip_jobs()?;
        Ok(result)
    }
    /// Engineering readback only. Production update/sampling never CPU-waits.
    pub fn read_texture_mips_for_probe(
        &mut self,
        metal: &NativeMetal,
        id: TextureId,
        timeout: std::time::Duration,
    ) -> Result<Vec<Vec<u8>>, String> {
        if timeout.is_zero() || timeout > std::time::Duration::from_secs(5) {
            return Err("Mip readback timeout outside bounded diagnostic domain".into());
        }
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        let texture = self
            .textures
            .get(&id)
            .cloned()
            .ok_or("Missing texture diagnostic")?;
        if texture.image.raw.pixelFormat() != MTLPixelFormat::RGBA8Unorm {
            return Err("RGBA mip readback cannot interpret compressed block layout".into());
        }
        let levels = texture.image.raw.mipmapLevelCount();
        let mut layout = Vec::new();
        let mut total = 0;
        for level in 0..levels {
            let width = (texture.size[0] >> level).max(1);
            let height = (texture.size[1] >> level).max(1);
            let row = (width * 4 + 255) & !255;
            layout.push((level, width, height, row, total));
            total += row * height;
        }
        let readback = metal.buffer(total, true)?;
        let command = metal
            .queue
            .commandBuffer()
            .ok_or("Metal mip readback command missing")?;
        let blit = command
            .blitCommandEncoder()
            .ok_or("Metal mip readback encoder missing")?;
        for &(level, width, height, row, offset) in &layout {
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(&texture.image.raw,0,level,MTLOrigin{x:0,y:0,z:0},MTLSize{width,height,depth:1},&readback.raw,offset,row,row*height)
            }
        }
        blit.endEncoding();
        let retained = MipReadback {
            owners: ManuallyDrop::new(MipReadbackOwners {
                command: command.clone(),
                _texture: texture,
                readback,
                poisoned: metal.poisoned.clone(),
                _queue: metal.queue.clone(),
                _device: metal.device.clone(),
            }),
        };
        command.commit();
        let start = std::time::Instant::now();
        loop {
            match command.status() {
                MTLCommandBufferStatus::Completed => break,
                MTLCommandBufferStatus::Error => {
                    return Err("Metal mip readback failed; completion ownership retained".into());
                }
                _ => {}
            }
            if start.elapsed() >= timeout {
                return Err("Metal mip readback bounded timeout; owners retained".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let raw = unsafe {
            std::slice::from_raw_parts(
                retained
                    .owners
                    .readback
                    .raw
                    .contents()
                    .as_ptr()
                    .cast::<u8>(),
                total,
            )
        };
        let result = layout
            .into_iter()
            .map(|(_, width, height, row, offset)| {
                (0..height)
                    .flat_map(|y| {
                        raw[offset + y * row..offset + y * row + width * 4]
                            .iter()
                            .copied()
                    })
                    .collect()
            })
            .collect();
        self.poll_mip_jobs()?;
        Ok(result)
    }
    pub fn poll_mip_jobs(&mut self) -> Result<usize, String> {
        let mut retired = 0;
        while let Some(job) = self.mip_jobs.front() {
            match job.owners.command.status() {
                MTLCommandBufferStatus::Completed => {
                    self.mip_jobs.pop_front();
                    retired += 1;
                }
                MTLCommandBufferStatus::Error => {
                    job.owners
                        .poisoned
                        .store(true, std::sync::atomic::Ordering::Release);
                    return Err("Metal mip generation failed; owners retained".into());
                }
                _ => break,
            }
        }
        Ok(retired)
    }
    pub fn free_textures(&mut self, ids: &[TextureId]) {
        for id in ids {
            if let Some(t) = self.textures.remove(id) {
                self.stats.cpu_texture_bytes -= t.pixels.len();
            }
        }
    }
    pub fn prepare(
        &mut self,
        metal: &NativeMetal,
        primitives: &[ClippedPrimitive],
        density: f32,
    ) -> Result<Arc<PreparedPaint>, String> {
        self.poll_mip_jobs()?;
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        if !density.is_finite() || density <= 0. {
            return Err("Invalid Metal egui pixel density".into());
        }
        if primitives.len()>8192 {return Err("Metal prepared paint exceeds8192 primitive admission".into());}
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut draws = Vec::new();
        let mut order = Vec::new();
        let mut count = 0usize;
        let mut bounds = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
        for primitive in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else {
                let egui::epaint::Primitive::Callback(callback)=&primitive.primitive else{unreachable!()};
                let producer=callback.callback.downcast_ref::<ingetrazo_render::cad_callback::Callback>().ok_or("Metal paint callback type unsupported; no alternate backend fallback")?;
                if !valid_clip(primitive.clip_rect)||!valid_clip(callback.rect){return Err("Invalid Metal CAD callback clip".into());}
                let clip=primitive.clip_rect.intersect(callback.rect);
                if let Some(compute)=self.compute.as_mut(){order.push(PaintOrder::Compute(compute.prepare(metal,producer.snapshot()?,density)?,clip));}
                else{order.push(PaintOrder::Cad(self.cad.prepare(metal,producer.snapshot()?,density)?,clip));}
                continue;
            };
            if !mesh.is_valid()
                || mesh.indices.len() % 3 != 0
                || mesh.vertices.iter().any(|v| {
                    !v.pos.is_finite()
                        || !v.uv.is_finite()
                })
                || !valid_clip(primitive.clip_rect)
            {
                return Err(format!("Invalid egui mesh geometry/clip: vertices={} indices={} valid_indices={} clip={:?} first_invalid_vertex={:?}", mesh.vertices.len(), mesh.indices.len(), mesh.is_valid(), primitive.clip_rect, mesh.vertices.iter().enumerate().find(|(_,v)| !v.pos.is_finite() || !v.uv.is_finite() ).map(|(index,v)|(index,v.pos,v.uv,!v.pos.is_finite(),!v.uv.is_finite(),v.pos.x.abs()>1e6||v.pos.y.abs()>1e6))));
            }
            let base = count;
            count = count
                .checked_add(mesh.vertices.len())
                .ok_or("Mesh overflow")?;
            if count > 1_000_000 || indices.len() / 4 + mesh.indices.len() > 3_000_000 {
                return Err("Metal egui geometry admission exceeded".into());
            }
            let texture = self
                .textures
                .get(&mesh.texture_id)
                .cloned()
                .ok_or("Metal egui texture unavailable")?;
            for v in &mesh.vertices {
                for (axis, value) in [v.pos.x, v.pos.y].into_iter().enumerate() {
                    bounds[axis][0] = bounds[axis][0].min(value);
                    bounds[axis][1] = bounds[axis][1].max(value);
                }
                for f in [v.pos.x, v.pos.y, v.uv.x, v.uv.y] {
                    vertices.extend_from_slice(&f.to_le_bytes());
                }
                vertices.extend_from_slice(&v.color.to_array());
            }
            let first = indices.len() / 4;
            for i in &mesh.indices {
                indices.extend_from_slice(
                    &u32::try_from(base + *i as usize)
                        .map_err(|_| "Index overflow")?
                        .to_le_bytes(),
                );
            }
            order.push(PaintOrder::Mesh(draws.len()));
            draws.push(Draw {
                first,
                count: mesh.indices.len(),
                clip: primitive.clip_rect,
                texture,
            });
        }
        if !indices.is_empty() {
            // Admit a domain safe at every supported physical extent, before
            // allocating or publishing new immutable geometry.
            shader_logical_size(bounds, density, (1, 1))?;
            shader_logical_size(bounds, density, (16384, 16384))?;
            for draw in &draws {
                clipped_scissor(draw.clip, density, (16384, 16384))?;
            }
        }
        let mesh = if indices.is_empty() {
            None
        } else {
            let mut hash = blake3::Hasher::new();
            hash.update(&vertices);
            hash.update(&indices);
            let key = hash.finalize();
            if let Some(old) = self.mesh.as_ref().filter(|old| old.key == key) {
                Some(old.clone())
            } else {
                let mut vb = metal.buffer(vertices.len(), true)?;
                vb.write(&vertices)?;
                let mut ib = metal.buffer(indices.len(), true)?;
                ib.write(&indices)?;
                let result = Arc::new(Mesh {
                    vertices: vb,
                    indices: ib,
                    key,
                    bounds,
                });
                self.stats.geometry_uploads += 1;
                self.stats.gpu_geometry_bytes = (vertices.len() + indices.len()) as u64;
                self.mesh = Some(result.clone());
                Some(result)
            }
        };
        Ok(Arc::new(PreparedPaint {
            pipeline: self.pipeline.clone(),
            mesh,
            draws,
            order,
            density,
            canvas:self.compute.is_some(),
        }))
    }
}
// egui legitimately uses Rect::EVERYTHING for unbounded painter clips.
// NaN is malformed, but infinities become finite only after intersecting with
// the actual physical render target. Never multiply an unbounded clip first.
fn valid_clip(clip: egui::Rect) -> bool {
    [clip.min.x, clip.min.y, clip.max.x, clip.max.y].iter().all(|x| !x.is_nan())
}
fn clipped_scissor(clip: egui::Rect, density: f32, extent: (u32, u32)) -> Result<[usize; 4], String> {
    if !valid_clip(clip) || !density.is_finite() || density <= 0. || extent.0 == 0 || extent.1 == 0 {
        return Err("Invalid Metal scissor domain".into());
    }
    let edge = |value: f32, limit: u32| -> usize {
        // Preserve the renderer's existing f32 multiplication/rounding for
        // admitted finite clips. Unbounded/overflowed endpoints intersect with
        // the target without ever becoming a nonfinite integer conversion.
        let scaled = value * density;
        if scaled == f32::NEG_INFINITY { return 0; }
        if scaled == f32::INFINITY { return limit as usize; }
        scaled.round().clamp(0., limit as f32) as usize
    };
    let x = edge(clip.min.x, extent.0);
    let y = edge(clip.min.y, extent.1);
    Ok([x, y, edge(clip.max.x, extent.0).max(x), edge(clip.max.y, extent.1).max(y)])
}
fn shader_logical_size(
    bounds: [[f32; 2]; 2],
    density: f32,
    extent: (u32, u32),
) -> Result<[f32; 2], String> {
    if !density.is_finite()
        || density <= 0.
        || extent.0 == 0
        || extent.1 == 0
        || extent.0 > 16384
        || extent.1 > 16384
    {
        return Err("Invalid Metal shader domain".into());
    }
    let logical = [extent.0 as f32 / density, extent.1 as f32 / density];
    for axis in 0..2 {
        if !logical[axis].is_finite() || logical[axis] <= 0. {
            return Err("Metal logical extent arithmetic overflow".into());
        }
        for value in bounds[axis] {
            let doubled = 2f32 * value;
            let projected = doubled / logical[axis] - 1f32;
            if !doubled.is_finite() || !projected.is_finite() {
                return Err("Metal vertex shader arithmetic overflow".into());
            }
        }
    }
    Ok(logical)
}
impl Drop for EguiPainter {
    fn drop(&mut self) {
        let _ = self.poll_mip_jobs();
    }
}
fn allocate_compressed_image(
    metal: &NativeMetal,
    size: [usize; 2],
    format: CompressedFormat,
    block_bytes: usize,
) -> Result<Image, String> {
    let ticket = metal
        .budget
        .lock()
        .map_err(|_| "Metal budget poisoned")?
        .reserve(0, block_bytes as u64)?;
    let descriptor = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2D);
    unsafe {
        descriptor.setWidth(size[0]);
        descriptor.setHeight(size[1]);
    }
    descriptor.setPixelFormat(format.pixel_format());
    descriptor.setUsage(MTLTextureUsage::ShaderRead);
    descriptor.setStorageMode(MTLStorageMode::Private);
    let Some(raw) = metal.device.newTextureWithDescriptor(&descriptor) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal budget poisoned")?
            .cancel(ticket)?;
        return Err("Supported compressed texture allocation failed".into());
    };
    let actual = MTLResource::allocatedSize(&*raw) as u64;
    let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
    let mut ledger = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
    if actual != block_bytes as u64 {
        ledger.cancel(ticket)?;
        let replacement = ledger.reserve(0, actual)?;
        ledger.commit(replacement, handle)?;
    } else {
        ledger.commit(ticket, handle)?;
    }
    drop(ledger);
    Ok(Image {
        raw: ManuallyDrop::new(raw),
        budget: metal.budget.clone(),
        handle,
    })
}
fn allocate_image(
    metal: &NativeMetal,
    size: [usize; 2],
    pixels: &[u8],
    mipped: bool,
) -> Result<Image, String> {
    if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
        return Err("Metal admission poisoned".into());
    }
    let levels = if mipped {
        usize::BITS as usize - size[0].max(size[1]).leading_zeros() as usize
    } else {
        1
    };
    let request = (0..levels)
        .map(|level| (size[0] >> level).max(1) as u64 * (size[1] >> level).max(1) as u64 * 4)
        .sum::<u64>();
    let ticket = metal
        .budget
        .lock()
        .map_err(|_| "Metal budget poisoned")?
        .reserve(0, request)?;
    let descriptor = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2D);
    unsafe {
        descriptor.setWidth(size[0]);
        descriptor.setHeight(size[1]);
    }
    descriptor.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    unsafe { descriptor.setMipmapLevelCount(levels) };
    descriptor.setUsage(MTLTextureUsage::ShaderRead);
    descriptor.setStorageMode(if metal.info.unified_memory {
        MTLStorageMode::Shared
    } else {
        MTLStorageMode::Managed
    });
    let Some(raw) = metal.device.newTextureWithDescriptor(&descriptor) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal budget poisoned")?
            .cancel(ticket)?;
        return Err("Metal egui image allocation failed".into());
    };
    let actual = MTLResource::allocatedSize(&*raw) as u64;
    let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
    let mut ledger = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
    if actual != request {
        ledger.cancel(ticket)?;
        let t = ledger.reserve(0, actual)?;
        ledger.commit(t, handle)?;
    } else {
        ledger.commit(ticket, handle)?;
    }
    drop(ledger);
    let image = Image {
        raw: ManuallyDrop::new(raw),
        budget: metal.budget.clone(),
        handle,
    };
    // Texture is unpublished/new, so no submitted GPU command can read it yet.
    unsafe {
        image.raw.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize {
                    width: size[0],
                    height: size[1],
                    depth: 1,
                },
            },
            0,
            std::ptr::NonNull::new(pixels.as_ptr().cast_mut().cast()).ok_or("Empty pixels")?,
            size[0] * 4,
        );
    }
    Ok(image)
}
fn make_sampler(
    metal: &NativeMetal,
    options: TextureOptions,
) -> Result<Retained<ProtocolObject<dyn MTLSamplerState>>, String> {
    let descriptor = MTLSamplerDescriptor::new();
    let filter = |f| {
        if f == TextureFilter::Nearest {
            MTLSamplerMinMagFilter::Nearest
        } else {
            MTLSamplerMinMagFilter::Linear
        }
    };
    descriptor.setMipFilter(match options.mipmap_mode {
        Some(TextureFilter::Nearest) => MTLSamplerMipFilter::Nearest,
        Some(TextureFilter::Linear) => MTLSamplerMipFilter::Linear,
        None => MTLSamplerMipFilter::NotMipmapped,
    });
    descriptor.setMinFilter(filter(options.minification));
    descriptor.setMagFilter(filter(options.magnification));
    let wrap = match options.wrap_mode {
        TextureWrapMode::ClampToEdge => MTLSamplerAddressMode::ClampToEdge,
        TextureWrapMode::Repeat => MTLSamplerAddressMode::Repeat,
        TextureWrapMode::MirroredRepeat => MTLSamplerAddressMode::MirrorRepeat,
    };
    descriptor.setSAddressMode(wrap);
    descriptor.setTAddressMode(wrap);
    metal
        .device
        .newSamplerStateWithDescriptor(&descriptor)
        .ok_or("Metal egui sampler allocation failed".into())
}
fn encoded_pixel(c: egui::Color32, font: bool) -> [u8; 4] {
    let mut out = c.to_array();
    let alpha = out[3] as f32 / 255.;
    if !font && alpha > 0. && alpha < 1. {
        for channel in &mut out[..3] {
            let x = *channel as f32 / 255.;
            let linear = if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            };
            let straight = linear / alpha;
            let encoded = if straight <= 0.0031308 {
                12.92 * straight
            } else {
                1.055 * straight.powf(1. / 2.4) - 0.055
            };
            *channel = (encoded * alpha * 255.).round().clamp(0., 255.) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn far_finite_source_positions_use_shader_domain_not_arbitrary_cap() {
        use super::*;
        let x=19_782_046_f32;let y=9_891_179_f32;
        assert!(x.abs()>1e6&&y.abs()>1e6); // retained old admission rejects
        for extent in [(1,1),(1280,800),(16384,16384)] {
            assert!(shader_logical_size([[-x,x],[-y,y]],1.,extent).is_ok());
        }
        assert!(shader_logical_size([[0.,f32::MAX],[0.,64.]],1.,(64,64)).is_err());
    }

    #[test]
    fn unbounded_clip_is_target_bounded_without_weakening_geometry() {
        use super::*;
        assert!(!egui::Rect::EVERYTHING.is_finite()); // retained old refusal
        assert_eq!(clipped_scissor(egui::Rect::EVERYTHING, 2., (800, 600)).unwrap(), [0,0,800,600]);
        assert_eq!(clipped_scissor(egui::Rect::from_min_max(egui::pos2(-20., 5.), egui::pos2(500., 500.)), 2., (800,600)).unwrap(), [0,10,800,600]);
        assert!(clipped_scissor(egui::Rect::from_min_max(egui::pos2(f32::NAN, 0.), egui::pos2(5.,5.)), 1., (800,600)).is_err());
        assert_eq!(clipped_scissor(egui::Rect::NOTHING, 1., (800,600)).unwrap(), [800,600,800,600]);
        // This literal rounds to 1 in f32, while f64 multiplication rounds to
        // 0: changing precision here changes an existing half-pixel scissor.
        let edge = f32::from_bits(0x3eaaaa9e);
        let scale = f32::from_bits(0x3fc0000e);
        assert_eq!((edge * scale).round(), 1.);
        assert_eq!((f64::from(edge) * f64::from(scale)).round(), 0.);
        let clip = egui::Rect::from_min_max(egui::pos2(edge, edge), egui::pos2(10.,10.));
        assert_eq!(clipped_scissor(clip, scale, (800,600)).unwrap(), [1,1,15,15]);

    }

    use super::*;
    #[test]
    fn retained_cpu_admission_and_last_owner_release() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let retained = Arc::new(AtomicUsize::new(CPU_TEXTURE_LIMIT - 3));
        assert!(CachedPixels::new(4, &retained).is_err());
        assert_eq!(retained.load(Ordering::Acquire), CPU_TEXTURE_LIMIT - 3);
        let pixels = Arc::new(CachedPixels::new(3, &retained).unwrap());
        let frame = pixels.clone();
        drop(pixels);
        assert_eq!(retained.load(Ordering::Acquire), CPU_TEXTURE_LIMIT);
        drop(frame);
        assert_eq!(retained.load(Ordering::Acquire), CPU_TEXTURE_LIMIT - 3);
    }
    #[test]
    fn admits_real_shader_domain_and_refuses_finite_overflow_inputs() {
        assert!(shader_logical_size([[0., 64.], [0., 64.]], 1., (64, 64)).is_ok());
        assert!(shader_logical_size([[0., 64.], [0., 64.]], f32::MAX, (64, 64)).is_err());
        assert!(
            shader_logical_size(
                [[0., 64.], [0., 64.]],
                f32::MIN_POSITIVE / 1000.,
                (16384, 16384)
            )
            .is_err()
        );
        assert!(shader_logical_size([[0., f32::MAX], [0., 64.]], 1., (64, 64)).is_err());
    }
    #[test]
    fn encoded_generic_alpha_does_not_gamma_premultiply_white_twice() {
        assert_eq!(
            encoded_pixel(
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 128),
                false
            ),
            [128, 128, 128, 128]
        );
        let c = egui::Color32::from_rgba_premultiplied(32, 64, 128, 128);
        assert_eq!(encoded_pixel(c, true), [32, 64, 128, 128]);
        assert_eq!(
            encoded_pixel(egui::Color32::from_rgb(12, 34, 56), false),
            [12, 34, 56, 255]
        );
        let additive = egui::Color32::from_rgba_premultiplied(12, 34, 56, 0);
        assert_eq!(encoded_pixel(additive, false), [12, 34, 56, 0]);
    }
}

// This explicit engineering readback owner is not used by the application host.
// Every production paint submits directly to the drawable without a readback.
struct DiagnosticOwners {
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    _paint: Arc<PreparedPaint>,
    _target: Image,
    _canvas:Option<Arc<crate::compute_canvas::Lease>>,
    readback: Buffer,
    gate: Option<Retained<ProtocolObject<dyn MTLSharedEvent>>>,
    width: usize,
    height: usize,
    row: usize,
    poison: Arc<std::sync::atomic::AtomicBool>,
    _queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    _device: Retained<ProtocolObject<dyn MTLDevice>>,
}
pub struct DiagnosticPaint {
    owner: ManuallyDrop<DiagnosticOwners>,
}
impl DiagnosticPaint {
    pub fn completed(&self) -> bool {
        self.owner.command.status() == MTLCommandBufferStatus::Completed
    }
    pub fn probe_gate(&self) -> Option<Retained<ProtocolObject<dyn MTLSharedEvent>>> {
        self.owner.gate.clone()
    }
    pub fn release_probe_gate(&self) {
        if let Some(gate) = self.owner.gate.as_ref() {
            gate.setSignaledValue(1);
        }
    }
    /// Bounded diagnostic wait, never invoked by production presentation.
    pub fn wait_readback(&mut self, timeout: std::time::Duration) -> Result<Vec<u8>, String> {
        if timeout > std::time::Duration::from_secs(5) {
            return Err("Metal diagnostic deadline exceeds5s".into());
        }
        let start = std::time::Instant::now();
        loop {
            if self.completed() {
                break;
            }
            if self.owner.command.status() == MTLCommandBufferStatus::Error
                || start.elapsed() >= timeout
            {
                self.owner
                    .poison
                    .store(true, std::sync::atomic::Ordering::Release);
                return Err(
                    "Metal diagnostic completion unknown; admission poisoned and owners retained"
                        .into(),
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        // Shared buffer is read only after actual Completed, not timeout elapsed.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                self.owner.readback.raw.contents().as_ptr().cast::<u8>(),
                self.owner.row * self.owner.height,
            )
        };
        let mut rgba = Vec::with_capacity(self.owner.width * self.owner.height * 4);
        for y in 0..self.owner.height {
            for pixel in
                bytes[y * self.owner.row..y * self.owner.row + self.owner.width * 4].chunks_exact(4)
            {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
        Ok(rgba)
    }
}
impl Drop for DiagnosticPaint {
    fn drop(&mut self) {
        if self.completed() {
            unsafe {
                ManuallyDrop::drop(&mut self.owner);
            }
        } else {
            self.owner
                .poison
                .store(true, std::sync::atomic::Ordering::Release);
            eprintln!("Metal diagnostic paint owner quarantined: unknown completion");
        }
    }
}
impl EguiPainter {
    /// Actual native render+GPU BLIT to shared readback for independent probes.
    /// A real shared-event gate may deliberately hold submitted work, exclusively
    /// for timeout/ownership controls. No production host calls this method.
    pub fn submit_diagnostic(
        &self,
        metal: &NativeMetal,
        paint: Arc<PreparedPaint>,
        width: usize,
        height: usize,
        blocked_for_probe: bool,
    ) -> Result<DiagnosticPaint, String> {
        let mut pool=crate::compute_canvas::Pool::default();
        self.submit_diagnostic_with_pool(metal,paint,width,height,blocked_for_probe,&mut pool)
    }
    pub fn submit_diagnostic_with_pool(&self,metal:&NativeMetal,paint:Arc<PreparedPaint>,width:usize,height:usize,blocked_for_probe:bool,pool:&mut crate::compute_canvas::Pool)->Result<DiagnosticPaint,String>{
        objc2::rc::autoreleasepool(|_| {
            if width == 0 || height == 0 || width > 4096 || height > 4096 {
                return Err("Invalid diagnostic target extent".into());
            }
            if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
                return Err("Metal admission poisoned".into());
            }
            let row = (width * 4 + 255) & !255;
            let target = allocate_target(metal, [width, height])?;
            let readback = metal.buffer(row * height, true)?;
            let command = metal
                .queue
                .commandBuffer()
                .ok_or("Diagnostic command creation failed")?;
            let gate = if blocked_for_probe {
                Some(
                    metal
                        .device
                        .newSharedEvent()
                        .ok_or("Metal shared-event gate unavailable")?,
                )
            } else {
                None
            };
            if let Some(gate) = gate.as_ref() {
                command.encodeWaitForEvent_value(ProtocolObject::from_ref(&**gate), 1);
            }
            let canvas=if paint.requires_canvas(){
                let canvas=pool.acquire(metal,(width as u32,height as u32))?.ok_or("Diagnostic canvas capacity unavailable")?;
                paint.encode_canvas(&command,&canvas,[1.;4])?;canvas.present(&command,&target.raw)?;Some(canvas)
            }else{
            let descriptor = MTLRenderPassDescriptor::new();
            let color = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
            color.setTexture(Some(&target.raw));
            color.setLoadAction(MTLLoadAction::Clear);
            color.setStoreAction(MTLStoreAction::Store);
            color.setClearColor(MTLClearColor {
                red: 1.,
                green: 1.,
                blue: 1.,
                alpha: 1.,
            });
            let encoder = command
                .renderCommandEncoderWithDescriptor(&descriptor)
                .ok_or("Diagnostic render encoder unavailable")?;
            if let Err(error) = paint.encode(&encoder, (width as u32, height as u32)) {
                encoder.endEncoding();
                return Err(error);
            }
            encoder.endEncoding();
                None
            };
            let blit = command
                .blitCommandEncoder()
                .ok_or("Diagnostic BLIT encoder unavailable")?;
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(&target.raw,0,0,MTLOrigin{x:0,y:0,z:0},MTLSize{width,height,depth:1},&readback.raw,0,row,row*height);
            }
            blit.endEncoding();
            let result = DiagnosticPaint {
                owner: ManuallyDrop::new(DiagnosticOwners {
                    command,
                    _paint: paint,
                    _target: target,
                    _canvas:canvas,
                    readback,
                    gate,
                    width,
                    height,
                    row,
                    poison: metal.poisoned.clone(),
                    _queue: metal.queue.clone(),
                    _device: metal.device.clone(),
                }),
            };
            result.owner.command.commit();
            Ok(result)
        })
    }
}
fn allocate_target(metal: &NativeMetal, size: [usize; 2]) -> Result<Image, String> {
    let ticket = metal
        .budget
        .lock()
        .map_err(|_| "Metal budget poisoned")?
        .reserve(0, (size[0] * size[1] * 4) as u64)?;
    let descriptor = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2D);
    unsafe {
        descriptor.setWidth(size[0]);
        descriptor.setHeight(size[1]);
    }
    descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    descriptor.setStorageMode(MTLStorageMode::Private);
    descriptor.setUsage(MTLTextureUsage::RenderTarget);
    let Some(raw) = metal.device.newTextureWithDescriptor(&descriptor) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal budget poisoned")?
            .cancel(ticket)?;
        return Err("Metal diagnostic target allocation failed".into());
    };
    let actual = MTLResource::allocatedSize(&*raw) as u64;
    let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
    let mut ledger = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
    ledger.cancel(ticket)?;
    let admitted = ledger.reserve(0, actual)?;
    ledger.commit(admitted, handle)?;
    drop(ledger);
    Ok(Image {
        raw: ManuallyDrop::new(raw),
        budget: metal.budget.clone(),
        handle,
    })
}
