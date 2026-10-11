//! Direct native world-space surface rendering. No CPU projection or submission.
//! Prepared frames retain buffers and color/depth targets through GPU completion.
//! This first opaque stage deliberately rejects paint policies not implemented here.
use crate::world_placement::{Packet as PlacementPacket, Pose as PlacementPose, PrototypeKey};
use crate::world_policy::{Mode, Plan, Policy};
use crate::{Buffer, NativeMetal};
use crate::world_section::Section;
use ingetrazo_core::gpu_memory::MemoryBudget;
use ingetrazo_render::buffer::MeshRenderBuffer;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{
    mem::ManuallyDrop,
    sync::{Arc, Mutex},
};
/// Retain native identities as well as comparing them: address reuse after a
/// context drop cannot make a new context alias this renderer's cache.
pub(crate) struct Owner {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    budget: Arc<Mutex<MemoryBudget>>,
    poison: Arc<std::sync::atomic::AtomicBool>,
}
impl Owner {
    pub(crate) fn new(metal: &NativeMetal) -> Self {
        Self {
            device: metal.device.clone(),
            queue: metal.queue.clone(),
            budget: metal.budget.clone(),
            poison: metal.poisoned.clone(),
        }
    }
    pub(crate) fn validate(&self, metal: &NativeMetal) -> Result<(), String> {
        if !std::ptr::eq(&*self.device, &*metal.device)
            || !std::ptr::eq(&*self.queue, &*metal.queue)
            || !Arc::ptr_eq(&self.budget, &metal.budget)
            || !Arc::ptr_eq(&self.poison, &metal.poisoned)
        {
            return Err("Metal world native owner mismatch".into());
        }
        Ok(())
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
struct Target {
    multisample: Option<Image>,
    samples: usize,
    fill_enabled: bool,
    color: Image,
    depth: Image,
    extent: (u32, u32),
    poses: Vec<PlacementPose>,
    instance_matrices: Option<Buffer>,
    ordering_packet: Option<Arc<PlacementPacket>>,
    order: Option<super::world_placement::Order>,
    material_draws:Option<Arc<MaterialDraws>>,
    // Weak pins the control block identity, not old GPU meshes. Comparing the
    // allocation address is safe while this Weak prevents allocation reuse.
    material_identity:Option<std::sync::Weak<PlacementMeshes>>,
}
enum Collection {Opaque(MeshRenderBuffer),Surfaces(ingetrazo_render::pipeline::SceneRenderData)}
struct SampledMesh {vertices:Buffer,asset:Arc<super::world_texture::Asset>,draws:Vec<ingetrazo_render::native_textures::Draw>}
struct Materials {instanced:bool,consolidated:bool,colored_vertices:Option<Buffer>,colored_indices:Option<Buffer>,runs:Vec<super::world_surface::Run>,textures:Vec<SampledMesh>,opaque_runs:Vec<(usize,usize,bool)>,retained_payload_bytes:usize}
struct Mesh {
    vertices: Option<Buffer>,
    indices: Option<Buffer>,
    count: usize,
    plan: Plan,
    materials:Option<Materials>,
}
pub(crate) struct BasePins([Option<(PrototypeKey,Option<Arc<Mesh>>)>;2]);
struct PlacementMeshes {
    has_materials:bool,
    packet: Arc<PlacementPacket>,
    meshes: Vec<Option<Arc<Mesh>>>,
    key_index: Vec<(PrototypeKey, usize)>,
    retained_payload_bytes: usize,
}
#[derive(Clone,Copy)]enum MaterialKind {Legacy,Opaque{first:usize,count:usize,cull:bool},DefaultBack,Color(usize),Texture{bucket:usize,draw:usize}}
#[derive(Clone,Copy)]struct MaterialDraw{opacity:f32,prototype:usize,packed:usize,instances:usize,reflected:bool,kind:MaterialKind}
struct MaterialDraws{guard:super::world_face_guard::FaceGuard<PrototypeKey>,operations:Vec<MaterialDraw>,retained_payload_bytes:usize}
struct MaterialPipeline {colored_alpha:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,textured:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,textured_alpha:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,depth:Retained<ProtocolObject<dyn MTLDepthStencilState>>}
struct FillPipeline { mask: Retained<ProtocolObject<dyn MTLRenderPipelineState>>, texture_mask: Retained<ProtocolObject<dyn MTLRenderPipelineState>>, mask_depth: Retained<ProtocolObject<dyn MTLDepthStencilState>>, cap_depth: Retained<ProtocolObject<dyn MTLDepthStencilState>> }
struct Pipeline {
    fill: Option<FillPipeline>,
    lines: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    background: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    ghost: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    color: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    depth: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_disabled: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    material:Option<MaterialPipeline>,
}
#[derive(Default, Debug, Clone, Copy)]
pub struct Stats {
    pub source_resolve_pipeline_builds:u64,
    pub section_contour_pipeline_builds:u64,
    pub billboard_builds:u64,
    pub billboard_uploads:u64,
    pub billboard_upload_bytes:u64,
    pub billboard_texture_uploads:u64,
    pub section_fill_uploads:u64,
    pub section_fill_upload_bytes:u64,
    /// Extra cached original/suppressed native buffers, excluding current placements and shared images.
    pub base_variant_gpu_bytes:u64,
    pub preview_builds:u64,
    pub profile_builds:u64,
    pub profile_uploads:u64,
    pub profile_upload_bytes:u64,
    pub preview_line_uploads:u64,
    pub base_line_uploads:u64,
    pub preview_line_upload_bytes:u64,
    pub base_line_upload_bytes:u64,
    pub ghost_uploads: u64,
    pub wire_uploads:u64,
    pub wire_upload_bytes:u64,
    pub ghost_upload_bytes: u64,
    pub ghost_builds: u64,
    pub geometry_uploads: u64,
    pub texture_uploads:u64,
    pub material_order_builds:u64,
    /// Current weak identity lookup capacity, excluding Arc control headers.
    pub texture_identity_payload_bytes:usize,
    /// Current target material operation capacities; detached Prepared owners separate.
    pub material_order_payload_bytes:usize,
    pub target_allocations: u64,
    pub prepared: u64,
    pub warm_geometry_hits: u64,
    pub instance_buffer_allocations: u64,
    pub instance_matrix_upload_bytes: u64,
    pub draw_order_builds: u64,
    /// Retained current target ordering payload, excludes old detached frames.
    pub draw_order_payload_bytes: usize,
    /// Current World cache paint-plan capacities only. Prior Prepared frames may
    /// independently retain older plans; excludes allocator/shared/GPU owners.
    pub policy_retained_payload_bytes: usize,
    /// Current cache records/plans only, excludes previous Prepared caches,
    /// allocator headers, shared renderer, geometry GPU buffers and Scene.
    pub placement_cache_payload_bytes: usize,
    pub placement_descriptor_payload_bytes: usize,
    /// Pose Vec capacities in all current World target slots (including pinned).
    pub pose_storage_payload_bytes: usize,
}
#[derive(Default)]
pub struct World {
    resolve_prototype:Option<Arc<super::world_resolve::Pipeline>>,
    source_tile:Option<Arc<super::world_resolve::Pipeline>>,
    diagnostic_native_resolve:bool,
    samples: usize,
    diagnostic_samples: bool,
    pub(crate) ghosts: Option<Arc<super::world_ghost::Ghosts>>,
    owner: Option<Arc<Owner>>,
    mesh: Option<Arc<Mesh>>,
    key: Option<u64>,
    placements: Option<Arc<PlacementMeshes>>,
    base_variants: [Option<(PrototypeKey,Option<Arc<Mesh>>)>;2],
    pipeline: Option<Arc<Pipeline>>,
    section_pipeline:Option<Arc<Pipeline>>,
    pub(crate) contour_pipeline:Option<Arc<super::world_contour::Pipeline>>,
    pub(crate) contour_logical_height:Option<f32>,
    targets: Vec<Arc<Target>>,
    texture_assets:Vec<Arc<super::world_texture::Asset>>,
    pub stats: Stats,
}
pub struct Prepared {
    resolve_prototype:Option<Arc<super::world_resolve::Pipeline>>,    diagnostic_samples: bool,
    pub(crate) billboards:Option<Arc<super::world_billboard::Prepared>>,
    pub(crate) section: Section,
    pub(crate) contour:Option<Arc<super::world_contour::Pipeline>>,
    pub(crate) contour_eye_width:[f32;4],
    pub(crate) fill: Option<Arc<crate::world_section::Fill>>,
    pub(crate) source_sample_positions:Option<[MTLSamplePosition;4]>,
    pub(crate) lines: [Option<Arc<super::world_preview::Lines>>;2],
    pub(crate) profiles: Option<Arc<super::world_preview::Lines>>,
    pub(crate) background: Option<super::world_background::Background>,
    pub(crate) overlay: Option<PrototypeKey>,
    pub(crate) ghosts: Option<Arc<super::world_ghost::Ghosts>>,
    pub(crate) ghost_edge: [f32;4],
    placements: Option<Arc<PlacementMeshes>>,
    owner: Arc<Owner>,
    mesh: Option<Arc<Mesh>>,
    pipeline: Arc<Pipeline>,
    target: Arc<Target>,
    mvp: [[f32; 4]; 4],
    clear: [f64; 4],
    policy: Policy,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PaintUniform {
    front: [f32; 4],
    back: [f32; 4],
    mode: u32,
    opacity: f32,
    padding: [u32; 2],
}
#[repr(C)]struct TexturePaint{opacity:f32,shade:f32,hard_cutout:u32,height:u32,mean:[f32;4]}
impl World {
    /// Cold diagnostic reference only: preserve the untouched native resolve.
    pub fn use_native_resolve_reference(&mut self)->Result<(),String>{
        if !self.targets.is_empty()||self.source_tile.is_some()||self.resolve_prototype.is_some(){return Err("Native resolve reference requires an unconfigured cold World".into());}
        self.diagnostic_native_resolve=true;Ok(())
    }
    fn active_resolve(&self)->Option<Arc<super::world_resolve::Pipeline>>{
        self.resolve_prototype.clone().or_else(||(self.samples.max(1)==4).then(||self.source_tile.clone()).flatten())
    }
    /// Cold diagnostic configuration only. Does not alter the production default.
    pub fn enable_source_resolve_prototype(&mut self,metal:&NativeMetal)->Result<(),String>{
        if !self.targets.is_empty(){return Err("Source resolve prototype requires no existing targets".into());}
        self.validate_context_owner(metal)?;
        if self.samples.max(1)!=4{return Err("Source resolve prototype requires configured four samples".into());}
        let pipeline=super::world_resolve::Pipeline::new(metal)?;
        self.owner=Some(pipeline.owner.clone());self.resolve_prototype=Some(pipeline);self.stats.source_resolve_pipeline_builds+=1;Ok(())
    }
    /// Probe-only cold opt-in. Ordinary targets retain their original usage/store policy.
    pub fn enable_sample_observation(&mut self)->Result<(),String>{
        if !self.targets.is_empty() {return Err("Sample observation must be enabled before any World target exists".into());}
        self.diagnostic_samples=true;Ok(())
    }
    pub(crate) fn sampled_assets(&self)->&[Arc<super::world_texture::Asset>]{&self.texture_assets}
    /// Diagnostic metadata only: counts billboard references that share the exact
    /// immutable image owner with the accepted ordinary World inventory.
    pub fn shared_billboard_texture_owners(&self,frame:&Prepared)->usize{
        frame.billboards.as_ref().map_or(0,|billboards|billboards.assets().iter()
            .filter(|(_,asset)|self.texture_assets.iter().any(|ordinary|Arc::ptr_eq(asset,ordinary))).count())
    }
    /// Literal widget height for section stroke sizing; targets use physical pixels.
    pub fn set_viewport_logical_height(&mut self,height:f32)->Result<(),String>{
        if !height.is_finite()||height<0.{return Err("Metal logical viewport height admission".into());}
        self.contour_logical_height=Some(height);Ok(())
    }
    pub(crate) fn configured_sample_count(&self)->usize{self.samples.max(1)}
    pub(crate) fn pin_bases(&self, keys:[Option<PrototypeKey>;2])->BasePins {
        BasePins(keys.map(|key|key.and_then(|key| {
            let mesh=self.placements.as_ref().and_then(|p|p.key_index.binary_search_by_key(&key,|p|p.0).ok().map(|i|p.meshes[p.key_index[i].1].clone()))
                .or_else(||self.base_variants.iter().flatten().find(|(old,_)|*old==key).map(|(_,mesh)|mesh.clone()));
            mesh.map(|mesh|(key,mesh))
        })))
    }
    pub(crate) fn publish_bases(&mut self, mut pins:BasePins, keys:[Option<PrototypeKey>;2]) {
        // Called only after full candidate admission; no allocation or GPU wait.
        let current=self.pin_bases(keys);
        for (old,new) in pins.0.iter_mut().zip(current.0) {if new.is_some(){*old=new;}}
        self.base_variants=pins.0;
        self.stats.base_variant_gpu_bytes=self.base_variants.iter().flatten().filter_map(|(_,m)|m.as_ref()).filter(|m|!self.placements.as_ref().is_some_and(|p|p.meshes.iter().flatten().any(|current|Arc::ptr_eq(m,current)))).map(|m|{
            let mut n=m.vertices.as_ref().map_or(0,Buffer::allocation_bytes)+m.indices.as_ref().map_or(0,Buffer::allocation_bytes);
            if let Some(material)=&m.materials {n+=material.colored_vertices.as_ref().map_or(0,Buffer::allocation_bytes)+material.colored_indices.as_ref().map_or(0,Buffer::allocation_bytes);for texture in &material.textures{n+=texture.vertices.allocation_bytes();}}
            n
        }).sum();
    }

    /// Cold sample policy; old Prepared owners retain their matching targets/pipelines.
    pub fn set_sample_count(&mut self,metal:&NativeMetal,samples:usize)->Result<(),String>{
        self.validate_context_owner(metal)?;
        if self.resolve_prototype.is_some()&&samples!=4{return Err("Source resolve prototype is four-sample only".into());}
        if self.samples.max(1)==samples{return Ok(());}
        if !matches!(samples,1|4)||!metal.device.supportsTextureSampleCount(samples){return Err("Metal world sample count unsupported".into());}
        // Compile once, before publishing the new sample policy. Intel compute
        // remains diagnostic-only; source ordering requires programmable slots.
        let tile=if samples==4 && !self.diagnostic_native_resolve && self.source_tile.is_none()
            && metal.programmable_sample_positions && metal.device.supportsFamily(MTLGPUFamily::Apple4){
            Some(super::world_resolve::Pipeline::new(metal)?)
        }else{None};
        if let Some(tile)=tile{self.owner=Some(tile.owner.clone());self.source_tile=Some(tile);self.stats.source_resolve_pipeline_builds+=1;}
        if self.samples.max(1)!=samples{self.samples=samples;self.pipeline=None;self.section_pipeline=None;}
        Ok(())
    }

    /// `collect` runs only on a changed immutable geometry identity. Pose always validates.
    /// Caller must change key on any geometry/material/visibility change; transform-only
    /// per-placement caching will use independently keyed meshes, never suppress revisions.
    pub fn prepare<F>(
        &mut self,
        metal: &NativeMetal,
        key: u64,
        mvp: [[f32; 4]; 4],
        extent: (u32, u32),
        clear: [f64; 4],
        collect: F,
    ) -> Result<Option<Arc<Prepared>>, String>
    where
        F: FnOnce() -> Result<MeshRenderBuffer, String>,
    {
        self.prepare_with_policy(metal, key, mvp, extent, clear, Policy::default(), collect)
    }
    pub fn prepare_with_policy<F>(
        &mut self,
        metal: &NativeMetal,
        key: u64,
        mvp: [[f32; 4]; 4],
        extent: (u32, u32),
        clear: [f64; 4],
        policy: Policy,
        collect: F,
    ) -> Result<Option<Arc<Prepared>>, String>
    where
        F: FnOnce() -> Result<MeshRenderBuffer, String>,
    {
        policy.validate()?;
        // Owner refusal precedes any cache retirement, collection or resource work.
        if let Some(owner) = &self.owner {
            owner.validate(metal)?;
        }
        super::world_admission::pose(mvp, extent, clear)?;
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal world admission poisoned".into());
        }
        if extent.0 == 0 || extent.1 == 0 {
            return Ok(None);
        }
        self.targets
            .retain(|t| Arc::strong_count(t) > 1 || (t.extent == extent && t.samples==self.samples.max(1) && !t.fill_enabled));
        let cached = self
            .targets
            .iter()
            .find(|t| (t.extent == extent && t.samples==self.samples.max(1) && !t.fill_enabled) && Arc::strong_count(t) == 1)
            .cloned();
        if cached.is_none() && self.targets.len() >= 3 {
            return Ok(None);
        }
        let mesh = if self.key == Some(key) {
            self.stats.warm_geometry_hits += 1;
            self.mesh.clone()
        } else {
            let data = collect()?;
            let plan = super::world_admission::opaque(&data)?;
            if data.indices.is_empty() {
                None
            } else {
                Some(Arc::new(Mesh {
                    vertices: Some(upload(metal, bytemuck::cast_slice(&data.vertices))?),
                    indices: Some(upload(metal, bytemuck::cast_slice(&data.indices))?),
                    count: data.indices.len(),
                    plan,
                materials:None,
                }))
            }
        };
        let pipeline = if let Some(p) = self.pipeline.as_ref().filter(|p|p.fill.is_none()) {
            p.clone()
        } else {
            Arc::new(pipeline(metal,false,self.samples.max(1),false)?)
        };
        let allocated = cached.is_none();
        let target = if let Some(t) = cached {
            t
        } else {
            Arc::new(Target {
                samples:self.samples.max(1),
                fill_enabled:false,
                multisample:if self.samples.max(1)>1{Some(image_samples(metal,extent,MTLPixelFormat::RGBA8Unorm,MTLTextureUsage::RenderTarget | if self.diagnostic_samples||self.resolve_prototype.as_ref().is_some_and(|r|r.is_compute()) {MTLTextureUsage::ShaderRead}else{MTLTextureUsage::empty()},4,self.samples.max(1))?)}else{None},
                color: image(
                    metal,
                    extent,
                    MTLPixelFormat::RGBA8Unorm,
                    MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead | if self.resolve_prototype.as_ref().is_some_and(|r|r.is_compute()){MTLTextureUsage::ShaderWrite}else{MTLTextureUsage::empty()},
                    4,
                )?,
                depth: image_samples(metal,extent,MTLPixelFormat::Depth32Float,MTLTextureUsage::RenderTarget|MTLTextureUsage::ShaderRead,4,self.samples.max(1))?,
                extent,
                poses: Vec::new(),
                instance_matrices: None,
                ordering_packet: None,
                order: None,material_draws:None,material_identity:None,
            })
        };
        let owner = self
            .owner
            .clone()
            .unwrap_or_else(|| Arc::new(Owner::new(metal)));
        // Publish only after all fallible resource admission succeeds.
        if allocated {
            self.targets.push(target.clone());
            self.stats.target_allocations += 1;
        }
        if self.key != Some(key) {
            self.stats.geometry_uploads += u64::from(mesh.is_some());
        }
        self.owner = Some(owner.clone());
        self.key = Some(key);
        self.stats.policy_retained_payload_bytes =
            mesh.as_ref().map_or(0, |m| m.plan.retained_payload_bytes);
        self.mesh = mesh.clone();
        self.placements = None;
        self.stats.placement_cache_payload_bytes = 0;
        self.stats.placement_descriptor_payload_bytes = 0;
        self.stats.draw_order_payload_bytes = self
            .targets
            .iter()
            .filter_map(|t| t.order.as_ref())
            .map(|o| o.retained_payload_bytes)
            .sum();
        self.stats.pose_storage_payload_bytes = self
            .targets
            .iter()
            .map(|t| t.poses.capacity() * std::mem::size_of::<PlacementPose>())
            .sum();
        self.pipeline = Some(pipeline.clone());
        self.stats.prepared += 1;
        Ok(Some(Arc::new(Prepared {
            diagnostic_samples:self.diagnostic_samples,
            resolve_prototype:self.active_resolve(),
            section: Section::OFF,
            contour:None,contour_eye_width:[0.;4],
            fill:None,
            source_sample_positions:super::annotation::source_raster_sample_positions(metal,self.samples.max(1)),
            billboards:None,
            placements: None,
            ghosts: None, ghost_edge: [0.;4], lines:[None,None],profiles:None,background:None,overlay:None,
            owner,
            mesh,
            pipeline,
            target,
            mvp,
            clear,
            policy,
        })))
    }
    /// Source-eligible own-mesh prototypes plus independently posed bindings.
    /// Unchanged packet Arc skips cold key/sort/collection; every pose validates.
    /// `collect` must return local source-placement coordinates with effective
    /// inherited paint, style and layer identity already represented in the key.
    /// Compatibility entry for already combined matrices. Production callers
    /// use prepare_placements_with_camera with stable model poses instead.
    pub(crate) fn validate_context_owner(&self, metal: &NativeMetal) -> Result<(), String> {
        if let Some(owner) = &self.owner {
            owner.validate(metal)?;
        }
        Ok(())
    }
    pub fn prepare_placements<F>(
        &mut self,
        metal: &NativeMetal,
        packet: Arc<PlacementPacket>,
        poses: &[PlacementPose],
        extent: (u32, u32),
        clear: [f64; 4],
        policy: Policy,
        collect: F,
    ) -> Result<Option<Arc<Prepared>>, String>
    where
        F: FnMut(&PrototypeKey) -> Result<MeshRenderBuffer, String>,
    {
        self.prepare_placements_with_camera(
            metal, packet, poses, IDENTITY, extent, clear, policy, collect,
        )
    }
    /// Stable model matrices are retained separately from the global camera.
    /// Camera-only frames bind one uniform without rewriting instance storage.
    pub fn prepare_placements_with_camera<F>(&mut self,metal:&NativeMetal,packet:Arc<PlacementPacket>,poses:&[PlacementPose],camera:[[f32;4];4],extent:(u32,u32),clear:[f64;4],policy:Policy,mut collect:F)->Result<Option<Arc<Prepared>>,String>
    where F:FnMut(&PrototypeKey)->Result<MeshRenderBuffer,String>{
        self.prepare_collections_with_camera(metal,packet,poses,camera,extent,clear,policy,None,&[],|key|Ok(Collection::Opaque(collect(key)?)),|_|Err("Unexpected sampled material in opaque consumer".into()))
    }
    pub fn prepare_material_placements_with_camera<'a,F,G>(&mut self,metal:&NativeMetal,packet:Arc<PlacementPacket>,poses:&[PlacementPose],camera:[[f32;4];4],extent:(u32,u32),clear:[f64;4],policy:Policy,collect:F,resolve:G)->Result<Option<Arc<Prepared>>,String>
    where F:FnMut(&PrototypeKey)->Result<ingetrazo_render::pipeline::SceneRenderData,String>,G:FnMut(&str)->Result<std::borrow::Cow<'a,[u8]>,String>{
        self.prepare_material_placements_with_overlay(metal,packet,poses,camera,extent,clear,policy,None,collect,resolve)
    }
    pub fn prepare_material_placements_with_overlay<'a,F,G>(&mut self,metal:&NativeMetal,packet:Arc<PlacementPacket>,poses:&[PlacementPose],camera:[[f32;4];4],extent:(u32,u32),clear:[f64;4],policy:Policy,overlay:Option<PrototypeKey>,collect:F,resolve:G)->Result<Option<Arc<Prepared>>,String>
    where F:FnMut(&PrototypeKey)->Result<ingetrazo_render::pipeline::SceneRenderData,String>,G:FnMut(&str)->Result<std::borrow::Cow<'a,[u8]>,String>{
        self.prepare_material_placements_with_section(metal,packet,poses,camera,extent,clear,policy,overlay,None,&[],collect,resolve)
    }
    pub(crate) fn prepare_material_placements_with_section<'a,F,G>(&mut self,metal:&NativeMetal,packet:Arc<PlacementPacket>,poses:&[PlacementPose],camera:[[f32;4];4],extent:(u32,u32),clear:[f64;4],policy:Policy,overlay:Option<PrototypeKey>,fill:Option<Arc<crate::world_section::Fill>>,shared_assets:&[(String,Arc<super::world_texture::Asset>)],mut collect:F,resolve:G)->Result<Option<Arc<Prepared>>,String>
    where F:FnMut(&PrototypeKey)->Result<ingetrazo_render::pipeline::SceneRenderData,String>,G:FnMut(&str)->Result<std::borrow::Cow<'a,[u8]>,String>{
        self.prepare_collections_with_camera(metal,packet,poses,camera,extent,clear,policy,fill,shared_assets,|key|{let data=collect(key)?;if overlay!=Some(*key) && matches!(policy.mode,Mode::HiddenLine|Mode::Monochrome){
            if !data.transparent_mesh.indices.is_empty()||!data.textured_meshes.is_empty()||!data.opaque_mesh.source_front_cull.is_empty(){return Err("Metal plan style mixed-material policy requires subsequent stage".into());}
            Ok(Collection::Opaque(data.opaque_mesh))
        }else{Ok(Collection::Surfaces(data))}},resolve)
    }
    fn prepare_collections_with_camera<'a,F,G>(
        &mut self,
        metal: &NativeMetal,
        packet: Arc<PlacementPacket>,
        poses: &[PlacementPose],
        camera: [[f32; 4]; 4],
        extent: (u32, u32),
        clear: [f64; 4],
        policy: Policy,
        fill:Option<Arc<crate::world_section::Fill>>,
        shared_assets:&[(String,Arc<super::world_texture::Asset>)],
        mut collect: F,
        mut resolve:G,
    ) -> Result<Option<Arc<Prepared>>, String>
    where
        F: FnMut(&PrototypeKey) -> Result<Collection, String>,
        G:FnMut(&str)->Result<std::borrow::Cow<'a,[u8]>,String>,
    {
        if let Some(owner) = &self.owner {
            owner.validate(metal)?;
        }
        if let Some(fill)=&fill{fill.validate(metal)?;}
        policy.validate()?;
        super::world_admission::pose(camera, extent, clear)?;
        super::world_placement::validate_poses(&packet, poses)?;
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Metal world admission poisoned".into());
        }
        if extent.0 == 0 || extent.1 == 0 {
            return Ok(None);
        }
        self.targets
            .retain(|t| Arc::strong_count(t) > 1 || (t.extent == extent && t.samples==self.samples.max(1) && t.fill_enabled==fill.is_some()));
        let idle = self
            .targets
            .iter()
            .position(|t| (t.extent == extent && t.samples==self.samples.max(1) && t.fill_enabled==fill.is_some()) && Arc::strong_count(t) == 1);
        if idle.is_none() && self.targets.len() >= 3 {
            return Ok(None);
        }
        let mut candidate_assets=Vec::<Arc<super::world_texture::Asset>>::new();
        let mut candidate_texture_uploads=0u64;
        let mut uploaded = 0;
        let placements = if let Some(old) = self
            .placements
            .as_ref()
            .filter(|old| Arc::ptr_eq(&old.packet, &packet))
        {
            old.clone()
        } else {
            const CACHE_BYTES: usize = 64 * 1024 * 1024;
            let list_preflight = packet
                .prototypes
                .len()
                .checked_mul(
                    std::mem::size_of::<Option<Arc<Mesh>>>()
                        + std::mem::size_of::<(PrototypeKey, usize)>(),
                )
                .and_then(|n| n.checked_add(std::mem::size_of::<PlacementMeshes>()))
                .filter(|n| *n <= CACHE_BYTES)
                .ok_or("Metal prototype cache list admission exceeds64MiB")?;
            let _ = list_preflight;
            let mut meshes = Vec::new();
            let mut key_index = Vec::new();
            meshes
                .try_reserve_exact(packet.prototypes.len())
                .map_err(|_| "Metal prototype cache allocation refused")?;
            key_index
                .try_reserve_exact(packet.prototypes.len())
                .map_err(|_| "Metal prototype lookup allocation refused")?;
            let mut retained = meshes
                .capacity()
                .checked_mul(std::mem::size_of::<Option<Arc<Mesh>>>())
                .and_then(|n| {
                    key_index
                        .capacity()
                        .checked_mul(std::mem::size_of::<(PrototypeKey, usize)>())
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| n.checked_add(std::mem::size_of::<PlacementMeshes>()))
                .ok_or("Metal cache actual capacity overflow")?;
            if retained > CACHE_BYTES {
                return Err("Metal prototype cache metadata exceeds64MiB".into());
            }
            for (index, key) in packet.prototypes.iter().enumerate() {
                let prior = self.placements.as_ref().and_then(|old| {
                    old.key_index
                        .binary_search_by_key(key, |p| p.0)
                        .ok()
                        .map(|i| old.meshes[old.key_index[i].1].clone())
                });
                let prior = prior.or_else(||self.base_variants.iter().flatten().find(|(old,_)|old==key).map(|(_,mesh)|mesh.clone()));
                let mesh = if let Some(prior) = prior {
                    retained = retained
                        .checked_add(prior.as_ref().map_or(0, |m| {
                            std::mem::size_of::<Mesh>() + m.plan.retained_payload_bytes
                                - std::mem::size_of::<Plan>()+m.materials.as_ref().map_or(0,|s|s.retained_payload_bytes)
                        }))
                        .filter(|n| *n <= CACHE_BYTES)
                        .ok_or("Metal retained prototype plans exceed64MiB")?;
                    prior
                } else {
                    let collection=collect(key)?;
                    let mesh=build_material_mesh(metal,collection,&mut resolve,&self.texture_assets,&mut candidate_assets,shared_assets,&mut candidate_texture_uploads)?;
                    if let Some(m)=&mesh{retained=retained.checked_add(std::mem::size_of::<Mesh>()+m.plan.retained_payload_bytes-std::mem::size_of::<Plan>()+m.materials.as_ref().map_or(0,|s|s.retained_payload_bytes)).filter(|n|*n<=CACHE_BYTES).ok_or("Metal retained material cache exceeds64MiB")?;uploaded+=1;}
                    mesh

                };
                meshes.push(mesh);
                key_index.push((*key, index));
            }
            key_index.sort_unstable_by_key(|p| p.0);
            let has_materials=meshes.iter().filter_map(|m|m.as_ref()).any(|m|m.materials.is_some());
            Arc::new(PlacementMeshes {
                has_materials,packet: packet.clone(),
                meshes,
                key_index,
                retained_payload_bytes: retained,
            })
        };
        let needs_materials=placements.has_materials;
        let cached_pipeline=if fill.is_some(){&self.section_pipeline}else{&self.pipeline};
        let pipeline=if let Some(p)=cached_pipeline{if (!needs_materials||p.material.is_some()) && p.fill.is_some()==fill.is_some(){p.clone()}else{Arc::new(pipeline(metal,true,self.samples.max(1),fill.is_some())? )}}else{Arc::new(pipeline(metal,needs_materials,self.samples.max(1),fill.is_some())?)};
        let mut new_target = if idle.is_none() {
            Some(Arc::new(Target {
                samples:self.samples.max(1),
                fill_enabled:fill.is_some(),
                multisample:if self.samples.max(1)>1{Some(image_samples(metal,extent,MTLPixelFormat::RGBA8Unorm,MTLTextureUsage::RenderTarget | if self.diagnostic_samples||self.resolve_prototype.as_ref().is_some_and(|r|r.is_compute()) {MTLTextureUsage::ShaderRead}else{MTLTextureUsage::empty()},4,self.samples.max(1))?)}else{None},
                color: image(
                    metal,
                    extent,
                    MTLPixelFormat::RGBA8Unorm,
                    MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead | if self.resolve_prototype.as_ref().is_some_and(|r|r.is_compute()){MTLTextureUsage::ShaderWrite}else{MTLTextureUsage::empty()},
                    4,
                )?,
                depth: image_samples(metal,extent,if fill.is_some(){MTLPixelFormat::Depth32Float_Stencil8}else{MTLPixelFormat::Depth32Float},MTLTextureUsage::RenderTarget|MTLTextureUsage::ShaderRead,if fill.is_some(){8}else{4},self.samples.max(1))?,
                extent,
                poses: Vec::new(),
                instance_matrices: None,
                ordering_packet: None,
                order: None,material_draws:None,material_identity:None,
            }))
        } else {
            None
        };
        let target = if let Some(i) = idle {
            &mut self.targets[i]
        } else {
            new_target.as_mut().unwrap()
        };
        let unique = Arc::get_mut(target).ok_or("Metal placement idle target ownership changed")?;
        let order_changed = unique
            .ordering_packet
            .as_ref()
            .is_none_or(|p| p.bindings != placements.packet.bindings)
            || unique.poses.len() != poses.len()
            || unique
                .poses
                .iter()
                .zip(poses)
                .any(|(a, b)| a.reflected != b.reflected);
        let candidate_order = if order_changed {
            Some(super::world_placement::Order::new(
                &placements.packet,
                poses,
            )?)
        } else {
            None
        };
        let matrix_bytes = poses
            .len()
            .checked_mul(64)
            .ok_or("Metal instance matrix extent overflow")?;
        let candidate_matrices = if matrix_bytes > 0
            && unique
                .instance_matrices
                .as_ref()
                .is_none_or(|b| b.length() < matrix_bytes)
        {
            Some(metal.buffer(matrix_bytes, true)?)
        } else {
            None
        };
        // Grow an unpublished candidate, so refused allocation/admission leaves
        // the idle target and its accounted capacity unchanged.
        let candidate_poses = if poses.len() > unique.poses.capacity() {
            let mut candidate = Vec::new();
            candidate
                .try_reserve_exact(poses.len())
                .map_err(|_| "Metal placement pose storage allocation refused")?;
            if candidate
                .capacity()
                .checked_mul(std::mem::size_of::<PlacementPose>())
                .is_none_or(|n| n > super::world_placement::MAX_POSE_BYTES)
            {
                return Err("Metal actual pose storage exceeds8MiB per target".into());
            }
            candidate.extend_from_slice(poses);
            Some(candidate)
        } else {
            None
        };
        // Grow an unpublished weak lookup only on cold new-asset admission.
        // Failed layout/target admission must not mutate its capacity/accounting.
        let mut candidate_lookup=None;
        if !candidate_assets.is_empty(){
            if candidate_assets.len()+self.texture_assets.len()>4096{return Err("Metal sampled identity publication admission exceeded".into());}
            let needed=self.texture_assets.len().checked_add(candidate_assets.len()).ok_or("Metal texture lookup size overflow")?;
            if needed>self.texture_assets.capacity(){let mut entries=Vec::new();entries.try_reserve_exact(needed).map_err(|_|"Metal texture lookup publication allocation refused")?;entries.extend(self.texture_assets.iter().cloned());candidate_lookup=Some(entries);}
        }
        let material_changed=needs_materials && (order_changed||unique.material_draws.is_none()||unique.material_identity.as_ref().is_none_or(|old|old.as_ptr()!=Arc::as_ptr(&placements)));
        let material_draws=if material_changed{
            Some(Arc::new(material_draws(&placements,candidate_order.as_ref().or(unique.order.as_ref()).unwrap(),poses)?))
        }else{None};
        let allocated_matrices = candidate_matrices.is_some();
        let order = candidate_order.as_ref().or(unique.order.as_ref()).unwrap();
        let buffer = candidate_matrices
            .as_ref()
            .or(unique.instance_matrices.as_ref());
        let mut copied_bytes = 0u64;
        if let Some(buffer) = buffer {
            // Admission is complete and the target is exclusively idle. Compare
            // against old packed slots before publishing poses/order/buffer.
            for (i, &source_index) in order.indices.iter().enumerate() {
                let pose = &poses[source_index];
                if allocated_matrices
                    || super::world_placement::packed_matrix_changed(
                        unique.order.as_ref(),
                        &unique.poses,
                        i,
                        pose,
                    )
                {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            pose.mvp.as_ptr().cast::<u8>(),
                            buffer.raw.contents().as_ptr().cast::<u8>().add(i * 64),
                            64,
                        );
                    }
                    copied_bytes += 64;
                }
            }
        }
        if let Some(candidate) = candidate_poses {
            unique.poses = candidate;
        } else {
            unique.poses.clear();
            unique.poses.extend_from_slice(poses);
        }
        if let Some(order) = candidate_order {
            unique.order = Some(order);
        }
        if let Some(draws)=material_draws{unique.material_draws=Some(draws);unique.material_identity=Some(Arc::downgrade(&placements));}
        if !needs_materials{unique.material_draws=None;unique.material_identity=None;}
        unique.ordering_packet = Some(placements.packet.clone());
        if let Some(buffer) = candidate_matrices {
            unique.instance_matrices = Some(buffer);
        }
        let target = target.clone();
        if new_target.is_some() {
            self.targets.push(target.clone());
            self.stats.target_allocations += 1;
        }
        let owner = self
            .owner
            .clone()
            .unwrap_or_else(|| Arc::new(Owner::new(metal)));
        self.stats.instance_buffer_allocations += u64::from(allocated_matrices);
        self.stats.instance_matrix_upload_bytes += copied_bytes;
        self.stats.draw_order_builds += u64::from(order_changed);
        self.stats.draw_order_payload_bytes = self
            .targets
            .iter()
            .filter_map(|t| t.order.as_ref())
            .map(|o| o.retained_payload_bytes)
            .sum();
        self.stats.geometry_uploads += uploaded;
        self.stats.texture_uploads+=candidate_texture_uploads;
        self.stats.material_order_builds+=u64::from(material_changed);
        self.stats.material_order_payload_bytes=self.targets.iter().filter_map(|t|t.material_draws.as_ref()).map(|p|p.retained_payload_bytes).sum();
        self.stats.prepared += 1;
        if uploaded == 0 {
            self.stats.warm_geometry_hits += 1;
        }
        self.stats.policy_retained_payload_bytes = placements
            .meshes
            .iter()
            .filter_map(|m| m.as_ref())
            .map(|m| m.plan.retained_payload_bytes)
            .sum();
        self.stats.placement_cache_payload_bytes = placements.retained_payload_bytes;
        self.stats.placement_descriptor_payload_bytes = placements.packet.retained_payload_bytes();
        self.stats.pose_storage_payload_bytes = self
            .targets
            .iter()
            .map(|t| t.poses.capacity() * std::mem::size_of::<PlacementPose>())
            .sum();
        if let Some(entries)=candidate_lookup{self.texture_assets=entries;}
        self.texture_assets.extend(candidate_assets.iter().cloned());
        self.stats.texture_identity_payload_bytes=self.texture_assets.capacity()*std::mem::size_of::<Arc<super::world_texture::Asset>>();
        self.owner = Some(owner.clone());
        self.mesh = None;
        self.key = None;
        self.placements = Some(placements.clone());
        if fill.is_some(){self.section_pipeline=Some(pipeline.clone());}else{self.pipeline=Some(pipeline.clone());}
        Ok(Some(Arc::new(Prepared {
            diagnostic_samples:self.diagnostic_samples,
            resolve_prototype:self.active_resolve(),
            section: Section::OFF,
            contour:None,contour_eye_width:[0.;4],
            fill,
            source_sample_positions:super::annotation::source_raster_sample_positions(metal,self.samples.max(1)),
            billboards:None,
            placements: Some(placements),
            ghosts: None, ghost_edge: [0.;4], lines:[None,None],profiles:None,background:None,overlay:None,
            owner,
            mesh: None,
            pipeline,
            target,
            mvp: camera,
            clear,
            policy,
        })))
    }
    /// Cold observation only; excludes unrelated renderer resources.
    /// Cold diagnostic: one kind per unique backing owner, matching allocation rows.
    pub fn sampled_backing_kinds(&self)->Result<Vec<&'static str>,String>{
        let mut seen=Vec::new();let mut kinds=Vec::new();
        seen.try_reserve_exact(self.texture_assets.len()).map_err(|_|"Metal sampled observation allocation refused")?;
        kinds.try_reserve_exact(self.texture_assets.len()).map_err(|_|"Metal sampled observation allocation refused")?;
        for asset in &self.texture_assets{let(id,_)=asset.heap_observation();if !seen.contains(&id){seen.push(id);kinds.push(asset.backing_kind());}}
        Ok(kinds)
    }
    pub fn sampled_heap_allocations(&self)->Result<Vec<[u64;3]>,String>{
        let mut seen=Vec::new();let mut rows=Vec::new();
        seen.try_reserve_exact(self.texture_assets.len()).map_err(|_|"Metal sampled observation allocation refused")?;
        rows.try_reserve_exact(self.texture_assets.len()).map_err(|_|"Metal sampled observation allocation refused")?;
        for asset in self.texture_assets.iter(){let(id,row)=asset.heap_observation();if !seen.contains(&id){seen.push(id);rows.push(row);}}
        Ok(rows)
    }
    pub fn clear_idle(&mut self) {
        self.targets.retain(|t| Arc::strong_count(t) > 1);
        self.mesh = None;
        self.ghosts = None;
        self.placements = None;
        self.base_variants = [None,None];
        self.texture_assets.clear();
        self.stats.texture_identity_payload_bytes = self.texture_assets.capacity()*std::mem::size_of::<Arc<super::world_texture::Asset>>();
        self.stats.policy_retained_payload_bytes = 0;
        self.stats.material_order_payload_bytes=self.targets.iter().filter_map(|t|t.material_draws.as_ref()).map(|p|p.retained_payload_bytes).sum();
        self.stats.placement_cache_payload_bytes = 0;
        self.stats.placement_descriptor_payload_bytes = 0;
        self.stats.pose_storage_payload_bytes = self
            .targets
            .iter()
            .map(|t| t.poses.capacity() * std::mem::size_of::<PlacementPose>())
            .sum();
        self.key = None;
    }
}
impl Prepared {
    pub fn ghost_counts(&self)->[usize;3]{self.ghosts.as_ref().map_or([0;3],|g|g.counts)}
    pub fn sample_count(&self)->usize{self.target.samples}
    /// Programmed raster positions retained by this exact immutable frame.
    pub fn programmed_sample_positions(&self)->Option<[[f32;2];4]>{
        self.source_sample_positions.map(|positions|positions.map(|p|[p.x,p.y]))
    }
    pub(crate) fn depth_texture(&self)->&ProtocolObject<dyn MTLTexture>{&self.target.depth.raw}
    /// Admitted contiguous instanced draw groups, observed by diagnostics only.
    pub fn instance_run_count(&self) -> usize {
        if self.placements.is_none() {
            return 0;
        }
        self.target.order.as_ref().map_or(0, |o| o.runs.len())
    }
    /// Diagnostic routing observation; pixels remain the acceptance oracle.
    pub fn section_contours_enabled(&self)->bool{self.section.enabled()&&self.contour.is_some()}
    pub fn validate_owner(&self, metal: &NativeMetal) -> Result<(), String> {
        self.owner.validate(metal)
    }
    pub fn texture(&self) -> &ProtocolObject<dyn MTLTexture> {
        &self.target.color.raw
    }
    pub fn extent(&self) -> (u32, u32) {
        self.target.extent
    }
    /// Encode before the UI samples texture(), retaining this Prepared until completion.
    pub fn encode(&self, command: &ProtocolObject<dyn MTLCommandBuffer>) -> Result<(), String> {
        self.encode_inner(command, true, false, false)
    }
    fn encode_material_draws(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,placements:&PlacementMeshes,draws:&MaterialDraws,depth_enabled:bool,front_probe:bool)->Result<(),String>{
        self.encode_material_draws_observed(encoder,placements,draws,depth_enabled,front_probe,None).map(|_|())
    }
    fn encode_material_draws_observed(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,placements:&PlacementMeshes,draws:&MaterialDraws,depth_enabled:bool,front_probe:bool,observation:Option<&super::world_iso_observation::Observation>)->Result<bool,String>{
        if draws.operations.is_empty(){self.encode_fill(encoder)?;self.encode_aux(encoder,depth_enabled,false);return Ok(false);}
        let pipelines=self.pipeline.material.as_ref().ok_or("Metal material pipelines unavailable")?;let matrices=self.target.instance_matrices.as_ref().ok_or("Metal material matrices unavailable")?;
        let mut filled=false;
        for overlay_pass in [false,true] {
            if overlay_pass && !filled {if self.encode_fill_observed(encoder,observation)?{return Ok(true);}filled=true;}
            if overlay_pass {self.encode_aux(encoder,depth_enabled,false);}
        for op in &draws.operations {
            let overlay=self.overlay.is_some_and(|key|placements.packet.prototypes[op.prototype]==key);
            if overlay!=overlay_pass {continue;}
            let transparent=match op.kind{MaterialKind::Color(i)=>mesh_run_transparent(placements,op.prototype,i),MaterialKind::Texture{bucket,draw}=>placements.meshes[op.prototype].as_ref().unwrap().materials.as_ref().unwrap().textures[bucket].draws[draw].source.opacity<0.999,_=>false};
            if !overlay && transparent && !filled{if self.encode_fill_observed(encoder,observation)?{return Ok(true);}filled=true;}
            if !overlay && matches!(op.kind,MaterialKind::DefaultBack) && observation.is_some_and(|o|o.checkpoint(encoder,0)){return Ok(true);}
            bind_section(encoder,if overlay {Section::OFF}else{self.section});
            // Original fvcol draw is inside the global _faces_count>0 block.
            if !overlay && matches!(op.kind,MaterialKind::Opaque{cull:true,..}) && !draws.guard.open(self.overlay) {continue;}
            let mesh=placements.meshes[op.prototype].as_ref().ok_or("Metal admitted material prototype unavailable")?;
            encoder.setFrontFacingWinding(if op.reflected{MTLWinding::Clockwise}else{MTLWinding::CounterClockwise});
            let depth=if depth_enabled{&self.pipeline.depth}else{&self.pipeline.depth_disabled};
            encoder.setDepthStencilState(Some(depth));encoder.setRenderPipelineState(&self.pipeline.color);
            let paint=PaintUniform{front:self.policy.front,back:self.policy.back,mode:if overlay{0}else{match self.policy.mode{Mode::Material|Mode::MaterialAverage=>0,Mode::HiddenLine=>1,Mode::Monochrome=>2}},opacity:1.,padding:[u32::from(front_probe),0]};
            match op.kind {
                MaterialKind::Legacy=>{encoder.setDepthBias_slopeScale_clamp(1.,1.,0.);encode_placement_mesh(encoder,mesh,matrices,op.packed,op.instances,if overlay{Policy{mode:Mode::Material,..self.policy}}else{self.policy});},
                MaterialKind::Opaque{first,count,cull}=>{encoder.setDepthBias_slopeScale_clamp(1.,1.,0.);encoder.setCullMode(if cull{MTLCullMode::Back}else{MTLCullMode::None});unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.as_ref().ok_or("Metal admitted opaque draw buffer missing")?.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);}bind_paint(encoder,&paint);draw_instances(encoder,mesh,first,count,1);},
                MaterialKind::DefaultBack=>{if !matches!(self.policy.mode,Mode::Material|Mode::MaterialAverage){continue;}encoder.setDepthBias_slopeScale_clamp(1.,1.,0.);encoder.setCullMode(MTLCullMode::Front);unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.as_ref().ok_or("Metal admitted opaque draw buffer missing")?.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);}bind_paint(encoder,&PaintUniform{front:self.policy.back,mode:3,..paint});for &(first,count)in &mesh.plan.default_back{draw_instances(encoder,mesh,first,count,1);}},
                MaterialKind::Color(index)=>{let material=mesh.materials.as_ref().ok_or("Metal admitted material payload missing")?;let run=*material.runs.get(index).ok_or("Metal admitted material run stale")?;let default=run.phase==super::world_surface::Phase::DefaultBack;let alpha=matches!(run.phase,super::world_surface::Phase::TransparentFront|super::world_surface::Phase::TransparentBack);encoder.setDepthBias_slopeScale_clamp(if overlay||alpha||default{1.}else{0.},if overlay||alpha||default{1.}else{0.},0.);encoder.setRenderPipelineState(if alpha{&pipelines.colored_alpha}else{&self.pipeline.color});encoder.setDepthStencilState(Some(if !depth_enabled{&self.pipeline.depth_disabled}else if alpha{&pipelines.depth}else{&self.pipeline.depth}));encoder.setCullMode(if run.cull_front{MTLCullMode::Front}else if run.front_only{MTLCullMode::Back}else{MTLCullMode::None});unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&material.colored_vertices.as_ref().ok_or("Metal admitted color vertex buffer missing")?.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);}bind_paint(encoder,&PaintUniform{front:if default{self.policy.back}else{paint.front},mode:if default{3}else{0},opacity:run.opacity,..paint});unsafe{encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(MTLPrimitiveType::Triangle,run.count,MTLIndexType::UInt32,&material.colored_indices.as_ref().ok_or("Metal admitted color index buffer missing")?.raw,run.first*4,1);}},
                MaterialKind::Texture{bucket,draw}=>{let texture=&mesh.materials.as_ref().ok_or("Metal admitted material payload missing")?.textures[bucket];let draw=&texture.draws[draw];let source=&draw.source;let alpha=source.opacity<0.999;encoder.setDepthBias_slopeScale_clamp(if !source.back_side||alpha{1.}else{0.},if !source.back_side||alpha{1.}else{0.},0.);encoder.setRenderPipelineState(if alpha{&pipelines.textured_alpha}else{&pipelines.textured});encoder.setDepthStencilState(Some(if !depth_enabled{&self.pipeline.depth_disabled}else if alpha{&pipelines.depth}else{&self.pipeline.depth}));encoder.setCullMode(if source.back_side{MTLCullMode::Front}else if source.front_cull{MTLCullMode::Back}else{MTLCullMode::None});let opacity=op.opacity;let uniform=TexturePaint{opacity,shade:source.shade,hard_cutout:u32::from(texture.asset.cutout),height:self.extent().1,mean:[texture.asset.mean[0],texture.asset.mean[1],texture.asset.mean[2],if front_probe{-1.}else if !overlay && self.policy.mode.texture_mean(source.opacity,source.back_side){1.}else{0.}]};unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&texture.vertices.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);encoder.setFragmentTexture_atIndex(Some(&texture.asset.image),0);encoder.setFragmentSamplerState_atIndex(Some(&texture.asset.sampler),0);encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&uniform).cast(),std::mem::size_of::<TexturePaint>(),0);encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(MTLPrimitiveType::Triangle,draw.first as usize,draw.count as usize,1);}},
            }
            if !overlay && matches!(op.kind,MaterialKind::DefaultBack) && observation.is_some_and(|o|o.checkpoint(encoder,1)){return Ok(true);}
        }
        }
        encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);Ok(false)
    }
    fn encode_contours(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>)->Result<(),String>{
        let Some(pipeline)=&self.contour else{return Ok(());};
        if !self.section.enabled(){return Ok(());}
        encoder.setRenderPipelineState(&pipeline.state);encoder.setDepthStencilState(Some(&pipeline.depth));
        encoder.setCullMode(MTLCullMode::None);encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);
        bind_section(encoder,self.section);
        unsafe{encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&self.mvp).cast(),64,2);}
        let draw=|vertices:&Buffer,indices:Option<&Buffer>,stride:u32,first:usize,count:usize,instances:usize|->Result<(),String>{
            if count==0{return Ok(());}
            let triangles=u32::try_from(count/3).map_err(|_|"Metal contour triangle count admission")?;
            let draws=(count/3).checked_mul(instances).ok_or("Metal contour instance count overflow")?;
            // MSL instance_id is uint even though Metal draw counts are NSUInteger.
            u32::try_from(draws).map_err(|_|"Metal contour draw exceeds uint instance domain")?;
            let parameters=super::world_contour::Parameters{eye_width:self.contour_eye_width,color:self.ghost_edge,info:[stride,u32::from(indices.is_none()),u32::try_from(first).map_err(|_|"Metal contour index offset admission")?,triangles]};
            unsafe{
                encoder.setVertexBuffer_offset_atIndex(Some(&vertices.raw),0,0);
                encoder.setVertexBuffer_offset_atIndex(Some(&indices.unwrap_or(vertices).raw),0,4);
                encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&parameters).cast(),std::mem::size_of_val(&parameters),5);
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(MTLPrimitiveType::Triangle,0,6,draws);
            }Ok(())
        };
        if let Some(placements)=&self.placements{
            let matrices=self.target.instance_matrices.as_ref().ok_or("Metal contour placement matrices absent")?;
            for run in &self.target.order.as_ref().ok_or("Metal contour placement order absent")?.runs{
                if self.overlay==Some(placements.packet.prototypes[run.prototype as usize]){continue;}
                let Some(mesh)=&placements.meshes[run.prototype as usize] else{continue;};
                unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),run.first*64,1);}
                if let (Some(vertices),Some(indices))=(&mesh.vertices,&mesh.indices){draw(vertices,Some(indices),12,0,mesh.count,run.count)?;}
                if let Some(material)=&mesh.materials{
                    if let (Some(vertices),Some(indices))=(&material.colored_vertices,&material.colored_indices){
                        for source in &material.runs{if source.phase==super::world_surface::Phase::TransparentFront{draw(vertices,Some(indices),12,source.first,source.count,run.count)?;}}
                    }
                    for texture in &material.textures{for source in &texture.draws{if !source.source.back_side{draw(&texture.vertices,None,5,source.first as usize,source.count as usize,run.count)?;}}}
                }
            }
        }else if let Some(mesh)=&self.mesh{
            let identity=glam::Mat4::IDENTITY.to_cols_array_2d();
            unsafe{encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&identity).cast(),64,1);}
            if let (Some(vertices),Some(indices))=(&mesh.vertices,&mesh.indices){draw(vertices,Some(indices),12,0,mesh.count,1)?;}
        }
        Ok(())
    }
    fn encode_aux(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,depth_enabled:bool,preview_only:bool){
        if !preview_only {if let Some(billboards)=&self.billboards{billboards.encode(encoder,&self.mvp,&self.section,self.extent().1,depth_enabled);}}
        bind_section(encoder,if preview_only {Section::OFF}else{self.section});
        for lines in self.lines[usize::from(preview_only)..usize::from(preview_only)+1].iter().flatten() {
            if let Some(buffer)=&lines.buffer {
                encoder.setRenderPipelineState(&self.pipeline.lines);
                encoder.setDepthStencilState(Some(if depth_enabled{&self.pipeline.depth}else{&self.pipeline.depth_disabled}));
                encoder.setCullMode(MTLCullMode::None);encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);
                let uniform=super::world_ghost::Uniform{edge:self.ghost_edge,height:self.extent().1,mode:2,padding:[0;2]};
                unsafe {encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&self.mvp).cast(),64,2);encoder.setVertexBuffer_offset_atIndex(Some(&buffer.raw),0,0);encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&uniform).cast(),32,0);encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Line,0,lines.count);}
            }
        }
        if !preview_only {
        if let Some(lines)=&self.profiles {if let Some(buffer)=&lines.buffer {
            encoder.setRenderPipelineState(&self.pipeline.lines);encoder.setDepthStencilState(Some(if depth_enabled{&self.pipeline.depth}else{&self.pipeline.depth_disabled}));encoder.setCullMode(MTLCullMode::None);encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);
            let uniform=super::world_ghost::Uniform{edge:self.ghost_edge,height:self.extent().1,mode:2,padding:[0;2]};
            unsafe{encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&self.mvp).cast(),64,2);encoder.setVertexBuffer_offset_atIndex(Some(&buffer.raw),0,0);encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&uniform).cast(),32,0);encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Line,0,lines.count);}
        }}
        bind_section(encoder,Section::OFF);
        if let Some(ghosts)=&self.ghosts {
            encoder.setRenderPipelineState(&self.pipeline.ghost);
            encoder.setDepthStencilState(Some(if depth_enabled{&self.pipeline.depth}else{&self.pipeline.depth_disabled}));
            encoder.setCullMode(MTLCullMode::None);
            unsafe { encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&self.mvp).cast(),64,2); }
            if let Some(buffer)=&ghosts.wire {
                encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);
                let uniform=super::world_ghost::Uniform{edge:self.ghost_edge,height:self.extent().1,mode:2,padding:[0;2]};
                unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&buffer.raw),0,0);encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&uniform).cast(),32,0);encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Line,0,ghosts.wire_count);}
            }
            for i in 0..3 {
                if let Some(buffer)=&ghosts.buffers[i] {
                    encoder.setDepthBias_slopeScale_clamp(if i==2{0.}else{1.},if i==2{0.}else{1.},0.);
                    let uniform=super::world_ghost::Uniform{edge:self.ghost_edge,height:self.extent().1,mode:u32::from(i==2),padding:[0;2]};
                    unsafe {
                        encoder.setVertexBuffer_offset_atIndex(Some(&buffer.raw),0,0);
                        encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&uniform).cast(),32,0);
                        encoder.drawPrimitives_vertexStart_vertexCount(if i==2{MTLPrimitiveType::Line}else{MTLPrimitiveType::Triangle},0,ghosts.counts[i]);
                    }
                }
            }
        }
        }
    }
    pub(crate) fn encode_with_annotation_depth(&self, command:&ProtocolObject<dyn MTLCommandBuffer>, retain_depth:bool)->Result<(),String>{
        self.encode_inner(command,true,false,retain_depth)
    }
    fn encode_inner(
        &self,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
        depth_enabled: bool,
        front_probe: bool,
        retain_depth: bool,
    ) -> Result<(), String> {
        self.encode_inner_observed(command,depth_enabled,front_probe,retain_depth,None)
    }
    fn encode_inner_observed(
        &self,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
        depth_enabled: bool,
        front_probe: bool,
        retain_depth: bool,
        observation:Option<&super::world_iso_observation::Observation>,
    ) -> Result<(), String> {
        if !std::ptr::eq(&*command.commandQueue(), &*self.owner.queue) {
            return Err("Metal world command queue mismatch".into());
        }
        let pass = MTLRenderPassDescriptor::new();
        if let Some(mut positions)=self.source_sample_positions{unsafe{pass.setSamplePositions_count(positions.as_mut_ptr(),4);}}
        let color = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        color.setTexture(Some(self.target.multisample.as_ref().map_or(&*self.target.color.raw,|i|&*i.raw)));
        if self.target.multisample.is_some()&&!self.resolve_prototype.as_ref().is_some_and(|r|r.is_compute()){color.setResolveTexture(Some(&self.target.color.raw));}
        color.setLoadAction(MTLLoadAction::Clear);
        if let Some(resolve)=&self.resolve_prototype {resolve.configure(&pass);}
        color.setStoreAction(if self.resolve_prototype.as_ref().is_some_and(|r|r.is_compute()){MTLStoreAction::Store}else if self.target.multisample.is_some(){if self.diagnostic_samples {MTLStoreAction::StoreAndMultisampleResolve}else{MTLStoreAction::MultisampleResolve}}else{MTLStoreAction::Store});
        color.setClearColor(MTLClearColor {
            red: self.clear[0],
            green: self.clear[1],
            blue: self.clear[2],
            alpha: self.clear[3],
        });
        let depth = pass.depthAttachment();
        depth.setTexture(Some(&self.target.depth.raw));
        depth.setLoadAction(MTLLoadAction::Clear);
        depth.setStoreAction(if retain_depth {MTLStoreAction::Store}else{MTLStoreAction::DontCare});
        depth.setClearDepth(1.);
        if self.fill.is_some(){let stencil=pass.stencilAttachment();stencil.setTexture(Some(&self.target.depth.raw));stencil.setLoadAction(MTLLoadAction::Clear);stencil.setStoreAction(MTLStoreAction::DontCare);stencil.setClearStencil(0);}
        let encoder = command
            .renderCommandEncoderWithDescriptor(&pass)
            .ok_or("Metal world encoder unavailable")?;
        if let Some(background)=&self.background {
            encoder.setRenderPipelineState(&self.pipeline.background);
            encoder.setDepthStencilState(Some(&self.pipeline.depth_disabled));
            encoder.setCullMode(MTLCullMode::None);
            unsafe {encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(background).cast(),96,0);encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle,0,12);if background.horizon>-1.&&background.horizon<1.{encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Line,12,2);}}
        }
        encoder.setRenderPipelineState(&self.pipeline.color);
        // A pipeline with a depth attachment still binds a valid native state.
        // The negative control disables comparison/writes explicitly rather than
        // submitting a missing depth-state object that Metal may reject.
        encoder.setDepthStencilState(Some(if depth_enabled {
            &self.pipeline.depth
        } else {
            &self.pipeline.depth_disabled
        }));
        encoder.setCullMode(MTLCullMode::None);
        encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
        let camera = if self.placements.is_some() {
            &self.mvp
        } else {
            &IDENTITY
        };
        unsafe {
            encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(camera).cast(), 64, 2);
        }
        bind_section(&encoder,self.section);
        let mut prefix_stopped=false;
        if let Some(placements) = &self.placements {
            if let Some(draws)=&self.target.material_draws{
                match self.encode_material_draws_observed(&encoder,placements,draws,depth_enabled,front_probe,observation){Ok(stopped)=>prefix_stopped=stopped,Err(error)=>{encoder.endEncoding();return Err(error);}}
            }else{
            for run in &self.target.order.as_ref().unwrap().runs {
                encoder.setFrontFacingWinding(if run.reflected {
                    MTLWinding::Clockwise
                } else {
                    MTLWinding::CounterClockwise
                });
                if let Some(mesh) = &placements.meshes[run.prototype as usize] {
                    encode_placement_mesh(
                        &encoder,
                        mesh,
                        self.target.instance_matrices.as_ref().unwrap(),
                        run.first,
                        run.count,
                        self.policy,
                    );
                }
            }
            }
        } else if let Some(mesh) = &self.mesh {
            encode_mesh(&encoder, mesh, &self.mvp, self.policy);
        }
        if self.target.material_draws.is_none(){self.encode_fill(&encoder)?;self.encode_aux(&encoder,depth_enabled,false);}
        if !prefix_stopped {self.encode_contours(&encoder)?;
        self.encode_aux(&encoder,depth_enabled,true);
        if let Some(observation)=observation {observation.tile(&encoder);}}
        if let Some(resolve)=&self.resolve_prototype {resolve.tile(&encoder);}
        encoder.endEncoding();
        if let Some(resolve)=&self.resolve_prototype {resolve.compute(command,&self.target.multisample.as_ref().ok_or("Resolve prototype requires multisample target")?.raw,&self.target.color.raw)?;}
        Ok(())
    }
}
fn encode_mesh(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    mesh: &Mesh,
    mvp: &[[f32; 4]; 4],
    policy: Policy,
) {
    if mesh.count==0{return;}
    encoder.setCullMode(MTLCullMode::None);

    unsafe {
        encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.as_ref().unwrap().raw), 0, 0);
        encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(mvp).cast(), 64, 1);
    }
    let mode = match policy.mode {
        Mode::Material | Mode::MaterialAverage => 0,
        Mode::HiddenLine => 1,
        Mode::Monochrome => 2,
    };
    let uniform = PaintUniform {
        front: policy.front,
        back: policy.back,
        mode,
        opacity: 1.,
        padding: [0; 2],
    };
    bind_paint(encoder, &uniform);
    draw(encoder, mesh, 0, mesh.count);
    if matches!(policy.mode,Mode::Material|Mode::MaterialAverage) {
        encoder.setCullMode(MTLCullMode::Front);
        let default = PaintUniform {
            front: policy.back,
            mode: 3,
            ..uniform
        };
        bind_paint(encoder, &default);
        for &(first, count) in &mesh.plan.default_back {
            draw(encoder, mesh, first, count);
        }
        for paint in &mesh.plan.overrides {
            let override_paint = PaintUniform {
                front: paint.color,
                mode: 3,
                ..uniform
            };
            bind_paint(encoder, &override_paint);
            draw(encoder, mesh, paint.first, paint.count);
        }
    }
}

