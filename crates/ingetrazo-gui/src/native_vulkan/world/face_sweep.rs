//! Cached native GPU owner for source-compatible bounded face sweeps.
use super::*;
use ingetrazo_render::face_sweep::{Input, Kind, Packet};
fn input(s: ingetrazo_tools::base::FaceSweepPreview) -> Input {
    Input {
        face: s.face,
        face_identity: s.face_identity,
        mesh_revision: s.mesh_revision,
        anchor: s.anchor,
        normal: s.normal,
        offset: s.offset,
        attached: s.attached,
        prism_cap: s.prism_cap,
        keep_base: s.keep_base,
    }
}

pub(super) struct Owner {
    device: Device,
    resources: Resources,
    textures: Option<super::super::textures::Textures>,
    solid: Option<BufferId>,
    lines: Option<BufferId>,
    painted: Option<BufferId>,
    ordinary_runs: Vec<(u32, u32, bool)>,
    default_back: Vec<(u32, u32)>,
    paint_runs: Vec<OpacityRun>,
    consolidated_opaque_count:usize,
    solid_count: u32,
    line_count: u32,
    key: Option<(Input, u64, bool)>,
    style: Option<ingetrazo_render::style::StyleConfig>,
    packet: Option<Packet>,
    pub uploads: u64,
    pub bytes: u64,
}
enum TextureStage {
    Full(
        super::super::textures::Candidate,
        super::super::textures::Publication,
    ),
    Refresh(
        super::super::textures::VertexRefresh,
        super::super::resources::CompletedRetirement,
    ),
}
pub(crate) struct Prepared {
    key: Option<(Input, u64, bool)>,
    style: Option<ingetrazo_render::style::StyleConfig>,
    pub(super) packet: Option<Packet>,
    textures: TextureStage,
    solid: Option<BufferId>,
    lines: Option<BufferId>,
    painted: Option<BufferId>,
    ordinary_runs: Vec<(u32, u32, bool)>,
    default_back: Vec<(u32, u32)>,
    paint_runs: Vec<OpacityRun>,
    consolidated_opaque_count:usize,
    solid_count: u32,
    line_count: u32,
    bytes: u64,
}
impl Owner {
    pub fn new(
        context: &VulkanContext,
        budget: Arc<VulkanBudget>,
        pass: vk::RenderPass,
        samples: vk::SampleCountFlags,
    ) -> Result<Self, String> {
        Ok(Self {
            device: context.device.clone(),
            resources: Resources::new(context, budget.clone()),
            textures: Some(super::super::textures::Textures::new(
                context, pass, samples, budget,
            )?),
            solid: None,
            lines: None,
            painted: None,
            ordinary_runs: Vec::new(),
            default_back: Vec::new(),
            paint_runs: Vec::new(),
            consolidated_opaque_count:0,
            solid_count: 0,
            line_count: 0,
            key: None,
            style: None,
            packet: None,
            uploads: 0,
            bytes: 0,
        })
    }
    pub fn packet(&self) -> Option<&Packet> {
        self.packet.as_ref()
    }
    pub fn image_stats(&self)->serde_json::Value {let s=self.textures.as_ref().unwrap().cache_stats();serde_json::json!({"hashes":s.hashes,"decodes":s.decodes,"uploads":s.uploads,"shared_images":s.shared_images})}
    pub fn stats(&self) -> (u64, u64, super::super::textures::Stats) {
        (
            self.uploads,
            self.bytes,
            self.textures.as_ref().unwrap().stats,
        )
    }
    pub fn prepare(
        &mut self,
        context: &VulkanContext,
        scene: &ingetrazo_core::scene::Scene,
        style: &ingetrazo_render::style::StyleConfig,
        sweep: Option<ingetrazo_tools::base::FaceSweepPreview>,
        eye: glam::DVec3,
        epoch: u64,
        fence: vk::Fence,
        completed: u64,
        source_textures:&super::super::textures::Textures,
    ) -> Result<Option<Prepared>, String> {
        let key = sweep.map(|s| {
            (
                input(s),
                epoch,
                s.offset.as_vec3().dot(s.anchor.as_vec3() - eye.as_vec3()) > 0.,
            )
        });
        if let Some((s, _, _)) = key {
            if scene.version != s.mesh_revision
                || scene
                    .mesh
                    .faces
                    .get(s.face)
                    .is_none_or(|f| f.history_id != s.face_identity)
            {
                return Err("Face sweep acquired identity/revision changed".into());
            }
        }
        let selected_style = key.filter(|(s, _, _)| s.prism_cap).map(|_| style);
        if self.key == key && self.style.as_ref() == selected_style {
            return Ok(None);
        }
        let packet = key
            .map(|(s, _, _)| Packet::build(scene, s, eye))
            .transpose()?;
        let data = packet
            .as_ref()
            .map(|p| match p.kind {
                Kind::Overlay => {
                    ingetrazo_render::pipeline::build_preview_mesh_surfaces(&p.mesh, scene)
                }
                Kind::Prism => ingetrazo_render::pipeline::build_borrowed_mesh_world_surfaces(
                    &p.mesh, scene, style,
                ),
            })
            .unwrap_or_default();
        let ordinary_runs = indexed_paint_runs(&data.opaque_mesh)?;
        let default_back = data
            .opaque_mesh
            .source_default_back
            .iter()
            .map(|&(a, b)| {
                Ok((
                    u32::try_from(a).map_err(|_| "Sweep back first")?,
                    u32::try_from(b).map_err(|_| "Sweep back count")?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (painted, paint_runs, _) = pack_transparent_with_textures(
            &data.transparent_mesh,
            &data.opaque_mesh,
            &data.textured_meshes,
        )?;
        let consolidated_opaque_count=data.opaque_mesh.source_consolidated_opaque_count;
        let solid_count = u32::try_from(data.opaque_mesh.vertices.len())
            .map_err(|_| "Sweep solid draw overflow")?;
        let line_count = u32::try_from(packet.as_ref().map_or(0, |p| p.lines.len()))
            .map_err(|_| "Sweep line draw overflow")?;
        let mut ids = [None; 3];
        let mut candidate = None;
        let mut refresh_candidate = None;
        let refresh = key
            .zip(self.key)
            .is_some_and(|((new, epoch, _), (old, previous, _))| {
                epoch == previous
                    && !new.prism_cap
                    && !old.prism_cap
                    && new.face_identity == old.face_identity
                    && new.mesh_revision == old.mesh_revision
                    && new.keep_base == old.keep_base
                    && self
                        .packet
                        .as_ref()
                        .is_some_and(|p| !p.mesh.faces.is_empty())
                    && packet.as_ref().is_some_and(|p| !p.mesh.faces.is_empty())
            });
        let result = (|| {
            ids[0] = if solid_count != 0 {
                Some(self.resources.upload(
                    context,
                    bytemuck::cast_slice(&data.opaque_mesh.vertices),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    false,
                )?)
            } else {
                None
            };
            ids[1] = if line_count != 0 {
                Some(self.resources.upload(
                    context,
                    bytemuck::cast_slice(&packet.as_ref().unwrap().lines),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    false,
                )?)
            } else {
                None
            };
            ids[2] = if painted.is_empty() {
                None
            } else {
                Some(self.resources.upload(
                    context,
                    bytemuck::cast_slice(&painted),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    false,
                )?)
            };
            if refresh {
                refresh_candidate = Some(
                    self.textures
                        .as_mut()
                        .unwrap()
                        .prepare_preview_vertex_refresh(
                            context,
                            &mut self.resources,
                            &data.textured_meshes,
                            fence,
                            completed,
                        )?,
                );
            } else {
                // Warm and offset-only updates do not scan sampled owners.
                if key.is_some(){self.textures.as_mut().unwrap().share_completed_images(context,source_textures)?;}
                candidate = Some(self.textures.as_mut().unwrap().prepare_preview(
                    context,
                    &mut self.resources,
                    &data.textured_meshes,
                    scene,
                    fence,
                    completed,
                    packet.as_ref().is_some_and(|p| p.kind == Kind::Prism)
                        && !style.show_textures
                        && style.visual_style
                            == ingetrazo_render::style::VisualStyle::ShadedWithTextures,
                )?);
            }
            let mut old = Vec::new();
            old.try_reserve_exact(3)
                .map_err(|_| "Sweep retirement admission")?;
            old.extend([self.solid, self.lines, self.painted].into_iter().flatten());
            let mut next = Vec::new();
            next.try_reserve_exact(3)
                .map_err(|_| "Sweep candidate identity admission")?;
            next.extend(ids.into_iter().flatten());
            let textures = if let Some(staged) = candidate.as_ref() {
                let publication = self.textures.as_mut().unwrap().preflight_publication(
                    context,
                    &self.resources,
                    staged,
                    fence,
                    completed,
                    &old,
                    &next,
                )?;
                TextureStage::Full(candidate.take().unwrap(), publication)
            } else {
                let retirement = self
                    .resources
                    .publication_preflight(context, fence, completed, &old, &next)?;
                TextureStage::Refresh(refresh_candidate.take().unwrap(), retirement)
            };
            let bytes = next
                .iter()
                .map(|&id| self.resources.logical_bytes(id).unwrap_or(0))
                .sum();
            Ok(Prepared {
                key,
                style: selected_style.cloned(),
                packet,
                textures,
                solid: ids[0],
                lines: ids[1],
                painted: ids[2],
                ordinary_runs,
                default_back,
                paint_runs,
                consolidated_opaque_count,
                solid_count,
                line_count,
                bytes,
            })
        })();
        if result.is_err() {
            if let Some(c) = refresh_candidate {
                self.textures.as_mut().unwrap().abort_vertex_refresh(
                    &mut self.resources,
                    c,
                    fence,
                    completed,
                )?;
            }
            if let Some(c) = candidate {
                self.textures
                    .as_mut()
                    .unwrap()
                    .abort(&mut self.resources, c, fence, completed)?;
            }
            for id in ids.into_iter().flatten() {
                self.resources.retire(id)?;
            }
            self.resources.collect(fence, completed)?;
        }
        result.map(Some)
    }
    pub fn publish(&mut self, p: Prepared, scene: &ingetrazo_core::scene::Scene, completed: u64) {
        match p.textures {
            TextureStage::Full(candidate, publication) => {
                self.textures.as_mut().unwrap().publish_admitted(
                    &mut self.resources,
                    candidate,
                    publication,
                    completed,
                    scene,
                )
            }
            TextureStage::Refresh(candidate, retirement) => {
                self.textures
                    .as_mut()
                    .unwrap()
                    .publish_vertex_refresh(&mut self.resources, candidate);
                self.resources.retire_completed(retirement);
            }
        }
        self.key = p.key;
        self.style = p.style;
        self.packet = p.packet;
        self.solid = p.solid;
        self.lines = p.lines;
        self.painted = p.painted;
        self.ordinary_runs = p.ordinary_runs;
        self.default_back = p.default_back;
        self.paint_runs = p.paint_runs;
        self.consolidated_opaque_count=p.consolidated_opaque_count;
        self.solid_count = p.solid_count;
        self.line_count = p.line_count;
        self.bytes = p.bytes;
        self.uploads += 1;
    }
    pub fn abort(&mut self, p: Prepared, fence: vk::Fence, completed: u64) -> Result<(), String> {
        match p.textures {
            TextureStage::Full(candidate, _) => self.textures.as_mut().unwrap().abort(
                &mut self.resources,
                candidate,
                fence,
                completed,
            )?,
            TextureStage::Refresh(candidate, _) => self
                .textures
                .as_mut()
                .unwrap()
                .abort_vertex_refresh(&mut self.resources, candidate, fence, completed)?,
        }
        for id in [p.solid, p.lines, p.painted].into_iter().flatten() {
            self.resources.retire(id)?;
        }
        self.resources.collect(fence, completed)?;
        Ok(())
    }
    pub fn draw_faces(
        &mut self,
        command: vk::CommandBuffer,
        serial: u64,
        pipeline: vk::Pipeline,
        layout: vk::PipelineLayout,
        mvp: &[f32; 16],
        height: u32,
        paint: &[vk::Pipeline; 10],
        transparent: vk::Pipeline,
        style: ingetrazo_render::style::VisualStyle,
        clip: &[f32; 4],
        clip_enabled: u32,
        colors: &[[f32; 4]; 2],
        base_ordinary:bool,
    ) -> Result<(), String> {
        if self.packet.as_ref().is_some_and(|p| p.kind == Kind::Prism) {
            use ingetrazo_render::style::VisualStyle;
            let wire = style == VisualStyle::Wireframe;
            let xray = style == VisualStyle::XRay;
            let material = matches!(style, VisualStyle::ShadedWithTextures | VisualStyle::XRay);
            let mode = match style {
                VisualStyle::HiddenLine => 1u32,
                VisualStyle::Monochrome => 2,
                _ => 0,
            };
            unsafe {
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(mvp),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    64,
                    bytemuck::cast_slice(clip),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    80,
                    bytemuck::bytes_of(&clip_enabled),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    84,
                    bytemuck::cast_slice(&[(if xray { 0.6f32 } else { 1. }).to_bits(), mode]),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    96,
                    bytemuck::cast_slice(colors),
                );
                if !wire {
                    if let Some(id) = self.solid {
                        self.resources.mark_used(id, serial)?;
                        self.device.cmd_bind_vertex_buffers(
                            command,
                            0,
                            &[self
                                .resources
                                .buffer(id)
                                .ok_or("Prism solid owner missing")?],
                            &[0],
                        );
                        for &(first, count, cull) in &self.ordinary_runs {
                        if cull&&!(base_ordinary||self.consolidated_opaque_count>0){continue;}
                            self.device.cmd_bind_pipeline(
                                command,
                                vk::PipelineBindPoint::GRAPHICS,
                                if xray {
                                    paint[if cull { 6 } else { 5 }]
                                } else if cull {
                                    paint[0]
                                } else {
                                    pipeline
                                },
                            );
                            self.device.cmd_draw(command, count, 1, first, 0);
                        }
                        if style == VisualStyle::ShadedWithTextures {
                            self.device.cmd_bind_pipeline(
                                command,
                                vk::PipelineBindPoint::GRAPHICS,
                                paint[4],
                            );
                            self.device.cmd_push_constants(
                                command,
                                layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                88,
                                bytemuck::bytes_of(&3u32),
                            );
                            for &(first, count) in &self.default_back {
                                self.device.cmd_draw(command, count, 1, first, 0);
                            }
                        }
                    }
                }
            }
            if !wire {
                for phase in [
                    super::super::textures::Phase::Opaque,
                    super::super::textures::Phase::BackOpaque,
                ] {
                    self.textures.as_mut().unwrap().record(
                        &mut self.resources,
                        command,
                        serial,
                        phase,
                        mvp,
                        clip,
                        clip_enabled,
                        height,
                        xray,
                    )?;
                }
            }
            if material {
                if let Some(id) = self.painted {
                    self.resources.mark_used(id, serial)?;
                    unsafe {
                        self.device.cmd_bind_vertex_buffers(
                            command,
                            0,
                            &[self
                                .resources
                                .buffer(id)
                                .ok_or("Prism paint owner missing")?],
                            &[0],
                        );
                        self.device.cmd_push_constants(
                            command,
                            layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            88,
                            bytemuck::bytes_of(&0u32),
                        );
                        for kind in [3u32, 0, 1, 2] {
                            for run in self.paint_runs.iter().filter(|r| r.kind == kind) {
                                self.device.cmd_bind_pipeline(
                                    command,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    match kind {
                                        1 => paint[1],
                                        2 => paint[2],
                                        3 => paint[3],
                                        _ => transparent,
                                    },
                                );
                                self.device.cmd_push_constants(
                                    command,
                                    layout,
                                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                    84,
                                    bytemuck::bytes_of(&run.opacity),
                                );
                                self.device.cmd_draw(command, run.count, 1, run.first, 0);
                            }
                        }
                    }
                }
                for phase in [
                    super::super::textures::Phase::TransparentFront,
                    super::super::textures::Phase::TransparentBack,
                ] {
                    self.textures.as_mut().unwrap().record(
                        &mut self.resources,
                        command,
                        serial,
                        phase,
                        mvp,
                        clip,
                        clip_enabled,
                        height,
                        xray,
                    )?;
                }
            }
            return Ok(());
        }
        if let Some(id) = self.solid {
            self.resources.mark_used(id, serial)?;
            unsafe {
                let state = [1f32.to_bits(), 0u32];
                self.device
                    .cmd_bind_pipeline(command, vk::PipelineBindPoint::GRAPHICS, pipeline);
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(mvp),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    80,
                    bytemuck::bytes_of(&0u32),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    84,
                    bytemuck::cast_slice(&state),
                );
                self.device.cmd_bind_vertex_buffers(
                    command,
                    0,
                    &[self
                        .resources
                        .buffer(id)
                        .ok_or("Sweep vertex owner missing")?],
                    &[0],
                );
                self.device.cmd_draw(command, self.solid_count, 1, 0, 0);
            }
        }
        self.textures.as_mut().unwrap().record(
            &mut self.resources,
            command,
            serial,
            super::super::textures::Phase::Opaque,
            mvp,
            &[0.; 4],
            0,
            height,
            false,
        )
    }
    pub fn draw_lines(
        &mut self,
        command: vk::CommandBuffer,
        serial: u64,
        pipeline: vk::Pipeline,
        layout: vk::PipelineLayout,
        mvp: &[f32; 16],
        clip_enabled: u32,
        edge_color: &[f32; 4],
        show_edges: bool,
    ) -> Result<(), String> {
        let prism = self.packet.as_ref().is_some_and(|p| p.kind == Kind::Prism);
        if prism && !show_edges {
            return Ok(());
        }
        let line_clip = if prism { clip_enabled } else { 0 };
        let line_color = if prism {
            *edge_color
        } else {
            [0.13f32, 0.17, 0.23, 1.]
        };
        if let Some(id) = self.lines {
            self.resources.mark_used(id, serial)?;
            unsafe {
                self.device
                    .cmd_bind_pipeline(command, vk::PipelineBindPoint::GRAPHICS, pipeline);
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(mvp),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    80,
                    bytemuck::bytes_of(&line_clip),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    84,
                    bytemuck::cast_slice(&[1f32.to_bits(), 1u32]),
                );
                self.device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    96,
                    bytemuck::cast_slice(&line_color),
                );
                self.device.cmd_bind_vertex_buffers(
                    command,
                    0,
                    &[self
                        .resources
                        .buffer(id)
                        .ok_or("Sweep line owner missing")?],
                    &[0],
                );
                self.device.cmd_draw(command, 2, self.line_count / 2, 0, 0);
            }
        }
        Ok(())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.resources.drain_graphics() {
            if let Some(textures) = self.textures.take() {
                std::mem::forget(textures);
            }
        } else {
            drop(self.textures.take());
        }
    }
}
