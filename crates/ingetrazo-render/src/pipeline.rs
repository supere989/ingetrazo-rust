// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Scene render pipeline.
//!
//! Converts high-level `Scene` data into optimized render buffers
//! according to active `OrbitCamera` and `StyleConfig`.
//! Also provides direct projection and rendering into `egui::Painter`.

use glam::{DMat4, DVec3, DVec4};
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_core::mesh::Mesh;
use ingetrazo_core::scene::Scene;

use crate::buffer::{
    LineRenderBuffer, LineStyle, MeshRenderBuffer, Vertex,
};

use crate::hlr::{build_edge_buffers, build_guide_buffers};
use crate::selection::{build_selection_buffers, SelectionRenderData};
use crate::snap_glyph::SnapGlyph;
use crate::style::{StyleConfig, VisualStyle};

#[derive(Clone, Debug)]
pub struct BillboardRenderItem {
    pub texture_name: String,
    pub anchor: DVec3,
    pub width: f64,
    pub height: f64,
}

/// Aggregated scene render data ready for the viewport canvas.
pub struct SceneRenderData {
    pub opaque_mesh: MeshRenderBuffer,
    pub section_stencil: Option<crate::section_clip::SectionStencil>,
    pub transparent_mesh: MeshRenderBuffer,
    pub textured_meshes: std::collections::HashMap<String, MeshRenderBuffer>,
    pub visible_edges: LineRenderBuffer,
    pub hidden_edges: LineRenderBuffer,
    pub guides: LineRenderBuffer,
    pub selection: SelectionRenderData,
    pub snap_glyphs: Vec<SnapGlyph>,
    pub dimension_lines: LineRenderBuffer,
    pub dimension_labels: Vec<(DVec3, String)>,
    pub text_labels: Vec<(DVec3, DVec3, String)>,
    pub billboards: Vec<BillboardRenderItem>,
}

impl Default for SceneRenderData {
    fn default() -> Self {
        Self {
            opaque_mesh: MeshRenderBuffer::new(),
            section_stencil: None,
            transparent_mesh: MeshRenderBuffer::new(),
            textured_meshes: std::collections::HashMap::new(),
            visible_edges: LineRenderBuffer::new(),
            hidden_edges: LineRenderBuffer::new(),
            guides: LineRenderBuffer::new(),
            selection: SelectionRenderData::new(),
            snap_glyphs: Vec::new(),
            dimension_lines: LineRenderBuffer::new(),
            dimension_labels: Vec::new(),
            text_labels: Vec::new(),
            billboards: Vec::new(),
        }
    }
}

const VERTICAL_TOLERANCE: f64 = 1e-3;

/// Basis vectors for .skp compatible planar texture projection.
pub fn projection_basis(normal: DVec3) -> (DVec3, DVec3) {
    let ln = normal.length();
    let n = if ln > 1e-30 { normal / ln } else { DVec3::Z };
    let xx = -n.y;
    let xy = n.x;
    let lx = (xx * xx + xy * xy).sqrt();
    if lx < VERTICAL_TOLERANCE {
        if n.z > 0.0 {
            (DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.0, 1.0, 0.0))
        } else {
            (DVec3::new(-1.0, 0.0, 0.0), DVec3::new(0.0, 1.0, 0.0))
        }
    } else {
        let xr = DVec3::new(xx / lx, xy / lx, 0.0);
        let yr = DVec3::new(-n.z * (xy / lx), n.z * (xx / lx), n.x * (xy / lx) - n.y * (xx / lx));
        (xr, yr)
    }
}

/// Planar projection axes turned by in-plane rotation angle (degrees).
pub fn projection_axes(normal: DVec3, rot_deg: f64) -> (DVec3, DVec3) {
    let (u_axis, v_axis) = projection_basis(normal);
    if rot_deg.abs() > 1e-6 {
        let a = rot_deg.to_radians();
        let cos_a = a.cos();
        let sin_a = a.sin();
        (u_axis * cos_a + v_axis * sin_a, v_axis * cos_a - u_axis * sin_a)
    } else {
        (u_axis, v_axis)
    }
}

/// Compute affine world->UV map `(gu, cu, gv, cv)` for face:
/// `u = gu · p + cu`, `v = gv · p + cv`.
pub fn face_uv_axes(tex: &serde_json::Value, normal: DVec3) -> (DVec3, f64, DVec3, f64) {
    if let Some(uvw) = tex.get("uvw").and_then(|v| v.as_array()) {
        if uvw.len() == 8 {
            let gu = DVec3::new(
                uvw[0].as_f64().unwrap_or(0.0),
                uvw[1].as_f64().unwrap_or(0.0),
                uvw[2].as_f64().unwrap_or(0.0),
            );
            let cu = uvw[3].as_f64().unwrap_or(0.0);
            let gv = DVec3::new(
                uvw[4].as_f64().unwrap_or(0.0),
                uvw[5].as_f64().unwrap_or(0.0),
                uvw[6].as_f64().unwrap_or(0.0),
            );
            let cv = uvw[7].as_f64().unwrap_or(0.0);
            return (gu, cu, gv, cv);
        }
    }
    let rot = tex.get("rot").and_then(|r| r.as_f64()).unwrap_or(0.0);
    let (u_axis, v_axis) = projection_axes(normal, rot);
    let sw = tex.get("sw").and_then(|s| s.as_f64()).unwrap_or(1.0);
    let sh = tex.get("sh").and_then(|s| s.as_f64()).unwrap_or(1.0);
    let sw = if sw.abs() > 1e-9 { sw } else { 1.0 };
    let sh = if sh.abs() > 1e-9 { sh } else { 1.0 };
    (u_axis / sw, 0.0, v_axis / sh, 0.0)
}

/// Diffuse factor (0.62..1.0) against fixed world light, matching reference.
pub fn shade_factor(normal: DVec3) -> f32 {
    let ln = normal.length();
    if ln < 1e-9 {
        return 0.84375;
    }
    let light_dir = DVec3::new(0.35, 0.25, 1.0).normalize();
    let d = (normal.normalize().dot(light_dir)).abs();
    ((0.62 + 0.38 * d) * 32.0).round() as f32 / 32.0
}

fn get_face_source_opacity(attrs: &std::collections::HashMap<String,serde_json::Value>,scene:&Scene)->f64 {
    attrs
        .get("opacity")
        .and_then(|v| v.as_f64())
        .or_else(|| {
            attrs
                .get("color")
                .and_then(|v| v.as_array())
                .and_then(|a| a.get(3))
                .and_then(|v| v.as_f64())
        })
        .or_else(|| {
            attrs
                .get("mat")
                .and_then(|m| m.as_str())
                .and_then(|mat_name| scene.materials.get(mat_name).map(|m| m.opacity as f64))
        })
        .unwrap_or(1.0)
}

fn get_face_texture(
    attrs: &std::collections::HashMap<String, serde_json::Value>,
    scene: &Scene,
) -> Option<serde_json::Value> {
    let tex_val = attrs.get("texture").cloned().or_else(|| {
        attrs.get("mat").and_then(|m| m.as_str()).and_then(|mat_name| {
            scene.materials.get(mat_name).and_then(|mat| {
                mat.texture_embed.as_ref().or(mat.texture_path.as_ref()).map(|emb| {
                    serde_json::json!({
                        "embed": emb,
                        "sw": 1.0,
                        "sh": 1.0,
                        "rot": 0.0,
                    })
                })
            })
        })
    });

    tex_val
}