fn encode_placement_mesh(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    mesh: &Mesh,
    matrices: &Buffer,
    first_instance: usize,
    instances: usize,
    policy: Policy,
) {
    // Source _instanced_eligible excludes explicit back override/alpha content.
    // Draw order follows the source ordinary pass then default-back pass.
    if mesh.count==0{return;}
    encoder.setCullMode(MTLCullMode::None);
    unsafe {
        encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.as_ref().unwrap().raw), 0, 0);
        encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw), first_instance * 64, 1);
    }
    let ordinary = PaintUniform {
        front: policy.front,
        back: policy.back,
        mode: match policy.mode {
            Mode::Material | Mode::MaterialAverage => 0,
            Mode::HiddenLine => 1,
            Mode::Monochrome => 2,
        },
        opacity: 1.,
        padding: [0; 2],
    };
    bind_paint(encoder, &ordinary);
    draw_instances(encoder, mesh, 0, mesh.count, instances);
    if matches!(policy.mode,Mode::Material|Mode::MaterialAverage) {
        // Literal source _draw_instanced_faces: all ordinary faces first,
        // then default backs with front culling and LEQUAL. A coplanar front
        // authored after a back must not erase this later default-back pass.
        encoder.setCullMode(MTLCullMode::Front);
        let back = PaintUniform {
            front: policy.back,
            mode: 3,
            ..ordinary
        };
        bind_paint(encoder, &back);
        for &(first, count) in &mesh.plan.default_back {
            draw_instances(encoder, mesh, first, count, instances);
        }
        encoder.setCullMode(MTLCullMode::None);
    }
}

