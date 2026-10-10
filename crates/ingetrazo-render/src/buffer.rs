// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Mesh, wireframe, and point render buffers.
//!
//! Vertex layouts are binary-compatible (`bytemuck::Pod` and `Zeroable`)
//! for direct GPU buffer upload (OpenGL, Vulkan) as well as software/egui drawing.

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Vec2, Vec3, Vec4};

/// Standard 3D vertex with position, normal, RGBA color, and UV texture coordinates.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            uv: [0.0, 0.0],
        }
    }
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], color: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            position,
            normal,
            color,
            uv,
        }
    }

    pub fn from_dvec3(position: DVec3, normal: DVec3, color: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            position: [position.x as f32, position.y as f32, position.z as f32],
            normal: [normal.x as f32, normal.y as f32, normal.z as f32],
            color,
            uv,
        }
    }

    pub fn pos_vec3(&self) -> Vec3 {
        Vec3::from_slice(&self.position)
    }

    pub fn norm_vec3(&self) -> Vec3 {
        Vec3::from_slice(&self.normal)
    }

    pub fn color_vec4(&self) -> Vec4 {
        Vec4::from_slice(&self.color)
    }

    pub fn uv_vec2(&self) -> Vec2 {
        Vec2::from_slice(&self.uv)
    }
}

/// CPU paint metadata for a contiguous face span; never uploaded.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceBackPaint {pub first:usize,pub count:usize,pub order:usize,pub color:[f32;3],pub opacity:f64}
/// CPU-only source texture run metadata. A GPU backend packs only position
/// plus UV (20 bytes); shade/opacity/culling are per-draw state, never expanded
/// into additional per-vertex uploaded color or normal attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceTextureRun {pub back_side:bool,pub order:usize,pub first:usize,pub count:usize,pub shade:f32,pub opacity:f64,pub front_cull:bool}
/// Mesh render buffer storing indexed triangle data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshRenderBuffer {
    /// CPU-only exact opacity change spans (index offset, source f64). Never uploaded.
    pub source_opacity_runs: Vec<(usize, f64)>,
    pub source_front_cull: Vec<(usize,usize)>,
    pub source_default_back:Vec<(usize,usize)>,
    /// Original consolidated opaque gate count; instanced-only placements exclude their indices.
    pub source_consolidated_opaque_count:usize,
    pub source_back_override_count:usize,
    pub source_back_paints: Vec<SourceBackPaint>,
    pub source_texture_runs:Vec<SourceTextureRun>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshRenderBuffer {
    pub fn new() -> Self {
        Self {
            source_opacity_runs: Vec::new(),
            source_texture_runs:Vec::new(),
            source_default_back:Vec::new(),source_front_cull:Vec::new(),source_back_paints:Vec::new(),source_consolidated_opaque_count:0,source_back_override_count:0,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(vert_cap: usize, ind_cap: usize) -> Self {
        Self {
            source_opacity_runs: Vec::new(),
            source_texture_runs:Vec::new(),
            source_default_back:Vec::new(),source_front_cull:Vec::new(),source_back_paints:Vec::new(),source_consolidated_opaque_count:0,source_back_override_count:0,
            vertices: Vec::with_capacity(vert_cap),
            indices: Vec::with_capacity(ind_cap),
        }
    }

    pub fn clear(&mut self) {
        self.source_opacity_runs.clear();self.source_texture_runs.clear();
        self.source_default_back.clear();self.source_front_cull.clear();self.source_back_paints.clear();self.source_consolidated_opaque_count=0;self.source_back_override_count=0;
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn mark_source_opacity(&mut self, opacity: f64) {
        if self.source_opacity_runs.last().map(|r|r.1)!=Some(opacity) {self.source_opacity_runs.push((self.indices.len(),opacity));}
    }

    /// Add a single triangle.
    pub fn push_triangle(&mut self, v0: Vertex, v1: Vertex, v2: Vertex) {
        let base = self.vertices.len() as u32;
        self.vertices.push(v0);
        self.vertices.push(v1);
        self.vertices.push(v2);
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// Add a quad split into two triangles (v0, v1, v2) and (v0, v2, v3).
    pub fn push_quad(&mut self, v0: Vertex, v1: Vertex, v2: Vertex, v3: Vertex) {
        let base = self.vertices.len() as u32;
        self.vertices.push(v0);
        self.vertices.push(v1);
        self.vertices.push(v2);
        self.vertices.push(v3);
        self.indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base, base + 2, base + 3,
        ]);
    }

    /// Append another mesh buffer with offset indices.
    pub fn append(&mut self, other: &MeshRenderBuffer) {
        let base = self.vertices.len() as u32;
        let offset=self.indices.len();
        self.source_consolidated_opaque_count+=other.source_consolidated_opaque_count;self.source_back_override_count+=other.source_back_override_count;
        self.source_default_back.extend(other.source_default_back.iter().map(|&(first,count)|(offset+first,count)));
        self.source_front_cull.extend(other.source_front_cull.iter().map(|&(first,count)|(offset+first,count)));
        self.source_back_paints.extend(other.source_back_paints.iter().cloned().map(|mut p|{p.first+=offset;p.order+=offset;p}));
        self.source_opacity_runs.extend(other.source_opacity_runs.iter().map(|&(index,alpha)|(offset+index,alpha)));
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|&idx| base + idx));
    }
}

