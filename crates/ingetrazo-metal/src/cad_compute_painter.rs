//! Exact source-ordered CAD composition into an existing native RGBA8 canvas.
//! Prepared owners must survive the command completion; encoding never submits/waits.
use crate::{Buffer, NativeMetal};
use ingetrazo_render::{cad2d::Batch, cad_callback::Snapshot};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{collections::HashMap, sync::Arc};
struct Plan {
    spans: Buffer,
    tiles: Buffer,
    references: Buffer,
    tile_count: u32,
    batches: Arc<[Batch]>,
}
struct Palette {
    owner: Arc<[Batch]>,
    _glyphs: Option<Arc<[ingetrazo_render::cad_callback::GlyphOperator]>>,
    buffer: Buffer,
}
#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub deep_admissions: u64,
    pub plan_uploads: u64,
    pub palette_uploads: u64,
    pub warm_hits: u64,
    pub uploaded_bytes: u64,
}
#[derive(Default)]
pub struct Painter {
    pipeline_annotation:bool,
    glyph_admissions:crate::glyph_admission::Cache,
    pipeline: Option<Arc<Retained<ProtocolObject<dyn MTLComputePipelineState>>>>,
    plans: HashMap<u64, Arc<Plan>>,
    palettes: HashMap<(u64, usize, usize), Arc<Palette>>,
    pub stats: Stats,
}
pub struct Prepared {
    pub(crate) annotation:Option<Arc<crate::annotation::Visibility>>,
    pipeline: Arc<Retained<ProtocolObject<dyn MTLComputePipelineState>>>,
    plan: Arc<Plan>,
    palette: Arc<Palette>,
    snapshot: Snapshot,
    translation: [i32; 2],
}
#[repr(C)]
struct Push {
    translation: [i32; 2],
    size: [u32; 2],
    clip: [u32; 4],
    tiles: u32,
    pad: [u32; 3],
}
impl Painter {
    pub fn supported(metal: &NativeMetal) -> bool {
        matches!(
            metal.device.readWriteTextureSupport(),
            MTLReadWriteTextureTier::Tier1 | MTLReadWriteTextureTier::Tier2
        )
    }
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
        if self.pipeline_annotation!=annotation.is_some(){self.pipeline=None;self.plans.clear();}
        if !Self::supported(metal) {
            return Err("Exact Metal R32Uint read/write composition requires texture tier1".into());
        }
        self.glyph_admissions.admit(&snapshot.batches,snapshot.glyphs.as_ref())?;
        snapshot
            .geometry
            .validate_ordered_pose(snapshot.pose, density)?;
        let translation=if annotation.is_some(){snapshot.geometry.annotation_layers().ok_or("Metal annotation source metadata missing")?.translation(snapshot.pose,density)?}else{snapshot.geometry.source_spans().ok_or("Exact Metal compute requires original source span metadata; arbitrary geometry remains unsupported")?.translation(snapshot.pose,density)?};
        let id = snapshot.geometry.id();
        // Immutable Arc identity is the admitted layout signature. Replacing
        // batches is a cold admission even if its lengths happen to match.
        let cached = self
            .plans
            .get(&id)
            .filter(|p| Arc::ptr_eq(&p.batches, &snapshot.batches))
            .cloned();
        let mut plan_bytes = 0;
        let plan = if let Some(plan) = cached {
            plan
        } else {
            self.stats.deep_admissions += 1;
            let own_bytes=|bytes:&[u8]|->Result<Vec<u8>,String>{let mut result=Vec::new();result.try_reserve_exact(bytes.len()).map_err(|_|"Metal span upload CPU allocation refused")?;result.extend_from_slice(bytes);Ok(result)};
            let (span_bytes,tiles_data,references_data)=if annotation.is_some(){let(s,t,r)=crate::span_plan::compile_annotation(&snapshot.geometry,&snapshot.batches,density)?;(own_bytes(bytemuck::cast_slice(&s))?,t,r)}else{let p=crate::span_plan::compile_source(&snapshot.geometry,&snapshot.batches)?;(own_bytes(bytemuck::cast_slice(&p.spans))?,p.tiles,p.references)};
            let upload = |bytes: &[u8]| -> Result<Buffer, String> {
                let mut b = metal.buffer(bytes.len(), true)?;
                b.write(bytes)?;
                Ok(b)
            };
            let spans = upload(&span_bytes)?;
            let tiles = upload(bytemuck::cast_slice(&tiles_data))?;
            let references = upload(bytemuck::cast_slice(&references_data))?;
            plan_bytes = (span_bytes.len()+tiles_data.len()*std::mem::size_of::<crate::span_plan::Tile>()+references_data.len()*4)as u64;
            Arc::new(Plan {
                spans,
                tiles,
                references,
                tile_count: tiles_data.len() as u32,
                batches: snapshot.batches.clone(),
            })
        };
        let key = (
            snapshot.geometry.palette_cache_identity(),
            Arc::as_ptr(&snapshot.batches) as *const Batch as usize,
            snapshot
                .glyphs
                .as_ref()
                .map(|g| {
                    Arc::as_ptr(g) as *const ingetrazo_render::cad_callback::GlyphOperator as usize
                })
                .unwrap_or(0),
        );
        let existing = self.palettes.get(&key).cloned();
        let mut palette_bytes = 0;
        let palette = if let Some(p) = existing {
            p
        } else {
            let colors = snapshot
                .geometry
                .resolve_batch_colors(&snapshot.batches)?
                .into_iter()
                .enumerate()
                .map(|(ordinal, c)| {
                    let color = c.map(|v| (v * 255.).round() as u32);
                    let op = snapshot.glyphs.as_ref().map(|g| g[ordinal]);
                    let straight = op.map(|g| g.straight_rgba.map(u32::from)).unwrap_or([0; 4]);
                    [
                        color[0],
                        color[1],
                        color[2],
                        color[3],
                        op.map(|g| g.composition).unwrap_or(0),
                        straight[0],
                        straight[1],
                        straight[2],
                        straight[3],
                        0,
                        0,
                        0,
                    ]
                })
                .collect::<Vec<_>>();
            let bytes = bytemuck::cast_slice(&colors);
            let mut buffer = metal.buffer(bytes.len(), true)?;
            buffer.write(bytes)?;
            palette_bytes = bytes.len() as u64;
            Arc::new(Palette {
                owner: snapshot.batches.clone(),
                _glyphs: snapshot.glyphs.clone(),
                buffer,
            })
        };
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            let options = MTLCompileOptions::new();
            options.setFastMathEnabled(false);
            let library = metal
                .device
                .newLibraryWithSource_options_error(
                    &NSString::from_str(&format!("#include <metal_stdlib>\nusing namespace metal;\n{}{}{}",if annotation.is_some(){"#define ANNOTATION 1\n"}else{""},if annotation.is_some(){include_str!("shaders/annotation_draw.metal")}else{""},include_str!("shaders/cad2d_compose.metal"))),
                    Some(&options),
                )
                .map_err(|e| format!("Exact Metal canvas shader:{e:?}"))?;
            let function = library
                .newFunctionWithName(&NSString::from_str("span_compose"))
                .ok_or("Metal span compose missing")?;
            Arc::new(
                metal
                    .device
                    .newComputePipelineStateWithFunction_error(&function)
                    .map_err(|e| format!("Exact Metal canvas pipeline:{e:?}"))?,
            )
        };
        if pipeline.maxTotalThreadsPerThreadgroup() < 64 {
            return Err("Metal compute pipeline cannot admit8x8 source tile workgroup".into());
        }
        // Publish only after all allocations/compiler admission succeed. Earlier frames
        // retain their immutable plan/palette even if cache eviction occurs.
        if self.plans.len() >= 64 && !self.plans.contains_key(&id) {
            self.plans.clear();
        }
        if self.palettes.len() >= 64 && !self.palettes.contains_key(&key) {
            self.palettes.clear();
        }
        self.plans.insert(id, plan.clone());
        self.palettes.insert(key, palette.clone());
        self.pipeline = Some(pipeline.clone());self.pipeline_annotation=annotation.is_some();
        self.stats.plan_uploads += u64::from(plan_bytes != 0);
        self.stats.palette_uploads += u64::from(palette_bytes != 0);
        self.stats.uploaded_bytes += plan_bytes + palette_bytes;
        self.stats.warm_hits += u64::from(plan_bytes == 0 && palette_bytes == 0);
        Ok(Arc::new(Prepared {
            annotation,
            pipeline,
            plan,
            palette,
            snapshot,
            translation,
        }))
    }
}
impl Prepared {
    pub fn encode(
        &self,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
        canvas: &ProtocolObject<dyn MTLTexture>,
        clip: [u32; 4],
    ) -> Result<(), String> {
        if canvas.textureType() != MTLTextureType::Type2D
            || canvas.sampleCount() != 1
            || canvas.width() == 0
            || canvas.height() == 0
            || canvas.pixelFormat() != MTLPixelFormat::R32Uint
            || !canvas
                .usage()
                .contains(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite)
            || canvas.width() > 16384
            || canvas.height() > 16384
        {
            return Err("Exact Metal canvas format/usage/extent unsupported".into());
        }
        if clip[0] > clip[2]
            || clip[1] > clip[3]
            || clip[2] as usize > canvas.width()
            || clip[3] as usize > canvas.height()
        {
            return Err("Metal canvas clip invalid".into());
        }
        let push = Push {
            translation: self.translation,
            size: [canvas.width() as u32, canvas.height() as u32],
            clip,
            tiles: self.plan.tile_count,
            pad: [0; 3],
        };
        let encoder = command
            .computeCommandEncoder()
            .ok_or("Metal canvas compute encoder failed")?;
        encoder.setComputePipelineState(&self.pipeline);
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&self.plan.spans.raw), 0, 0);
            encoder.setBuffer_offset_atIndex(Some(&self.plan.tiles.raw), 0, 1);
            encoder.setBuffer_offset_atIndex(Some(&self.plan.references.raw), 0, 2);
            encoder.setBuffer_offset_atIndex(Some(&self.palette.buffer.raw), 0, 3);
            encoder.setBytes_length_atIndex(
                std::ptr::NonNull::from(&push).cast(),
                std::mem::size_of::<Push>(),
                4,
            );
            if let Some(annotation)=&self.annotation{encoder.setBuffer_offset_atIndex(Some(&annotation.draws.raw),0,5);encoder.setBuffer_offset_atIndex(Some(&annotation.hidden.raw),0,6);}
            encoder.setTexture_atIndex(Some(canvas), 0);
            encoder.dispatchThreadgroups_threadsPerThreadgroup(
                MTLSize {
                    width: self.plan.tile_count as usize,
                    height: 1,
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
    pub fn revision(&self) -> u64 {
        self.snapshot.revision
    }
    pub fn retained_palette_batches(&self) -> usize {
        self.palette.owner.len()
    }
}