fn material_draws(placements:&PlacementMeshes,order:&super::world_placement::Order,poses:&[PlacementPose])->Result<MaterialDraws,String>{
    const CAP:usize=8*1024*1024;
    let mut count=0usize;for &prototype in &placements.packet.bindings{if let Some(mesh)=&placements.meshes[prototype as usize]{let n=mesh.materials.as_ref().map_or(1,|m|m.opaque_runs.len()+1+m.runs.len()+m.textures.iter().map(|t|t.draws.len()).sum::<usize>());count=count.checked_add(n).ok_or("Metal material operation count overflow")?;}}
    let record_size=std::mem::size_of::<((u8,i32,bool,usize,usize),MaterialDraw)>();let bound=count.checked_mul(record_size+std::mem::size_of::<MaterialDraw>()+256).and_then(|n|order.indices.len().checked_mul(8).and_then(|m|n.checked_add(m))).filter(|n|*n<=CAP).ok_or("Metal material order admission exceeds8MiB")?;let _=bound;
    let mut reverse=Vec::new();reverse.try_reserve_exact(order.indices.len()).map_err(|_|"Metal material reverse order allocation refused")?;reverse.resize(order.indices.len(),0);for(packed,&original)in order.indices.iter().enumerate(){reverse[original]=packed;}
    let mut guard=super::world_face_guard::FaceGuard::new();
    let mut records=Vec::new();records.try_reserve_exact(count).map_err(|_|"Metal material order allocation refused")?;
    let mut buckets=std::collections::HashMap::<(super::world_texture::Identity,u32,i32,bool,bool,u8),usize>::new();buckets.try_reserve(count).map_err(|_|"Metal sampled order bucket allocation refused")?;
    for(original,&prototype)in placements.packet.bindings.iter().enumerate(){let Some(mesh)=&placements.meshes[prototype as usize]else{continue};let Some(material)=&mesh.materials else{continue};if material.instanced{continue;}if material.consolidated{guard.add(placements.packet.prototypes[prototype as usize]);}let base=MaterialDraw{opacity:1.,prototype:prototype as usize,packed:reverse[original],instances:1,reflected:poses[original].reflected,kind:MaterialKind::DefaultBack};
        for &(first,count,cull)in &material.opaque_runs{records.push(((0,0,false,original,first),MaterialDraw{kind:MaterialKind::Opaque{first,count,cull},..base}));}
        if !mesh.plan.default_back.is_empty(){records.push(((2,0,false,original,0),base));}
        for(index,run)in material.runs.iter().enumerate(){let phase=match run.phase{super::world_surface::Phase::OpaqueBack=>4,super::world_surface::Phase::DefaultBack=>2,super::world_surface::Phase::TransparentFront=>6,super::world_surface::Phase::TransparentBack=>7};records.push(((phase,(run.opacity*1000.).round()as i32,run.front_only,original,index),MaterialDraw{kind:MaterialKind::Color(index),..base}));}
        for(bucket,texture)in material.textures.iter().enumerate(){for(draw,d)in texture.draws.iter().enumerate(){let source=&d.source;let opaque=source.opacity>=0.999;let phase=match(opaque,source.back_side){(true,false)=>1,(true,true)=>5,(false,false)=>8,(false,true)=>9};let alpha=if opaque{1000}else{(format!("{:.3}",source.opacity).parse::<f64>().map_err(|_|"Metal texture opacity rounding refused")?*1000.).round()as i32};let key=(texture.asset.id,source.shade.to_bits(),alpha,source.front_cull,source.back_side,phase);let next=buckets.len();let first=*buckets.entry(key).or_insert(next);records.push(((phase,alpha,source.front_cull,first,original),MaterialDraw{opacity:alpha as f32/1000.,kind:MaterialKind::Texture{bucket,draw},..base}));}}
    }
    for (index,run)in order.runs.iter().enumerate(){let Some(mesh)=&placements.meshes[run.prototype as usize]else{continue};if mesh.materials.as_ref().is_none_or(|m|m.instanced){records.push(((3,0,false,index,0),MaterialDraw{opacity:1.,prototype:run.prototype as usize,packed:run.first,instances:run.count,reflected:run.reflected,kind:MaterialKind::Legacy}));}}
    records.sort_unstable_by_key(|r|r.0);let mut operations=Vec::new();operations.try_reserve_exact(records.len()).map_err(|_|"Metal retained material operations allocation refused")?;operations.extend(records.into_iter().map(|r|r.1));let retained_payload_bytes=operations.capacity().checked_mul(std::mem::size_of::<MaterialDraw>()).and_then(|n|n.checked_add(std::mem::size_of::<MaterialDraws>())).ok_or("Metal material operation capacity overflow")?;if retained_payload_bytes>CAP{return Err("Metal retained material operations exceeds8MiB".into());}Ok(MaterialDraws{guard,operations,retained_payload_bytes})
}

