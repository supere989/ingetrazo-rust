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
}
struct Mesh {
    vertices: Buffer,
    indices: Buffer,
    count: usize,
    plan: Plan,
}
struct PlacementMeshes {
    packet: Arc<PlacementPacket>,
    meshes: Vec<Option<Arc<Mesh>>>,
    key_index: Vec<(PrototypeKey, usize)>,
    retained_payload_bytes: usize,
}
struct Pipeline {
    color: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    depth: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_disabled: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
}
#[derive(Default, Debug, Clone, Copy)]
pub struct Stats {
    pub geometry_uploads: u64,
    pub target_allocations: u64,
    pub prepared: u64,
    pub warm_geometry_hits: u64,
    pub instance_buffer_allocations: u64,
    pub instance_matrix_upload_bytes: u64,
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
                    vertices: upload(metal, bytemuck::cast_slice(&data.vertices))?,
                    indices: upload(metal, bytemuck::cast_slice(&data.indices))?,
                    count: data.indices.len(),
                    plan,
                }))
            }
        };
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            Arc::new(pipeline(metal)?)
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
    pub fn prepare_placements_with_camera<F>(
        &mut self,
        metal: &NativeMetal,
        packet: Arc<PlacementPacket>,
        poses: &[PlacementPose],
        camera: [[f32; 4]; 4],
        extent: (u32, u32),
        clear: [f64; 4],
        policy: Policy,
        mut collect: F,
    ) -> Result<Option<Arc<Prepared>>, String>
    where
        F: FnMut(&PrototypeKey) -> Result<MeshRenderBuffer, String>,
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
                                - std::mem::size_of::<Plan>()
                        }))
                        .filter(|n| *n <= CACHE_BYTES)
                        .ok_or("Metal retained prototype plans exceed64MiB")?;
                    prior
                } else {
                    let data = collect(key)?;
                    let plan = super::world_admission::opaque(&data)?;
                    if !data.indices.is_empty() {
                        retained = retained
                            .checked_add(
                                std::mem::size_of::<Mesh>() + plan.retained_payload_bytes
                                    - std::mem::size_of::<Plan>(),
                            )
                            .filter(|n| *n <= CACHE_BYTES)
                            .ok_or("Metal retained prototype plans exceed64MiB")?;
                    }
                    if !plan.overrides.is_empty() {
                        return Err("Metal source placement with back overrides requires consolidated stage".into());
                    }
                    if data.indices.is_empty() {
                        None
                    } else {
                        uploaded += 1;
                        Some(Arc::new(Mesh {
                            vertices: upload(metal, bytemuck::cast_slice(&data.vertices))?,
                            indices: upload(metal, bytemuck::cast_slice(&data.indices))?,
                            count: data.indices.len(),
                            plan,
                        }))
                    }
                };
                meshes.push(mesh);
                key_index.push((*key, index));
            }
            key_index.sort_unstable_by_key(|p| p.0);
            Arc::new(PlacementMeshes {
                packet: packet.clone(),
                meshes,
                key_index,
                retained_payload_bytes: retained,
            })
        };
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            Arc::new(pipeline(metal)?)
        };
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
        let poses_changed = unique.poses.as_slice() != poses;
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
        if poses.len() > unique.poses.capacity() {
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
            unique.poses = candidate;
        } else {
            unique.poses.clear();
            unique.poses.extend_from_slice(poses);
        }
        let allocated_matrices = candidate_matrices.is_some();
        if let Some(buffer) = candidate_matrices {
            unique.instance_matrices = Some(buffer);
        }
        if poses_changed {
            if let Some(buffer) = &mut unique.instance_matrices {
                // Exclusive idle target ownership: no submitted sampler or encoder
                // can observe writes. No temporary packed matrix allocation.
                for (i, pose) in poses.iter().enumerate() {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            pose.mvp.as_ptr().cast::<u8>(),
                            buffer.raw.contents().as_ptr().cast::<u8>().add(i * 64),
                            64,
                        );
                    }
                }
            }
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
        if poses_changed {
            self.stats.instance_matrix_upload_bytes += matrix_bytes as u64;
        }
        self.stats.geometry_uploads += uploaded;
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
        let Some(p) = &self.placements else {
            return 0;
        };
        let mut first = 0;
        let mut count = 0;
        while first < self.target.poses.len() {
            first = super::world_placement::compatible_run_end(
                &p.packet.bindings,
                &self.target.poses,
                first,
            );
            count += 1;
        }
        count
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
            let mut first = 0;
            while first < self.target.poses.len() {
                let prototype = placements.packet.bindings[first];
                let reflected = self.target.poses[first].reflected;
                let end = super::world_placement::compatible_run_end(
                    &placements.packet.bindings,
                    &self.target.poses,
                    first,
                );
                encoder.setFrontFacingWinding(if reflected {
                    MTLWinding::Clockwise
                } else {
                    MTLWinding::CounterClockwise
                });
                if let Some(mesh) = &placements.meshes[prototype as usize] {
                    encode_placement_mesh(
                        &encoder,
                        mesh,
                        self.target.instance_matrices.as_ref().unwrap(),
                        first,
                        end - first,
                        self.policy,
                    );
                }
                first = end;
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
    encoder.setCullMode(MTLCullMode::None);

    unsafe {
        encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.raw), 0, 0);
        encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(mvp).cast(), 64, 1);
    }
    let mode = match policy.mode {
        Mode::Material => 0,
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
    if policy.mode == Mode::Material {
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
    encoder.setCullMode(MTLCullMode::None);
    unsafe {
        encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices.raw), 0, 0);
        encoder.setVertexBuffer_offset_atIndex(Some(&matrices.raw), first_instance * 64, 1);
    }
    let ordinary = PaintUniform {
        front: policy.front,
        back: policy.back,
        mode: match policy.mode {
            Mode::Material => 0,
            Mode::HiddenLine => 1,
            Mode::Monochrome => 2,
        },
        padding: [0; 3],
    };
    bind_paint(encoder, &ordinary);
    draw_instances(encoder, mesh, 0, mesh.count, instances);
    if policy.mode == Mode::Material {
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

fn pipeline(metal: &NativeMetal) -> Result<Pipeline, String> {
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
    Ok(Pipeline {
        color,
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
        // Observation only: never use a different descriptor's query to
        // admit this resource. The original tracked descriptor is restored.
        d.setHazardTrackingMode(MTLHazardTrackingMode::Default);
        let default_query=metal.device.heapTextureSizeAndAlignWithDescriptor(&d);
        d.setHazardTrackingMode(MTLHazardTrackingMode::Untracked);
        let untracked_query=metal.device.heapTextureSizeAndAlignWithDescriptor(&d);
        d.setHazardTrackingMode(MTLHazardTrackingMode::Tracked);
        return Err(format!("World native texture size query unavailable: requested_bytes={requested}, queried_size={}, queried_align={}, extent={}x{}, format={format:?}, storage={:?}, hazard={:?}, usage={usage:?}, device={}, default_query={}/{}, untracked_query={}/{}, family_apple2={}, family_mac2={}, family_common3={}, low_power={}, headless={}",native.size,native.align,extent.0,extent.1,d.storageMode(),d.hazardTrackingMode(),metal.device.name(),default_query.size,default_query.align,untracked_query.size,untracked_query.align,metal.device.supportsFamily(MTLGPUFamily::Apple2),metal.device.supportsFamily(MTLGPUFamily::Mac2),metal.device.supportsFamily(MTLGPUFamily::Common3),metal.device.isLowPower(),metal.device.isHeadless()));
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
            &mesh.indices.raw,
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
                &mesh.indices.raw,
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
