//! Borrowed parked-owner geometry; source RGB wash is applied by the GPU after lighting.
use super::{BufferId, ColoredVertex};
use std::cell::RefCell;
#[derive(PartialEq)]
pub(super) struct Key {
    pub revision: u64,
    pub scene: u64,
    pub context: Vec<uuid::Uuid>,
    pub mode: u8,
    pub style: [u32; 14],
}
pub(super) struct Packet {
    pub key: Key,
    pub faces: Option<BufferId>,
    pub edges: Option<BufferId>,
    pub face_count: u32,
    pub edge_count: u32,
    pub fade: f32,
}
pub(super) fn key(app: &crate::IngeTrazoApp) -> Key {
    let view = crate::edit_context_view::ContextView::new(app);
    Key {
        revision: app.native_geometry_revision,
        scene: app.scene.version,
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
        ],
    }
}
pub(super) fn collect(
    app: &crate::IngeTrazoApp,
) -> Result<(Vec<ColoredVertex>, Vec<ColoredVertex>, f32), String> {
    let view = crate::edit_context_view::ContextView::new(app);
    let scene = view.active_scene();
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
    let data = data.into_inner();
    if !data.textured_meshes.is_empty() || !data.billboards.is_empty() {
        return Err(
            "Native parked-context texture/billboard wash pass is not yet supported".into(),
        );
    }
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
    for mesh in [&data.opaque_mesh, &data.transparent_mesh] {
        for &index in &mesh.indices {
            let v = mesh
                .vertices
                .get(index as usize)
                .ok_or("Context surface index invalid")?;
            faces.push(ColoredVertex {
                position: v.position,
                color: [v.color[0], v.color[1], v.color[2]],
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
    Ok((faces, lines, view.context_fade()))
}