fn build_material_mesh<'a,G>(metal:&NativeMetal,collection:Collection,resolve:&mut G,published:&[Arc<super::world_texture::Asset>],candidates:&mut Vec<Arc<super::world_texture::Asset>>,shared:&[(String,Arc<super::world_texture::Asset>)],uploads:&mut u64)->Result<Option<Arc<Mesh>>,String>
where G:FnMut(&str)->Result<std::borrow::Cow<'a,[u8]>,String>{
    let (data,materials)=match collection {
        Collection::Opaque(data)=>(data,None),
        Collection::Surfaces(data)=>{
            let packet=super::world_surface::Packet::prepare(data)?;
            let mut opaque=packet.opaque;let mut opaque_runs=Vec::new();opaque_runs.try_reserve_exact(opaque.source_front_cull.len()*2+1).map_err(|_|"Metal opaque source run allocation refused")?;let mut first=0;
            for &(start,count)in &opaque.source_front_cull{if start>first{opaque_runs.push((first,start-first,false));}opaque_runs.push((start,count,true));first=start+count;}if first<opaque.indices.len(){opaque_runs.push((first,opaque.indices.len()-first,false));}
            // Back overrides are drawn by the separately ordered material pass.
            opaque.source_back_paints.clear();opaque.source_front_cull.clear();opaque.source_opacity_runs.clear();for v in &mut opaque.vertices{v.color[3]=1.;}
            let colored_vertices=if packet.colored.indices.is_empty(){None}else{Some(upload(metal,bytemuck::cast_slice(&packet.colored.vertices))?)};
            let colored_indices=if packet.colored.indices.is_empty(){None}else{Some(upload(metal,bytemuck::cast_slice(&packet.colored.indices))?)};
            let mut textures=Vec::new();textures.try_reserve_exact(packet.textures.len()).map_err(|_|"Metal sampled mesh allocation refused")?;
            for texture in packet.textures {
                let encoded=resolve(&texture.path)?;let id=super::world_texture::identity(&encoded)?;
                let asset=candidates.iter().find(|a|a.id==id).cloned().or_else(||published.iter().find(|a|a.id==id).cloned());
                let asset=if let Some(asset)=asset{asset}else{if candidates.len()+published.len()>=4096{return Err("Metal retained sampled asset identities exceed4096".into());}candidates.try_reserve(1).map_err(|_|"Metal sampled asset publication allocation refused")?;let asset=if let Some((_,asset))=shared.iter().find(|(_,a)|a.id==id){asset.clone()}else{let asset=super::world_texture::upload(metal,id,&encoded,published,candidates)?;*uploads+=1;asset};candidates.push(asset.clone());asset};
                textures.push(SampledMesh{vertices:upload(metal,texture.geometry.upload_bytes())?,asset,draws:texture.geometry.draws});
            }
            let retained_payload_bytes=std::mem::size_of::<Materials>()+packet.runs.capacity()*std::mem::size_of::<super::world_surface::Run>()+opaque_runs.capacity()*std::mem::size_of::<(usize,usize,bool)>()+textures.capacity()*std::mem::size_of::<SampledMesh>()+textures.iter().map(|t|t.draws.capacity()*std::mem::size_of::<ingetrazo_render::native_textures::Draw>()).sum::<usize>();
            if retained_payload_bytes>8*1024*1024{return Err("Metal retained material runs exceed8MiB".into());}
            let consolidated=opaque.source_consolidated_opaque_count>0;
            let instanced=opaque.source_consolidated_opaque_count==0&&packet.runs.is_empty()&&textures.is_empty();
            (opaque,Some(Materials{instanced,consolidated,colored_vertices,colored_indices,runs:packet.runs,textures,opaque_runs,retained_payload_bytes}))
        }
    };
    let plan=super::world_admission::opaque(&data)?;
    let has_materials=materials.as_ref().is_some_and(|m|m.colored_indices.is_some()||!m.textures.is_empty());
    if data.indices.is_empty()&&!has_materials{return Ok(None);}
    let vertices=if data.indices.is_empty(){None}else{Some(upload(metal,bytemuck::cast_slice(&data.vertices))?)};
    let indices=if data.indices.is_empty(){None}else{Some(upload(metal,bytemuck::cast_slice(&data.indices))?)};
    Ok(Some(Arc::new(Mesh{vertices,indices,count:data.indices.len(),plan,materials})))
}

