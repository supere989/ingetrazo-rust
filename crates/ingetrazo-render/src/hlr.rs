// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Hidden Line Removal (HLR) and edge classification styling.
//!
//! Categorizes edges into:
//! - Boundary edges (incident to 1 face): always visible silhouette profile.
//! - Silhouette / Profile edges (incident to 2 faces with opposite camera orientations).
//! - Crease edges (incident to 2 faces meeting at a sharp dihedral angle).
//! - Soft / smooth edges (tangent faces, typically hidden unless soft edges enabled).
//! - Back / occluded edges (drawn as dashed lines when HLR back edges is enabled).
//! - Wireframe standalone edges (0 incident faces).

use glam::DVec3;
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_core::guides::{Guide, GuideType};
use ingetrazo_core::mesh::Mesh;

use crate::buffer::{LineRenderBuffer, LineSegment, LineStyle, LineVertex};
use crate::style::StyleConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Standalone,
    Boundary,
    Silhouette,
    Crease,
    Smooth,
    BackEdge,
}

pub struct ClassifiedEdge {
    pub p0: DVec3,
    pub p1: DVec3,
    pub kind: EdgeKind,
    pub is_soft: bool,
    pub is_hidden: bool,
}

/// Classify all edges in a mesh relative to the current camera.
pub fn classify_mesh_edges(
    mesh: &Mesh,
    camera: &OrbitCamera,
    scene: Option<&ingetrazo_core::scene::Scene>,
) -> Vec<ClassifiedEdge> {
    let eye = camera.eye();
    let mut results = Vec::with_capacity(mesh.edges.len());

    // Precalculate face normals and whether they face the camera
    let face_normals: Vec<DVec3> = (0..mesh.faces.len())
        .map(|f_id| mesh.face_normal(f_id))
        .collect();

    for edge in &mesh.edges {
        let mut is_hidden = edge.hidden;
        if let Some(sc) = scene {
            if !sc.is_layer_visible(edge.layer.as_deref()) {
                is_hidden = true;
            } else if !edge.faces.is_empty() {
                let any_face_vis = edge.faces.iter().any(|&f_id| {
                    if f_id < mesh.faces.len() {
                        let f = &mesh.faces[f_id];
                        let f_layer = f.layer.as_deref().or_else(|| f.attrs.get("layer").and_then(|v| v.as_str()));
                        let f_hid = f.attrs.get("hidden").and_then(|v| v.as_bool()) == Some(true);
                        !f_hid && sc.is_layer_visible(f_layer)
                    } else {
                        false
                    }
                });
                if !any_face_vis {
                    is_hidden = true;
                }
            }
        }

        let p0 = mesh.vertices[edge.v0].position;
        let p1 = mesh.vertices[edge.v1].position;
        let mid = (p0 + p1) * 0.5;
        let view_dir = (eye - mid).normalize_or_zero();

        let kind = match edge.faces.len() {
            0 => EdgeKind::Standalone,
            1 => EdgeKind::Boundary,
            2 => {
                let f0 = edge.faces[0];
                let f1 = edge.faces[1];
                let n0 = face_normals[f0];
                let n1 = face_normals[f1];

                let dot0 = n0.dot(view_dir);
                let dot1 = n1.dot(view_dir);

                // Opposite signs mean one face is front-facing, one is back-facing:
                if (dot0 > 0.0 && dot1 <= 0.0) || (dot1 > 0.0 && dot0 <= 0.0) {
                    EdgeKind::Silhouette
                } else if dot0 <= 0.0 && dot1 <= 0.0 {
                    EdgeKind::BackEdge
                } else {
                    // Both front-facing: check dihedral angle
                    let cos_angle = n0.dot(n1).clamp(-1.0, 1.0);
                    // ~20 degrees crease angle: cos(20 deg) ≈ 0.94
                    if cos_angle < 0.94 {
                        EdgeKind::Crease
                    } else {
                        EdgeKind::Smooth
                    }
                }
            }
            _ => EdgeKind::Crease, // Non-manifold 3+ incident faces
        };

        results.push(ClassifiedEdge {
            p0,
            p1,
            kind,
            is_soft: edge.soft,
            is_hidden,
        });
    }

    results
}