fn get_face_base_color(
    attrs: &std::collections::HashMap<String, serde_json::Value>,
    scene: &Scene,
    style: &StyleConfig,
) -> [f32; 4] {
    match style.visual_style {
        VisualStyle::HiddenLine | VisualStyle::Monochrome => style.front_color,
        _ => {
            if let Some(color_val) = attrs.get("color") {
                if let Some(arr) = color_val.as_array() {
                    let r = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.9) as f32;
                    let g = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.9) as f32;
                    let b = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.9) as f32;
                    let a = arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    [r, g, b, a]
                } else {
                    style.front_color
                }
            } else if let Some(mat_name) = attrs.get("mat").and_then(|v| v.as_str()) {
                if let Some(mat) = scene.materials.get(mat_name) {
                    [mat.color[0], mat.color[1], mat.color[2], mat.opacity]
                } else {
                    match mat_name.to_lowercase().as_str() {
                        "metal_dark" => [0.22, 0.24, 0.26, 1.0],
                        "beech" | "wood" => [0.78, 0.58, 0.40, 1.0],
                        "concrete_smooth" | "concrete" => [0.72, 0.72, 0.70, 1.0],
                        _ => style.front_color,
                    }
                }
            } else {
                style.front_color
            }
        }
    }
}

/// Build full scene render data.
pub fn build_scene_render_data(
    scene: &Scene,
    camera: &OrbitCamera,
    style: &StyleConfig,
    active_snap: Option<&SnapGlyph>,
) -> SceneRenderData {
    let mut data = SceneRenderData::default();

    // 1. Loose mesh faces
    if style.visual_style != VisualStyle::Wireframe {
        build_mesh_buffers(
            &scene.mesh,
            scene,
            style,
            &mut data.opaque_mesh,
            &mut data.transparent_mesh,
            &mut data.textured_meshes,
        );
    }

    // 2. Loose mesh edges
    let (vis_edges, hid_edges) = build_edge_buffers(&scene.mesh, camera, style, Some(scene));
    data.visible_edges = vis_edges;
    data.hidden_edges = hid_edges;

    // 3. Groups (and nested components)
    for group in &scene.groups {
        build_group_buffers(
            group,
            &DMat4::IDENTITY,
            scene,
            camera,
            style,
            &mut data.opaque_mesh,
            &mut data.transparent_mesh,
            &mut data.visible_edges,
            &mut data.billboards,
            &mut data.textured_meshes,
            true,
        );
    }

    // 4. Dimensions
    for dim in &scene.dimensions {
        if !scene.is_layer_visible(dim.layer.as_deref()){continue;}
        let (p0,p1)=scene.dimension_points(dim);
        let mut live=dim.clone();live.start=p0;live.end=p1;
        let off = dim.offset;
        let (d0,d1)=live.line_points();

        // Witness lines
        data.dimension_lines.push_line(p0, d0, [0.15, 0.15, 0.2, 0.7], 1.0, LineStyle::Solid);
        data.dimension_lines.push_line(p1, d1, [0.15, 0.15, 0.2, 0.7], 1.0, LineStyle::Solid);

        // Dimension span line
        data.dimension_lines.push_line(d0, d1, [0.1, 0.1, 0.15, 1.0], 1.5, LineStyle::Solid);

        // Tick marks at endpoints
        let dir = (d1 - d0).normalize_or_zero();
        let up = if off.length_squared() > 1e-6 { off.normalize() } else { DVec3::Z };
        let tick_v = (dir + up).normalize_or_zero() * 0.08;
        data.dimension_lines.push_line(d0 - tick_v, d0 + tick_v, [0.1, 0.1, 0.15, 1.0], 1.5, LineStyle::Solid);
        data.dimension_lines.push_line(d1 - tick_v, d1 + tick_v, [0.1, 0.1, 0.15, 1.0], 1.5, LineStyle::Solid);

        let mid = (d0 + d1) * 0.5 + up * 0.05;
        data.dimension_labels.push((mid, live.display_text(scene.unit, 2)));
    }

    // 4b. Text labels (leader annotations)
    for lab in &scene.text_labels {
        data.text_labels.push((lab.anchor, lab.position(), lab.text.clone()));
    }

    // 5. Guides
    data.guides = build_guide_buffers(&scene.guides);

    // 6. Selection
    data.selection = build_selection_buffers(scene, style);

    // 7. Active snap glyph
    if let Some(glyph) = active_snap {
        data.snap_glyphs.push(glyph.clone());
    }

    crate::section_clip::clip_scene(&mut data,scene,style);
    data
}

/// World-space surfaces only: no CPU projection, painter sorting or HLR work.
/// GPU backends reuse the same visibility, material and recursive transform logic.
pub fn build_world_surfaces(scene:&Scene,camera:&OrbitCamera,style:&StyleConfig)->SceneRenderData {
    let mut data=SceneRenderData::default();
    if style.visual_style!=VisualStyle::Wireframe {
        build_mesh_buffers(&scene.mesh,scene,style,&mut data.opaque_mesh,&mut data.transparent_mesh,&mut data.textured_meshes);
        for group in &scene.groups {build_group_buffers(group,&DMat4::IDENTITY,scene,camera,style,&mut data.opaque_mesh,&mut data.transparent_mesh,&mut data.visible_edges,&mut data.billboards,&mut data.textured_meshes,false);}
    }
    data
}

fn build_mesh_buffers(
    mesh: &Mesh,
    scene: &Scene,
    style: &StyleConfig,
    opaque: &mut MeshRenderBuffer,
    transparent: &mut MeshRenderBuffer,
    textured_meshes: &mut std::collections::HashMap<String, MeshRenderBuffer>,
) {
    for (f_id, face) in mesh.faces.iter().enumerate() {
        let face_layer = face.layer.as_deref().or_else(|| face.attrs.get("layer").and_then(|v| v.as_str()));
        if !scene.is_layer_visible(face_layer) {
            continue;
        }
        if face.attrs.get("hidden").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }

        let norm = mesh.face_normal(f_id);
        let shade = shade_factor(norm);
        let tex_val = get_face_texture(&face.attrs, scene);
        let source_opacity = get_face_source_opacity(&face.attrs, scene);

        let is_xray = style.visual_style == VisualStyle::XRay;
        let source_final_opacity = if is_xray {style.xray_opacity as f64} else {source_opacity};
        let final_opacity=source_final_opacity as f32;

        let has_texture = style.show_textures && (style.visual_style == VisualStyle::ShadedWithTextures || is_xray) && tex_val.is_some();

        if has_texture {
            let tex = tex_val.unwrap();
            let tex_key = tex.get("embed").and_then(|e| e.as_str())
                .or_else(|| tex.get("path").and_then(|p| p.as_str()))
                .unwrap_or("texture")
                .to_string();

            let (gu, cu, gv, cv) = face_uv_axes(&tex, norm);
            let vert_color = [shade, shade, shade, final_opacity];

            let tris = mesh.triangulate_face(f_id);
            for (p0, p1, p2) in tris {
                let u0 = gu.dot(p0) + cu;
                let v0 = gv.dot(p0) + cv;
                let u1 = gu.dot(p1) + cu;
                let v1 = gv.dot(p1) + cv;
                let u2 = gu.dot(p2) + cu;
                let v2 = gv.dot(p2) + cv;

                let v0 = Vertex::from_dvec3(p0, norm, vert_color, [u0 as f32, v0 as f32]);
                let v1 = Vertex::from_dvec3(p1, norm, vert_color, [u1 as f32, v1 as f32]);
                let v2 = Vertex::from_dvec3(p2, norm, vert_color, [u2 as f32, v2 as f32]);

                textured_meshes.entry(tex_key.clone()).or_default().push_triangle(v0, v1, v2);
            }
        } else {
            let base_color = get_face_base_color(&face.attrs, scene, style);
            let shaded_color = [
                (base_color[0] * shade).clamp(0.0, 1.0),
                (base_color[1] * shade).clamp(0.0, 1.0),
                (base_color[2] * shade).clamp(0.0, 1.0),
                final_opacity,
            ];

            let first=if is_xray||source_final_opacity<0.999{transparent.indices.len()}else{opaque.indices.len()};
            let tris = mesh.triangulate_face(f_id);
            if (is_xray || source_final_opacity<0.999)&&!tris.is_empty(){transparent.mark_source_opacity(source_final_opacity);}
            for (p0, p1, p2) in tris {
                let v0 = Vertex::from_dvec3(p0, norm, shaded_color, [0.0, 0.0]);
                let v1 = Vertex::from_dvec3(p1, norm, shaded_color, [0.0, 0.0]);
                let v2 = Vertex::from_dvec3(p2, norm, shaded_color, [0.0, 0.0]);

                if is_xray || source_final_opacity < 0.999 {
                    transparent.push_triangle(v0, v1, v2);
                } else {
                    opaque.push_triangle(v0, v1, v2);
                }
            }
            let order=opaque.indices.len()+transparent.indices.len();
            let target=if is_xray||source_final_opacity<0.999{&mut *transparent}else{&mut *opaque};
            record_source_back_paint(target,first,order,&face.attrs,shade);
            if !(is_xray||source_final_opacity<0.999)&&!target.source_front_cull.last().is_some_and(|r|r.0==first){target.source_consolidated_opaque_count+=target.indices.len()-first;}
        }
    }
}