fn pipeline(metal: &NativeMetal, rich:bool, samples:usize, fill:bool) -> Result<Pipeline, String> {
    let lib = metal
        .device
        .newLibraryWithSource_options_error(
            &NSString::from_str(include_str!("shaders/world.metal")),
            None,
        )
        .map_err(|e| format!("Metal world shader:{e:?}"))?;
    let vertex = lib
        .newFunctionWithName(&NSString::from_str("world_vertex"))
        .ok_or("World vertex missing")?;
    let fragment = lib
        .newFunctionWithName(&NSString::from_str("world_fragment"))
        .ok_or("World fragment missing")?;
    let d = MTLRenderPipelineDescriptor::new();
    d.setVertexFunction(Some(&vertex));
    d.setFragmentFunction(Some(&fragment));
    d.setDepthAttachmentPixelFormat(if fill{MTLPixelFormat::Depth32Float_Stencil8}else{MTLPixelFormat::Depth32Float});if fill{d.setStencilAttachmentPixelFormat(MTLPixelFormat::Depth32Float_Stencil8);}d.setRasterSampleCount(samples);
    unsafe { d.colorAttachments().objectAtIndexedSubscript(0) }
        .setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    let color = metal
        .device
        .newRenderPipelineStateWithDescriptor_error(&d)
        .map_err(|e| format!("Metal world pipeline:{e:?}"))?;
    let depth = MTLDepthStencilDescriptor::new();
    depth.setDepthCompareFunction(MTLCompareFunction::LessEqual);
    depth.setDepthWriteEnabled(true);
    let disabled = MTLDepthStencilDescriptor::new();
    disabled.setDepthCompareFunction(MTLCompareFunction::Always);
    disabled.setDepthWriteEnabled(false);
    let material=if rich{
        let alpha=MTLDepthStencilDescriptor::new();alpha.setDepthCompareFunction(MTLCompareFunction::LessEqual);alpha.setDepthWriteEnabled(false);
        Some(MaterialPipeline{colored_alpha:material_pipeline(metal,&lib,"world_vertex","world_fragment",true,samples,fill)?,textured:material_pipeline(metal,&lib,"world_texture_vertex","world_texture_fragment",true,samples,fill)?,textured_alpha:material_pipeline(metal,&lib,"world_texture_vertex","world_texture_fragment",true,samples,fill)?,depth:metal.device.newDepthStencilStateWithDescriptor(&alpha).ok_or("Metal material depth state unavailable")?})
    }else{None};
    Ok(Pipeline {
        fill:if fill{Some(fill_pipeline(metal,&lib,samples)?)}else{None},
        background: material_pipeline(metal,&lib,"world_background_vertex","world_background_fragment",false,samples,fill)?,
        lines: material_pipeline(metal,&lib,"world_line_vertex","world_ghost_fragment",false,samples,fill)?,
        ghost: material_pipeline(metal,&lib,"world_ghost_vertex","world_ghost_fragment",false,samples,fill)?,
        color,material,
        depth_disabled: metal
            .device
            .newDepthStencilStateWithDescriptor(&disabled)
            .ok_or("World disabled depth state missing")?,
        depth: metal
            .device
            .newDepthStencilStateWithDescriptor(&depth)
            .ok_or("World depth state missing")?,
    })
}
fn material_pipeline(metal:&NativeMetal,lib:&ProtocolObject<dyn MTLLibrary>,vertex:&str,fragment:&str,alpha:bool,samples:usize,fill:bool)->Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>,String>{
 let vertex=lib.newFunctionWithName(&NSString::from_str(vertex)).ok_or("Metal material vertex function missing")?;let fragment=lib.newFunctionWithName(&NSString::from_str(fragment)).ok_or("Metal material fragment function missing")?;let d=MTLRenderPipelineDescriptor::new();d.setVertexFunction(Some(&vertex));d.setFragmentFunction(Some(&fragment));d.setDepthAttachmentPixelFormat(if fill{MTLPixelFormat::Depth32Float_Stencil8}else{MTLPixelFormat::Depth32Float});if fill{d.setStencilAttachmentPixelFormat(MTLPixelFormat::Depth32Float_Stencil8);}d.setRasterSampleCount(samples);let color=unsafe{d.colorAttachments().objectAtIndexedSubscript(0)};color.setPixelFormat(MTLPixelFormat::RGBA8Unorm);color.setBlendingEnabled(alpha);
 if alpha{color.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);color.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);color.setRgbBlendOperation(MTLBlendOperation::Add);color.setSourceAlphaBlendFactor(MTLBlendFactor::Zero);color.setDestinationAlphaBlendFactor(MTLBlendFactor::One);color.setAlphaBlendOperation(MTLBlendOperation::Add);}
 metal.device.newRenderPipelineStateWithDescriptor_error(&d).map_err(|e|format!("Metal material pipeline:{e:?}"))
}
fn image(
    metal: &NativeMetal,
    extent: (u32, u32),
    format: MTLPixelFormat,
    usage: MTLTextureUsage,
    bpp: u64,
) -> Result<Image, String> {
    image_samples(metal,extent,format,usage,bpp,1)
}
fn image_samples(metal:&NativeMetal,extent:(u32,u32),format:MTLPixelFormat,usage:MTLTextureUsage,bpp:u64,samples:usize)->Result<Image,String>{
    let requested = u64::from(extent.0).checked_mul(u64::from(extent.1)).and_then(|n|n.checked_mul(bpp)).and_then(|n|n.checked_mul(samples as u64)).ok_or("Metal sample target size overflow")?;
    let d = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            format,
            extent.0 as usize,
            extent.1 as usize,
            false,
        )
    };
    if samples>1 {d.setTextureType(MTLTextureType::Type2DMultisample);unsafe {d.setSampleCount(samples);}}
    d.setStorageMode(MTLStorageMode::Private);
    d.setHazardTrackingMode(MTLHazardTrackingMode::Tracked);
    d.setUsage(usage);
    // Apple's heap query describes creation FROM A HEAP, not the allocation
    // size of newTextureWithDescriptor. Observe it, but do not use it as an
    // upper bound for this standalone resource. Reserve remaining logical
    // capacity before allocation, then reconcile actual allocatedSize under
    // the budget lock before publication. This is not a physical allocator bound.
    let native = metal.device.heapTextureSizeAndAlignWithDescriptor(&d);
    if (native.size == 0) != (native.align == 0) {
        return Err(format!("World native texture size query invalid: requested_bytes={requested}, queried_size={}, queried_align={}, extent={}x{}, format={format:?}, samples={samples}, device={}", native.size, native.align, extent.0, extent.1, metal.device.name()));
    }
    let (admitted, ticket) = {
        let mut budget = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
        let admitted = super::world_admission::standalone_capacity(&budget)?;
        if admitted == 0 || (native.size == 0 && admitted < requested) {
            return Err(format!("World target exceeds available allocation admission: ticket_bytes={admitted}, requested_bytes={requested}, queried_size={}, queried_align={}, extent={}x{}, format={format:?}, samples={samples}, device={}", native.size, native.align, extent.0, extent.1, metal.device.name()));
        }
        (admitted, budget.reserve(0, admitted)?)
    };
    let Some(raw) = metal.device.newTextureWithDescriptor(&d) else {
        metal
            .budget
            .lock()
            .map_err(|_| "Metal budget poisoned")?
            .cancel(ticket)?;
        return Err("Metal world target allocation failed".into());
    };
    let handle = (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
    let actual = MTLResource::allocatedSize(&*raw) as u64;
    let mut budget = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
    if actual == 0 || actual > admitted {
        budget.cancel(ticket)?;
        return Err(format!("World standalone texture has unusable size or exceeds admission ticket: actual_bytes={actual}, ticket_bytes={admitted}, requested_bytes={requested}, queried_size={}, queried_align={}, extent={}x{}, format={format:?}, samples={samples}, device={}", native.size, native.align, extent.0, extent.1, metal.device.name()));
    }
    if admitted != actual {
        budget.cancel(ticket)?;
        let ticket = budget.reserve(0, actual)?;
        budget.commit(ticket, handle)?;
    } else {
        budget.commit(ticket, handle)?;
    }
    drop(budget);
    Ok(Image {
        raw: ManuallyDrop::new(raw),
        budget: metal.budget.clone(),
        handle,
    })
}

