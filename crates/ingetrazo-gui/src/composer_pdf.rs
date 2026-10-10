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
    let saved = frame
        .view_key
        .strip_prefix("scene:")
        .and_then(|name| scene.saved_views.iter().find(|v| v.name == name));
    let snapshot = scene;
    let paper_view = ingetrazo_render::paper_scene::PaperSceneView::new(scene, saved);
    // Source SavedView.apply changes camera fields but inherits the live up
    // vector. Applying to an empty layer slice avoids changing model visibility.
    let mut base = live.clone();
    if let Some(saved) = saved {
        saved.apply(&mut base, &mut []);
    }
    let mut camera = super::frame_camera_with_base(snapshot, frame, &base);
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
            base.up
        } else {
            live.up
        };
    }
    if camera.perspective || !camera.view_matrix().is_finite() {
        return Err("Invalid orthographic PDF frame camera".into());
    }
    let drawing = compute.original_qt_paper_drawing_for_view(
        &paper_view,
        &camera,
        document,
        revision,
        &frame.view_key,
        frame.profiles,
        frame.hidden_lines,
    )?;
    let ink = &drawing.segments;
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
        clip_intersection: None,
        flatten_transform: false,
    }];
    if let Some(fill) = section_fill(frame, &drawing.section_loops, &point)? {
        draws.push(fill);
    }
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
            clip_intersection: None,
            flatten_transform: false,
        });
    }
    draws.extend(view_grid(frame, metadata)?);
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
            clip_intersection: None,
            flatten_transform: false,
        });
    }
    Ok(draws)
}

/// Source _paint_cut_fills_mm: one even-odd compound path, filled below ink;
/// hatch phase is local to the frame and clipped by both material and frame.
fn section_fill(
    frame: &ViewFrame,
    loops: &[Vec<glam::DVec3>],
    point: &impl Fn(glam::DVec3) -> [f64; 2],
) -> Result<Option<Draw>, String> {
    if loops.is_empty() || frame.cut_fill == "none" {
        return Ok(None);
    }
    let mut path = Vec::new();
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for ring in loops {
        if ring.len() < 3 {
            continue;
        }
        for (i, p) in ring.iter().enumerate() {
            let p = point(*p);
            if p.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.) {
                return Err("Invalid projected PDF section loop".into());
            }
            for axis in 0..2 {
                lo[axis] = lo[axis].min(p[axis]);
                hi[axis] = hi[axis].max(p[axis]);
            }
            path.push(if i == 0 {
                PathOp::Move(p)
            } else {
                PathOp::Line(p)
            });
            if path.len() > MAX_COMMANDS - 25 {
                return Err("PDF section loop command budget exceeded".into());
            }
        }
        path.push(PathOp::Close);
    }
    if path.is_empty() {
        return Ok(None);
    }
    let color =
        ingetrazo_core::sheet_color::parse_sheet_color(if frame.cut_fill_color.is_empty() {
            "#595e69"
        } else {
            &frame.cut_fill_color
        })?
        .map(|v| v as f32 / 255.);
    let mut draw = Draw {
        origin_mm: [frame.rect.x, frame.rect.y],
        even_odd: true,
        path,
        stroke: None,
        stroke_options: None,
        fill: Some(color),
        clip: Some(rect(frame.rect.w, frame.rect.h)),
        clip_intersection: None,
        flatten_transform: false,
    };
    if frame.cut_fill == "hatch" {
        if !frame.cut_hatch_mm.is_finite() {
            return Err("Invalid physical PDF hatch step".into());
        }
        let step = if frame.cut_hatch_mm == 0. {
            1.5
        } else {
            frame.cut_hatch_mm
        }
        .max(0.3);
        let mut c = ((lo[0] + lo[1]) / step).floor() * step;
        let end = hi[0] + hi[1];
        let count = ((end - c) / step).floor() + 1.;
        if !count.is_finite() || count > (MAX_COMMANDS / 2) as f64 {
            return Err("PDF hatch command budget exceeded".into());
        }
        let mut hatch = Vec::new();
        while c <= end {
            let a = lo[0].max(c - hi[1]);
            let b = hi[0].min(c - lo[1]);
            if b > a {
                hatch.push(PathOp::Move([a, c - a]));
                hatch.push(PathOp::Line([b, c - b]));
            }
            let next = c + step;
            if next <= c {
                return Err("PDF hatch phase cannot advance".into());
            }
            c = next;
        }
        draw.clip_intersection = Some(std::mem::replace(&mut draw.path, hatch));
        draw.fill = None;
        let width = if frame.pen_edge_mm == 0. {
            0.18
        } else {
            frame.pen_edge_mm
        }
        .max(0.05);
        draw.stroke = Some((color, width));
        draw.stroke_options = Some(StrokeOptions {
            cap: 2,
            join: 2,
            dash: Vec::new(),
        });
    }
    Ok(Some(draw))
}