fn build_group_buffers(
    group: &ingetrazo_core::groups::Group,
    parent_transform: &DMat4,
    scene: &Scene,
    camera: &OrbitCamera,
    style: &StyleConfig,
    opaque: &mut MeshRenderBuffer,
    transparent: &mut MeshRenderBuffer,
    vis_edges: &mut LineRenderBuffer,
    billboards: &mut Vec<BillboardRenderItem>,
    textured_meshes: &mut std::collections::HashMap<String, MeshRenderBuffer>,
    include_edges: bool,
) {
    if group.hidden || !scene.is_layer_visible(group.layer.as_deref()) {
        return;
    }

    let world_t = *parent_transform * group.transform;
    // Original viewport._instance_entry.tp32 preserves painted front winding
    // under reflected placements. Reorder local corners before UV evaluation.
    let reflected = world_t.determinant() < 0.0;
    let source_placement = source_group_has_placement(group);

    // Billboard / Face-Me cutout figures
    if group.billboard {
        if let Some(tex_name) = &group.billboard_texture {
            if let Some(face) = group.mesh.faces.first() {
                if face.vertices.len() >= 4 {
                    let mut xs = Vec::new();
                    let mut ys = Vec::new();
                    let mut zs = Vec::new();
                    for &vid in &face.vertices {
                        let wp = world_t.transform_point3(group.mesh.vertices[vid].position);
                        xs.push(wp.x);
                        ys.push(wp.y);
                        zs.push(wp.z);
                    }
                    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let min_z = zs.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max_z = zs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                    let anchor = DVec3::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5, min_z);
                    let w = (max_x - min_x).hypot(max_y - min_y);
                    let h = max_z - min_z;

                    billboards.push(BillboardRenderItem {
                        texture_name: tex_name.clone(),
                        anchor,
                        width: w,
                        height: h,
                    });
                    return;
                }
            }
        }
    }

    let local_start=(opaque.indices.len(),transparent.indices.len(),opaque.source_front_cull.len(),opaque.source_back_override_count+transparent.source_back_override_count);
    for (f_id, face) in group.mesh.faces.iter().enumerate() {
        let face_layer = face.layer.as_deref().or_else(|| face.attrs.get("layer").and_then(|v| v.as_str()));
        if !scene.is_layer_visible(face_layer) {
            continue;
        }
        if face.attrs.get("hidden").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }

        let local_norm = group.mesh.face_normal(f_id);
        let world_norm = world_t.transform_vector3(local_norm).normalize_or_zero();
        // Original _instance_entry transforms position columns only: prototype
        // RGB is shaded in local coordinates and retained through placement.
        let shade = shade_factor(if source_placement {local_norm} else {world_norm});
        let tex_val = get_face_texture(&face.attrs, scene);
        let source_opacity = get_face_source_opacity(&face.attrs, scene);

        let is_xray = style.visual_style == VisualStyle::XRay;
        let source_final_opacity = if is_xray {style.xray_opacity as f64} else {source_opacity};
        let final_opacity=source_final_opacity as f32;

        let has_texture = style.show_textures && (style.visual_style == VisualStyle::ShadedWithTextures || is_xray) && tex_val.is_some();

        if has_texture {
            let tex = tex_val.unwrap();
            let tex_key = tex.get("embed").and_then(|e| e.as_str())
                .or_else(|| tex.get("path").and_then(|p| p.as_str()))
                .unwrap_or("texture")
                .to_string();

            let (gu, cu, gv, cv) = face_uv_axes(&tex, world_norm);
            let vert_color = [shade, shade, shade, final_opacity];

            let tris = group.mesh.triangulate_face(f_id);
            for (p0, p1, p2) in tris {
                let (p1, p2) = if reflected {(p2, p1)} else {(p1, p2)};
                let wp0 = world_t.transform_point3(p0);
                let wp1 = world_t.transform_point3(p1);
                let wp2 = world_t.transform_point3(p2);

                let u0 = gu.dot(wp0) + cu;
                let v0 = gv.dot(wp0) + cv;
                let u1 = gu.dot(wp1) + cu;
                let v1 = gv.dot(wp1) + cv;
                let u2 = gu.dot(wp2) + cu;
                let v2 = gv.dot(wp2) + cv;

                let v0 = Vertex::from_dvec3(wp0, world_norm, vert_color, [u0 as f32, v0 as f32]);
                let v1 = Vertex::from_dvec3(wp1, world_norm, vert_color, [u1 as f32, v1 as f32]);
                let v2 = Vertex::from_dvec3(wp2, world_norm, vert_color, [u2 as f32, v2 as f32]);

                textured_meshes.entry(tex_key.clone()).or_default().push_triangle(v0, v1, v2);
            }
        } else {
            let base_color = get_face_base_color(&face.attrs, scene, style);
            let shaded_color = [
                (base_color[0] * shade).clamp(0.0, 1.0),
                (base_color[1] * shade).clamp(0.0, 1.0),
                (base_color[2] * shade).clamp(0.0, 1.0),
                final_opacity,
            ];

            let first=if is_xray||source_final_opacity<0.999{transparent.indices.len()}else{opaque.indices.len()};
            let tris = group.mesh.triangulate_face(f_id);
            if (is_xray || source_final_opacity<0.999)&&!tris.is_empty(){transparent.mark_source_opacity(source_final_opacity);}
            for (p0, p1, p2) in tris {
                let (p1, p2) = if reflected {(p2, p1)} else {(p1, p2)};
                let wp0 = world_t.transform_point3(p0);
                let wp1 = world_t.transform_point3(p1);
                let wp2 = world_t.transform_point3(p2);

                let v0 = Vertex::from_dvec3(wp0, world_norm, shaded_color, [0.0, 0.0]);
                let v1 = Vertex::from_dvec3(wp1, world_norm, shaded_color, [0.0, 0.0]);
                let v2 = Vertex::from_dvec3(wp2, world_norm, shaded_color, [0.0, 0.0]);

                if is_xray || source_final_opacity < 0.999 {
                    transparent.push_triangle(v0, v1, v2);
                } else {
                    opaque.push_triangle(v0, v1, v2);
                }
            }
            let order=opaque.indices.len()+transparent.indices.len();
            let target=if is_xray||source_final_opacity<0.999{&mut *transparent}else{&mut *opaque};
            record_source_back_paint(target,first,order,&face.attrs,shade);
        }
    }

    // Original _instanced_eligible uses a prototype-local cache: transparent/back
    // content stays consolidated. Counts/ranges already built above answer that
    // question without another face traversal or a per-frame placement copy.
    let local_transparent=transparent.indices.len()!=local_start.1;
    let local_back=opaque.source_back_override_count+transparent.source_back_override_count!=local_start.3;
    let instanced=source_placement&&!group.billboard&&!local_transparent&&!local_back&&!source_instancing_disabled();
    if !instanced {let front_only:usize=opaque.source_front_cull[local_start.2..].iter().map(|r|r.1).sum();opaque.source_consolidated_opaque_count+=opaque.indices.len()-local_start.0-front_only;}

    let eye = camera.eye();
    if include_edges { for edge in &group.mesh.edges {
        if edge.hidden || !scene.is_layer_visible(edge.layer.as_deref()) {
            continue;
        }
        if !edge.faces.is_empty() {
            let any_face_vis = edge.faces.iter().any(|&f_id| {
                if f_id < group.mesh.faces.len() {
                    let f = &group.mesh.faces[f_id];
                    let f_layer = f.layer.as_deref().or_else(|| f.attrs.get("layer").and_then(|v| v.as_str()));
                    let f_hid = f.attrs.get("hidden").and_then(|v| v.as_bool()) == Some(true);
                    !f_hid && scene.is_layer_visible(f_layer)
                } else {
                    false
                }
            });
            if !any_face_vis {
                continue;
            }
        }
        let lp0 = group.mesh.vertices[edge.v0].position;
        let lp1 = group.mesh.vertices[edge.v1].position;
        let wp0 = world_t.transform_point3(lp0);
        let wp1 = world_t.transform_point3(lp1);

        if edge.soft && !style.show_soft_edges {
            if style.show_profiles && edge.faces.len() == 2 {
                let f0 = edge.faces[0];
                let f1 = edge.faces[1];
                let n0 = world_t.transform_vector3(group.mesh.face_normal(f0)).normalize_or_zero();
                let n1 = world_t.transform_vector3(group.mesh.face_normal(f1)).normalize_or_zero();
                let mid = (wp0 + wp1) * 0.5;
                let view_dir = (mid - eye).normalize_or_zero();
                let d0 = n0.dot(view_dir);
                let d1 = n1.dot(view_dir);
                if (d0 < 0.0 && d1 >= 0.0) || (d0 >= 0.0 && d1 < 0.0) {
                    vis_edges.push_line(wp0, wp1, style.edge_color, style.profile_width, LineStyle::Solid);
                }
            }
            continue;
        }

        let width = if edge.faces.len() <= 1 && style.show_profiles {
            style.profile_width
        } else {
            style.line_width
        };
        vis_edges.push_line(wp0, wp1, style.edge_color, width, LineStyle::Solid);
    }

    }
    for child in &group.children {
        build_group_buffers(
            child,
            &world_t,
            scene,
            camera,
            style,
            opaque,
            transparent,
            vis_edges,
            billboards,
            textured_meshes,
            include_edges,
        );
    }
}


