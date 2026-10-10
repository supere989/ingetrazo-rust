// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-faithful physical sheet view ink, using the application's existing HLR cache.
use crate::composer_compute::ComposerCompute;
use ingetrazo_core::{camera::OrbitCamera, composition::ViewFrame, scene::Scene};
use ingetrazo_formats::sheet_pdf::{Draw, PathOp, StrokeOptions, MAX_COMMANDS};
use ingetrazo_render::vector_hlr::LineKind;
fn rect(w: f64, h: f64) -> Vec<PathOp> {
    vec![
        PathOp::Move([0., 0.]),
        PathOp::Line([w, 0.]),
        PathOp::Line([w, h]),
        PathOp::Line([0., h]),
        PathOp::Close,
    ]
}
pub(super) fn view_draws(
    scene: &Scene,
    live: &OrbitCamera,
    compute: &mut ComposerCompute,
    document: uuid::Uuid,
    revision: u64,
    frame: &ViewFrame,
) -> Result<Vec<Draw>, String> {
    if frame.style != "vectorial" || frame.perspective {
        return Err("PDF raster/perspective view painter remains unavailable; orthographic vectorial views are supported".into());
    }
    let metadata = scene
        .sheets
        .iter()
        .find_map(|s| s.native_items.get(&frame.id));
    if frame.show_title
        || metadata
            .and_then(|v| v.get("show_scale"))
            .and_then(|v| v.as_bool())
            == Some(true)
    {
        return Err("PDF view title/scale text painter remains unavailable".into());
    }
    if metadata
        .and_then(|v| v.get("grid_m"))
        .and_then(|v| v.as_f64())
        .is_some_and(|v| v != 0.)
    {
        return Err("PDF view grid painter remains unavailable".into());
    }
    if frame.annotations
        && (!scene.dimensions.is_empty()
            || !scene.text_labels.is_empty()
            || !scene.geo_paths.is_empty()
            || (frame.section_marks && !scene.section_planes.is_empty()))
    {
        return Err("PDF model annotation painter remains unavailable".into());
    }
    let values = [
        frame.rect.x,
        frame.rect.y,
        frame.rect.w,
        frame.rect.h,
        frame.scale_n,
        frame.pen_edge_mm,
        frame.pen_profile_mm,
        frame.pen_cut_mm,
    ];
    if values
        .iter()
        .any(|x| !x.is_finite() || x.abs() > 1_000_000.)
        || frame.rect.w <= 0.
        || frame.rect.h <= 0.
        || frame.scale_n <= 0.
    {
        return Err("Invalid physical PDF view frame".into());
    }
    if frame
        .view_key
        .strip_prefix("scene:")
        .is_some_and(|name| scene.saved_views.iter().any(|v| v.name == name))
    {
        return Err("PDF saved-view visibility/section adapter remains unavailable".into());
    }
    let snapshot = scene;
    let mut camera = super::frame_camera_live(&snapshot, frame, live);
    // Original roll_camera is a no-op at zero degrees; preserve the source up
    // vector instead of recomputing an equivalent f64 screen-up before Qt storage.
    if frame.rot_deg == 0. {
        let bound = frame
            .view_key
            .strip_prefix("scene:")
            .is_some_and(|name| snapshot.saved_views.iter().any(|v| v.name == name));
        camera.up = if matches!(frame.view_key.as_str(), "std:top" | "std:bottom") {
            glam::DVec3::Y
        } else if frame
            .cam_pitch
            .is_some_and(|p| p.abs() < 89.5_f64.to_radians())
        {
            glam::DVec3::Z
        } else if bound {
            snapshot.camera.up
        } else {
            live.up
        };
    }
    if camera.perspective || !camera.view_matrix().is_finite() {
        return Err("Invalid orthographic PDF frame camera".into());
    }
    if frame.cut_fill != "none"
        && !ingetrazo_render::vector_hlr::paper_section_triangles(&snapshot).is_empty()
    {
        return Err("PDF section fill painter remains unavailable".into());
    }
    let ink = compute.original_qt_paper_segments(
        &snapshot,
        &camera,
        document,
        revision,
        &frame.view_key,
        frame.profiles,
        frame.hidden_lines,
    )?;
    if ink.is_empty() {
        return Err("PDF empty view placeholder text painter remains unavailable".into());
    }
    let basis = ingetrazo_render::vector_hlr::OriginalQtPaperBasis::from_camera(&camera);
    let right = basis.right;
    let up = basis.up;
    let eye = basis.eye;
    let k = 1000. / frame.scale_n;
    let point = |p: glam::DVec3| {
        let p = p - eye;
        [
            (p.dot(right) * k) + frame.rect.w / 2.,
            frame.rect.h / 2. - p.dot(up) * k,
        ]
    };
    let mut paths: [Vec<PathOp>; 4] = Default::default();
    let mut commands = 0usize;
    for segment in ink.iter() {
        let index = match segment.kind {
            LineKind::Hidden => 0,
            LineKind::Edge => 1,
            LineKind::Profile => 2,
            LineKind::Cut => 3,
        };
        if index == 0 && !frame.hidden_lines {
            continue;
        }
        commands = commands
            .checked_add(2)
            .ok_or("PDF command count overflow")?;
        if commands > MAX_COMMANDS - 25 {
            return Err("PDF view command budget exceeded".into());
        }
        let a = point(segment.a);
        let b = point(segment.b);
        if a.iter()
            .chain(b.iter())
            .any(|x| !x.is_finite() || x.abs() > 1_000_000.)
        {
            return Err("Invalid projected PDF view ink".into());
        }
        paths[index].push(PathOp::Move(a));
        paths[index].push(PathOp::Line(b));
    }
    let mut draws = vec![Draw {
        origin_mm: [frame.rect.x, frame.rect.y],
        even_odd: false,
        path: rect(frame.rect.w, frame.rect.h),
        stroke: None,
        stroke_options: None,
        fill: Some([1.; 4]),
        clip: None,
    }];
    for (index, path) in paths.into_iter().enumerate() {
        if path.is_empty() {
            continue;
        }
        let default = match index {
            2 => 0.35,
            3 => 0.5,
            _ => 0.18,
        };
        let width = match index {
            2 => frame.pen_profile_mm,
            3 => frame.pen_cut_mm,
            _ => frame.pen_edge_mm,
        };
        let width = if width == 0. { default } else { width }.max(0.05);
        draws.push(Draw {
            origin_mm: [frame.rect.x, frame.rect.y],
            even_odd: false,
            path,
            stroke: Some(([30. / 255., 36. / 255., 44. / 255., 1.], width)),
            stroke_options: Some(StrokeOptions {
                cap: if index == 0 { 0 } else { 1 },
                join: 1,
                dash: if index == 0 { vec![1.5, 0.8] } else { vec![] },
            }),
            fill: None,
            clip: Some(rect(frame.rect.w, frame.rect.h)),
        });
    }
    if frame.border {
        let width = metadata
            .and_then(|v| v.get("border_mm"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.3)
            .max(0.1);
        let color = metadata
            .and_then(|v| v.get("border_color"))
            .and_then(|v| v.as_str())
            .unwrap_or("#282e36");
        let color =
            ingetrazo_core::sheet_color::parse_sheet_color(color)?.map(|x| f32::from(x) / 255.);
        draws.push(Draw {
            origin_mm: [frame.rect.x, frame.rect.y],
            even_odd: false,
            path: rect(frame.rect.w, frame.rect.h),
            stroke: Some((color, width)),
            stroke_options: Some(StrokeOptions {
                cap: 2,
                join: 2,
                dash: vec![],
            }),
            fill: None,
            clip: None,
        });
    }
    Ok(draws)
}
