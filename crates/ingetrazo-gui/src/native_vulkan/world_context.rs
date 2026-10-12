//! Borrowed parked-owner geometry; source RGB wash is applied by the GPU after lighting.
use super::{BufferId, ColoredVertex};
use std::cell::RefCell;
#[derive(PartialEq)]
pub(super) struct Key {
    pub revision: u64,
    pub scene: u64,
    pub view: u64,
    pub context: Vec<uuid::Uuid>,
    pub mode: u8,
    pub style: [u32; 15],
}
pub(super) struct Run {
    pub first: u32,
    pub count: u32,
    pub mode: u32,
    pub opacity: f32,
}
pub(super) struct Packet {
    pub key: Key,
    pub faces: Option<BufferId>,
    pub edges: Option<BufferId>,
    pub face_count: u32,
    pub edge_count: u32,
    pub fade: f32,
    pub runs: Vec<Run>,
    pub camera: [u64; 9],
    pub billboards: ingetrazo_render::billboards::Packet,
}
pub(super) fn key(app: &crate::IngeTrazoApp) -> Key {
    let view = crate::edit_context_view::ContextView::new(app);
    Key {
        revision: app.native_geometry_revision,
        scene: app.scene.version,
        view: app.scene.view_version,
        context: app
            .component_edit_stack
            .iter()
            .map(|s| s.edited_group_id())
            .collect(),
        mode: match view.mode() {
            crate::edit_context_view::RestMode::Normal => 0,
            crate::edit_context_view::RestMode::Fade => 1,
            crate::edit_context_view::RestMode::Hide => 2,
        },
        style: [
            app.style.background_color[0].to_bits(),
            app.style.background_color[1].to_bits(),
            app.style.background_color[2].to_bits(),
            app.style.front_color[0].to_bits(),
            app.style.front_color[1].to_bits(),
            app.style.front_color[2].to_bits(),
            app.style.edge_color[0].to_bits(),
            app.style.edge_color[1].to_bits(),
            app.style.edge_color[2].to_bits(),
            app.style.back_color[0].to_bits(),
            app.style.back_color[1].to_bits(),
            app.style.back_color[2].to_bits(),
            app.style.visual_style as u32,
            u32::from(app.style.show_edges),
            u32::from(app.style.show_textures),
        ],
    }
}
pub(super) fn collect(
    app: &crate::IngeTrazoApp,
) -> Result<
    (
        Vec<ColoredVertex>,
        Vec<ColoredVertex>,
        f32,
        Vec<Run>,
        ingetrazo_render::buffer::TextureBuffers,
        ingetrazo_render::billboards::Packet,
    ),
    String,