/// Project 3D point in world coordinates to screen pixel coordinates.
/// Returns (x, y, depth_ndc) where ndc is within [-1, 1].
pub fn project_point(
    point: DVec3,
    view_proj: &DMat4,
    viewport_rect: egui::Rect,
) -> Option<(egui::Pos2, f64)> {
    let clip = *view_proj * DVec4::new(point.x, point.y, point.z, 1.0);
    if clip.w <= 0.0001 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let ndc_z = clip.z / clip.w;

    // Viewport transform
    let screen_x = viewport_rect.min.x + ((ndc_x + 1.0) * 0.5) as f32 * viewport_rect.width();
    let screen_y = viewport_rect.min.y + ((1.0 - ndc_y) * 0.5) as f32 * viewport_rect.height();

    Some((egui::pos2(screen_x, screen_y), ndc_z))
}

/// Project a 3D line segment to screen pixel coordinates, clipping against the near plane.
pub fn project_segment(
    mut p0: DVec3,
    mut p1: DVec3,
    view_proj: &DMat4,
    viewport_rect: egui::Rect,
) -> Option<((egui::Pos2, f64), (egui::Pos2, f64))> {
    let c0 = *view_proj * DVec4::new(p0.x, p0.y, p0.z, 1.0);
    let c1 = *view_proj * DVec4::new(p1.x, p1.y, p1.z, 1.0);

    const NEAR_W: f64 = 0.001;

    if c0.w < NEAR_W && c1.w < NEAR_W {
        return None;
    }

    if c0.w < NEAR_W {
        let t = (NEAR_W - c0.w) / (c1.w - c0.w);
        p0 = p0 + (p1 - p0) * t;
    } else if c1.w < NEAR_W {
        let t = (NEAR_W - c0.w) / (c1.w - c0.w);
        p1 = p0 + (p1 - p0) * t;
    }

    if let (Some(s0), Some(s1)) = (
        project_point(p0, view_proj, viewport_rect),
        project_point(p1, view_proj, viewport_rect),
    ) {
        Some((s0, s1))
    } else {
        None
    }
}

