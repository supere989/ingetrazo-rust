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
struct Texture {
    image: Image,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    size: [usize; 2],
    pixels: Vec<u8>,
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
pub struct PreparedPaint {
    pipeline: Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    mesh: Option<Arc<Mesh>>,
    draws: Vec<Draw>,
    density: f32,
}
impl PreparedPaint {
    pub(crate) fn encode(
        &self,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
        extent: (u32, u32),
    ) -> Result<(), String> {
        let Some(mesh) = self.mesh.as_ref() else {
            return Ok(());
        };
        let logical = shader_logical_size(mesh.bounds, self.density, extent)?;
        // Reject actual f32 scissor multiplication overflow before encoding.
        for draw in &self.draws {
            for point in [draw.clip.min, draw.clip.max] {
                if !(point * self.density).is_finite() {
                    return Err("Metal scissor arithmetic overflow".into());
                }
            }
        }
        encoder.setRenderPipelineState(&self.pipeline);
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.raw), 0, 0);
            encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&logical).cast(), 8, 1);
        }
        for draw in &self.draws {
            let min = draw.clip.min * self.density;
            let max = draw.clip.max * self.density;
            let x = min.x.round().clamp(0., extent.0 as f32) as usize;
            let y = min.y.round().clamp(0., extent.1 as f32) as usize;
            let right = max.x.round().clamp(x as f32, extent.0 as f32) as usize;
            let bottom = max.y.round().clamp(y as f32, extent.1 as f32) as usize;
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
}
#[derive(Default, Debug, Clone, Copy)]
pub struct PainterStats {
    pub geometry_uploads: u64,
    pub texture_uploads: u64,
    pub gpu_geometry_bytes: u64,
    pub cpu_texture_bytes: usize,
}
pub struct EguiPainter {
    pipeline: Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    textures: HashMap<TextureId, Arc<Texture>>,
    mesh: Option<Arc<Mesh>>,
    stats: PainterStats,
}
impl EguiPainter {
    pub fn new(metal: &NativeMetal) -> Result<Self, String> {
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
            color.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
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
                textures: HashMap::new(),
                mesh: None,
                stats: PainterStats::default(),
            })
        })
    }
    pub fn stats(&self) -> PainterStats {
        self.stats
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
        let mut staged = HashMap::new();
        let mut cpu_bytes = self.stats.cpu_texture_bytes;
        for (id, change) in &delta.set {
            let size = change.image.size();
            if size.contains(&0) || size.iter().any(|x| *x > 4096) {
                return Err("Metal egui texture extent exceeds4096".into());
            }
            let font = matches!(&change.image, egui::ImageData::Font(_));
            let colors: Vec<egui::Color32> = match &change.image {
                egui::ImageData::Color(c) => c.pixels.clone(),
                egui::ImageData::Font(f) => f.srgba_pixels(None).collect(),
            };
            if colors.len() != size[0] * size[1] {
                return Err("Malformed egui texture pixel count".into());
            }
            let rgba: Vec<u8> = colors
                .into_iter()
                .flat_map(|c| encoded_pixel(c, font))
                .collect();
            let old = staged.get(id).or_else(|| self.textures.get(id));
            let (target_size, mut pixels) = if let Some(pos) = change.pos {
                let old: &Arc<Texture> =
                    old.ok_or("Metal partial update has no existing texture")?;
                if pos[0].checked_add(size[0]).is_none_or(|n| n > old.size[0])
                    || pos[1].checked_add(size[1]).is_none_or(|n| n > old.size[1])
                {
                    return Err("Metal partial texture update out of bounds".into());
                }
                let mut bytes = old.pixels.clone();
                for y in 0..size[1] {
                    let offset = ((pos[1] + y) * old.size[0] + pos[0]) * 4;
                    bytes[offset..offset + size[0] * 4]
                        .copy_from_slice(&rgba[y * size[0] * 4..(y + 1) * size[0] * 4]);
                }
                (old.size, bytes)
            } else {
                (size, rgba)
            };
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
            let image = allocate_image(metal, target_size, &pixels)?;
            let sampler = make_sampler(metal, change.options)?;
            staged.insert(
                *id,
                Arc::new(Texture {
                    image,
                    sampler,
                    size: target_size,
                    pixels: std::mem::take(&mut pixels),
                }),
            );
        }
        self.stats.texture_uploads += staged.len() as u64;
        self.stats.cpu_texture_bytes = cpu_bytes;
        self.textures.extend(staged);
        // Previous frame owns any texture that is still referenced by its draws.
        Ok(())
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
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        if !density.is_finite() || density <= 0. {
            return Err("Invalid Metal egui pixel density".into());
        }
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut draws = Vec::new();
        let mut count = 0usize;
        let mut bounds = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
        for primitive in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else {
                return Err(
                    "Metal egui paint callback unsupported: no alternate backend fallback".into(),
                );
            };
            if !mesh.is_valid()
                || mesh.indices.len() % 3 != 0
                || mesh.vertices.iter().any(|v| {
                    !v.pos.is_finite()
                        || !v.uv.is_finite()
                        || v.pos.x.abs() > 1e6
                        || v.pos.y.abs() > 1e6
                })
                || !primitive.clip_rect.is_finite()
            {
                return Err("Invalid egui mesh geometry/clip".into());
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
                for point in [draw.clip.min, draw.clip.max] {
                    if !(point * density).is_finite() {
                        return Err("Metal scissor arithmetic overflow".into());
                    }
                }
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
            density,
        }))
    }
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
fn allocate_image(metal: &NativeMetal, size: [usize; 2], pixels: &[u8]) -> Result<Image, String> {
    if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
        return Err("Metal admission poisoned".into());
    }
    let request = pixels.len() as u64;
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
    if options.mipmap_mode.is_some() {
        return Err("Metal egui mipmap filtering unsupported".into());
    }
    let descriptor = MTLSamplerDescriptor::new();
    let filter = |f| {
        if f == TextureFilter::Nearest {
            MTLSamplerMinMagFilter::Nearest
        } else {
            MTLSamplerMinMagFilter::Linear
        }
    };
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
    use super::*;
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