pub(crate) fn upload(metal: &NativeMetal, bytes: &[u8]) -> Result<Buffer, String> {
    let mut buffer = metal.buffer(bytes.len(), true)?;
    buffer.write(bytes)?;
    Ok(buffer)
}

// These immutable upload buffers are allocated shared and only read by the GPU.
// Cold engineering receipt reads one validated vertex per admitted color run.
fn diagnostic_first_color(vertices: &Buffer, indices: &Buffer, first: usize) -> Result<[f32;4],String> {
    let offset=first.checked_mul(4).filter(|o|o.checked_add(4).is_some_and(|e|e<=indices.length())).ok_or("Diagnostic index read exceeds admitted buffer")?;
    let index=unsafe{indices.raw.contents().as_ptr().cast::<u8>().add(offset).cast::<u32>().read_unaligned()} as usize;
    let size=std::mem::size_of::<ingetrazo_render::buffer::Vertex>();
    let offset=index.checked_mul(size).filter(|o|o.checked_add(size).is_some_and(|e|e<=vertices.length())).ok_or("Diagnostic vertex read exceeds admitted buffer")?;
    Ok(unsafe{vertices.raw.contents().as_ptr().cast::<u8>().add(offset).cast::<ingetrazo_render::buffer::Vertex>().read_unaligned()}.color)
}
/// Offscreen acceptance only. Production hosts encode in their existing command.
/// Completion-unknown drops quarantine all owners; no unbounded GPU wait.
pub struct Diagnostic {
    command: ManuallyDrop<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    frame: ManuallyDrop<Arc<Prepared>>,
    readback: ManuallyDrop<Buffer>,
    row: usize,
    sample_owner: ManuallyDrop<Option<(Buffer, Retained<ProtocolObject<dyn MTLComputePipelineState>>)>>,
    iso_owner:ManuallyDrop<Option<super::world_iso_observation::Observation>>,
    poison: Arc<std::sync::atomic::AtomicBool>,
}
impl Prepared {
    /// Engineering-only front-facing mask, using the admitted native draw order,
    /// culls, depth/bias and buffers. Green is front, red is back. Restoring the
    /// ordinary render afterward is required before the UI samples this target.
    pub fn submit_front_facing_diagnostic(self: &Arc<Self>, metal: &NativeMetal) -> Result<Diagnostic, String> {
        if self.target.material_draws.as_ref().is_none_or(|d|d.operations.iter().any(|op|matches!(op.kind,MaterialKind::Legacy))) {
            return Err("Front-facing diagnostic requires explicit material runs".into());
        }
        self.submit_diagnostic_inner(metal, true, true, None,None,false,None)
    }
    /// Cold bounded receipt of actual admitted commands, not recomputed Scene metadata.
    pub fn material_diagnostic(&self) -> Result<serde_json::Value, String> {
        let draws=self.target.material_draws.as_ref().ok_or("No admitted material draws")?;
        if draws.operations.len()>65536 {return Err("Material diagnostic exceeds65536 operations".into());}
        let placements=self.placements.as_ref().ok_or("No admitted placements")?;
        let mut rows=Vec::new();rows.try_reserve_exact(draws.operations.len()).map_err(|_|"Material diagnostic capacity refused")?;
        for op in &draws.operations {
            let mesh=placements.meshes[op.prototype].as_ref().ok_or("Diagnostic mesh absent")?;
            let row=match op.kind {
                MaterialKind::Opaque{first,count,cull}=>serde_json::json!({"kind":"opaque","first":first,"count":count,"cull":if cull{"back"}else{"none"},"depth_write":true,"bias":[1,1],"submitted":!cull||self.overlay==Some(placements.packet.prototypes[op.prototype])||draws.guard.open(self.overlay)}),
                MaterialKind::Color(index)=>{let material=mesh.materials.as_ref().ok_or("Diagnostic materials absent")?;let run=&material.runs[index];let first_color=diagnostic_first_color(material.colored_vertices.as_ref().ok_or("Diagnostic color buffer absent")?,material.colored_indices.as_ref().ok_or("Diagnostic index buffer absent")?,run.first)?;let alpha=matches!(run.phase,super::world_surface::Phase::TransparentFront|super::world_surface::Phase::TransparentBack);serde_json::json!({"kind":"color","phase":format!("{:?}",run.phase),"first_color":first_color,"first":run.first,"count":run.count,"opacity":run.opacity,"cull":if run.cull_front{"front"}else if run.front_only{"back"}else{"none"},"depth_write":!alpha,"bias":if alpha||run.phase==super::world_surface::Phase::DefaultBack{[1,1]}else{[0,0]}})},
                MaterialKind::DefaultBack=>serde_json::json!({"kind":"default_back","cull":"front","depth_write":true,"bias":[1,1]}),
                MaterialKind::Legacy=>serde_json::json!({"kind":"legacy"}),
                MaterialKind::Texture{bucket,draw}=>{let source=&mesh.materials.as_ref().ok_or("Diagnostic materials absent")?.textures[bucket].draws[draw].source;serde_json::json!({"kind":"texture","back_side":source.back_side,"opacity":source.opacity,"cull":if source.back_side{"front"}else if source.front_cull{"back"}else{"none"},"depth_write":source.opacity>=0.999,"bias":if !source.back_side||source.opacity<0.999{[1,1]}else{[0,0]}})},
            };
            rows.push(serde_json::json!({"prototype":op.prototype,"reflected":op.reflected,"winding":if op.reflected{"clockwise"}else{"counterclockwise"},"draw":row}));
        }
        Ok(serde_json::json!({"depth_compare":"less_equal","sample_count":self.sample_count(),"operations":rows}))
    }
    pub fn submit_diagnostic(self: &Arc<Self>, metal: &NativeMetal) -> Result<Diagnostic, String> {
        self.submit_diagnostic_inner(metal, true, false, None,None,false,None)
    }
    /// Negative control only: literal disabled depth state must fail the near-first oracle.
    pub fn submit_diagnostic_without_depth(
        self: &Arc<Self>,
        metal: &NativeMetal,
    ) -> Result<Diagnostic, String> {
        self.submit_diagnostic_inner(metal, false, false, None,None,false,None)
    }
    pub fn source_resolve_diagnostic(&self)->serde_json::Value{
        self.resolve_prototype.as_ref().map_or(serde_json::json!({"path":"untouched-native-reference"}),|r|r.diagnostic(self.extent()))
    }
    /// Four fixed diagnostic pixels, in Metal top-origin coordinates. No resolve replacement.
    pub fn submit_sample_diagnostic(self:&Arc<Self>,metal:&NativeMetal,points:[[u32;2];4])->Result<Diagnostic,String>{
        if !self.diagnostic_samples || self.sample_count()!=4 || self.target.multisample.is_none(){return Err("Cold four-sample observation opt-in required".into());}
        if points.iter().any(|p|p[0]>=self.extent().0||p[1]>=self.extent().1){return Err("Sample observation pixel outside target".into());}
        self.submit_diagnostic_inner(metal,true,false,Some(points),None,false,None)
    }
    /// Probe only, final color before tile normalization and final post-cap depth.
    pub fn submit_iso_observation(self:&Arc<Self>,metal:&NativeMetal,points:[[u32;2];3])->Result<Diagnostic,String>{
        if self.sample_count()!=4 || self.source_resolve_diagnostic()["path"]!="same-pass-tile" {return Err("Iso observation requires production four-sample tile resolve".into());}
        if points.iter().any(|p|p[0]>=self.extent().0||p[1]>=self.extent().1){return Err("Iso point outside admitted target".into());}
        self.submit_diagnostic_inner(metal,true,false,None,Some(points),false,None)
    }
    /// Probe-only five in-pass checkpoints or one terminal prefix; never resumes a pass.
    pub fn submit_iso_stages(self:&Arc<Self>,metal:&NativeMetal,points:[[u32;2];3],stop:Option<u8>)->Result<Diagnostic,String>{
        if stop.is_some_and(|s|s>3){return Err("Iso prefix stage outside0..3".into());}
        if self.sample_count()!=4 || self.source_resolve_diagnostic()["path"]!="same-pass-tile" {return Err("Iso stages require actual production tile path".into());}
        if points!=[[297,329],[297,328],[298,303]]{return Err("Iso stage points differ from literal inventory".into());}
        if self.overlay.is_some() || self.fill.is_none(){return Err("Iso stages require literal non-overlay filled scene".into());}
        let commands=self.material_diagnostic()?;
        if commands["operations"].as_array().map(Vec::len)!=Some(2) || commands["operations"][0]["draw"]["kind"]!="opaque" || commands["operations"][1]["draw"]["kind"]!="default_back" {return Err(format!("Iso stage draw inventory differs:{commands:?}"));}
        self.submit_diagnostic_inner(metal,true,false,None,Some(points),true,stop)
    }
    fn submit_diagnostic_inner(
        self: &Arc<Self>,
        metal: &NativeMetal,
        depth_enabled: bool,
        front_probe: bool,
        points:Option<[[u32;2];4]>,
        iso_points:Option<[[u32;2];3]>,
        iso_stages:bool,iso_stop:Option<u8>,
    ) -> Result<Diagnostic, String> {
        self.owner.validate(metal)?;
        let (w, h) = self.extent();
        if w > 4096 || h > 4096 {
            return Err("World diagnostic extent exceeds4096".into());
        }
        let row = (w as usize * 4 + 255) & !255;
        let readback = metal.buffer(row * h as usize, true)?;
        let descriptor = MTLCommandBufferDescriptor::new();
        descriptor.setErrorOptions(MTLCommandBufferErrorOption::EncoderExecutionStatus);
        let command = metal
            .queue
            .commandBufferWithDescriptor(&descriptor)
            .ok_or("World diagnostic command unavailable")?;
        command.setLabel(Some(&NSString::from_str(if depth_enabled {
            "World depth positive diagnostic"
        } else {
            "World explicit Always/no-write negative diagnostic"
        })));
        let iso_owner=iso_points.map(|p|super::world_iso_observation::Observation::new(metal,p,iso_stages,iso_stop)).transpose()?;
        if let Some(o)=&iso_owner {
            o.owner.validate(metal)?;
            let Some(resolve)=&self.resolve_prototype else{return Err("Iso production resolve owner absent".into());};
            let super::world_resolve::Kind::Tile(p)=&resolve.kind else{return Err("Iso production tile owner absent".into());};
            if o.sample_length()>p.imageblockSampleLength(){return Err("Iso observer exceeds admitted imageblock sample length".into());}
        }
        self.encode_inner_observed(&command, depth_enabled, front_probe, iso_owner.is_some(),iso_owner.as_ref())?;
        let blit = command
            .blitCommandEncoder()
            .ok_or("World diagnostic blit unavailable")?;
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(self.texture(),0,0,MTLOrigin{x:0,y:0,z:0},MTLSize{width:w as usize,height:h as usize,depth:1},&readback.raw,0,row,row*h as usize);
        }
        blit.endEncoding();
        let sample_owner=if let Some(points)=points {
            let source="#include <metal_stdlib>\nusing namespace metal; kernel void observe(texture2d_ms<float,access::read> image [[texture(0)]], constant uint2* points [[buffer(0)]], device float4* output [[buffer(1)]], uint id [[thread_position_in_grid]]) { if(id<16) output[id]=image.read(points[id/4],id%4); }";
            let options=MTLCompileOptions::new();options.setFastMathEnabled(false);
            let library=metal.device.newLibraryWithSource_options_error(&NSString::from_str(source),Some(&options)).map_err(|e|format!("Sample observation shader: {e:?}"))?;
            let function=library.newFunctionWithName(&NSString::from_str("observe")).ok_or("Sample observation function unavailable")?;
            let pipeline=metal.device.newComputePipelineStateWithFunction_error(&function).map_err(|e|format!("Sample observation pipeline: {e:?}"))?;
            let output=metal.buffer(16*16,true)?;
            let encoder=command.computeCommandEncoder().ok_or("Sample observation encoder unavailable")?;
            encoder.setComputePipelineState(&pipeline);
            unsafe {encoder.setTexture_atIndex(Some(&self.target.multisample.as_ref().unwrap().raw),0);encoder.setBytes_length_atIndex(std::ptr::NonNull::from(&points).cast(),std::mem::size_of_val(&points),0);encoder.setBuffer_offset_atIndex(Some(&output.raw),0,1);}
            encoder.dispatchThreads_threadsPerThreadgroup(MTLSize{width:16,height:1,depth:1},MTLSize{width:16,height:1,depth:1});encoder.endEncoding();Some((output,pipeline))
        }else{None};
        if let Some(o)=&iso_owner{o.depth(&command,&self.target.depth.raw)?;}
        command.commit();
        Ok(Diagnostic {
            command: ManuallyDrop::new(command),
            frame: ManuallyDrop::new(self.clone()),
            readback: ManuallyDrop::new(readback),
            row,
            sample_owner:ManuallyDrop::new(sample_owner),
            iso_owner:ManuallyDrop::new(iso_owner),
            poison: metal.poisoned.clone(),
        })
    }
}
impl Diagnostic {
    pub fn iso_values(&self)->Result<([[f32;4];12],[f32;12]),String>{
        if self.command.status()!=MTLCommandBufferStatus::Completed{return Err("Iso completion not acknowledged".into());}
        let o=self.iso_owner.as_ref().ok_or("No iso observation submitted")?;
        let stage=o.stop.unwrap_or(4) as usize;
        let colors=unsafe{std::ptr::read(o.colors.raw.contents().as_ptr().cast::<u8>().add(stage*192).cast::<[[f32;4];12]>())};
        let depths=unsafe{std::ptr::read(o.depths.raw.contents().as_ptr().cast::<[f32;12]>())};
        if colors.iter().flatten().chain(depths.iter()).any(|v|!v.is_finite()){return Err("Iso observer left nonfinite/unwritten samples".into());}
        Ok((colors,depths))
    }

