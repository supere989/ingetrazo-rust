// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Selection highlight outlines, overlays, and markers.
//!
//! Visual cues for active selection:
//! - Selected faces: semi-transparent cyan/blue fill overlay and boundary highlight edges.
//! - Selected edges: prominent blue line overlay.
//! - Selected vertices: point markers.
//! - Selected groups: bounding box wireframes.

use glam::DVec3;
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::buffer::{
    LineRenderBuffer, LineSegment, LineStyle, LineVertex, MeshRenderBuffer, PointRenderBuffer,
    PointVertex, Vertex,
};
use crate::style::StyleConfig;

pub struct SelectionRenderData {
    pub overlay_mesh: MeshRenderBuffer,
    pub outline_lines: LineRenderBuffer,
    pub vertex_points: PointRenderBuffer,
}

impl Default for SelectionRenderData {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionRenderData {
    pub fn new() -> Self {
        Self {
            overlay_mesh: MeshRenderBuffer::new(),
            outline_lines: LineRenderBuffer::new(),
            vertex_points: PointRenderBuffer::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.overlay_mesh.is_empty()
            && self.outline_lines.is_empty()
            && self.vertex_points.is_empty()
    }
}

/// Build selection highlight buffers from the scene state.
pub fn build_selection_buffers(scene: &Scene, style: &StyleConfig) -> SelectionRenderData {
    let mut data = SelectionRenderData::new();
    if scene.selection.is_empty() {
        return data;
    }

    let highlight_color = [0.12, 0.55, 0.95, 1.0]; // Intense selection blue
    let overlay_fill_color = style.selection_color; // Semi-transparent blue fill

    let mesh = &scene.mesh;

    for item in &scene.selection {
        match item {
            SelectionItem::Guide(_) | SelectionItem::Dimension(_) | SelectionItem::TextLabel(_) => {},
            // Reference-image drawing/picking is a separate native renderer stage.
            SelectionItem::ImagePlane(_)|SelectionItem::ContextFace{..} | SelectionItem::ContextGroup{..}=>{}
            SelectionItem::SectionPlane(id)=>{if scene.show_section_planes{if let Some(frame)=scene.section_frame(*id){for edge in 0..4{data.outline_lines.push_line(frame[edge],frame[(edge+1)%4],highlight_color,3.,LineStyle::Solid);}}}}
            SelectionItem::Face(f_id) => {
                if *f_id >= mesh.faces.len() {
                    continue;
                }
                let face = &mesh.faces[*f_id];
                let norm = mesh.face_normal(*f_id);

                // Triangulate face for translucent fill overlay
                let tris = mesh.triangulate_face(*f_id);
                for (p0, p1, p2) in tris {
                    let v0 = Vertex::from_dvec3(p0, norm, overlay_fill_color, [0.0, 0.0]);
                    let v1 = Vertex::from_dvec3(p1, norm, overlay_fill_color, [0.0, 0.0]);
                    let v2 = Vertex::from_dvec3(p2, norm, overlay_fill_color, [0.0, 0.0]);
                    data.overlay_mesh.push_triangle(v0, v1, v2);
                }

                // Outline boundary edges
                let n = face.vertices.len();
                for i in 0..n {
                    let v0 = face.vertices[i];
                    let v1 = face.vertices[(i + 1) % n];
                    let p0 = mesh.vertices[v0].position;
                    let p1 = mesh.vertices[v1].position;
                    data.outline_lines.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(p0, highlight_color),
                        v1: LineVertex::from_dvec3(p1, highlight_color),
                        width: style.line_width * 2.0,
                        style: LineStyle::Solid,
                    });
                }
            }
            SelectionItem::Edge(e_id) => {
                if *e_id >= mesh.edges.len() {
                    continue;
                }
                let edge = &mesh.edges[*e_id];
                let p0 = mesh.vertices[edge.v0].position;
                let p1 = mesh.vertices[edge.v1].position;
                data.outline_lines.push_segment(LineSegment {
                    v0: LineVertex::from_dvec3(p0, highlight_color),
                    v1: LineVertex::from_dvec3(p1, highlight_color),
                    width: style.line_width * 2.5,
                    style: LineStyle::Solid,
                });
            }
            SelectionItem::Vertex(v_id) => {
                if *v_id >= mesh.vertices.len() {
                    continue;
                }
                let pos = mesh.vertices[*v_id].position;
                data.vertex_points.push(PointVertex::from_dvec3(
                    pos,
                    highlight_color,
                    8.0,
                ));
            }
            SelectionItem::Group(g_idx) => {
                if let Some(group) = scene.groups.get(*g_idx) {
                    if !group.mesh.vertices.is_empty() {
                        let mut min = DVec3::splat(f64::INFINITY);
                        let mut max = DVec3::splat(f64::NEG_INFINITY);
                        for v in &group.mesh.vertices {
                            let world_p = group.transform.transform_point3(v.position);
                            min = min.min(world_p);
                            max = max.max(world_p);
                        }

                        let corners = [
                            DVec3::new(min.x, min.y, min.z),
                            DVec3::new(max.x, min.y, min.z),
                            DVec3::new(max.x, max.y, min.z),
                            DVec3::new(min.x, max.y, min.z),
                            DVec3::new(min.x, min.y, max.z),
                            DVec3::new(max.x, min.y, max.z),
                            DVec3::new(max.x, max.y, max.z),
                            DVec3::new(min.x, max.y, max.z),
                        ];
                        let edges = [
                            (0, 1), (1, 2), (2, 3), (3, 0),
                            (4, 5), (5, 6), (6, 7), (7, 4),
                            (0, 4), (1, 5), (2, 6), (3, 7),
                        ];
                        for (i0, i1) in edges {
                            data.outline_lines.push_segment(LineSegment {
                                v0: LineVertex::from_dvec3(corners[i0], highlight_color),
                                v1: LineVertex::from_dvec3(corners[i1], highlight_color),
                                width: style.line_width * 1.5,
                                style: LineStyle::Dashed { dash_len: 0.1, gap_len: 0.05 },
                            });
                        }
                    }
                }
            }
            SelectionItem::GeoPoint(p_id) => {
                if *p_id < scene.geo_points.len() {
                    let pt = scene.geo_points[*p_id].position;
                    let cross = 0.2;
                    data.outline_lines.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(pt - glam::DVec3::X * cross, highlight_color),
                        v1: LineVertex::from_dvec3(pt + glam::DVec3::X * cross, highlight_color),
                        width: style.line_width * 2.0,
                        style: LineStyle::Solid,
                    });
                    data.outline_lines.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(pt - glam::DVec3::Y * cross, highlight_color),
                        v1: LineVertex::from_dvec3(pt + glam::DVec3::Y * cross, highlight_color),
                        width: style.line_width * 2.0,
                        style: LineStyle::Solid,
                    });
                }
            }
            SelectionItem::GeoPath(p_id) => {
                if *p_id < scene.geo_paths.len() {
                    let path = &scene.geo_paths[*p_id];
                    for (a, b) in path.segments() {
                        data.outline_lines.push_segment(LineSegment {
                            v0: LineVertex::from_dvec3(a, highlight_color),
                            v1: LineVertex::from_dvec3(b, highlight_color),
                            width: style.line_width * 2.0,
                            style: LineStyle::Solid,
                        });
                    }
                }
            }
        }
    }

    data
}