// views/composer.py paint_frame_mm: physical graticule after ink, before border.
fn view_grid(frame: &ViewFrame, metadata: Option<&serde_json::Value>) -> Result<Vec<Draw>, String> {
    let grid = metadata
        .and_then(|v| v.get("grid_m"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.);
    if !grid.is_finite() {
        return Err("Invalid PDF model grid spacing".into());
    }
    if frame.perspective || grid <= 0. {
        return Ok(vec![]);
    }
    let model_h = frame.rect.h * frame.scale_n / 1000.;
    let step = grid * frame.rect.h / model_h;
    if !step.is_finite() {
        return Err("Invalid PDF physical grid spacing".into());
    }
    if step < 2. {
        return Ok(vec![]);
    }
    let mut draws = Vec::new();
    for (limit, vertical) in [(frame.rect.w, true), (frame.rect.h, false)] {
        let mut value = step;
        while value < limit {
            if draws.len() >= MAX_COMMANDS / 15 {
                return Err("PDF grid command budget exceeded".into());
            }
            // Source paints each translucent line separately; an aggregate
            // stroke would blend grid intersections only once.
            let h = 0.06;
            // QPdfEngine expands nonopaque QPen into the Qt stroker outline.
            // Preserve its side/cap vertices and paint each line independently.
            let points = if vertical {
                [
                    [value + h, 0.],
                    [value + h, frame.rect.h],
                    [value + h, frame.rect.h + h],
                    [value - h, frame.rect.h + h],
                    [value - h, frame.rect.h],
                    [value - h, 0.],
                    [value - h, -h],
                    [value + h, -h],
                    [value + h, 0.],
                ]
            } else {
                [
                    [0., value - h],
                    [frame.rect.w, value - h],
                    [frame.rect.w + h, value - h],
                    [frame.rect.w + h, value + h],
                    [frame.rect.w, value + h],
                    [0., value + h],
                    [-h, value + h],
                    [-h, value - h],
                    [0., value - h],
                ]
            };
            let mut outline = vec![PathOp::Move(points[0])];
            outline.extend(points[1..].iter().copied().map(PathOp::Line));
            outline.push(PathOp::Close);
            draws.push(Draw {
                origin_mm: [frame.rect.x, frame.rect.y],
                path: outline,
                stroke: None,
                stroke_options: None,
                fill: Some([90. / 255., 140. / 255., 190. / 255., 120. / 255.]),
                clip: Some(rect(frame.rect.w, frame.rect.h)),
                clip_intersection: None,
                flatten_transform: true,
                even_odd: false,
            });
            let next = value + step;
            if next <= value {
                return Err("PDF grid spacing cannot advance".into());
            }
            value = next;
        }
    }
    Ok(draws)
}

#[cfg(test)]
mod grid_tests {
    use super::*;
    #[test]
    fn literal_original_grid_cutoff_and_perspective_controls() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/sheet-view-grid-pdf/source-grid-controls.json"
        ))
        .unwrap();
        for case in source["cases"].as_array().unwrap() {
            let mut frame = ViewFrame::default();
            frame.rect.w = 100.;
            frame.rect.h = 100.;
            frame.scale_n = 100.;
            frame.perspective = case["perspective"].as_bool().unwrap();
            let metadata = serde_json::json!({"grid_m":case["grid_m"]});
            let draws = view_grid(&frame, Some(&metadata)).unwrap();
            let lines = case["lines"].as_array().unwrap();
            assert_eq!(draws.len(), lines.len(), "original cutoff {:?}", case);
            for (draw, line) in draws.iter().zip(lines) {
                let a = [line[0][0].as_f64().unwrap(), line[0][1].as_f64().unwrap()];
                let b = [line[1][0].as_f64().unwrap(), line[1][1].as_f64().unwrap()];
                let first = if a[0] == b[0] {
                    [a[0] + 0.06, a[1]]
                } else {
                    [a[0], a[1] - 0.06]
                };
                assert!(matches!(draw.path[0],PathOp::Move(p) if p == first));
                assert!(draw.flatten_transform && draw.stroke.is_none());
            }
        }
    }
}