    /// Full-pass color checkpoints only; rejected contenders are not inferred.
    pub fn iso_stage_colors(&self)->Result<[[[f32;4];12];5],String>{
        if self.command.status()!=MTLCommandBufferStatus::Completed{return Err("Iso stage completion absent".into());}
        let o=self.iso_owner.as_ref().ok_or("Iso stage owner absent")?;
        if !o.stages || o.stop.is_some(){return Err("Full stage colors require an uninterrupted stage observation".into());}
        let values=unsafe{std::ptr::read(o.colors.raw.contents().as_ptr().cast::<[[[f32;4];12];5]>())};
        if values.iter().flatten().flatten().any(|v|!v.is_finite()){return Err("Iso stage slice unwritten/nonfinite".into());}
        Ok(values)
    }

    /// Read-only metadata; readback itself remains gated on Completed.
    pub fn resource_observation(&self)->serde_json::Value {
        let f=&self.frame;
        let texture=|t:&ProtocolObject<dyn MTLTexture>|serde_json::json!({"identity":format!("{:p}",t),"storage":format!("{:?}",t.storageMode()),"usage":format!("{:?}",t.usage()),"format":format!("{:?}",t.pixelFormat()),"extent":[t.width(),t.height()],"samples":t.sampleCount()});
        let compute=f.resolve_prototype.as_ref().is_some_and(|r|r.is_compute());
        serde_json::json!({"command":format!("{:p}",&**self.command),"completed":self.command.status()==MTLCommandBufferStatus::Completed,"status":format!("{:?}",self.command.status()),"error":format!("{:?}",self.command.error()),
            "device":format!("{:p}",&*f.owner.device),"queue":format!("{:p}",&*f.owner.queue),
            "color":texture(&f.target.color.raw),"multisample":f.target.multisample.as_ref().map(|i|texture(&i.raw)),"depth":texture(&f.target.depth.raw),
            "camera_column_major_f32_bits":f.mvp.map(|v|v.map(f32::to_bits)),
            "clear_f64_bits":f.clear.map(f64::to_bits),"load":"Clear",
            "store":if compute{"Store"}else if f.target.multisample.is_some(){if f.diagnostic_samples{"StoreAndMultisampleResolve"}else{"MultisampleResolve"}}else{"Store"},
            "resolve_destination":if compute{None}else{f.target.multisample.as_ref().map(|_|format!("{:p}",&**f.target.color.raw))},
            "resolve_route":f.source_resolve_diagnostic(),"sample_positions":f.programmed_sample_positions(),
            "readback":{"identity":format!("{:p}",&**self.readback.raw),"shared":self.readback.raw.storageMode()==MTLStorageMode::Shared,"storage":format!("{:?}",self.readback.raw.storageMode()),"contents":format!("{:p}",self.readback.raw.contents().as_ptr()),"length":self.readback.raw.length(),"row_bytes":self.row,"copied_extent":f.extent(),"completion_required":"Completed"}})
    }