/// Render the complete SceneRenderData onto an egui::Painter.
pub fn render_scene_to_egui(
    data: &SceneRenderData,
    camera: &OrbitCamera,
    painter: &egui::Painter,
    rect: egui::Rect,
    style: &StyleConfig,
    textures: &std::collections::HashMap<String, egui::TextureId>,
) {
    let view_proj = camera.projection_matrix() * camera.view_matrix();

    // 1. Render Sky & Ground Backdrop if enabled
    if style.show_sky {
        let sky_c = egui::Color32::from_rgba_premultiplied(
            (style.sky_color[0] * 255.0) as u8,
            (style.sky_color[1] * 255.0) as u8,
            (style.sky_color[2] * 255.0) as u8,
            255,
        );
        let gnd_c = egui::Color32::from_rgba_premultiplied(
            (style.ground_color[0] * 255.0) as u8,
            (style.ground_color[1] * 255.0) as u8,
            (style.ground_color[2] * 255.0) as u8,
            255,
        );
        let eye = camera.eye();
        let fwd = camera.target - eye;
        let mut dh = DVec3::new(fwd.x, fwd.y, 0.0);
        let ndc_y = if dh.length_squared() < 1e-10 {
            if fwd.z < 0.0 { 2.0 } else { -2.0 }
        } else {
            dh = dh.normalize();
            let far = eye + dh * 1.0e6;
            let clip = view_proj * far.extend(1.0);
            if clip.w.abs() < 1e-9 {
                if fwd.z < 0.0 { 2.0 } else { -2.0 }
            } else {
                clip.y / clip.w
            }
        };
        let h_y = (rect.min.y + ((1.0 - ndc_y) * 0.5) as f32 * rect.height()).clamp(rect.min.y, rect.max.y);
        painter.rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, h_y)),
            0.0,
            sky_c,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(rect.min.x, h_y), rect.max),
            0.0,
            gnd_c,
        );
        if h_y > rect.min.y && h_y < rect.max.y {
            painter.line_segment(
                [egui::pos2(rect.min.x, h_y), egui::pos2(rect.max.x, h_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 185, 190)),
            );
        }
    } else {
        let bg_c = egui::Color32::from_rgba_premultiplied(
            (style.background_color[0] * 255.0) as u8,
            (style.background_color[1] * 255.0) as u8,
            (style.background_color[2] * 255.0) as u8,
            255,
        );
        painter.rect_filled(rect, 0.0, bg_c);
    }

    // 2. Render Ground Grid & Axes
    render_grid_and_axes(camera, painter, rect, &view_proj, style);

    // 3. Unified depth-sorted rendering (triangles + edges)
    // Front faces occlude back edges, exactly matching hardware depth testing!
    enum RenderItem {
        Triangle {
            pts: [egui::Pos2; 3],
            depth: f64,
            color: egui::Color32,
        },
        TexturedTriangle {
            pts: [egui::Pos2; 3],
            uvs: [egui::Pos2; 3],
            depth: f64,
            color: egui::Color32,
            tex_id: egui::TextureId,
        },
        Segment {
            s0: egui::Pos2,
            s1: egui::Pos2,
            depth: f64,
            color: egui::Color32,
            width: f32,
            style: LineStyle,
        },
    }

    let mut render_items: Vec<RenderItem> = Vec::new();

    fn collect_mesh_into(
        buf: &MeshRenderBuffer,
        view_proj: &DMat4,
        rect: egui::Rect,
        render_items: &mut Vec<RenderItem>,
    ) {
        for chunk in buf.indices.chunks_exact(3) {
            let v0 = &buf.vertices[chunk[0] as usize];
            let v1 = &buf.vertices[chunk[1] as usize];
            let v2 = &buf.vertices[chunk[2] as usize];

            let p0 = DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
            let p1 = DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
            let p2 = DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);

            if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                project_point(p0, view_proj, rect),
                project_point(p1, view_proj, rect),
                project_point(p2, view_proj, rect),
            ) {
                let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                // In screen coords (Y down), front-facing CCW triangles have cross < -0.25.
                // For opaque meshes (v0.color[3] >= 0.99), cull back faces (cross >= -0.25).
                // For transparent meshes, keep both sides but skip degenerate edges (|cross| < 0.25).
                if v0.color[3] >= 0.99 {
                    if cross >= -0.25 {
                        continue;
                    }
                } else if cross.abs() < 0.25 {
                    continue;
                }

                let depth = (d0 + d1 + d2) / 3.0;
                let c = egui::Color32::from_rgba_unmultiplied(
                    (v0.color[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (v0.color[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (v0.color[2] * 255.0).clamp(0.0, 255.0) as u8,
                    (v0.color[3] * 255.0).clamp(0.0, 255.0) as u8,
                );
                render_items.push(RenderItem::Triangle {
                    pts: [s0, s1, s2],
                    depth,
                    color: c,
                });
            }
        }
    }

    fn collect_textured_mesh_into(
        tex_name: &str,
        buf: &MeshRenderBuffer,
        view_proj: &DMat4,
        rect: egui::Rect,
        textures: &std::collections::HashMap<String, egui::TextureId>,
        render_items: &mut Vec<RenderItem>,
    ) {
        let tex_id_opt = textures.get(tex_name)
            .or_else(|| textures.get(tex_name.trim_start_matches("textures/")))
            .or_else(|| {
                if let Some(pos) = tex_name.rfind('/') {
                    textures.get(&tex_name[pos + 1..])
                } else {
                    None
                }
            })
            .or_else(|| {
                textures.iter().find(|(k, _)| k.contains(tex_name) || tex_name.contains(k.as_str())).map(|(_, id)| id)
            });

        for chunk in buf.indices.chunks_exact(3) {
            let v0 = &buf.vertices[chunk[0] as usize];
            let v1 = &buf.vertices[chunk[1] as usize];
            let v2 = &buf.vertices[chunk[2] as usize];

            let p0 = DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
            let p1 = DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
            let p2 = DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);

            if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                project_point(p0, view_proj, rect),
                project_point(p1, view_proj, rect),
                project_point(p2, view_proj, rect),
            ) {
                let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                if v0.color[3] >= 0.99 {
                    if cross >= -0.25 {
                        continue;
                    }
                } else if cross.abs() < 0.25 {
                    continue;
                }

                let depth = (d0 + d1 + d2) / 3.0;
                let c = egui::Color32::from_rgba_unmultiplied(
                    (v0.color[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (v0.color[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (v0.color[2] * 255.0).clamp(0.0, 255.0) as u8,
                    (v0.color[3] * 255.0).clamp(0.0, 255.0) as u8,
                );

                if let Some(&tex_id) = tex_id_opt {
                    render_items.push(RenderItem::TexturedTriangle {
                        pts: [s0, s1, s2],
                        uvs: [egui::pos2(v0.uv[0], v0.uv[1]), egui::pos2(v1.uv[0], v1.uv[1]), egui::pos2(v2.uv[0], v2.uv[1])],
                        depth,
                        color: c,
                        tex_id,
                    });
                } else {
                    render_items.push(RenderItem::Triangle {
                        pts: [s0, s1, s2],
                        depth,
                        color: c,
                    });
                }
            }
        }
    }

    fn collect_lines_into(
        buf: &LineRenderBuffer,
        depth_bias: f64,
        view_proj: &DMat4,
        rect: egui::Rect,
        render_items: &mut Vec<RenderItem>,
    ) {
        for seg in &buf.segments {
            let p0 = DVec3::new(seg.v0.position[0] as f64, seg.v0.position[1] as f64, seg.v0.position[2] as f64);
            let p1 = DVec3::new(seg.v1.position[0] as f64, seg.v1.position[1] as f64, seg.v1.position[2] as f64);

            if let Some(((s0, d0), (s1, d1))) = project_segment(p0, p1, view_proj, rect) {
                let depth = ((d0 + d1) * 0.5) - depth_bias;
                let color = egui::Color32::from_rgba_unmultiplied(
                    (seg.v0.color[0] * 255.0) as u8,
                    (seg.v0.color[1] * 255.0) as u8,
                    (seg.v0.color[2] * 255.0) as u8,
                    (seg.v0.color[3] * 255.0) as u8,
                );
                render_items.push(RenderItem::Segment {
                    s0,
                    s1,
                    depth,
                    color,
                    width: seg.width,
                    style: seg.style,
                });
            }
        }
    }

    if style.visual_style != VisualStyle::Wireframe {
        collect_mesh_into(&data.opaque_mesh, &view_proj, rect, &mut render_items);
        collect_mesh_into(&data.transparent_mesh, &view_proj, rect, &mut render_items);
        collect_mesh_into(&data.selection.overlay_mesh, &view_proj, rect, &mut render_items);
        for (tex_name, buf) in &data.textured_meshes {
            collect_textured_mesh_into(tex_name, buf, &view_proj, rect, textures, &mut render_items);
        }
    }

    if let Some(section)=&data.section_stencil {
        let width=rect.width().ceil().max(1.)as usize;let height=rect.height().ceil().max(1.)as usize;
        let mask=section.rasterize(camera,width,height);
        let color=egui::Color32::from_rgb((section.color[0]*255.)as u8,(section.color[1]*255.)as u8,(section.color[2]*255.)as u8);
        for y in 0..height {let mut x=0;while x<width{if !mask[y*width+x]{x+=1;continue;}let start=x;while x<width&&mask[y*width+x]{x+=1;}let points=[egui::pos2(rect.left()+start as f32,rect.top()+y as f32),egui::pos2(rect.left()+x as f32,rect.top()+y as f32),egui::pos2(rect.left()+x as f32,rect.top()+y as f32+1.),egui::pos2(rect.left()+start as f32,rect.top()+y as f32+1.)];let ray=camera.screen_to_ray((start+x)as f64/2.,y as f64+0.5,width as f64,height as f64);let denominator=ray.dir.dot(section.plane.normal);if denominator.abs()<1e-12{continue;}let t=(section.plane.point-ray.origin).dot(section.plane.normal)/denominator;let point=ray.origin+ray.dir*t;let clip=view_proj*point.extend(1.);let depth=clip.z/clip.w;render_items.push(RenderItem::Triangle{pts:[points[0],points[1],points[2]],depth,color});render_items.push(RenderItem::Triangle{pts:[points[0],points[2],points[3]],depth,color});}}
    }

    collect_lines_into(&data.visible_edges, 0.00003, &view_proj, rect, &mut render_items);
    collect_lines_into(&data.guides, 0.00003, &view_proj, rect, &mut render_items);
    collect_lines_into(&data.selection.outline_lines, 0.00005, &view_proj, rect, &mut render_items);
    collect_lines_into(&data.dimension_lines, 0.00005, &view_proj, rect, &mut render_items);

    // Billboards / Face-me figures
    let eye = camera.eye();
    for bb in &data.billboards {
        let mut d = eye - bb.anchor;
        d.z = 0.0;
        let d = if d.length_squared() > 1e-6 { d.normalize() } else { DVec3::X };
        let r = DVec3::new(-d.y, d.x, 0.0);
        let up = DVec3::Z;
        let c0 = bb.anchor - r * (bb.width * 0.5); // bottom-left
        let c1 = bb.anchor + r * (bb.width * 0.5); // bottom-right
        let c2 = c1 + up * bb.height;              // top-right
        let c3 = c0 + up * bb.height;              // top-left

        if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2)), Some((s3, d3))) = (
            project_point(c0, &view_proj, rect),
            project_point(c1, &view_proj, rect),
            project_point(c2, &view_proj, rect),
            project_point(c3, &view_proj, rect),
        ) {
            let depth = (d0 + d1 + d2 + d3) * 0.25;
            let tex_id_opt = textures.get(&bb.texture_name)
                .or_else(|| textures.get(bb.texture_name.trim_start_matches("textures/")));
            if let Some(&tex_id) = tex_id_opt {
                render_items.push(RenderItem::TexturedTriangle {
                    pts: [s3, s0, s1],
                    uvs: [egui::pos2(0.0, 0.0), egui::pos2(0.0, 1.0), egui::pos2(1.0, 1.0)],
                    depth,
                    color: egui::Color32::WHITE,
                    tex_id,
                });
                render_items.push(RenderItem::TexturedTriangle {
                    pts: [s3, s1, s2],
                    uvs: [egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0), egui::pos2(1.0, 0.0)],
                    depth,
                    color: egui::Color32::WHITE,
                    tex_id,
                });
            }
        }
    }

    // Sort furthest first (Painter's algorithm)
    render_items.sort_by(|a, b| {
        let da = match a {
            RenderItem::Triangle { depth, .. } => *depth,
            RenderItem::TexturedTriangle { depth, .. } => *depth,
            RenderItem::Segment { depth, .. } => *depth,
        };
        let db = match b {
            RenderItem::Triangle { depth, .. } => *depth,
            RenderItem::TexturedTriangle { depth, .. } => *depth,
            RenderItem::Segment { depth, .. } => *depth,
        };
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });

    for item in render_items {
        match item {
            RenderItem::Triangle { pts, color, .. } => {
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(pts[0], color);
                mesh.colored_vertex(pts[1], color);
                mesh.colored_vertex(pts[2], color);
                mesh.add_triangle(0, 1, 2);
                painter.add(egui::Shape::mesh(mesh));
            }
            RenderItem::TexturedTriangle { pts, uvs, depth: _, color, tex_id } => {
                let mut mesh = egui::Mesh::with_texture(tex_id);
                for (pos, uv) in pts.into_iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex { pos, uv, color });
                }
                mesh.add_triangle(0, 1, 2);
                painter.add(egui::Shape::mesh(mesh));
            }
            RenderItem::Segment { s0, s1, color, width, style, .. } => {
                match style {
                    LineStyle::Solid => {
                        painter.line_segment([s0, s1], egui::Stroke::new(width, color));
                    }
                    LineStyle::HiddenDashed | LineStyle::Dashed { .. } | LineStyle::GuideDotted => {
                        let dir = s1 - s0;
                        let len = dir.length();
                        if len > 2.0 {
                            let step = 8.0;
                            let num_steps = (len / step).max(1.0) as usize;
                            let norm_dir = dir / len;
                            for i in (0..num_steps).step_by(2) {
                                let start = s0 + norm_dir * (i as f32 * step);
                                let end = s0 + norm_dir * (((i + 1) as f32 * step).min(len));
                                painter.line_segment([start, end], egui::Stroke::new(width, color));
                            }
                        }
                    }
                }
            }
        }
    }

    // Dimension labels
    for (pos3d, text) in &data.dimension_labels {
        if let Some((pos2d, _)) = project_point(*pos3d, &view_proj, rect) {
            let label_rect = egui::Rect::from_center_size(pos2d, egui::vec2(56.0, 18.0));
            painter.rect_filled(
                label_rect,
                2.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230),
            );
            painter.rect_stroke(
                label_rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 190)),
            );
            painter.text(
                pos2d,
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(15, 20, 30),
            );
        }
    }

    // 7b. Text labels (leader annotations)
    for (p_anchor_3d, p_pos_3d, text) in &data.text_labels {
        let s_anchor_opt = project_point(*p_anchor_3d, &view_proj, rect);
        let s_pos_opt = project_point(*p_pos_3d, &view_proj, rect);

        if let (Some((p_anchor, _)), Some((p_pos, _))) = (s_anchor_opt, s_pos_opt) {
            // Anchor dot
            painter.circle_filled(p_anchor, 2.5, egui::Color32::from_rgb(45, 55, 75));

            // Leader line
            painter.line_segment([p_anchor, p_pos], egui::Stroke::new(1.2, egui::Color32::from_rgb(45, 55, 75)));

            // Text block placement: away from anchor
            let lines: Vec<&str> = text.lines().collect();
            let font_id = egui::FontId::proportional(11.0);
            let line_height = 13.0;

            let align = if p_pos.x < p_anchor.x {
                egui::Align2::RIGHT_TOP
            } else {
                egui::Align2::LEFT_TOP
            };

            let offset_x = if p_pos.x < p_anchor.x { -6.0 } else { 6.0 };
            let start_pt = egui::pos2(p_pos.x + offset_x, p_pos.y - 6.0);

            for (i, line) in lines.iter().enumerate() {
                let pt = egui::pos2(start_pt.x, start_pt.y + i as f32 * line_height);
                // Subtle white outline/halo
                painter.text(
                    egui::pos2(pt.x + 1.0, pt.y + 1.0),
                    align,
                    line,
                    font_id.clone(),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
                // Dark ink text
                painter.text(
                    pt,
                    align,
                    line,
                    font_id.clone(),
                    egui::Color32::from_rgb(35, 45, 60),
                );
            }
        }
    }

    // 8. Render Selection Vertices
    for pt in &data.selection.vertex_points.points {
        let p3d = DVec3::new(pt.position[0] as f64, pt.position[1] as f64, pt.position[2] as f64);
        if let Some((pos, _)) = project_point(p3d, &view_proj, rect) {
            let color = egui::Color32::from_rgba_premultiplied(
                (pt.color[0] * 255.0) as u8,
                (pt.color[1] * 255.0) as u8,
                (pt.color[2] * 255.0) as u8,
                255,
            );
            painter.circle_filled(pos, pt.size * 0.5, color);
        }
    }

    // 9. Render Snap Glyphs and Axis Rays
    for glyph in &data.snap_glyphs {
        // Draw axis line if present
        if let Some((p0, p1, col)) = glyph.axis_line {
            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(p0, &view_proj, rect),
                project_point(p1, &view_proj, rect),
            ) {
                let stroke_color = egui::Color32::from_rgba_unmultiplied(
                    (col[0] * 255.0) as u8,
                    (col[1] * 255.0) as u8,
                    (col[2] * 255.0) as u8,
                    (col[3] * 255.0) as u8,
                );
                painter.line_segment([s0, s1], egui::Stroke::new(1.5, stroke_color));
            }
        }

        // Draw dashed tracking guides (memory points / references)
        for (p0, p1, col) in &glyph.guides {
            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(*p0, &view_proj, rect),
                project_point(*p1, &view_proj, rect),
            ) {
                let stroke_color = egui::Color32::from_rgba_unmultiplied(
                    (col[0] * 255.0) as u8,
                    (col[1] * 255.0) as u8,
                    (col[2] * 255.0) as u8,
                    (col[3] * 255.0) as u8,
                );
                let dir = s1 - s0;
                let len = dir.length();
                if len > 2.0 {
                    let step = 8.0;
                    let num_steps = (len / step).max(1.0) as usize;
                    let norm_dir = dir / len;
                    for i in (0..num_steps).step_by(2) {
                        let start = s0 + norm_dir * (i as f32 * step);
                        let end = s0 + norm_dir * (((i + 1) as f32 * step).min(len));
                        painter.line_segment([start, end], egui::Stroke::new(1.5, stroke_color));
                    }
                }
            }
        }

        // Draw glyph shape
        if let Some((pos, _)) = project_point(glyph.point, &view_proj, rect) {
            let glyph_color = egui::Color32::from_rgba_premultiplied(
                (glyph.color[0] * 255.0) as u8,
                (glyph.color[1] * 255.0) as u8,
                (glyph.color[2] * 255.0) as u8,
                (glyph.color[3] * 255.0) as u8,
            );
            let s = glyph.size_px * 0.5;

            match glyph.shape {
                crate::snap_glyph::GlyphShape::Square => {
                    painter.rect_filled(
                        egui::Rect::from_center_size(pos, egui::vec2(s * 2.0, s * 2.0)),
                        0.0,
                        glyph_color,
                    );
                    painter.rect_stroke(
                        egui::Rect::from_center_size(pos, egui::vec2(s * 2.0, s * 2.0)),
                        0.0,
                        egui::Stroke::new(1.0, egui::Color32::BLACK),
                    );
                }
                crate::snap_glyph::GlyphShape::Circle => {
                    painter.circle_filled(pos, s, glyph_color);
                    painter.circle_stroke(pos, s, egui::Stroke::new(1.0, egui::Color32::BLACK));
                }
                crate::snap_glyph::GlyphShape::Diamond => {
                    let pts = vec![
                        pos + egui::vec2(0.0, -s),
                        pos + egui::vec2(s, 0.0),
                        pos + egui::vec2(0.0, s),
                        pos + egui::vec2(-s, 0.0),
                    ];
                    painter.add(egui::Shape::convex_polygon(pts, glyph_color, egui::Stroke::new(1.0, egui::Color32::BLACK)));
                }
                crate::snap_glyph::GlyphShape::Cross | crate::snap_glyph::GlyphShape::Target => {
                    painter.line_segment([pos - egui::vec2(s, 0.0), pos + egui::vec2(s, 0.0)], egui::Stroke::new(2.0, glyph_color));
                    painter.line_segment([pos - egui::vec2(0.0, s), pos + egui::vec2(0.0, s)], egui::Stroke::new(2.0, glyph_color));
                }
                crate::snap_glyph::GlyphShape::Triangle => {
                    let pts = vec![
                        pos + egui::vec2(0.0, -s),
                        pos + egui::vec2(s, s),
                        pos + egui::vec2(-s, s),
                    ];
                    painter.add(egui::Shape::convex_polygon(pts, glyph_color, egui::Stroke::new(1.0, egui::Color32::BLACK)));
                }
            }

            // Draw label tooltip
            if !glyph.label.is_empty() {
                let text_pos = pos + egui::vec2(10.0, -10.0);
                let label_text = ingetrazo_core::i18n::tr(&glyph.label);
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_BOTTOM,
                    label_text,
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgb(20, 20, 20),
                );
            }
        }
    }
}