> {
    let view = crate::edit_context_view::ContextView::new(app);
    let scene = view.active_scene();
    let groups = RefCell::new(Vec::new());
    let group_error = RefCell::new(None);
    let data = RefCell::new(ingetrazo_render::pipeline::SceneRenderData::default());
    let edges = RefCell::new(ingetrazo_render::native_edges::HardEdges::default());
    view.visit_context(
        |mesh| {
            ingetrazo_render::pipeline::append_borrowed_context_mesh(
                mesh,
                scene,
                &app.style,
                &mut data.borrow_mut(),
            );
            ingetrazo_render::native_edges::append_borrowed_mesh(
                mesh,
                scene,
                &mut edges.borrow_mut(),
            );
        },
        |group| {
            if groups.borrow().len() >= 100_000 || groups.borrow_mut().try_reserve(1).is_err() {
                *group_error.borrow_mut() = Some("Context group reference admission exceeded");
                return;
            }
            groups.borrow_mut().push(group);
            ingetrazo_render::pipeline::append_borrowed_context_group(
                group,
                scene,
                &app.camera,
                &app.style,
                &mut data.borrow_mut(),
            );
            ingetrazo_render::native_edges::append_borrowed_group(
                group,
                scene,
                &mut edges.borrow_mut(),
            );
        },
    );
    if let Some(error) = group_error.into_inner() {
        return Err(error.into());
    }
    let data = data.into_inner();
    let billboards =
        ingetrazo_render::billboards::Packet::build_borrowed_groups(scene, groups.into_inner())?;
    let count = data
        .opaque_mesh
        .indices
        .len()
        .checked_add(data.transparent_mesh.indices.len())
        .ok_or("Context surface count overflow")?;
    if count > 3_000_000 {
        return Err("Context surface inventory exceeds admission".into());
    }
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(count)
        .map_err(|_| "Context surface allocation failed")?;
    let mut runs: Vec<Run> = Vec::new();
    runs.try_reserve(
        count / 3
            + data.opaque_mesh.source_back_paints.len()
            + data.transparent_mesh.source_back_paints.len(),
    )
    .map_err(|_| "Context run allocation failed")?;
    for mesh in [&data.opaque_mesh, &data.transparent_mesh] {
        let mut front_cursor = 0;
        let mut back_cursor = 0;
        let mut opacity_cursor = 0;
        for (first, triangle) in mesh.indices.chunks_exact(3).enumerate() {
            let first = first * 3;
            let flat = matches!(
                app.style.visual_style,
                ingetrazo_render::style::VisualStyle::HiddenLine
                    | ingetrazo_render::style::VisualStyle::Monochrome
            );
            let contains = |spans: &[(usize, usize)], cursor: &mut usize| {
                while *cursor < spans.len() && spans[*cursor].0 + spans[*cursor].1 <= first {
                    *cursor += 1;
                }
                spans
                    .get(*cursor)
                    .is_some_and(|&(a, n)| first >= a && first < a + n)
            };
            let cull = contains(&mesh.source_front_cull, &mut front_cursor);
            let back = contains(&mesh.source_default_back, &mut back_cursor);
            let mode = if cull {
                9
            } else if back || flat {
                7
            } else {
                6
            };
            let opacity = if app.style.visual_style == ingetrazo_render::style::VisualStyle::XRay {
                0.6
            } else {
                while opacity_cursor + 1 < mesh.source_opacity_runs.len()
                    && mesh.source_opacity_runs[opacity_cursor + 1].0 <= first
                {
                    opacity_cursor += 1;
                }
                mesh.source_opacity_runs
                    .get(opacity_cursor)
                    .filter(|(a, _)| *a <= first)
                    .map_or(1., |(_, v)| *v as f32)
            };
            let offset = faces.len() as u32;
            for &index in triangle {
                let v = mesh
                    .vertices
                    .get(index as usize)
                    .ok_or("Context surface index invalid")?;
                let color = if flat {
                    [
                        app.style.front_color[0],
                        app.style.front_color[1],
                        app.style.front_color[2],
                    ]
                } else {
                    [v.color[0], v.color[1], v.color[2]]
                };
                faces.push(ColoredVertex {
                    position: v.position,
                    color,
                });
            }
            if let Some(last) = runs.last_mut().filter(|r: &&mut Run| {
                r.mode == mode && r.opacity == opacity && r.first + r.count == offset
            }) {
                last.count += 3;
            } else {
                runs.try_reserve(1)
                    .map_err(|_| "Context draw run allocation failed")?;
                runs.push(Run {
                    first: offset,
                    count: 3,
                    mode,
                    opacity,
                });
            }
        }
        for paint in &mesh.source_back_paints {
            let offset = faces.len() as u32;
            let end = paint
                .first
                .checked_add(paint.count)
                .ok_or("Context back paint overflow")?;
            let indices = mesh
                .indices
                .get(paint.first..end)
                .ok_or("Context back paint index range invalid")?;
            if faces
                .len()
                .checked_add(indices.len())
                .is_none_or(|n| n > 3_000_000)
            {
                return Err("Context back paint admission exceeded".into());
            }
            faces
                .try_reserve(indices.len())
                .map_err(|_| "Context back paint allocation failed")?;
            for &index in indices {
                let v = mesh
                    .vertices
                    .get(index as usize)
                    .ok_or("Context back paint vertex invalid")?;
                faces.push(ColoredVertex {
                    position: v.position,
                    color: paint.color,
                });
            }
            runs.push(Run {
                first: offset,
                count: paint.count as u32,
                mode: 8,
                opacity: paint.opacity as f32,
            });
        }
    }
    let edge_positions = if app.style.show_edges
        || app.style.visual_style == ingetrazo_render::style::VisualStyle::Wireframe
    {
        edges.into_inner().positions
    } else {
        Vec::new()
    };
    if edge_positions.len() > 2_000_000 {
        return Err("Context edge inventory exceeds admission".into());
    }
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(edge_positions.len())
        .map_err(|_| "Context edge allocation failed")?;
    let color = [
        app.style.edge_color[0],
        app.style.edge_color[1],
        app.style.edge_color[2],
    ];
    for position in edge_positions {
        lines.push(ColoredVertex { position, color });
    }
    Ok((
        faces,
        lines,
        view.context_fade(),
        runs,
        data.textured_meshes,
        billboards,
    ))
}

pub(super) fn camera_key(camera: &ingetrazo_core::camera::OrbitCamera) -> [u64; 9] {
    [
        camera.target.x.to_bits(),
        camera.target.y.to_bits(),
        camera.target.z.to_bits(),
        camera.yaw.to_bits(),
        camera.pitch.to_bits(),
        camera.distance.to_bits(),
        camera.fov_deg.to_bits(),
        camera.aspect.to_bits(),
        u64::from(camera.perspective),
    ]
}