/// Build visible wireframe and HLR dashed back-edge line buffers.
pub fn build_edge_buffers(
    mesh: &Mesh,
    camera: &OrbitCamera,
    style: &StyleConfig,
    scene: Option<&ingetrazo_core::scene::Scene>,
) -> (LineRenderBuffer, LineRenderBuffer) {
    let mut visible_buf = LineRenderBuffer::new();
    let mut hidden_buf = LineRenderBuffer::new();

    if !style.show_edges && style.visual_style != crate::style::VisualStyle::Wireframe {
        return (visible_buf, hidden_buf);
    }

    let classified = classify_mesh_edges(mesh, camera, scene);

    for edge in classified {
        if edge.is_hidden {
            continue;
        }

        // Soft edge handling
        if edge.is_soft && !style.show_soft_edges && edge.kind != EdgeKind::Silhouette && edge.kind != EdgeKind::Boundary {
            continue;
        }

        let width = if (edge.kind == EdgeKind::Silhouette || edge.kind == EdgeKind::Boundary)
            && style.show_profiles
        {
            style.profile_width
        } else {
            style.line_width
        };

        match edge.kind {
            EdgeKind::BackEdge => {
                if style.back_edges_hidden {
                    // Draw dashed hidden line
                    let hidden_color = [
                        style.edge_color[0] * 0.7,
                        style.edge_color[1] * 0.7,
                        style.edge_color[2] * 0.7,
                        0.6,
                    ];
                    hidden_buf.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(edge.p0, hidden_color),
                        v1: LineVertex::from_dvec3(edge.p1, hidden_color),
                        width: style.line_width,
                        style: LineStyle::HiddenDashed,
                    });
                } else if style.visual_style == crate::style::VisualStyle::Wireframe
                    || style.visual_style == crate::style::VisualStyle::XRay
                {
                    visible_buf.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(edge.p0, style.edge_color),
                        v1: LineVertex::from_dvec3(edge.p1, style.edge_color),
                        width,
                        style: LineStyle::Solid,
                    });
                }
            }
            EdgeKind::Smooth => {
                // Coplanarity does not hide an explicitly hard drafting edge.
                if !edge.is_soft {
                    visible_buf.push_line(edge.p0, edge.p1, style.edge_color, style.line_width, LineStyle::Solid);
                } else if style.show_soft_edges {
                    let soft_color = [
                        style.edge_color[0],
                        style.edge_color[1],
                        style.edge_color[2],
                        0.4,
                    ];
                    visible_buf.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(edge.p0, soft_color),
                        v1: LineVertex::from_dvec3(edge.p1, soft_color),
                        width: style.line_width * 0.8,
                        style: LineStyle::Solid,
                    });
                }
            }
            EdgeKind::Standalone | EdgeKind::Boundary | EdgeKind::Silhouette | EdgeKind::Crease => {
                visible_buf.push_segment(LineSegment {
                    v0: LineVertex::from_dvec3(edge.p0, style.edge_color),
                    v1: LineVertex::from_dvec3(edge.p1, style.edge_color),
                    width,
                    style: LineStyle::Solid,
                });
            }
        }
    }

    (visible_buf, hidden_buf)
}

/// Build guide line buffer with dashed / dotted styling.
pub fn build_guide_buffers(guides: &[Guide]) -> LineRenderBuffer {
    let mut buffer = LineRenderBuffer::new();
    let guide_color = [0.2, 0.45, 0.85, 0.85]; // Classic CAD guide blue

    for guide in guides {
        match &guide.kind {
            GuideType::InfiniteLine { point, dir } => {
                let extent = 500.0;
                let p0 = *point - *dir * extent;
                let p1 = *point + *dir * extent;
                buffer.push_segment(LineSegment {
                    v0: LineVertex::from_dvec3(p0, guide_color),
                    v1: LineVertex::from_dvec3(p1, guide_color),
                    width: 1.2,
                    style: LineStyle::GuideDotted,
                });
            }
            GuideType::Segment { a, b } => {
                buffer.push_segment(LineSegment {
                    v0: LineVertex::from_dvec3(*a, guide_color),
                    v1: LineVertex::from_dvec3(*b, guide_color),
                    width: 1.5,
                    style: LineStyle::Dashed {
                        dash_len: 0.2,
                        gap_len: 0.1,
                    },
                });
            }
            GuideType::Point(point) => {
                // Cross mark at guide point
                let s = 0.15;
                for dir in [DVec3::X, DVec3::Y, DVec3::Z] {
                    buffer.push_segment(LineSegment {
                        v0: LineVertex::from_dvec3(*point - dir * s, guide_color),
                        v1: LineVertex::from_dvec3(*point + dir * s, guide_color),
                        width: 1.5,
                        style: LineStyle::Solid,
                    });
                }
            }
        }
    }

    buffer
}


#[cfg(test)] mod hard_coplanar_tests {
use super::*;
#[test] fn hard_coplanar_division_remains_visible_while_soft_division_is_hidden(){
let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(vec![ids[0],ids[1],ids[2]],vec![]);mesh.add_face(vec![ids[0],ids[2],ids[3]],vec![]);
let mut camera=OrbitCamera::default();camera.target=DVec3::new(0.5,0.5,0.);camera.pitch=1.;
let style=StyleConfig::default();let (hard,_)=build_edge_buffers(&mesh,&camera,&style,None);assert_eq!(hard.segments.len(),5);
let edge=mesh.find_edge(ids[0],ids[2]).unwrap();mesh.edges[edge].soft=true;let(soft,_)=build_edge_buffers(&mesh,&camera,&style,None);assert_eq!(soft.segments.len(),4);
}
}