#[allow(dead_code)]
fn render_line_buffer(
    buffer: &LineRenderBuffer,
    view_proj: &DMat4,
    painter: &egui::Painter,
    rect: egui::Rect,
) {
    for seg in &buffer.segments {
        let p0 = DVec3::new(seg.v0.position[0] as f64, seg.v0.position[1] as f64, seg.v0.position[2] as f64);
        let p1 = DVec3::new(seg.v1.position[0] as f64, seg.v1.position[1] as f64, seg.v1.position[2] as f64);

        if let (Some((s0, _)), Some((s1, _))) = (
            project_point(p0, view_proj, rect),
            project_point(p1, view_proj, rect),
        ) {
            let color = egui::Color32::from_rgba_unmultiplied(
                (seg.v0.color[0] * 255.0) as u8,
                (seg.v0.color[1] * 255.0) as u8,
                (seg.v0.color[2] * 255.0) as u8,
                (seg.v0.color[3] * 255.0) as u8,
            );

            match seg.style {
                LineStyle::Solid => {
                    painter.line_segment([s0, s1], egui::Stroke::new(seg.width, color));
                }
                LineStyle::HiddenDashed | LineStyle::Dashed { .. } | LineStyle::GuideDotted => {
                    // Draw dashed segments
                    let dir = s1 - s0;
                    let len = dir.length();
                    if len > 2.0 {
                        let step = 8.0;
                        let num_steps = (len / step).max(1.0) as usize;
                        let norm_dir = dir / len;
                        for i in (0..num_steps).step_by(2) {
                            let start = s0 + norm_dir * (i as f32 * step);
                            let end = s0 + norm_dir * (((i + 1) as f32 * step).min(len));
                            painter.line_segment([start, end], egui::Stroke::new(seg.width, color));
                        }
                    }
                }
            }
        }
    }
}

