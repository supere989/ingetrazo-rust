// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! # IngeTrazo Render Engine
//!
//! Provides rendering buffers, visual styles, wireframes with HLR (Hidden Line Removal),
//! selection highlights, inference snap glyphs, and viewport projection.

pub mod buffer;
pub mod annotations;
pub mod hlr;
pub mod pipeline;
pub mod selection;
pub mod snap_glyph;
pub mod style;

pub use buffer::*;
pub use hlr::*;
pub use pipeline::*;
pub use selection::*;
pub use snap_glyph::*;
pub use style::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::camera::OrbitCamera;
    use ingetrazo_core::mesh::Mesh;
    use ingetrazo_core::scene::{Scene, SelectionItem};
    use ingetrazo_core::snap::{SnapKind, SnapResult};

    #[test]
    fn test_buffer_creation_and_append() {
        let mut mb = MeshRenderBuffer::new();
        let v0 = Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [0.0, 0.0]);
        let v1 = Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [1.0, 0.0]);
        let v2 = Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [0.0, 1.0]);
        mb.push_triangle(v0, v1, v2);

        assert_eq!(mb.triangle_count(), 1);
        assert_eq!(mb.vertices.len(), 3);
        assert_eq!(mb.indices.len(), 3);

        let mut mb2 = MeshRenderBuffer::new();
        mb2.append(&mb);
        assert_eq!(mb2.triangle_count(), 1);
        assert_eq!(mb2.indices, vec![0, 1, 2]);

        mb2.append(&mb);
        assert_eq!(mb2.triangle_count(), 2);
        assert_eq!(mb2.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_visual_styles_presets() {
        let shaded = StyleConfig::default_shaded();
        assert_eq!(shaded.visual_style, VisualStyle::ShadedWithTextures);
        assert!(shaded.show_edges);

        let wireframe = StyleConfig::wireframe();
        assert_eq!(wireframe.visual_style, VisualStyle::Wireframe);

        let hidden_line = StyleConfig::hidden_line();
        assert_eq!(hidden_line.visual_style, VisualStyle::HiddenLine);

        let monochrome = StyleConfig::monochrome();
        assert_eq!(monochrome.visual_style, VisualStyle::Monochrome);

        let xray = StyleConfig::xray();
        assert_eq!(xray.visual_style, VisualStyle::XRay);
        assert!(xray.xray_opacity < 1.0);
    }

    #[test]
    fn test_edge_classification_and_hlr() {
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v3 = mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        mesh.add_face(vec![v0, v1, v2, v3], vec![]);

        let cam = OrbitCamera::new();
        let mut style = StyleConfig::default_shaded();
        style.back_edges_hidden = true;

        let (vis, hid) = build_edge_buffers(&mesh, &cam, &style, None);
        assert_eq!(vis.segments.len(), 4); // 4 boundary edges
        assert!(hid.segments.is_empty());
    }

    #[test]
    fn test_selection_buffers() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(2.0, 2.0, 0.0));
        let f = scene.mesh.add_face(vec![v0, v1, v2], vec![]).unwrap();
        scene.select(SelectionItem::Face(f));

        let style = StyleConfig::default_shaded();
        let sel_data = build_selection_buffers(&scene, &style);
        assert!(!sel_data.is_empty());
        assert_eq!(sel_data.overlay_mesh.triangle_count(), 1);
        assert_eq!(sel_data.outline_lines.segments.len(), 3);
    }

    #[test]
    fn test_snap_marker_glyphs() {
        let snap = SnapResult {
            point: DVec3::new(1.0, 2.0, 3.0),
            kind: SnapKind::Endpoint,
            color: [0.1, 0.8, 0.2],
            label: "Endpoint".to_string(),
            axis: None,
            guides: Vec::new(),
        };

        let glyph = SnapGlyph::from_snap_result(&snap, None).expect("Glyph should be created");
        assert_eq!(glyph.shape, GlyphShape::Square);
        assert_eq!(glyph.label, "Endpoint");
        assert_eq!(glyph.point, DVec3::new(1.0, 2.0, 3.0));
    }
}

mod section_clip;

pub mod vector_hlr;
pub mod paper_scene;
pub mod paper_section;

pub mod process_hlr;

mod triangle_index;
pub mod native_edges;

/// Precise backend-neutral authored2D paint/geometry contract.
pub mod cad2d;

pub mod qt_gray;

pub mod qt_cosmetic;
pub mod texture_layout;

pub mod native_textures;

pub mod cad_callback;

pub mod qt_selected_line;

pub mod image_outline;

pub mod qt_gl_image_line;

pub mod source_texture_average;

pub mod source_texture_decode;

pub mod qt_line;

pub mod scene_revision;

pub mod block_compression;

pub mod hidden_ghosts;
