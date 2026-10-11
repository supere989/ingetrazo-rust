//! Direct native world-space surface rendering. No CPU projection or submission.
//! Prepared frames retain buffers and color/depth targets through GPU completion.
//! This first opaque stage deliberately rejects paint policies not implemented here.
use crate::world_placement::{Packet as PlacementPacket, Pose as PlacementPose, PrototypeKey};
use crate::world_policy::{Mode, Plan, Policy};
use crate::{Buffer, NativeMetal};
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
struct Owner {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    budget: Arc<Mutex<MemoryBudget>>,
    poison: Arc<std::sync::atomic::AtomicBool>,
}
impl Owner {
    fn new(metal: &NativeMetal) -> Self {
        Self {
            device: metal.device.clone(),
            queue: metal.queue.clone(),
            budget: metal.budget.clone(),
            poison: metal.poisoned.clone(),
        }
    }
    fn validate(&self, metal: &NativeMetal) -> Result<(), String> {
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
struct Materials {instanced:bool,colored_vertices:Option<Buffer>,colored_indices:Option<Buffer>,runs:Vec<super::world_surface::Run>,textures:Vec<SampledMesh>,opaque_runs:Vec<(usize,usize,bool)>,retained_payload_bytes:usize}
struct Mesh {
    vertices: Option<Buffer>,
    indices: Option<Buffer>,
    count: usize,
    plan: Plan,
    materials:Option<Materials>,
}
struct PlacementMeshes {
    has_materials:bool,
    packet: Arc<PlacementPacket>,
    meshes: Vec<Option<Arc<Mesh>>>,
    key_index: Vec<(PrototypeKey, usize)>,
    retained_payload_bytes: usize,
}
#[derive(Clone,Copy)]enum MaterialKind {Legacy,Opaque{first:usize,count:usize,cull:bool},DefaultBack,Color(usize),Texture{bucket:usize,draw:usize}}
#[derive(Clone,Copy)]struct MaterialDraw{opacity:f32,prototype:usize,packed:usize,instances:usize,reflected:bool,kind:MaterialKind}
struct MaterialDraws{operations:Vec<MaterialDraw>,retained_payload_bytes:usize}
struct MaterialPipeline {colored_alpha:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,textured:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,textured_alpha:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,depth:Retained<ProtocolObject<dyn MTLDepthStencilState>>}
struct Pipeline {
    color: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    depth: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_disabled: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    material:Option<MaterialPipeline>,
}
#[derive(Default, Debug, Clone, Copy)]
pub struct Stats {
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
    owner: Option<Arc<Owner>>,
    mesh: Option<Arc<Mesh>>,
    key: Option<u64>,
    placements: Option<Arc<PlacementMeshes>>,
    pipeline: Option<Arc<Pipeline>>,
    targets: Vec<Arc<Target>>,
    texture_assets:Vec<std::sync::Weak<super::world_texture::Asset>>,
    pub stats: Stats,
}
pub struct Prepared {
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
    padding: [u32; 3],
}
#[repr(C)]struct TexturePaint{opacity:f32,shade:f32,hard_cutout:u32,height:u32,mean:[f32;4]}
impl World {
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
            .retain(|t| Arc::strong_count(t) > 1 || t.extent == extent);
        let cached = self
            .targets
            .iter()
            .find(|t| t.extent == extent && Arc::strong_count(t) == 1)
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
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            Arc::new(pipeline(metal,false)?)
        };
        let allocated = cached.is_none();
        let target = if let Some(t) = cached {
            t
        } else {
            Arc::new(Target {
                color: image(
                    metal,
                    extent,
                    MTLPixelFormat::RGBA8Unorm,
                    MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead,
                    4,
                )?,
                depth: image(
                    metal,
                    extent,
                    MTLPixelFormat::Depth32Float,
                    MTLTextureUsage::RenderTarget,
                    4,
                )?,
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
            placements: None,
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
        self.prepare_collections_with_camera(metal,packet,poses,camera,extent,clear,policy,|key|Ok(Collection::Opaque(collect(key)?)),|_|Err("Unexpected sampled material in opaque consumer".into()))
    }
    pub fn prepare_material_placements_with_camera<'a,F,G>(&mut self,metal:&NativeMetal,packet:Arc<PlacementPacket>,poses:&[PlacementPose],camera:[[f32;4];4],extent:(u32,u32),clear:[f64;4],policy:Policy,mut collect:F,resolve:G)->Result<Option<Arc<Prepared>>,String>
    where F:FnMut(&PrototypeKey)->Result<ingetrazo_render::pipeline::SceneRenderData,String>,G:FnMut(&str)->Result<std::borrow::Cow<'a,[u8]>,String>{
        self.prepare_collections_with_camera(metal,packet,poses,camera,extent,clear,policy,|key|{let data=collect(key)?;if matches!(policy.mode,Mode::HiddenLine|Mode::Monochrome){
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
            .retain(|t| Arc::strong_count(t) > 1 || t.extent == extent);
        let idle = self
            .targets
            .iter()
            .position(|t| t.extent == extent && Arc::strong_count(t) == 1);
        if idle.is_none() && self.targets.len() >= 3 {
            return Ok(None);
        }
        let mut candidate_assets=Vec::<Arc<super::world_texture::Asset>>::new();
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
                    let mesh=build_material_mesh(metal,collection,&mut resolve,&self.texture_assets,&mut candidate_assets)?;
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
        let pipeline=if let Some(p)=&self.pipeline{if !needs_materials||p.material.is_some(){p.clone()}else{Arc::new(pipeline(metal,true)? )}}else{Arc::new(pipeline(metal,needs_materials)?)};
        let mut new_target = if idle.is_none() {
            Some(Arc::new(Target {
                color: image(
                    metal,
                    extent,
                    MTLPixelFormat::RGBA8Unorm,
                    MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead,
                    4,
                )?,
                depth: image(
                    metal,
                    extent,
                    MTLPixelFormat::Depth32Float,
                    MTLTextureUsage::RenderTarget,
                    4,
                )?,
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
            if candidate_assets.len()+self.texture_assets.iter().filter(|a|a.strong_count()>0).count()>4096{return Err("Metal sampled identity publication admission exceeded".into());}
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
        self.stats.texture_uploads+=candidate_assets.len()as u64;
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
        if uploaded>0{self.texture_assets.retain(|a|a.strong_count()>0);self.texture_assets.extend(candidate_assets.iter().map(Arc::downgrade));}
        self.stats.texture_identity_payload_bytes=self.texture_assets.capacity()*std::mem::size_of::<std::sync::Weak<super::world_texture::Asset>>();
        self.owner = Some(owner.clone());
        self.mesh = None;
        self.key = None;
        self.placements = Some(placements.clone());
        self.pipeline = Some(pipeline.clone());
        Ok(Some(Arc::new(Prepared {
            placements: Some(placements),
            owner,
            mesh: None,
            pipeline,
            target,
            mvp: camera,
            clear,
            policy,
        })))
    }
    pub fn clear_idle(&mut self) {
        self.targets.retain(|t| Arc::strong_count(t) > 1);
        self.mesh = None;
        self.placements = None;
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
    /// Admitted contiguous instanced draw groups, observed by diagnostics only.
    pub fn instance_run_count(&self) -> usize {
        if self.placements.is_none() {
            return 0;
        }
        self.target.order.as_ref().map_or(0, |o| o.runs.len())
    }
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
        self.encode_inner(command, true)
    }
    fn encode_material_draws(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,placements:&PlacementMeshes,draws:&MaterialDraws,depth_enabled:bool)->Result<(),String>{
        let pipelines=self.pipeline.material.as_ref().ok_or("Metal material pipelines unavailable")?;let matrices=self.target.instance_matrices.as_ref().ok_or("Metal material matrices unavailable")?;
        for op in &draws.operations {
            let mesh=placements.meshes[op.prototype].as_ref().ok_or("Metal admitted material prototype unavailable")?;
            encoder.setFrontFacingWinding(if op.reflected{MTLWinding::Clockwise}else{MTLWinding::CounterClockwise});
            let depth=if depth_enabled{&self.pipeline.depth}else{&self.pipeline.depth_disabled};
            encoder.setDepthStencilState(Some(depth));encoder.setRenderPipelineState(&self.pipeline.color);
            let paint=PaintUniform{front:self.policy.front,back:self.policy.back,mode:match self.policy.mode{Mode::Material|Mode::MaterialAverage=>0,Mode::HiddenLine=>1,Mode::Monochrome=>2},padding:[0;3]};
            match op.kind {
                MaterialKind::Legacy=>{encoder.setDepthBias_slopeScale_clamp(1.,1.,0.);encode_placement_mesh(encoder,mesh,matrices,op.packed,op.instances,self.policy);},
                MaterialKind::Opaque{first,count,cull}=>{encoder.setDepthBias_slopeScale_clamp(1.,1.,0.);encoder.setCullMode(if cull{MTLCullMode::Back}else{MTLCullMode::None});unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.as_ref().ok_or("Metal admitted opaque draw buffer missing")?.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);}bind_paint(encoder,&paint);draw_instances(encoder,mesh,first,count,1);},
                MaterialKind::DefaultBack=>{if !matches!(self.policy.mode,Mode::Material|Mode::MaterialAverage){continue;}encoder.setDepthBias_slopeScale_clamp(1.,1.,0.);encoder.setCullMode(MTLCullMode::Front);unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.as_ref().ok_or("Metal admitted opaque draw buffer missing")?.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);}bind_paint(encoder,&PaintUniform{front:self.policy.back,mode:3,..paint});for &(first,count)in &mesh.plan.default_back{draw_instances(encoder,mesh,first,count,1);}},
                MaterialKind::Color(index)=>{let material=mesh.materials.as_ref().ok_or("Metal admitted material payload missing")?;let run=*material.runs.get(index).ok_or("Metal admitted material run stale")?;let default=run.phase==super::world_surface::Phase::DefaultBack;let alpha=matches!(run.phase,super::world_surface::Phase::TransparentFront|super::world_surface::Phase::TransparentBack);encoder.setDepthBias_slopeScale_clamp(if alpha||default{1.}else{0.},if alpha||default{1.}else{0.},0.);encoder.setRenderPipelineState(if alpha{&pipelines.colored_alpha}else{&self.pipeline.color});encoder.setDepthStencilState(Some(if !depth_enabled{&self.pipeline.depth_disabled}else if alpha{&pipelines.depth}else{&self.pipeline.depth}));encoder.setCullMode(if run.cull_front{MTLCullMode::Front}else if run.front_only{MTLCullMode::Back}else{MTLCullMode::None});unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&material.colored_vertices.as_ref().ok_or("Metal admitted color vertex buffer missing")?.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);}bind_paint(encoder,&PaintUniform{front:if default{self.policy.back}else{paint.front},mode:if default{3}else{0},..paint});unsafe{encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(MTLPrimitiveType::Triangle,run.count,MTLIndexType::UInt32,&material.colored_indices.as_ref().ok_or("Metal admitted color index buffer missing")?.raw,run.first*4,1);}},
                MaterialKind::Texture{bucket,draw}=>{let texture=&mesh.materials.as_ref().ok_or("Metal admitted material payload missing")?.textures[bucket];let draw=&texture.draws[draw];let source=&draw.source;let alpha=source.opacity<0.999;encoder.setDepthBias_slopeScale_clamp(if !source.back_side||alpha{1.}else{0.},if !source.back_side||alpha{1.}else{0.},0.);encoder.setRenderPipelineState(if alpha{&pipelines.textured_alpha}else{&pipelines.textured});encoder.setDepthStencilState(Some(if !depth_enabled{&self.pipeline.depth_disabled}else if alpha{&pipelines.depth}else{&self.pipeline.depth}));encoder.setCullMode(if source.back_side{MTLCullMode::Front}else if source.front_cull{MTLCullMode::Back}else{MTLCullMode::None});let opacity=op.opacity;let uniform=TexturePaint{opacity,shade:source.shade,hard_cutout:u32::from(texture.asset.cutout),height:self.extent().1,mean:[texture.asset.mean[0],texture.asset.mean[1],texture.asset.mean[2],if self.policy.mode.texture_mean(source.opacity,source.back_side){1.}else{0.}]};unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&texture.vertices.raw),0,0);encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw),op.packed*64,1);encoder.setFragmentTexture_atIndex(Some(&texture.asset.image),0);encoder.setFragmentSamplerState_atIndex(Some(&texture.asset.sampler),0);encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&uniform).cast(),std::mem::size_of::<TexturePaint>(),0);encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(MTLPrimitiveType::Triangle,draw.first as usize,draw.count as usize,1);}},
            }
        }
        encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);Ok(())
    }
    fn encode_inner(
        &self,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
        depth_enabled: bool,
    ) -> Result<(), String> {
        if !std::ptr::eq(&*command.commandQueue(), &*self.owner.queue) {
            return Err("Metal world command queue mismatch".into());
        }
        let pass = MTLRenderPassDescriptor::new();
        let color = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        color.setTexture(Some(&self.target.color.raw));
        color.setLoadAction(MTLLoadAction::Clear);
        color.setStoreAction(MTLStoreAction::Store);
        color.setClearColor(MTLClearColor {
            red: self.clear[0],
            green: self.clear[1],
            blue: self.clear[2],
            alpha: self.clear[3],
        });
        let depth = pass.depthAttachment();
        depth.setTexture(Some(&self.target.depth.raw));
        depth.setLoadAction(MTLLoadAction::Clear);
        depth.setStoreAction(MTLStoreAction::DontCare);
        depth.setClearDepth(1.);
        let encoder = command
            .renderCommandEncoderWithDescriptor(&pass)
            .ok_or("Metal world encoder unavailable")?;
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
        if let Some(placements) = &self.placements {
            if let Some(draws)=&self.target.material_draws{
                if let Err(error)=self.encode_material_draws(&encoder,placements,draws,depth_enabled){encoder.endEncoding();return Err(error);}
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
        encoder.endEncoding();
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
        padding: [0; 3],
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
        padding: [0; 3],
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
    let mut records=Vec::new();records.try_reserve_exact(count).map_err(|_|"Metal material order allocation refused")?;
    let mut buckets=std::collections::HashMap::<(super::world_texture::Identity,u32,i32,bool,bool,u8),usize>::new();buckets.try_reserve(count).map_err(|_|"Metal sampled order bucket allocation refused")?;
    for(original,&prototype)in placements.packet.bindings.iter().enumerate(){let Some(mesh)=&placements.meshes[prototype as usize]else{continue};let Some(material)=&mesh.materials else{continue};if material.instanced{continue;}let base=MaterialDraw{opacity:1.,prototype:prototype as usize,packed:reverse[original],instances:1,reflected:poses[original].reflected,kind:MaterialKind::DefaultBack};
        for &(first,count,cull)in &material.opaque_runs{records.push(((0,0,false,original,first),MaterialDraw{kind:MaterialKind::Opaque{first,count,cull},..base}));}
        if !mesh.plan.default_back.is_empty(){records.push(((2,0,false,original,0),base));}
        for(index,run)in material.runs.iter().enumerate(){let phase=match run.phase{super::world_surface::Phase::OpaqueBack=>4,super::world_surface::Phase::DefaultBack=>2,super::world_surface::Phase::TransparentFront=>6,super::world_surface::Phase::TransparentBack=>7};records.push(((phase,(run.opacity*1000.).round()as i32,run.front_only,original,index),MaterialDraw{kind:MaterialKind::Color(index),..base}));}
        for(bucket,texture)in material.textures.iter().enumerate(){for(draw,d)in texture.draws.iter().enumerate(){let source=&d.source;let opaque=source.opacity>=0.999;let phase=match(opaque,source.back_side){(true,false)=>1,(true,true)=>5,(false,false)=>8,(false,true)=>9};let alpha=if opaque{1000}else{(format!("{:.3}",source.opacity).parse::<f64>().map_err(|_|"Metal texture opacity rounding refused")?*1000.).round()as i32};let key=(texture.asset.id,source.shade.to_bits(),alpha,source.front_cull,source.back_side,phase);let next=buckets.len();let first=*buckets.entry(key).or_insert(next);records.push(((phase,alpha,source.front_cull,first,original),MaterialDraw{opacity:alpha as f32/1000.,kind:MaterialKind::Texture{bucket,draw},..base}));}}
    }
    for (index,run)in order.runs.iter().enumerate(){let Some(mesh)=&placements.meshes[run.prototype as usize]else{continue};if mesh.materials.as_ref().is_none_or(|m|m.instanced){records.push(((3,0,false,index,0),MaterialDraw{opacity:1.,prototype:run.prototype as usize,packed:run.first,instances:run.count,reflected:run.reflected,kind:MaterialKind::Legacy}));}}
    records.sort_unstable_by_key(|r|r.0);let mut operations=Vec::new();operations.try_reserve_exact(records.len()).map_err(|_|"Metal retained material operations allocation refused")?;operations.extend(records.into_iter().map(|r|r.1));let retained_payload_bytes=operations.capacity().checked_mul(std::mem::size_of::<MaterialDraw>()).and_then(|n|n.checked_add(std::mem::size_of::<MaterialDraws>())).ok_or("Metal material operation capacity overflow")?;if retained_payload_bytes>CAP{return Err("Metal retained material operations exceeds8MiB".into());}Ok(MaterialDraws{operations,retained_payload_bytes})
}

fn build_material_mesh<'a,G>(metal:&NativeMetal,collection:Collection,resolve:&mut G,published:&[std::sync::Weak<super::world_texture::Asset>],candidates:&mut Vec<Arc<super::world_texture::Asset>>)->Result<Option<Arc<Mesh>>,String>
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
                let asset=candidates.iter().find(|a|a.id==id).cloned().or_else(||published.iter().filter_map(std::sync::Weak::upgrade).find(|a|a.id==id));
                let asset=if let Some(asset)=asset{asset}else{if candidates.len()+published.iter().filter(|a|a.strong_count()>0).count()>=4096{return Err("Metal retained sampled asset identities exceed4096".into());}candidates.try_reserve(1).map_err(|_|"Metal sampled asset publication allocation refused")?;let asset=super::world_texture::upload(metal,id,&encoded)?;candidates.push(asset.clone());asset};
                textures.push(SampledMesh{vertices:upload(metal,texture.geometry.upload_bytes())?,asset,draws:texture.geometry.draws});
            }
            let retained_payload_bytes=std::mem::size_of::<Materials>()+packet.runs.capacity()*std::mem::size_of::<super::world_surface::Run>()+opaque_runs.capacity()*std::mem::size_of::<(usize,usize,bool)>()+textures.capacity()*std::mem::size_of::<SampledMesh>()+textures.iter().map(|t|t.draws.capacity()*std::mem::size_of::<ingetrazo_render::native_textures::Draw>()).sum::<usize>();
            if retained_payload_bytes>8*1024*1024{return Err("Metal retained material runs exceed8MiB".into());}
            let instanced=opaque.source_consolidated_opaque_count==0&&packet.runs.is_empty()&&textures.is_empty();
            (opaque,Some(Materials{instanced,colored_vertices,colored_indices,runs:packet.runs,textures,opaque_runs,retained_payload_bytes}))
        }
    };
    let plan=super::world_admission::opaque(&data)?;
    let has_materials=materials.as_ref().is_some_and(|m|m.colored_indices.is_some()||!m.textures.is_empty());
    if data.indices.is_empty()&&!has_materials{return Ok(None);}
    let vertices=if data.indices.is_empty(){None}else{Some(upload(metal,bytemuck::cast_slice(&data.vertices))?)};
    let indices=if data.indices.is_empty(){None}else{Some(upload(metal,bytemuck::cast_slice(&data.indices))?)};
    Ok(Some(Arc::new(Mesh{vertices,indices,count:data.indices.len(),plan,materials})))
}

