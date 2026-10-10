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

/// Mesh render buffer storing indexed triangle data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshRenderBuffer {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshRenderBuffer {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(vert_cap: usize, ind_cap: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vert_cap),
            indices: Vec::with_capacity(ind_cap),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
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