fn render_grid_and_axes(
    _camera: &OrbitCamera,
    painter: &egui::Painter,
    rect: egui::Rect,
    view_proj: &DMat4,
    style: &StyleConfig,
) {
    if style.show_grid {
        let grid_size = 12;
        let step = 1.0_f64;
        let grid_color = egui::Color32::from_rgba_unmultiplied(185, 190, 195, 80);

        // Ground plane grid (Z = 0)
        for i in -grid_size..=grid_size {
            let coord = i as f64 * step;
            let p0_x = DVec3::new(-grid_size as f64 * step, coord, 0.0);
            let p1_x = DVec3::new(grid_size as f64 * step, coord, 0.0);
            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(p0_x, view_proj, rect),
                project_point(p1_x, view_proj, rect),
            ) {
                painter.line_segment([s0, s1], egui::Stroke::new(0.7, grid_color));
            }

            let p0_y = DVec3::new(coord, -grid_size as f64 * step, 0.0);
            let p1_y = DVec3::new(coord, grid_size as f64 * step, 0.0);
            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(p0_y, view_proj, rect),
                project_point(p1_y, view_proj, rect),
            ) {
                painter.line_segment([s0, s1], egui::Stroke::new(0.7, grid_color));
            }
        }
    }

    // World XYZ drawing axes (Python 1:1 match)
    // Red: (0.86, 0.22, 0.27), Green: (0.16, 0.62, 0.36), Blue: (0.20, 0.40, 0.78)
    let axes_spec = [
        (DVec3::X, egui::Color32::from_rgb(220, 56, 68)),   // Red: X
        (DVec3::Y, egui::Color32::from_rgb(41, 158, 92)),   // Green: Y
        (DVec3::Z, egui::Color32::from_rgb(51, 102, 199)),  // Blue: Z
    ];

    let pos_reach = 1000.0_f64;
    let neg_reach = 500.0_f64;
    let origin = DVec3::ZERO;

    for (dir, col) in axes_spec {
        // Positive axis: solid line from origin to pos_reach
        let p_pos = origin + dir * pos_reach;
        if let Some(((s_orig, _), (s_pos, _))) = project_segment(origin, p_pos, view_proj, rect) {
            painter.line_segment([s_orig, s_pos], egui::Stroke::new(1.8, col));
        }

        // Negative axis: dashed line from -neg_reach to origin
        let p_neg = origin - dir * neg_reach;
        if let Some(((s_neg, _), (s_orig, _))) = project_segment(p_neg, origin, view_proj, rect) {
            let seg_dir = s_orig - s_neg;
            let len = seg_dir.length();
            if len > 2.0 {
                let step = 8.0;
                let num_steps = (len / step).max(1.0) as usize;
                let norm = seg_dir / len;
                for i in (0..num_steps).step_by(2) {
                    let a = s_neg + norm * (i as f32 * step);
                    let b = s_neg + norm * (((i + 1) as f32 * step).min(len));
                    painter.line_segment([a, b], egui::Stroke::new(1.2, col));
                }
            }
        }
    }
}


