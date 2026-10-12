//! Cached immutable physical picture buffers, on the existing native paper target.
use crate::{Buffer, NativeMetal};
use ingetrazo_render::paper_picture::{Pixels, Sampling, Snapshot};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc, Weak},
};
struct Image {
    owner: Arc<Pixels>,
    buffer: Buffer,
}
struct Rows {
    owner: Arc<Sampling>,
    buffer: Buffer,
}
enum Pipeline {
    Inline(Retained<ProtocolObject<dyn MTLRenderPipelineState>>),
    Compute(Retained<ProtocolObject<dyn MTLComputePipelineState>>),
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub image_uploads: u64,
    pub row_uploads: u64,
    pub uploaded_bytes: u64,
    pub warm_hits: u64,
}
#[derive(Default)]
pub struct Painter {
    owner: Option<Weak<AtomicBool>>,
    pipeline: Option<Arc<Pipeline>>,
    images: HashMap<u64, Arc<Image>>,
    rows: HashMap<usize, Arc<Rows>>,
    pub stats: Stats,
}
pub struct Prepared {
    pipeline: Arc<Pipeline>,
    image: Arc<Image>,
    rows: Arc<Rows>,
    snapshot: Snapshot,
    push: [i32; 24],
}
impl Painter {
    pub fn prepare(
        &mut self,
        metal: &NativeMetal,
        snapshot: Snapshot,
        compute: bool,
    ) -> Result<Option<Arc<Prepared>>, String> {
        if self.owner.as_ref().is_some_and(|v| {
            v.upgrade()
                .is_none_or(|v| !Arc::ptr_eq(&v, &metal.poisoned))
        }) {
            return Err("Paper picture differs from native device owner".into());
        }
        let origin = snapshot
            .origin
            .map(|v| u32::try_from(v).map_err(|_| "Paper picture tile origin is negative"));
        let origin = [origin[0].clone()?, origin[1].clone()?];
        let push = snapshot.sampling.native_push([1, 1], origin)?;
        if snapshot.sampling.fy.is_empty() || snapshot.sampling.x[2] < snapshot.sampling.x[0] {
            return Ok(None);
        }
        let mode = match self.pipeline.as_deref() {
            Some(Pipeline::Inline(_)) => false,
            Some(Pipeline::Compute(_)) => true,
            None => compute,
        };
        if mode != compute {
            return Err("Paper picture pipeline mode changed".into());
        }
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            let options = MTLCompileOptions::new();
            options.setFastMathEnabled(false);
            let source = format!(
                "{}{}",
                if compute {
                    "#define PP_COMPUTE_ONLY 1\n"
                } else {
                    ""
                },
                include_str!("shaders/paper_picture.metal")
            );
            let library = metal
                .device
                .newLibraryWithSource_options_error(&NSString::from_str(&source), Some(&options))
                .map_err(|e| format!("Metal paper picture shader:{e:?}"))?;
            Arc::new(if compute {
                let function = library
                    .newFunctionWithName(&NSString::from_str("pp_compute"))
                    .ok_or("Paper picture compute missing")?;
                let p = metal
                    .device
                    .newComputePipelineStateWithFunction_error(&function)
                    .map_err(|e| format!("Metal picture compute pipeline:{e:?}"))?;
                if p.maxTotalThreadsPerThreadgroup() < 64 {
                    return Err("Paper picture requires8x8 compute workgroup".into());
                }
                Pipeline::Compute(p)
            } else {
                let descriptor = MTLRenderPipelineDescriptor::new();
                let vertex = library
                    .newFunctionWithName(&NSString::from_str("pp_vertex"))
                    .ok_or("Paper picture vertex missing")?;
                let fragment = library
                    .newFunctionWithName(&NSString::from_str("pp_fragment"))
                    .ok_or("Paper picture fragment missing")?;
                descriptor.setVertexFunction(Some(&vertex));
                descriptor.setFragmentFunction(Some(&fragment));
                let color = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
                color.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
                color.setBlendingEnabled(false);
                Pipeline::Inline(
                    metal
                        .device
                        .newRenderPipelineStateWithDescriptor_error(&descriptor)
                        .map_err(|e| format!("Metal picture inline pipeline:{e:?}"))?,
                )
            })
        };
        let image_id = snapshot.sampling.image.id();
        let cached = self.images.get(&image_id).cloned();
        let fresh_image = cached.is_none();
        let image = if let Some(p) = cached {
            if !Arc::ptr_eq(&p.owner, &snapshot.sampling.image) {
                return Err("Paper picture image identity conflict".into());
            }
            p
        } else {
            let mut buffer = metal.buffer(snapshot.sampling.image.rgba().len(), true)?;
            buffer.write(snapshot.sampling.image.rgba())?;
            Arc::new(Image {
                owner: snapshot.sampling.image.clone(),
                buffer,
            })
        };
        let key = Arc::as_ptr(&snapshot.sampling) as usize;
        let cached = self.rows.get(&key).cloned();
        let fresh_rows = cached.is_none();
        let rows = if let Some(p) = cached {
            if !Arc::ptr_eq(&p.owner, &snapshot.sampling) {
                return Err("Paper picture row identity conflict".into());
            }
            p
        } else {
            let data = bytemuck::cast_slice(&snapshot.sampling.fy);
            let mut buffer = metal.buffer(data.len(), true)?;
            buffer.write(data)?;
            Arc::new(Rows {
                owner: snapshot.sampling.clone(),
                buffer,
            })
        };
        // Retained Prepared owners keep evicted buffers alive; nothing publishes on failure.
        self.images
            .try_reserve(1)
            .map_err(|_| "Paper picture image cache allocation failed")?;
        self.rows
            .try_reserve(1)
            .map_err(|_| "Paper picture row cache allocation failed")?;
        if self.images.len() >= 24 && !self.images.contains_key(&image_id) {
            self.images.clear();
        }
        if self.rows.len() >= 64 && !self.rows.contains_key(&key) {
            self.rows.clear();
        }
        self.images.insert(image_id, image.clone());
        self.rows.insert(key, rows.clone());
        self.pipeline = Some(pipeline.clone());
        self.owner = Some(Arc::downgrade(&metal.poisoned));
        self.stats.image_uploads += u64::from(fresh_image);
        self.stats.row_uploads += u64::from(fresh_rows);
        self.stats.uploaded_bytes += if fresh_image {
            image.buffer.length() as u64
        } else {
            0
        } + if fresh_rows {
            rows.buffer.length() as u64
        } else {
            0
        };
        self.stats.warm_hits += u64::from(!fresh_image && !fresh_rows);
        Ok(Some(Arc::new(Prepared {
            pipeline,
            image,
            rows,
            snapshot,
            push,
        })))
    }
}
impl Prepared {
    fn push(&self, extent: [u32; 2]) -> Result<[i32; 24], String> {
        if extent.contains(&0) || extent.iter().any(|&n| n > 16384) {
            return Err("Paper picture target extent invalid".into());
        }
        let mut p = self.push;
        p[0] = extent[0] as i32;
        p[1] = extent[1] as i32;
        Ok(p)
    }
    pub fn encode_inline(
        &self,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
        extent: (u32, u32),
    ) -> Result<(), String> {
        let Pipeline::Inline(pipeline) = &*self.pipeline else {
            return Err("Paper picture compute requires canvas".into());
        };
        let push = self.push([extent.0, extent.1])?;
        encoder.setRenderPipelineState(pipeline);
        unsafe {
            encoder.setFragmentBuffer_offset_atIndex(Some(&self.image.buffer.raw), 0, 0);
            encoder.setFragmentBuffer_offset_atIndex(Some(&self.rows.buffer.raw), 0, 1);
            encoder.setFragmentBytes_length_atIndex(
                std::ptr::NonNull::from(&push).cast(),
                std::mem::size_of_val(&push),
                2,
            );
            // The shader generates one full-screen triangle from vertex_id;
            // both fragment buffers and the push constants are bound above.
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        Ok(())
    }
    pub fn encode_compute(
        &self,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
        canvas: &ProtocolObject<dyn MTLTexture>,
        clip: [u32; 4],
    ) -> Result<(), String> {
        let Pipeline::Compute(pipeline) = &*self.pipeline else {
            return Err("Paper picture inline cannot bind compute canvas".into());
        };
        if canvas.pixelFormat() != MTLPixelFormat::R32Uint
            || canvas.textureType() != MTLTextureType::Type2D
            || canvas.sampleCount() != 1
            || !canvas
                .usage()
                .contains(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite)
        {
            return Err("Paper picture canvas format unsupported".into());
        }
        let mut push = self.push([canvas.width() as u32, canvas.height() as u32])?;
        if clip[0] > clip[2]
            || clip[1] > clip[3]
            || clip[2] > canvas.width() as u32
            || clip[3] > canvas.height() as u32
        {
            return Err("Paper picture clip invalid".into());
        }
        if clip[0] == clip[2] || clip[1] == clip[3] {
            return Ok(());
        }
        push[0] = (clip[2] - clip[0]) as i32;
        push[1] = (clip[3] - clip[1]) as i32;
        push[2] += clip[0] as i32;
        push[3] += clip[1] as i32;
        push[23] = (clip[0] | (clip[1] << 16)) as i32;
        let encoder = command
            .computeCommandEncoder()
            .ok_or("Paper picture compute encoder failed")?;
        encoder.setComputePipelineState(pipeline);
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&self.image.buffer.raw), 0, 0);
            encoder.setBuffer_offset_atIndex(Some(&self.rows.buffer.raw), 0, 1);
            encoder.setBytes_length_atIndex(
                std::ptr::NonNull::from(&push).cast(),
                std::mem::size_of_val(&push),
                2,
            );
            encoder.setTexture_atIndex(Some(canvas), 0);
            encoder.dispatchThreadgroups_threadsPerThreadgroup(
                MTLSize {
                    width: ((clip[2] - clip[0] + 7) / 8) as usize,
                    height: ((clip[3] - clip[1] + 7) / 8) as usize,
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
        Ok(())
    }
}