fn pipeline(metal: &NativeMetal, rich:bool) -> Result<Pipeline, String> {
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
    d.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
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
        Some(MaterialPipeline{colored_alpha:material_pipeline(metal,&lib,"world_vertex","world_fragment",true)?,textured:material_pipeline(metal,&lib,"world_texture_vertex","world_texture_fragment",true)?,textured_alpha:material_pipeline(metal,&lib,"world_texture_vertex","world_texture_fragment",true)?,depth:metal.device.newDepthStencilStateWithDescriptor(&alpha).ok_or("Metal material depth state unavailable")?})
    }else{None};
    Ok(Pipeline {
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
fn material_pipeline(metal:&NativeMetal,lib:&ProtocolObject<dyn MTLLibrary>,vertex:&str,fragment:&str,alpha:bool)->Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>,String>{
 let vertex=lib.newFunctionWithName(&NSString::from_str(vertex)).ok_or("Metal material vertex function missing")?;let fragment=lib.newFunctionWithName(&NSString::from_str(fragment)).ok_or("Metal material fragment function missing")?;let d=MTLRenderPipelineDescriptor::new();d.setVertexFunction(Some(&vertex));d.setFragmentFunction(Some(&fragment));d.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);let color=unsafe{d.colorAttachments().objectAtIndexedSubscript(0)};color.setPixelFormat(MTLPixelFormat::RGBA8Unorm);color.setBlendingEnabled(alpha);
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
    let requested = u64::from(extent.0) * u64::from(extent.1) * bpp;
    let d = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            format,
            extent.0 as usize,
            extent.1 as usize,
            false,
        )
    };
    d.setStorageMode(MTLStorageMode::Private);
    d.setHazardTrackingMode(MTLHazardTrackingMode::Tracked);
    d.setUsage(usage);
    // Query native layout before allocation. This is a heap-size estimate for a
    // standalone texture: verify allocatedSize before publication, never admit a
    // larger allocation by silently increasing the ticket after allocation.
    let native = metal.device.heapTextureSizeAndAlignWithDescriptor(&d);
    let admitted = u64::try_from(native.size).map_err(|_| "World native texture size overflow")?;
    if admitted < requested || native.align == 0 {
        return Err(format!("World native texture size query unavailable: requested_bytes={requested}, queried_size={}, queried_align={}, extent={}x{}, format={format:?}, storage={:?}, hazard={:?}, usage={usage:?}, device={}", native.size, native.align, extent.0, extent.1, d.storageMode(), d.hazardTrackingMode(), metal.device.name()));
    }
    let ticket = metal
        .budget
        .lock()
        .map_err(|_| "Metal budget poisoned")?
        .reserve(0, admitted)?;
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
    if actual > admitted {
        budget.cancel(ticket)?;
        return Err("World standalone texture exceeds preallocation native estimate".into());
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

fn upload(metal: &NativeMetal, bytes: &[u8]) -> Result<Buffer, String> {
    let mut buffer = metal.buffer(bytes.len(), true)?;
    buffer.write(bytes)?;
    Ok(buffer)
}

/// Offscreen acceptance only. Production hosts encode in their existing command.
/// Completion-unknown drops quarantine all owners; no unbounded GPU wait.
pub struct Diagnostic {
    command: ManuallyDrop<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    frame: ManuallyDrop<Arc<Prepared>>,
    readback: ManuallyDrop<Buffer>,
    row: usize,
    poison: Arc<std::sync::atomic::AtomicBool>,
}
impl Prepared {
    pub fn submit_diagnostic(self: &Arc<Self>, metal: &NativeMetal) -> Result<Diagnostic, String> {
        self.submit_diagnostic_inner(metal, true)
    }
    /// Negative control only: literal disabled depth state must fail the near-first oracle.
    pub fn submit_diagnostic_without_depth(
        self: &Arc<Self>,
        metal: &NativeMetal,
    ) -> Result<Diagnostic, String> {
        self.submit_diagnostic_inner(metal, false)
    }
    fn submit_diagnostic_inner(
        self: &Arc<Self>,
        metal: &NativeMetal,
        depth_enabled: bool,
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
        self.encode_inner(&command, depth_enabled)?;
        let blit = command
            .blitCommandEncoder()
            .ok_or("World diagnostic blit unavailable")?;
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(self.texture(),0,0,MTLOrigin{x:0,y:0,z:0},MTLSize{width:w as usize,height:h as usize,depth:1},&readback.raw,0,row,row*h as usize);
        }
        blit.endEncoding();
        command.commit();
        Ok(Diagnostic {
            command: ManuallyDrop::new(command),
            frame: ManuallyDrop::new(self.clone()),
            readback: ManuallyDrop::new(readback),
            row,
            poison: metal.poisoned.clone(),
        })
    }
}
impl Diagnostic {
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