#[cfg(test)]mod native_surface_regression {
 use super::*;
 #[test]fn surface_upload_keeps_nested_transform_and_hidden_visibility_without_hlr(){
  let mut scene=Scene::new();let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]);
  let mut child=ingetrazo_core::groups::Group::with_mesh("leaf",mesh);child.transform=DMat4::from_translation(DVec3::Y*2.);
  let mut parent=ingetrazo_core::groups::Group::new("parent");parent.transform=DMat4::from_translation(DVec3::X*3.);parent.children.push(child);scene.groups.push(parent);
  let camera=OrbitCamera::default();let style=StyleConfig::default();let visible=build_world_surfaces(&scene,&camera,&style);
  assert_eq!(visible.opaque_mesh.indices.len(),3);assert!(visible.opaque_mesh.vertices.iter().all(|v|v.position[0]>=3.&&v.position[1]>=2.));assert!(visible.visible_edges.vertices.is_empty());assert!(visible.hidden_edges.vertices.is_empty());
  scene.groups[0].hidden=true;assert!(build_world_surfaces(&scene,&camera,&style).opaque_mesh.indices.is_empty());
 }
 #[test]fn prototype_shade_is_retained_but_classic_baked_geometry_shades_its_own_plane(){
  let mut scene=Scene::new();let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));let f=mesh.add_face(ids.to_vec(),vec![]).unwrap();mesh.faces[f].attrs.insert("color".into(),serde_json::json!([1.,0.,0.]));mesh.faces[f].attrs.insert("opacity".into(),serde_json::json!(0.5));
  let mut group=ingetrazo_core::groups::Group::with_mesh("prototype",mesh);group.transform=DMat4::from_rotation_y(-std::f64::consts::FRAC_PI_4);group.native_document=serde_json::json!({"proto":0,"xform":group.transform.to_cols_array()});scene.groups.push(group);
  let camera=OrbitCamera::default();let style=StyleConfig::default();let v=build_world_surfaces(&scene,&camera,&style);assert_eq!(v.transparent_mesh.vertices[0].color[0],shade_factor(DVec3::Z));
  let group=&mut scene.groups[0];for vertex in &mut group.mesh.vertices{vertex.position=group.transform.transform_point3(vertex.position);}group.transform=DMat4::IDENTITY;group.native_document=serde_json::json!({"faces":[]});
  let v=build_world_surfaces(&scene,&camera,&style);assert_eq!(v.transparent_mesh.vertices[0].color[0],shade_factor(DVec3::new(-1.,0.,1.)));assert!(v.transparent_mesh.vertices[0].color[0]<shade_factor(DVec3::Z));
 }

 #[test]fn reflected_nested_placements_preserve_painted_front_and_uv_correspondence(){
  for textured in [false,true] {for double_reflection in [false,true] {
   let mut scene=Scene::new();let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));let face=mesh.add_face(ids.to_vec(),vec![]).unwrap();
   mesh.faces[face].attrs.insert("back".into(),serde_json::json!({"color":[0.,0.,1.],"opacity":0.5}));
   if textured {mesh.faces[face].attrs.insert("texture".into(),serde_json::json!({"path":"test.png","uvw":[1.,0.,0.,0.,0.,1.,0.,0.]}));}
   let mut child=ingetrazo_core::groups::Group::with_mesh("leaf",mesh);child.transform=DMat4::from_scale(DVec3::new(-2.,3.,1.));
   let mut parent=ingetrazo_core::groups::Group::new("parent");if double_reflection {parent.transform=DMat4::from_scale(DVec3::new(-1.,1.,1.));}parent.children.push(child);scene.groups.push(parent);
   let mut style=StyleConfig::default();style.show_textures=textured;style.visual_style=VisualStyle::ShadedWithTextures;
   let data=build_world_surfaces(&scene,&OrbitCamera::default(),&style);
   let buf=if textured {data.textured_meshes.get("test.png").expect("textured placement")}else {&data.opaque_mesh};
   assert_eq!(buf.indices.len(),3);let pts:Vec<_>=buf.indices.iter().map(|&i|{let p=buf.vertices[i as usize].position;DVec3::new(p[0]as f64,p[1]as f64,p[2]as f64)}).collect();
   assert!((pts[1]-pts[0]).cross(pts[2]-pts[0]).z>0.,"reflection must retain front paint");
   if textured {for v in &buf.vertices {assert_eq!(v.uv,[v.position[0],v.position[1]],"corner reorder must retain position/UV pairing");}}
  }}
 }

}

// Original viewport _bucket_back_face: explicit back paint only; opacity<.999
// makes the front copy front-only. Texture-backed runs remain a separate GPU gate.
fn record_source_back_paint(buffer:&mut MeshRenderBuffer,first:usize,order:usize,attrs:&std::collections::HashMap<String,serde_json::Value>,shade:f32){
 let count=buffer.indices.len()-first;if count==0{return;}let Some(back)=attrs.get("back").and_then(|v|v.as_object())else{return;};
 let opacity=back.get("opacity").and_then(|v|v.as_f64()).unwrap_or(1.);
 let color=back.get("color").and_then(|v|v.as_array()).filter(|c|c.len()>=3);
 let texture=back.get("texture").and_then(|v|v.get("path")).and_then(|v|v.as_str()).is_some();
 if color.is_some()||texture{buffer.source_back_override_count+=1;}
 if (color.is_some()||texture)&&opacity<0.999 {buffer.source_front_cull.push((first,count));}
 if let Some(c)=color {let color=[0,1,2].map(|i|(c[i].as_f64().unwrap_or(0.)as f32*shade).clamp(0.,1.));buffer.source_back_paints.push(crate::buffer::SourceBackPaint{first,count,order,color,opacity});}
}
fn source_instancing_disabled()->bool {static DISABLED:std::sync::OnceLock<bool>=std::sync::OnceLock::new();*DISABLED.get_or_init(||std::env::var("INGETRAZO_NO_INSTANCING").as_deref()==Ok("1"))}

// Original formats.igz._group_from applies xform only to a numeric proto
// reference. Legacy inline groups remain classic even if a component flag is
// present. Group.adopt promotes containers with children to placements.
fn source_group_has_placement(group:&ingetrazo_core::groups::Group)->bool {
 if group.native_document.is_object() {
  group.native_document.get("proto").and_then(|p|p.as_u64()).is_some()
   &&group.native_document.get("xform").is_some_and(|x|x.is_array())
   || !group.children.is_empty()
 } else {group.component_definition.is_some()||!group.children.is_empty()}
}
#[cfg(test)]mod source_placement_regression {
 use super::*;
 use ingetrazo_core::groups::Group;
 #[test]fn classic_legacy_and_component_flags_do_not_imply_instancing(){
  let mut g=Group::new("legacy");
  g.native_document=serde_json::json!({"component":true,"proto":null,"xform":DMat4::IDENTITY.to_cols_array()});assert!(!source_group_has_placement(&g));
  g.native_document=serde_json::json!({"component":true,"edges":[],"faces":[]});assert!(!source_group_has_placement(&g));
  g.native_document=serde_json::json!({"proto":0});assert!(!source_group_has_placement(&g));
  g.native_document=serde_json::json!({"proto":0,"xform":DMat4::IDENTITY.to_cols_array(),"component":false});assert!(source_group_has_placement(&g));
  g.native_document=serde_json::json!({"faces":[]});g.children.push(Group::new("nested"));assert!(source_group_has_placement(&g));
 }
}
