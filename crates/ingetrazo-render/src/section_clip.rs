//! Display-only section clipping; interpolated vertices retain texture coordinates.
use crate::{
    buffer::{LineRenderBuffer, MeshRenderBuffer, Vertex},
    pipeline::SceneRenderData,
};
use glam::DVec3;
use ingetrazo_core::{scene::Scene, section::SectionPlane};
fn position(vertex: &Vertex) -> DVec3 {
    DVec3::new(
        vertex.position[0] as f64,
        vertex.position[1] as f64,
        vertex.position[2] as f64,
    )
}
fn interpolate(a: Vertex, b: Vertex, t: f64) -> Vertex {
    let t = t as f32;
    let mut result = a;
    for i in 0..3 {
        result.position[i] = a.position[i] + (b.position[i] - a.position[i]) * t;
        result.normal[i] = a.normal[i] + (b.normal[i] - a.normal[i]) * t;
    }
    for i in 0..4 {
        result.color[i] = a.color[i] + (b.color[i] - a.color[i]) * t;
    }
    for i in 0..2 {
        result.uv[i] = a.uv[i] + (b.uv[i] - a.uv[i]) * t;
    }
    result
}
pub(crate) fn mesh(buffer: &mut MeshRenderBuffer, plane: &SectionPlane) {
    let old = std::mem::take(buffer);
    for triangle in old.indices.chunks_exact(3) {
        let vertices = triangle
            .iter()
            .map(|&i| old.vertices[i as usize])
            .collect::<Vec<_>>();
        let mut polygon = vec![];
        for i in 0..3 {
            let a = vertices[i];
            let b = vertices[(i + 1) % 3];
            let da = plane.signed_distance(position(&a));
            let db = plane.signed_distance(position(&b));
            if da <= 1e-7 {
                polygon.push(a);
            }
            if (da > 1e-7) != (db > 1e-7) {
                polygon.push(interpolate(a, b, da / (da - db)));
            }
        }
        if polygon.len() >= 3 {
            for i in 1..polygon.len() - 1 {
                let base = buffer.vertices.len() as u32;
                buffer
                    .vertices
                    .extend([polygon[0], polygon[i], polygon[i + 1]]);
                buffer.indices.extend([base, base + 1, base + 2]);
            }
        }
    }
}
fn lines(buffer: &mut LineRenderBuffer, plane: &SectionPlane) {
    let old = std::mem::take(buffer);
    for mut segment in old.segments {
        let a = DVec3::from_array(segment.v0.position.map(f64::from));
        let b = DVec3::from_array(segment.v1.position.map(f64::from));
        let da = plane.signed_distance(a);
        let db = plane.signed_distance(b);
        if da > 1e-7 && db > 1e-7 {
            continue;
        }
        if (da > 1e-7) != (db > 1e-7) {
            let t = da / (da - db);
            let p = (a + (b - a) * t).as_vec3().to_array();
            let mut color = segment.v0.color;
            for (i, c) in color.iter_mut().enumerate() {
                *c = segment.v0.color[i] + (segment.v1.color[i] - segment.v0.color[i]) * t as f32;
            }
            if da > 1e-7 {
                segment.v0.position = p;
                segment.v0.color = color;
            } else {
                segment.v1.position = p;
                segment.v1.color = color;
            }
        }
        buffer.push_segment(segment);
    }
}
pub(crate) fn cap_segments(
    buffers: impl Iterator<Item = MeshRenderBuffer>,
    plane: &SectionPlane,
) -> Vec<(DVec3, DVec3)> {
    let mut segments = vec![];
    for buffer in buffers {
        for triangle in buffer.indices.chunks_exact(3) {
            let points = [triangle[0], triangle[1], triangle[2]]
                .map(|i| position(&buffer.vertices[i as usize]));
            let distances = points.map(|p| plane.signed_distance(p));
            if distances.iter().all(|d| d.abs() < 1e-7) {
                continue;
            }
            let mut hits = vec![];
            for i in 0..3 {
                let j = (i + 1) % 3;
                if distances[i].abs() < 1e-7 {
                    hits.push(points[i]);
                }
                if distances[i] * distances[j] < -1e-14 {
                    hits.push(
                        points[i]
                            + (points[j] - points[i])
                                * (distances[i] / (distances[i] - distances[j])),
                    );
                }
            }
            let mut unique = vec![];
            for point in hits {
                if !unique.iter().any(|p: &DVec3| (*p - point).length() < 1e-7) {
                    unique.push(point);
                }
            }
            if unique.len() == 2 {
                let direction = (points[1] - points[0])
                    .cross(points[2] - points[0])
                    .cross(plane.normal);
                if (unique[1] - unique[0]).dot(direction) < 0. {
                    unique.reverse();
                }
                segments.push((unique[0], unique[1]));
            }
        }
    }
    segments
}
// Contours use source surfaces even when the current style renders no faces.
pub(crate) fn contour_geometry(scene: &Scene) -> MeshRenderBuffer {
    fn add(
        out: &mut MeshRenderBuffer,
        scene: &Scene,
        mesh: &ingetrazo_core::mesh::Mesh,
        transform: glam::DMat4,
    ) {
        for id in 0..mesh.faces.len() {
            if !scene.mesh_face_visible(mesh, id) {
                continue;
            }
            for (a, b, c) in mesh.triangulate_face(id) {
                let base = out.vertices.len() as u32;
                for point in [a, b, c] {
                    out.vertices.push(Vertex::from_dvec3(
                        transform.transform_point3(point),
                        DVec3::ZERO,
                        [1.; 4],
                        [0.; 2],
                    ));
                }
                out.indices.extend([base, base + 1, base + 2]);
            }
        }
    }
    fn group(
        out: &mut MeshRenderBuffer,
        scene: &Scene,
        item: &ingetrazo_core::groups::Group,
        parent: glam::DMat4,
    ) {
        if item.hidden || !scene.is_layer_visible(item.layer.as_deref()) {
            return;
        }
        let transform = parent * item.transform;
        add(out, scene, &item.mesh, transform);
        for child in &item.children {
            group(out, scene, child, transform);
        }
    }
    let mut result = MeshRenderBuffer::new();
    add(&mut result, scene, &scene.mesh, glam::DMat4::IDENTITY);
    for item in &scene.groups {
        group(&mut result, scene, item, glam::DMat4::IDENTITY);
    }
    result
}
pub(crate) fn clip_scene(
    data: &mut SceneRenderData,
    scene: &Scene,
    style: &crate::style::StyleConfig,
) {
    if !scene.show_section_cuts {
        return;
    }
    let Some(plane) = scene.section_planes.iter().find(|plane| plane.active) else {
        return;
    };
    if scene.show_section_fill
        && !matches!(
            style.visual_style,
            crate::style::VisualStyle::Wireframe | crate::style::VisualStyle::XRay
        )
    {
        data.section_stencil = Some(SectionStencil {
            plane: plane.clone(),
            frame: scene
                .section_frame(
                    scene
                        .section_planes
                        .iter()
                        .position(|p| p.id == plane.id)
                        .unwrap(),
                )
                .unwrap(),
            color: style.section_fill_color,
            surfaces: std::iter::once(data.opaque_mesh.clone())
                .chain(data.textured_meshes.values().cloned())
                .collect(),
        });
    }
    mesh(&mut data.opaque_mesh, plane);
    mesh(&mut data.transparent_mesh, plane);
    for buffer in data.textured_meshes.values_mut() {
        mesh(buffer, plane);
    }
    mesh(&mut data.selection.overlay_mesh, plane);
    for (a, b) in cap_segments(std::iter::once(contour_geometry(scene)), plane) {
        data.visible_edges
            .push_line(a, b, style.edge_color, 2.5, crate::buffer::LineStyle::Solid);
    }
    for buffer in [
        &mut data.visible_edges,
        &mut data.hidden_edges,
        &mut data.guides,
        &mut data.selection.outline_lines,
        &mut data.dimension_lines,
    ] {
        lines(buffer, plane);
    }
    data.selection
        .vertex_points
        .points
        .retain(|point| !plane.is_clipped(DVec3::from_array(point.position.map(f64::from))));
    data.dimension_labels.retain(|(p, _)| !plane.is_clipped(*p));
    data.text_labels
        .retain(|(anchor, _, _)| !plane.is_clipped(*anchor));
    data.billboards
        .retain(|item| !plane.is_clipped(item.anchor));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contours_survive_styles_and_fill_toggle_without_mutating_geometry() {
        let mut scene = Scene::new();
        let ids = [
            DVec3::new(-1., -1., -1.),
            DVec3::new(1., -1., 1.),
            DVec3::new(-1., 1., -1.),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        scene.mesh.add_face(ids.to_vec(), vec![]);
        scene
            .section_planes
            .push(SectionPlane::new(DVec3::ZERO, DVec3::Z, "A", "A"));
        scene.show_section_cuts = true;
        scene.show_section_fill = false;
        let before = scene.clone();
        for visual in [
            crate::style::VisualStyle::Wireframe,
            crate::style::VisualStyle::XRay,
            crate::style::StyleConfig::default().visual_style,
        ] {
            let mut style = crate::style::StyleConfig::default();
            style.visual_style = visual;
            let data =
                crate::pipeline::build_scene_render_data(&scene, &Default::default(), &style, None);
            let cuts = data
                .visible_edges
                .segments
                .iter()
                .filter(|s| {
                    s.width == 2.5 && s.v0.position[2].abs() < 1e-7 && s.v1.position[2].abs() < 1e-7
                })
                .collect::<Vec<_>>();
            assert_eq!(cuts.len(), 1);
            for cut in cuts {
                assert!(cut.v0.position[2].abs() < 1e-7 && cut.v1.position[2].abs() < 1e-7);
            }
        }
        assert_eq!(scene, before);
        scene.mesh.faces[0]
            .attrs
            .insert("hidden".into(), serde_json::json!(true));
        assert!(contour_geometry(&scene).is_empty());
    }
    #[test]
    fn original_stencil_closed_partition_transparent_and_duplicate_controls() {
        let mut scene = Scene::new();
        let ids = [
            (0., 0., 0.),
            (1., 0., 0.),
            (1., 1., 0.),
            (0., 1., 0.),
            (0., 0., 2.),
            (1., 0., 2.),
            (1., 1., 2.),
            (0., 1., 2.),
        ]
        .map(|(x, y, z)| scene.mesh.add_vertex(DVec3::new(x, y, z)));
        for ring in [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ] {
            scene.mesh.add_face(ring.map(|i| ids[i]).to_vec(), vec![]);
        }
        scene.section_planes.push(SectionPlane::new(
            DVec3::new(0., 0., 1.5),
            DVec3::Z,
            "A",
            "A",
        ));
        scene.show_section_cuts = true;
        scene.show_section_fill = true;
        let mut camera = ingetrazo_core::camera::OrbitCamera::default();
        camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
        camera.perspective = false;
        camera.target = DVec3::new(0.5, 0.5, 1.5);
        camera.distance = 5.;
        camera.aspect = 800. / 600.;
        let style = crate::style::StyleConfig::default();
        let count = |scene: &Scene| {
            let data = crate::pipeline::build_scene_render_data(scene, &camera, &style, None);
            data.section_stencil
                .unwrap()
                .rasterize(&camera, 800, 600)
                .into_iter()
                .filter(|v| *v)
                .count()
        };
        let closed = count(&scene);
        assert_eq!(closed, 20880); // Polygon-offset side fragments extend one edge row; contours occlude that border.
        let mut coplanar = scene.clone();
        coplanar.mesh = ingetrazo_core::mesh::Mesh::new();
        let ids = [
            DVec3::new(0., 1., 1.5),
            DVec3::new(1., 1., 1.5),
            DVec3::new(1., 0., 1.5),
            DVec3::new(0., 0., 1.5),
        ]
        .map(|p| coplanar.mesh.add_vertex(p));
        coplanar.mesh.add_face(ids.to_vec(), vec![]);
        assert_eq!(
            count(&coplanar),
            0,
            "plane offset(2,2) must lose to coincident opaque surface offset(1,1)"
        );
        let mut cavity = scene.clone();
        let inner = [
            (0.25, 0.25, 0.25),
            (0.75, 0.25, 0.25),
            (0.75, 0.75, 0.25),
            (0.25, 0.75, 0.25),
            (0.25, 0.25, 1.75),
            (0.75, 0.25, 1.75),
            (0.75, 0.75, 1.75),
            (0.25, 0.75, 1.75),
        ]
        .map(|(x, y, z)| cavity.mesh.add_vertex(DVec3::new(x, y, z)));
        for ring in [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ] {
            let mut vertices = ring.map(|i| inner[i]).to_vec();
            vertices.reverse();
            cavity.mesh.add_face(vertices, vec![]);
        }
        assert_eq!(
            count(&cavity),
            20880 - 5184,
            "front-facing cavity surface must suppress fill in the void"
        );
        for visual in [
            crate::style::VisualStyle::Wireframe,
            crate::style::VisualStyle::XRay,
        ] {
            let mut disabled = style.clone();
            disabled.visual_style = visual;
            assert!(
                crate::pipeline::build_scene_render_data(&scene, &camera, &disabled, None)
                    .section_stencil
                    .is_none()
            );
        }
        let mut duplicate = scene.clone();
        duplicate.mesh.faces.extend(scene.mesh.faces.clone());
        assert_eq!(count(&duplicate), closed);
        let slab = [
            DVec3::new(0., 0., 1.),
            DVec3::new(1., 0., 1.),
            DVec3::new(1., 1., 1.),
            DVec3::new(0., 1., 1.),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        scene.mesh.add_face(slab.to_vec(), vec![]);
        assert_eq!(count(&scene), 0);
        scene.mesh.faces.last_mut().unwrap().vertices.reverse();
        assert_eq!(
            count(&scene),
            closed,
            "reversing the actual slab loop restores the original back-face stencil fill"
        );
        for face in &mut duplicate.mesh.faces {
            face.attrs.insert("opacity".into(), serde_json::json!(0.5));
        }
        let transparent =
            crate::pipeline::build_scene_render_data(&duplicate, &camera, &style, None);
        assert!(!transparent.transparent_mesh.is_empty());
        assert!(transparent
            .section_stencil
            .unwrap()
            .rasterize(&camera, 800, 600)
            .into_iter()
            .all(|v| !v));
    }
    #[test]
    fn scene_cut_is_display_only_transforms_nested_geometry_and_respects_toggle() {
        use crate::style::StyleConfig;
        use glam::DMat4;
        use ingetrazo_core::{camera::OrbitCamera, groups::Group, mesh::Mesh};
        let mut source = Mesh::new();
        let ids = [
            DVec3::new(-2., 0., 0.),
            DVec3::new(2., 0., 0.),
            DVec3::new(-2., 2., 0.),
        ]
        .map(|p| source.add_vertex(p));
        source.add_face(ids.to_vec(), vec![]);
        let mut child = Group::with_mesh("child", source);
        child.transform = DMat4::from_translation(DVec3::X);
        let mut parent = Group::new("parent");
        parent.children.push(child);
        let mut scene = Scene::new();
        scene.groups.push(parent);
        scene
            .section_planes
            .push(SectionPlane::new(DVec3::ZERO, DVec3::X, "cut", "A"));
        scene.show_section_cuts = true;
        let before = scene.clone();
        let data = crate::pipeline::build_scene_render_data(
            &scene,
            &OrbitCamera::default(),
            &StyleConfig::default(),
            None,
        );
        assert_eq!(scene, before);
        assert!(data
            .opaque_mesh
            .vertices
            .iter()
            .all(|v| v.position[0] <= 1e-7));
        assert!(data
            .visible_edges
            .vertices
            .iter()
            .all(|v| v.position[0] <= 1e-7));
        assert!(!data.opaque_mesh.is_empty());
        scene.show_section_cuts = false;
        let uncut = crate::pipeline::build_scene_render_data(
            &scene,
            &OrbitCamera::default(),
            &StyleConfig::default(),
            None,
        );
        assert!(uncut
            .opaque_mesh
            .vertices
            .iter()
            .any(|v| v.position[0] > 1.));
    }
    #[test]
    fn crossing_triangle_interpolates_uv_and_keeps_plane_boundary() {
        let plane = SectionPlane::new(DVec3::ZERO, DVec3::X, "A", "A");
        let mut buffer = MeshRenderBuffer::new();
        buffer.vertices = vec![
            Vertex::from_dvec3(DVec3::new(-1., 0., 0.), DVec3::Z, [1.; 4], [0., 0.]),
            Vertex::from_dvec3(DVec3::new(1., 0., 0.), DVec3::Z, [1.; 4], [1., 0.]),
            Vertex::from_dvec3(DVec3::new(-1., 2., 0.), DVec3::Z, [1.; 4], [0., 1.]),
        ];
        buffer.indices = vec![0, 1, 2];
        mesh(&mut buffer, &plane);
        assert_eq!(buffer.triangle_count(), 2);
        assert!(buffer.vertices.iter().all(|v| v.position[0] <= 1e-7));
        assert!(buffer
            .vertices
            .iter()
            .any(|v| v.position[0] == 0. && v.uv[0] == 0.5));
    }
}

/// Original two-pass stencil equivalent: opaque depth, visible back faces, plane.
pub struct SectionStencil {
    pub plane: SectionPlane,
    pub frame: [DVec3; 4],
    pub color: [f32; 4],
    pub surfaces: Vec<MeshRenderBuffer>,
}
impl SectionStencil {
    pub fn rasterize(
        &self,
        camera: &ingetrazo_core::camera::OrbitCamera,
        width: usize,
        height: usize,
    ) -> Vec<bool> {
        let matrix = camera.projection_matrix() * camera.view_matrix();
        let mut depths = vec![f64::INFINITY; width * height];
        let mut stencil = vec![false; width * height];
        let mut surfaces = self.surfaces.clone();
        for buffer in &mut surfaces {
            mesh(buffer, &self.plane);
        }
        for buffer in &surfaces {
            for ids in buffer.indices.chunks_exact(3) {
                let points = ids
                    .iter()
                    .map(|&i| position(&buffer.vertices[i as usize]))
                    .collect::<Vec<_>>();
                raster_triangle(
                    [points[0], points[1], points[2]],
                    matrix,
                    width,
                    height,
                    1.,
                    |index, z, _| {
                        depths[index] = depths[index].min(z);
                    },
                );
            }
        }
        for buffer in &surfaces {
            for ids in buffer.indices.chunks_exact(3) {
                let points = ids
                    .iter()
                    .map(|&i| position(&buffer.vertices[i as usize]))
                    .collect::<Vec<_>>();
                raster_triangle(
                    [points[0], points[1], points[2]],
                    matrix,
                    width,
                    height,
                    0.,
                    |index, z, back| {
                        if back && z <= depths[index] + 1e-12 {
                            stencil[index] = true;
                        }
                    },
                );
            }
        }
        let mut mask = vec![false; width * height];
        for ring in [[0, 1, 2], [0, 2, 3]] {
            raster_triangle(
                ring.map(|i| self.frame[i]),
                matrix,
                width,
                height,
                2.,
                |index, z, _| {
                    if stencil[index] && z <= depths[index] + 1e-12 {
                        mask[index] = true;
                    }
                },
            );
        }
        mask
    }
}
fn raster_triangle(
    points: [DVec3; 3],
    matrix: glam::DMat4,
    width: usize,
    height: usize,
    offset: f64,
    mut visit: impl FnMut(usize, f64, bool),
) {
    let mut polygon = points.map(|p| matrix * p.extend(1.)).to_vec();
    for axis in 0..6 {
        let distance = |p: glam::DVec4| match axis {
            0 => p.w + p.x,
            1 => p.w - p.x,
            2 => p.w + p.y,
            3 => p.w - p.y,
            4 => p.z,
            _ => p.w - p.z,
        };
        let old = std::mem::take(&mut polygon);
        if old.is_empty() {
            return;
        }
        for i in 0..old.len() {
            let a = old[i];
            let b = old[(i + 1) % old.len()];
            let da = distance(a);
            let db = distance(b);
            if da >= 0. {
                polygon.push(a);
            }
            if (da >= 0.) != (db >= 0.) {
                polygon.push(a + (b - a) * (da / (da - db)));
            }
        }
    }
    for i in 1..polygon.len().saturating_sub(1) {
        raster_projected(
            [polygon[0], polygon[i], polygon[i + 1]],
            width,
            height,
            offset,
            &mut visit,
        );
    }
}
fn raster_projected(
    clips: [glam::DVec4; 3],
    width: usize,
    height: usize,
    offset: f64,
    visit: &mut impl FnMut(usize, f64, bool),
) {
    if clips.iter().any(|p| p.w <= 1e-12) {
        return;
    }
    let p = clips.map(|c| {
        DVec3::new(
            (c.x / c.w + 1.) * width as f64 / 2.,
            (1. - c.y / c.w) * height as f64 / 2.,
            c.z / c.w,
        )
    });
    let area = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[1].y - p[0].y) * (p[2].x - p[0].x);
    if area.abs() < 1e-12 {
        return;
    }
    // Original opaque pass uses glPolygonOffset(1,1), plane uses (2,2).
    // Depth24 units match the reference Mesa stencil8 framebuffer.
    let dzdx =
        ((p[1].z - p[0].z) * (p[2].y - p[0].y) - (p[2].z - p[0].z) * (p[1].y - p[0].y)) / area;
    let dzdy =
        ((p[1].x - p[0].x) * (p[2].z - p[0].z) - (p[2].x - p[0].x) * (p[1].z - p[0].z)) / area;
    let bias = offset * (dzdx.abs().max(dzdy.abs()) + 1. / 16777216.);
    let xmin = p
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .max(0.) as usize;
    let xmax = p
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .max(0.)
        .min(width as f64) as usize;
    let ymin = p
        .iter()
        .map(|p| p.y)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .max(0.) as usize;
    let ymax = p
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        .max(0.)
        .min(height as f64) as usize;
    for y in ymin..ymax {
        for x in xmin..xmax {
            let q = glam::DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
            let edge = |a: DVec3, b: DVec3| (b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x);
            let weights = [
                edge(p[1], p[2]) / area,
                edge(p[2], p[0]) / area,
                edge(p[0], p[1]) / area,
            ];
            if weights.iter().all(|w| *w >= -1e-10) {
                let z = weights[0] * p[0].z + weights[1] * p[1].z + weights[2] * p[2].z;
                if (0. ..=1.).contains(&z) {
                    visit(y * width + x, z + bias, area > 0.);
                }
            }
        }
    }
}

#[cfg(test)]
mod raster_tests {
    use super::*;
    #[test]
    fn triangle_crossing_near_plane_retains_visible_pixels() {
        let matrix = glam::DMat4::perspective_rh(60_f64.to_radians(), 1., 0.1, 10.);
        let mut hits = vec![];
        raster_triangle(
            [
                DVec3::new(-0.5, -0.5, -1.),
                DVec3::new(0.5, -0.5, -1.),
                DVec3::new(0., 0.5, 0.2),
            ],
            matrix,
            100,
            100,
            0.,
            |i, z, _| {
                assert!((0. ..=1.).contains(&z));
                hits.push(i);
            },
        );
        assert!(hits.len() > 1000);
        let mut invisible = 0;
        raster_triangle(
            [
                DVec3::new(-0.5, -0.5, 1.),
                DVec3::new(0.5, -0.5, 1.),
                DVec3::new(0., 0.5, 1.),
            ],
            matrix,
            100,
            100,
            0.,
            |_, _, _| invisible += 1,
        );
        assert_eq!(invisible, 0);
    }
}
