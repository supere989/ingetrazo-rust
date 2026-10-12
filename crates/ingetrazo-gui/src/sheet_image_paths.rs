//! Literal raster View pen dispatch consuming the shared physical Draw protocol.
//! Qt6.12 qpaintengineex.cpp drawLines batches16; qcosmeticstroker.cpp FastDraw
//! scales the QRgba64 color before coverage; qmath_p.h clips rectangle endpoints.
use glam::DVec2;
use ingetrazo_formats::sheet_pdf::{Draw, PathOp};
use ingetrazo_render::{
    cad2d::{EncodedRgba, PremultipliedRgba8},
    qt_gray::{PathContour, Segment, Span},
};
fn paths(ops: &[PathOp]) -> Result<Vec<PathContour>, String> {
    let mut out: Vec<PathContour> = Vec::new();
    for op in ops {
        match op {
            PathOp::Move(p) => out.push(PathContour {
                start: *p,
                segments: Vec::new(),
            }),
            PathOp::Line(p) => out
                .last_mut()
                .ok_or("Path line before origin")?
                .segments
                .push(Segment::Line(*p)),
            PathOp::Cubic(p) => out
                .last_mut()
                .ok_or("Path curve before origin")?
                .segments
                .push(Segment::Cubic {
                    control1: [p[0], p[1]],
                    control2: [p[2], p[3]],
                    to: [p[4], p[5]],
                }),
            PathOp::Close => {
                let path = out.last_mut().ok_or("Path closure before origin")?;
                path.segments.push(Segment::Line(path.start));
            }
        }
    }
    Ok(out)
}
fn transformed(path: &PathContour, scale: f64, origin: [f64; 2]) -> PathContour {
    let map = |p: [f64; 2]| [p[0] * scale + origin[0], p[1] * scale + origin[1]];
    PathContour {
        start: map(path.start),
        segments: path
            .segments
            .iter()
            .map(|s| match s {
                Segment::Line(p) => Segment::Line(map(*p)),
                Segment::Cubic {
                    control1,
                    control2,
                    to,
                } => Segment::Cubic {
                    control1: map(*control1),
                    control2: map(*control2),
                    to: map(*to),
                },
            })
            .collect(),
    }
}
fn clip_bounds(draw: &Draw, density: f64, origin: [f64; 2]) -> Result<Option<[i32; 4]>, String> {
    let mut result: Option<[i32; 4]> = None;
    for ops in [&draw.clip, &draw.clip_intersection].into_iter().flatten() {
        let [PathOp::Move(a), PathOp::Line(b), PathOp::Line(c), PathOp::Line(d), PathOp::Close] =
            ops.as_slice()
        else {
            return Err("Physical vector nonrectangle clip remains unavailable".into());
        };
        if a[1] != b[1] || b[0] != c[0] || c[1] != d[1] || d[0] != a[0] {
            return Err("Physical vector clip is not axis aligned".into());
        }
        let point = |p: [f64; 2]| -> Result<[i32; 2], String> {
            let q = [p[0] * density + origin[0], p[1] * density + origin[1]];
            if q.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.) {
                return Err("Physical vector clip domain exceeded".into());
            }
            Ok(q.map(|v| {
                if v >= 0. {
                    (v + 0.5).floor() as i32
                } else {
                    (v - 0.5).ceil() as i32
                }
            }))
        };
        let a = point(*a)?;
        let c = point(*c)?;
        let rect = [
            a[0].min(c[0]),
            a[1].min(c[1]),
            a[0].max(c[0]),
            a[1].max(c[1]),
        ];
        result = Some(result.map_or(rect, |old| {
            [
                old[0].max(rect[0]),
                old[1].max(rect[1]),
                old[2].min(rect[2]),
                old[3].min(rect[3]),
            ]
        }));
    }
    Ok(result)
}
fn clip(mut spans: Vec<Span>, bounds: Option<[i32; 4]>) -> Vec<Span> {
    if let Some([left, top, right, bottom]) = bounds {
        spans.retain_mut(|s| {
            if s.y < top || s.y >= bottom {
                return false;
            }
            let end = (i64::from(s.x) + i64::from(s.len)).min(i64::from(right));
            s.x = s.x.max(left);
            if end <= i64::from(s.x) {
                return false;
            }
            s.len = (end - i64::from(s.x)) as u32;
            true
        });
    }
    spans
}
fn stroke_line(a: DVec2, b: DVec2, width: f64, cap: u8) -> Result<Option<PathContour>, String> {
    let d = b - a;
    let length = d.length();
    if length == 0. {
        return Ok(None);
    }
    if cap == 1 {
        return Ok(crate::composer_cad::round_cap_outline(a, b, width * 0.5));
    }
    let u = d / length;
    let normal = DVec2::new(u.y, -u.x) * (width * 0.5);
    let extension = if cap == 2 {
        u * (width * 0.5)
    } else {
        DVec2::ZERO
    };
    let points = [
        a - extension + normal,
        b + extension + normal,
        b + extension - normal,
        a - extension - normal,
    ];
    Ok(Some(PathContour {
        start: points[0].to_array(),
        segments: points
            .iter()
            .skip(1)
            .chain(points.iter().take(1))
            .map(|p| Segment::Line(p.to_array()))
            .collect(),
    }))
}
fn fast_color(rgba: [f32; 4], opacity: u32) -> Result<PremultipliedRgba8, String> {
    let bytes = rgba.map(|v| (v * 255.).round() as u32);
    let alpha = bytes[3] * 257;
    let output = std::array::from_fn(|i| {
        let channel = bytes[i] * 257;
        let premul = if i == 3 {
            channel
        } else {
            let t = channel * alpha;
            (t + (t >> 16) + 0x8000) >> 16
        };
        let value = (premul * opacity) >> 8;
        let value = value + 128;
        ((value - (value >> 8)) >> 8) as u8
    });
    PremultipliedRgba8::new(output)
}
pub(crate) fn records(
    draws: &[Draw],
    density: f64,
    device: [u32; 2],
) -> Result<Vec<crate::sheet_image::Records>, String> {
    let mut records = Vec::new();
    for draw in draws {
        if draw.text.is_some() || draw.even_odd {
            return Err("Physical vector text/even-odd painter remains unavailable".into());
        }
        let origin = draw.origin_mm.map(|v| v * density);
        let paths = paths(&draw.path)?;
        let bounds = clip_bounds(draw, density, origin)?;
        let color =
            |rgba: [f32; 4]| EncodedRgba::from_bytes(rgba.map(|v| (v * 255.).round() as u8), 1.);
        if let Some(fill) = draw.fill {
            let spans = clip(
                ingetrazo_render::qt_gray::rasterize_transformed_paths(
                    &paths, density, origin, 250_000, 100_000,
                )?,
                bounds,
            );
            if !spans.is_empty() {
                records.push(crate::composer_border::physical_layers(vec![(
                    spans,
                    color(fill)?,
                )])?);
            }
        }
        let Some((rgba, width)) = draw.stroke else {
            continue;
        };
        if !width.is_finite() || width <= 0. {
            return Err("Invalid physical vector pen width".into());
        }
        let options = draw.stroke_options.as_ref();
        let cap = options.map_or(2, |o| o.cap);
        let dash = options.map_or(&[][..], |o| o.dash.as_slice());
        if dash.len() % 2 != 0 || dash.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return Err("Invalid physical vector dash pattern".into());
        }
        if width * density <= 1. {
            // Explicit rectangle clipping selects QCosmeticStroker FastDraw RGB32.
            // Without it the original pen uses ordinary span opacity (e.g. frame border).
            let fast = bounds.is_some();
            let raster_width = if fast { 1. } else { width * density };
            let premul = fast
                .then(|| fast_color(rgba, (width * density * 256.) as u32))
                .transpose()?;
            for path in &paths {
                let closed = path
                    .segments
                    .last()
                    .is_some_and(|s| matches!(s,Segment::Line(p)if *p==path.start));
                let spans = if dash.is_empty() {
                    ingetrazo_render::qt_cosmetic::rasterize_path_device(
                        &transformed(path, density, origin),
                        closed,
                        raster_width,
                        cap != 0,
                        Some(device),
                        100_000,
                    )?
                } else {
                    let mut spans = Vec::new();
                    let pattern = dash.iter().map(|v| v / width).collect::<Vec<_>>();
                    let mut current = path.start;
                    for segment in &path.segments {
                        let Segment::Line(to) = segment else {
                            return Err(
                                "Curved physical vector cosmetic dash remains unavailable".into()
                            );
                        };
                        let map =
                            |p: [f64; 2]| [p[0] * density + origin[0], p[1] * density + origin[1]];
                        spans.extend(ingetrazo_render::qt_cosmetic::rasterize_custom_line_device(
                            map(current),
                            map(*to),
                            raster_width,
                            cap != 0,
                            &pattern,
                            Some(device),
                            100_000,
                        )?);
                        current = *to;
                    }
                    spans
                };
                let spans = clip(spans, bounds);
                if !spans.is_empty() {
                    records.push(if let Some(premul) = premul {
                        crate::composer_border::physical_premultiplied_layer(spans, premul)?
                    } else {
                        crate::composer_border::physical_layers(vec![(spans, color(rgba)?)])?
                    });
                }
            }
            continue;
        }
        // QPaintEngineEx::drawLines emits at most16 source lines into one stroked
        // winding path, so touching antialiased caps combine before SourceOver.
        for batch in paths.chunks(16) {
            let mut outlines = Vec::new();
            for path in batch {
                let closed = path
                    .segments
                    .last()
                    .is_some_and(|s| matches!(s,Segment::Line(p)if *p==path.start));
                let mut a = DVec2::from_array(path.start);
                if closed && dash.is_empty() {
                    if options.is_some_and(|o| o.join != 2) {
                        return Err("Physical vector closed join remains unavailable".into());
                    }
                    let mut points = vec![a];
                    for segment in &path.segments[..path.segments.len() - 1] {
                        if let Segment::Line(p) = segment {
                            points.push(DVec2::from_array(*p));
                        } else {
                            return Err("Curved physical vector stroke remains unavailable".into());
                        }
                    }
                    for contour in crate::composer_cad::convex_bevel_outline(&points, width * 0.5)?
                    {
                        outlines.push(PathContour {
                            start: contour[0].to_array(),
                            segments: contour
                                .iter()
                                .skip(1)
                                .chain(contour.iter().take(1))
                                .map(|p| Segment::Line(p.to_array()))
                                .collect(),
                        });
                    }
                    continue;
                }
                for segment in &path.segments {
                    let Segment::Line(to) = segment else {
                        return Err("Curved physical vector stroke remains unavailable".into());
                    };
                    let b = DVec2::from_array(*to);
                    let length = (b - a).length();
                    if length > 0. {
                        let mut at = 0.;
                        let mut i = 0;
                        while at < length {
                            let end = if dash.is_empty() {
                                length
                            } else {
                                (at + dash[i % dash.len()]).min(length)
                            };
                            if dash.is_empty() || i % 2 == 0 {
                                if let Some(outline) = stroke_line(
                                    a + (b - a) * (at / length),
                                    a + (b - a) * (end / length),
                                    width,
                                    cap,
                                )? {
                                    outlines.push(outline);
                                }
                            }
                            at = end;
                            i += 1;
                            if i > 100_000 {
                                return Err("Physical vector dash admission exceeded".into());
                            }
                        }
                    }
                    a = b;
                }
            }
            let spans = clip(
                ingetrazo_render::qt_gray::rasterize_transformed_paths(
                    &outlines, density, origin, 250_000, 100_000,
                )?,
                bounds,
            );
            if !spans.is_empty() {
                records.push(crate::composer_border::physical_layers(vec![(
                    spans,
                    color(rgba)?,
                )])?);
            }
        }
    }
    Ok(records)
}
