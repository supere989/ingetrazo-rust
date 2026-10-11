//! Native Apple framebuffer-fetch consumption of frozen CAD producers.
//! Intel/AMD exact span-compute presentation remains an explicit boundary.
use crate::{Buffer, NativeMetal};
use ingetrazo_render::{cad2d::Batch, cad_callback::Snapshot};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{collections::HashMap, sync::Arc};
use crate::cad_abi::DrawPush as Push;
struct Cached {
    vertices: Buffer,
    indices: Buffer,
    bounds: [[f32; 2]; 2],
}
#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub deep_admissions: u64,
    pub uploads: u64,
    pub warm_hits: u64,
    pub palette_preparations: u64,
    pub bytes: u64,
}
pub struct Painter {
    pipeline_annotation:bool,
    glyph_admissions:crate::glyph_admission::Cache,
    pipeline: Option<Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>>,
    geometry: HashMap<u64, Arc<Cached>>,
    palette: HashMap<(u64, usize), (Arc<[Batch]>, Arc<[[f32; 4]]>)>,
    pub stats: Stats,
}
impl Default for Painter {
    fn default() -> Self {
        Self {
            pipeline_annotation:false,
            glyph_admissions:crate::glyph_admission::Cache::default(),
            pipeline: None,
            geometry: HashMap::new(),
            palette: HashMap::new(),
            stats: Stats::default(),
        }
    }
}
pub struct Prepared {
    pub(crate) annotation:Option<Arc<crate::annotation::Visibility>>,
    pipeline: Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    geometry: Arc<Cached>,
    snapshot: Snapshot,
    colors: Arc<[[f32; 4]]>,
}
impl Painter {
    pub(crate) fn reusing_pipeline(ready:&Prepared)->Self{let mut painter=Self::default();painter.pipeline=Some(ready.pipeline.clone());painter.pipeline_annotation=ready.annotation.is_some();painter}
    pub fn prepare(
        &mut self,
        metal: &NativeMetal,
        snapshot: Snapshot,
        density: f32,
    ) -> Result<Arc<Prepared>, String> {
        self.prepare_annotation(metal,snapshot,density,None)
    }
    pub(crate) fn prepare_annotation(&mut self,metal:&NativeMetal,snapshot:Snapshot,density:f32,annotation:Option<Arc<crate::annotation::Visibility>>)->Result<Arc<Prepared>,String>{
        if self.pipeline_annotation!=annotation.is_some(){self.pipeline=None;}
        if !crate::cad2d::MetalCad2d::supported(metal) {
            return Err("Metal CAD callback exact Apple framebuffer-fetch unsupported; Intel/AMD span-compute presentation still required".into());
        }
        if snapshot.batches.len() > 4096 {
            return Err("Metal CAD palette exceeds4096 batch admission".into());
        }
        self.glyph_admissions.admit(&snapshot.batches,snapshot.glyphs.as_ref())?;
        snapshot
            .geometry
            .validate_ordered_pose(snapshot.pose, density)?;
        let id = snapshot.geometry.id();
        let cached = self.geometry.get(&id).cloned();
        let bounds = if let Some(c) = &cached {
            c.bounds
        } else {
            let mut b = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
            for v in snapshot.geometry.vertices() {
                for axis in 0..2 {
                    b[axis][0] = b[axis][0].min(v.position[axis]);
                    b[axis][1] = b[axis][1].max(v.position[axis]);
                }
            }
            b
        };
        validate_pose(bounds, &snapshot, density)?;
        let palette_key = (
            snapshot.geometry.palette_cache_identity(),
            Arc::as_ptr(&snapshot.batches) as *const Batch as usize,
        );
        let palette_changed = !self.palette.contains_key(&palette_key);
        if cached.is_none() || palette_changed {
            self.stats.deep_admissions += 1;
            snapshot.geometry.validate_batches(&snapshot.batches)?;
        }
        let colors: Arc<[[f32; 4]]> = if let Some((_, colors)) = self.palette.get(&palette_key) {
            colors.clone()
        } else {
            snapshot
                .geometry
                .resolve_batch_colors(&snapshot.batches)?
                .into()
        };
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            let options = MTLCompileOptions::new();
            options.setFastMathEnabled(false);
            let library = metal
                .device
                .newLibraryWithSource_options_error(
                    &NSString::from_str(&format!("{}{}",if annotation.is_some(){"#include <metal_stdlib>\nusing namespace metal;\n#define ANNOTATION 1\n"}else{"#include <metal_stdlib>\nusing namespace metal;\n"},format!("{}{}",if annotation.is_some(){include_str!("shaders/annotation_draw.metal")}else{""},include_str!("shaders/cad2d.metal")))),
                    Some(&options),
                )
                .map_err(|e| format!("Metal inline CAD shader: {e:?}"))?;
            let desc = MTLRenderPipelineDescriptor::new();
            let vertex = library
                .newFunctionWithName(&NSString::from_str("cad_vertex"))
                .ok_or("CAD vertex missing")?;
            let fragment = library
                .newFunctionWithName(&NSString::from_str("cad_fragment"))
                .ok_or("CAD fragment missing")?;
            desc.setVertexFunction(Some(&vertex));
            desc.setFragmentFunction(Some(&fragment));
            let color = unsafe { desc.colorAttachments().objectAtIndexedSubscript(0) };
            color.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            color.setBlendingEnabled(false);
            Arc::new(
                metal
                    .device
                    .newRenderPipelineStateWithDescriptor_error(&desc)
                    .map_err(|e| format!("Metal inline CAD pipeline: {e:?}"))?,
            )
        };
        let geometry = if let Some(c) = cached {
            self.stats.warm_hits += 1;
            c
        } else {
            let vertex_bytes = bytemuck::cast_slice(snapshot.geometry.vertices());
            let index_bytes = bytemuck::cast_slice(snapshot.geometry.indices());
            if vertex_bytes.is_empty() || index_bytes.is_empty() {
                return Err("Metal inline CAD empty geometry".into());
            }
            let mut vertices = metal.buffer(vertex_bytes.len(), true)?;
            vertices.write(vertex_bytes)?;
            let mut indices = metal.buffer(index_bytes.len(), true)?;
            indices.write(index_bytes)?;
            let c = Arc::new(Cached {
                vertices,
                indices,
                bounds,
            });
            self.stats.uploads += 1;
            self.stats.bytes += snapshot.geometry.logical_bytes();
            c
        };
        // Publish only after every allocation/validation succeeds. Earlier prepared
        // snapshots retain their buffers independent of eviction and producer edits.
        if self.geometry.len() >= 64 && !self.geometry.contains_key(&id) {
            self.geometry.clear();
        }
        self.geometry.insert(id, geometry.clone());
        self.pipeline = Some(pipeline.clone());self.pipeline_annotation=annotation.is_some();
        if palette_changed {
            if self.palette.len() >= 64 {
                self.palette.clear();
            }
            self.stats.palette_preparations += 1;
            self.palette
                .insert(palette_key, (snapshot.batches.clone(), colors.clone()));
        }
        Ok(Arc::new(Prepared {
            annotation,
            pipeline,
            geometry,
            snapshot,
            colors,
        }))
    }
}
fn validate_pose(bounds: [[f32; 2]; 2], snapshot: &Snapshot, density: f32) -> Result<(), String> {
    snapshot.pose.validate()?;
    if !density.is_finite() || density <= 0. {
        return Err("Invalid Metal CAD density".into());
    }
    for axis in 0..2 {
        for v in bounds[axis] {
            let product = v * snapshot.pose.scale[axis];
            let sum = product + snapshot.pose.translation[axis];
            let physical = sum * density;
            if !product.is_finite()
                || !sum.is_finite()
                || !physical.is_finite()
                || physical.abs() > 1e6
                || !(sum * 2.).is_finite()
            {
                return Err("Metal CAD callback shader pose domain".into());
            }
        }
    }
    Ok(())
}
impl Prepared {
    pub(crate) fn encode(
        &self,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
        extent: (u32, u32),
        density: f32,
    ) -> Result<(), String> {
        validate_pose(self.geometry.bounds, &self.snapshot, density)?;
        let screen = [extent.0 as f32 / density, extent.1 as f32 / density];
        if screen.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return Err("Metal CAD callback screen domain".into());
        }
        encoder.setRenderPipelineState(&self.pipeline);
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&self.geometry.vertices.raw), 0, 0);
        }
        for (ordinal, (batch, color)) in self
            .snapshot
            .batches
            .iter()
            .zip(self.colors.iter())
            .enumerate()
        {
            let push = Push {
                scale: self.snapshot.pose.scale,
                translation: self.snapshot.pose.translation,
                screen,
                pad: [0.; 2],
                rgba: *color,
                glyph: self
                    .snapshot
                    .glyphs
                    .as_ref()
                    .map(|g| {
                        let op = g[ordinal];
                        [op.composition, u32::from_le_bytes(op.straight_rgba), 0, 0]
                    })
                    .unwrap_or([0; 4]),
            };
            if let Some(annotation)=&self.annotation {unsafe{encoder.setFragmentBuffer_offset_atIndex(Some(&annotation.draws.raw),ordinal*std::mem::size_of::<crate::annotation::Draw>(),2);encoder.setFragmentBuffer_offset_atIndex(Some(&annotation.hidden.raw),0,3);}}
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
                    &self.geometry.indices.raw,
                    batch.first as usize * 4,
                );
            }
        }
        Ok(())
    }
}