    /// Read only after the same completion gate as the resolved diagnostic.
    pub fn sample_values(&self)->Result<[[f32;4];16],String>{
        if self.command.status()!=MTLCommandBufferStatus::Completed{return Err("Sample completion not acknowledged".into());}
        let (buffer,_)=self.sample_owner.as_ref().ok_or("No sample observation submitted")?;
        Ok(unsafe {std::ptr::read(buffer.raw.contents().as_ptr().cast::<[[f32;4];16]>())})
    }
    pub fn readback(&self, timeout: std::time::Duration) -> Result<Vec<u8>, String> {
        if timeout > std::time::Duration::from_secs(5) {
            return Err("World diagnostic deadline exceeds5s".into());
        }
        let start = std::time::Instant::now();
        while self.command.status() != MTLCommandBufferStatus::Completed {
            if self.command.status() == MTLCommandBufferStatus::Error || start.elapsed() >= timeout
            {
                self.poison
                    .store(true, std::sync::atomic::Ordering::Release);
                return Err(format!("World diagnostic completion unknown; owners quarantined; status={:?}; native_error={:?}; elapsed_ms={}",self.command.status(),self.command.error(),start.elapsed().as_millis()));
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (w, h) = self.frame.extent();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                self.readback.raw.contents().as_ptr().cast::<u8>(),
                self.row * h as usize,
            )
        };
        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..h as usize {
            rgba.extend_from_slice(&bytes[y * self.row..y * self.row + w as usize * 4]);
        }
        Ok(rgba)
    }
}
impl Drop for Diagnostic {
    fn drop(&mut self) {
        if self.command.status() == MTLCommandBufferStatus::Completed {
            unsafe {
                ManuallyDrop::drop(&mut self.iso_owner);
                ManuallyDrop::drop(&mut self.sample_owner);
                ManuallyDrop::drop(&mut self.readback);
                ManuallyDrop::drop(&mut self.frame);
                ManuallyDrop::drop(&mut self.command)
            }
        } else {
            self.poison
                .store(true, std::sync::atomic::Ordering::Release);
            eprintln!("World diagnostic owner quarantined: completion unknown");
        }
    }
}

fn bind_paint(encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>, paint: &PaintUniform) {
    unsafe {
        encoder.setFragmentBytes_length_atIndex(
            std::ptr::NonNull::from(paint).cast(),
            std::mem::size_of::<PaintUniform>(),
            0,
        );
    }
}
fn draw(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    mesh: &Mesh,
    first: usize,
    count: usize,
) {
    unsafe {
        encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
            MTLPrimitiveType::Triangle,
            count,
            MTLIndexType::UInt32,
            &mesh.indices.as_ref().unwrap().raw,
            first * 4,
        );
    }
}

fn draw_instances(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    mesh: &Mesh,
    first: usize,
    count: usize,
    instances: usize,
) {
    unsafe {
        encoder
            .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                MTLPrimitiveType::Triangle,
                count,
                MTLIndexType::UInt32,
                &mesh.indices.as_ref().unwrap().raw,
                first * 4,
                instances,
            );
    }
}

const IDENTITY: [[f32; 4]; 4] = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

fn bind_section(encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,section:Section){
 unsafe {encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&section).cast(),std::mem::size_of::<Section>(),3);}
}

fn fill_pipeline(
    metal: &NativeMetal,
    lib: &ProtocolObject<dyn MTLLibrary>,
    samples: usize,
) -> Result<FillPipeline, String> {
    fn mask(
        metal: &NativeMetal,
        lib: &ProtocolObject<dyn MTLLibrary>,
        name: &str,
        samples: usize,
    ) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, String> {
        let d = MTLRenderPipelineDescriptor::new();
        let vertex=lib.newFunctionWithName(&NSString::from_str(name)).ok_or("Metal stencil vertex missing")?;
        let fragment=lib.newFunctionWithName(&NSString::from_str("world_mask_fragment")).ok_or("Metal stencil fragment missing")?;
        d.setVertexFunction(Some(&vertex));
        d.setFragmentFunction(Some(&fragment));
        d.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float_Stencil8);
        d.setStencilAttachmentPixelFormat(MTLPixelFormat::Depth32Float_Stencil8);
        d.setRasterSampleCount(samples);
        let color = unsafe { d.colorAttachments().objectAtIndexedSubscript(0) };
        color.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
        color.setWriteMask(MTLColorWriteMask::empty());
        metal
            .device
            .newRenderPipelineStateWithDescriptor_error(&d)
            .map_err(|e| format!("Metal stencil pipeline:{e:?}"))
    }
    fn state(
        metal: &NativeMetal,
        mark: bool,
    ) -> Result<Retained<ProtocolObject<dyn MTLDepthStencilState>>, String> {
        let d = MTLDepthStencilDescriptor::new();
        d.setDepthCompareFunction(MTLCompareFunction::LessEqual);
        d.setDepthWriteEnabled(!mark);
        let s = MTLStencilDescriptor::new();
        s.setStencilCompareFunction(if mark {
            MTLCompareFunction::Always
        } else {
            MTLCompareFunction::Equal
        });
        s.setStencilFailureOperation(MTLStencilOperation::Keep);
        s.setDepthFailureOperation(MTLStencilOperation::Keep);
        s.setDepthStencilPassOperation(if mark {
            MTLStencilOperation::Replace
        } else {
            MTLStencilOperation::Keep
        });
        s.setReadMask(255);
        s.setWriteMask(if mark { 255 } else { 0 });
        d.setFrontFaceStencil(Some(&s));
        d.setBackFaceStencil(Some(&s));
        metal
            .device
            .newDepthStencilStateWithDescriptor(&d)
            .ok_or("Metal stencil depth state unavailable".into())
    }
    Ok(FillPipeline {
        mask: mask(metal, lib, "world_vertex", samples)?,
        texture_mask: mask(metal, lib, "world_texture_mask_vertex", samples)?,
        mask_depth: state(metal, true)?,
        cap_depth: state(metal, false)?,
    })
}
impl Prepared {
    fn encode_fill(
        &self,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> Result<(), String> {
        self.encode_fill_observed(encoder,None).map(|_|())
    }
    fn encode_fill_observed(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,observation:Option<&super::world_iso_observation::Observation>)->Result<bool,String>{
        let Some(fill) = &self.fill else {return Ok(false);};
        let pipelines = self
            .pipeline
            .fill
            .as_ref()
            .ok_or("Metal section stencil pipeline absent")?;
        encoder.setDepthStencilState(Some(&pipelines.mask_depth));
        encoder.setStencilReferenceValue(1);
        encoder.setCullMode(MTLCullMode::Front);
        encoder.setDepthBias_slopeScale_clamp(0., 0., 0.);
        bind_section(encoder, self.section);
        if let Some(placements) = &self.placements {
            let matrices = self
                .target
                .instance_matrices
                .as_ref()
                .ok_or("Metal section placement matrices absent")?;
            for run in &self
                .target
                .order
                .as_ref()
                .ok_or("Metal section placement order absent")?
                .runs
            {
                if self.overlay == Some(placements.packet.prototypes[run.prototype as usize]) {
                    continue;
                }
                let Some(mesh) = &placements.meshes[run.prototype as usize] else {
                    continue;
                };
                encoder.setFrontFacingWinding(if run.reflected {
                    MTLWinding::Clockwise
                } else {
                    MTLWinding::CounterClockwise
                });
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw), run.first * 64, 1);
                }
                encoder.setRenderPipelineState(&pipelines.mask);
                if let (Some(vertices), Some(indices)) = (&mesh.vertices, &mesh.indices) {
                    unsafe {
                        encoder.setVertexBuffer_offset_atIndex(Some(&vertices.raw), 0, 0);
                    }
                    // Source _faces_count stops before translucent, back-paint,
                    // and front-cull tails. Eligible _draw_instanced_raw instead
                    // submits its complete prototype vcol stream.
                    if let Some(material) = mesh.materials.as_ref().filter(|m| !m.instanced) {
                        for &(first, count, front_only) in &material.opaque_runs {
                            if front_only { continue; }
                            unsafe {
                                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(MTLPrimitiveType::Triangle,count,MTLIndexType::UInt32,&indices.raw,first*4,run.count);
                            }
                        }
                    } else {
                        unsafe {
                            encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(MTLPrimitiveType::Triangle,mesh.count,MTLIndexType::UInt32,&indices.raw,0,run.count);
                        }
                    }
                }
                if let Some(material) = &mesh.materials {
                    // Expanded colored runs are source tails, not _faces_count.
                    // In contrast _tex_faces_count includes ALL appended texture
                    // tails (including translucent and independently painted backs).
                    encoder.setRenderPipelineState(&pipelines.texture_mask);
                    for texture in &material.textures {
                        unsafe {
                            encoder.setVertexBuffer_offset_atIndex(
                                Some(&texture.vertices.raw),
                                0,
                                0,
                            );
                        }
                        for draw in &texture.draws {
                            unsafe {
                                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                                    MTLPrimitiveType::Triangle,
                                    draw.first as usize,
                                    draw.count as usize,
                                    run.count,
                                );
                            }
                        }
                    }
                }
            }
        } else if let Some(mesh) = &self.mesh {
            encoder.setRenderPipelineState(&pipelines.mask);
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    std::ptr::NonNull::from(&self.mvp).cast(),
                    64,
                    1,
                );
            }
            if let (Some(vertices), Some(indices)) = (&mesh.vertices, &mesh.indices) {
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(Some(&vertices.raw), 0, 0);
                    encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(MTLPrimitiveType::Triangle,mesh.count,MTLIndexType::UInt32,&indices.raw,0,1);
                }
            }
        }
        if observation.is_some_and(|o|o.checkpoint(encoder,2)){return Ok(true);}
        encoder.setCullMode(MTLCullMode::None);
        encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
        encoder.setDepthStencilState(Some(&pipelines.cap_depth));
        encoder.setRenderPipelineState(&self.pipeline.color);
        encoder.setDepthBias_slopeScale_clamp(2., 2., 0.);
        bind_section(encoder, Section::OFF);
        let paint = PaintUniform {
            front: fill.color,
            back: fill.color,
            mode: 1,
            opacity: 1.,
            padding: [0; 2],
        };
        bind_paint(encoder, &paint);
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&fill.vertices.raw), 0, 0);
            encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&IDENTITY).cast(), 64, 1);
            encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&self.mvp).cast(), 64, 2);
            encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                MTLPrimitiveType::Triangle,
                0,
                6,
                1,
            );
        }
        if observation.is_some_and(|o|o.checkpoint(encoder,3)){return Ok(true);}
        encoder.setDepthBias_slopeScale_clamp(0., 0., 0.);
        encoder.setDepthStencilState(Some(&self.pipeline.depth));
        bind_section(encoder, self.section);
        Ok(false)
    }
}

fn mesh_run_transparent(placements: &PlacementMeshes, prototype: usize, index: usize) -> bool {
    matches!(
        placements.meshes[prototype]
            .as_ref()
            .unwrap()
            .materials
            .as_ref()
            .unwrap()
            .runs[index]
            .phase,
        super::world_surface::Phase::TransparentFront
            | super::world_surface::Phase::TransparentBack
    )
}
