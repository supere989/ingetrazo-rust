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
        let logical = [
            extent.0 as f32 / self.density,
            extent.1 as f32 / self.density,
        ];
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