/// Line vertex with position and color.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl Default for LineVertex {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl LineVertex {
    pub fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }

    pub fn from_dvec3(position: DVec3, color: [f32; 4]) -> Self {
        Self {
            position: [position.x as f32, position.y as f32, position.z as f32],
            color,
        }
    }

    pub fn pos_vec3(&self) -> Vec3 {
        Vec3::from_slice(&self.position)
    }

    pub fn color_vec4(&self) -> Vec4 {
        Vec4::from_slice(&self.color)
    }
}

/// Styling for line rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineStyle {
    Solid,
    Dashed { dash_len: f32, gap_len: f32 },
    HiddenDashed,
    GuideDotted,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self::Solid
    }
}

/// Single line segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSegment {
    pub v0: LineVertex,
    pub v1: LineVertex,
    pub width: f32,
    pub style: LineStyle,
}

impl LineSegment {
    pub fn new(v0: LineVertex, v1: LineVertex, width: f32, style: LineStyle) -> Self {
        Self { v0, v1, width, style }
    }

    pub fn solid(p0: DVec3, p1: DVec3, color: [f32; 4], width: f32) -> Self {
        Self {
            v0: LineVertex::from_dvec3(p0, color),
            v1: LineVertex::from_dvec3(p1, color),
            width,
            style: LineStyle::Solid,
        }
    }

    pub fn dashed(p0: DVec3, p1: DVec3, color: [f32; 4], width: f32, dash: f32, gap: f32) -> Self {
        Self {
            v0: LineVertex::from_dvec3(p0, color),
            v1: LineVertex::from_dvec3(p1, color),
            width,
            style: LineStyle::Dashed { dash_len: dash, gap_len: gap },
        }
    }
}

/// Line render buffer for wireframe, hidden lines, guides, and profile silhouettes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineRenderBuffer {
    pub vertices: Vec<LineVertex>,
    pub indices: Vec<u32>,
    pub segments: Vec<LineSegment>,
}

impl LineRenderBuffer {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            segments: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.segments.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.indices.is_empty()
    }

    pub fn push_segment(&mut self, segment: LineSegment) {
        let base = self.vertices.len() as u32;
        self.vertices.push(segment.v0);
        self.vertices.push(segment.v1);
        self.indices.extend_from_slice(&[base, base + 1]);
        self.segments.push(segment);
    }

    pub fn push_line(&mut self, p0: DVec3, p1: DVec3, color: [f32; 4], width: f32, style: LineStyle) {
        let seg = LineSegment {
            v0: LineVertex::from_dvec3(p0, color),
            v1: LineVertex::from_dvec3(p1, color),
            width,
            style,
        };
        self.push_segment(seg);
    }

    pub fn append(&mut self, other: &LineRenderBuffer) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|&idx| base + idx));
        self.segments.extend_from_slice(&other.segments);
    }
}

/// Point vertex for snaps and vertex selection markers.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct PointVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub size: f32,
    pub _padding: [f32; 3], // 16-byte alignment
}

impl Default for PointVertex {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            size: 5.0,
            _padding: [0.0; 3],
        }
    }
}

impl PointVertex {
    pub fn new(position: [f32; 3], color: [f32; 4], size: f32) -> Self {
        Self {
            position,
            color,
            size,
            _padding: [0.0; 3],
        }
    }

    pub fn from_dvec3(position: DVec3, color: [f32; 4], size: f32) -> Self {
        Self {
            position: [position.x as f32, position.y as f32, position.z as f32],
            color,
            size,
            _padding: [0.0; 3],
        }
    }
}

/// Point render buffer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointRenderBuffer {
    pub points: Vec<PointVertex>,
}

impl PointRenderBuffer {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    pub fn push(&mut self, p: PointVertex) {
        self.points.push(p);
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Texture buckets retain first-occurrence draw order across paths/shades.
/// Original opaque dictionary insertion order matters on coplanar paints;
/// a randomized HashMap alone cannot reconstruct it.
#[derive(Default,Clone,Debug)]pub struct TextureBuffers{buffers:std::collections::HashMap<String,MeshRenderBuffer>,next_order:usize}
impl TextureBuffers{pub fn take_order(&mut self)->usize{let order=self.next_order;self.next_order+=1;order}}
impl std::ops::Deref for TextureBuffers{type Target=std::collections::HashMap<String,MeshRenderBuffer>;fn deref(&self)->&Self::Target{&self.buffers}}
impl std::ops::DerefMut for TextureBuffers{fn deref_mut(&mut self)->&mut Self::Target{&mut self.buffers}}
impl<'a>IntoIterator for &'a TextureBuffers{type Item=(&'a String,&'a MeshRenderBuffer);type IntoIter=std::collections::hash_map::Iter<'a,String,MeshRenderBuffer>;fn into_iter(self)->Self::IntoIter{self.buffers.iter()}}
impl<'a>IntoIterator for &'a mut TextureBuffers{type Item=(&'a String,&'a mut MeshRenderBuffer);type IntoIter=std::collections::hash_map::IterMut<'a,String,MeshRenderBuffer>;fn into_iter(self)->Self::IntoIter{self.buffers.iter_mut()}}
impl IntoIterator for TextureBuffers{type Item=(String,MeshRenderBuffer);type IntoIter=std::collections::hash_map::IntoIter<String,MeshRenderBuffer>;fn into_iter(self)->Self::IntoIter{self.buffers.into_iter()}}
