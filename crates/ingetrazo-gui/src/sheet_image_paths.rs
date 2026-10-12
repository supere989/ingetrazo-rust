//! Literal raster View pen dispatch consuming the shared physical Draw protocol.
//! Qt6.12 qpaintengineex.cpp drawLines batches16; qcosmeticstroker.cpp FastDraw
//! scales the QRgba64 color before coverage; qmath_p.h clips rectangle endpoints.
use glam::DVec2;
use ingetrazo_formats::sheet_pdf::{Draw, PathOp};
use ingetrazo_render::{
    cad2d::{EncodedRgba, PremultipliedRgba8},
    qt_gray::{PathContour, Segment, Span},
};
thread_local! {
    static SPAN_OBSERVATIONS: std::cell::RefCell<Option<Vec<serde_json::Value>>> = const { std::cell::RefCell::new(None) };
}
/// Explicit cold diagnostic scope. Ordinary preparation has no observation owner.
pub(crate) fn observe_spans<R>(operation: impl FnOnce()->R)->(R,Vec<serde_json::Value>){
    struct Reset;
    impl Drop for Reset {fn drop(&mut self){SPAN_OBSERVATIONS.with(|v|{v.borrow_mut().take();});}}
    SPAN_OBSERVATIONS.with(|v|{assert!(v.borrow().is_none(),"nested span observation");*v.borrow_mut()=Some(Vec::new());});
    let guard=Reset;let result=operation();
    let observations=SPAN_OBSERVATIONS.with(|v|v.borrow_mut().take().unwrap());drop(guard);(result,observations)
}
fn span_rows(spans:&[Span])->serde_json::Value{serde_json::json!(spans.iter().map(|s|[i64::from(s.x),i64::from(s.y),i64::from(s.len),i64::from(s.coverage)]).collect::<Vec<_>>())}
fn path_rows(ops:&[PathOp])->serde_json::Value{serde_json::json!(ops.iter().map(|o|match o{PathOp::Move(p)=>serde_json::json!(["move",p]),PathOp::Line(p)=>serde_json::json!(["line",p]),PathOp::Cubic(p)=>serde_json::json!(["cubic",p]),PathOp::Close=>serde_json::json!(["close"])}).collect::<Vec<_>>())}
fn start_observation(draw:&Draw,spans:&[Span],density:f64,origin:[f64;2],bounds:Option<[i32;4]>,masks:&[Vec<Span>])->Option<serde_json::Value>{
    SPAN_OBSERVATIONS.with(|v|v.borrow().as_ref().map(|_|serde_json::json!({"path":path_rows(&draw.path),"origin_mm":draw.origin_mm,"density":density,"translation_px":origin,"stroke":draw.stroke,"options":draw.stroke_options.as_ref().map(|o|serde_json::json!({"cap":o.cap,"join":o.join,"dash":o.dash})),"clip":draw.clip.as_ref().map(|p|path_rows(p)),"clip_intersection":draw.clip_intersection.as_ref().map(|p|path_rows(p)),"clip_bounds":bounds,"masks":masks.iter().map(|m|span_rows(m)).collect::<Vec<_>>(),"preclip_spans":span_rows(spans)})))
}
fn finish_observation(mut observation:Option<serde_json::Value>,spans:&[Span],record:&crate::sheet_image::Records){
    if let Some(mut observation)=observation.take(){
        observation["postmask_spans"]=span_rows(spans);observation["geometry_id"]=serde_json::json!(record.0.id());
        SPAN_OBSERVATIONS.with(|v|{if let Some(rows)=v.borrow_mut().as_mut(){assert!(rows.len()<1000,"span observation bound");rows.push(observation);}});
    }
}
pub(crate) fn paths(ops: &[PathOp]) -> Result<Vec<PathContour>, String> {
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
        else { continue; };
        if a[1] != b[1] || b[0] != c[0] || c[1] != d[1] || d[0] != a[0] { continue; }
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
pub(crate) fn axis_rectangle(ops:&[PathOp])->Option<[f64;4]>{
 let [PathOp::Move(a),PathOp::Line(b),PathOp::Line(c),PathOp::Line(d),PathOp::Close]=ops else{return None;};
 (a[1]==b[1]&&b[0]==c[0]&&c[1]==d[1]&&d[0]==a[0]).then_some([a[0].min(c[0]),a[1].min(c[1]),a[0].max(c[0]),a[1].max(c[1])])
}
fn masks(draw:&Draw,density:f64,origin:[f64;2])->Result<Vec<Vec<Span>>,String>{
 let mut masks=Vec::new();
 for ops in [&draw.clip,&draw.clip_intersection].into_iter().flatten(){
  if axis_rectangle(ops).is_some(){continue;}
  masks.push(ingetrazo_render::qt_gray::rasterize_transformed_paths(&paths(ops)?,density,origin,250_000,100_000)?);
 }Ok(masks)
}
fn mask_clip(spans: Vec<Span>, masks:&[Vec<Span>])->Result<Vec<Span>,String>{
 let mut out=spans;
 for mask in masks{
  let mut clipped=Vec::new();
  for span in &out {
   let first=mask.partition_point(|s|s.y<span.y);let end=mask.partition_point(|s|s.y<=span.y);let row=&mask[first..end];
   let first=row.partition_point(|s|i64::from(s.x)+i64::from(s.len)<=i64::from(span.x));
   let end=row.partition_point(|s|i64::from(s.x)<i64::from(span.x)+i64::from(span.len));
   let part=ingetrazo_render::qt_gray::intersect_spans(std::slice::from_ref(span),&row[first..end],100_000)?;
   clipped.try_reserve(part.len()).map_err(|_|"Symbol clip allocation refused")?;clipped.extend(part);if clipped.len()>100_000{return Err("Symbol clip span budget exceeded".into());}
  }out=clipped;
 }Ok(out)
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
/// Cull only a complete, already-stroked outline whose conservative control hull
/// cannot touch the global page intersected with the literal rectangular clip.
/// Full stroke/cap/join geometry is already present; one device pixel adds AA fringe.
fn outline_visible(path:&PathContour,density:f64,origin:[f64;2],bounds:Option<[i32;4]>,device:[u32;2])->Result<bool,String>{
    let mut min=[f64::INFINITY;2];let mut max=[f64::NEG_INFINITY;2];
    let mut include=|p:[f64;2]|->Result<(),String>{
        if p.iter().any(|v|!v.is_finite()||v.abs()>1_000_000.){return Err("Gray raster input outside finite bounded coordinates".into());}
        let q=[p[0]*density+origin[0],p[1]*density+origin[1]];
        if q.iter().any(|v|!v.is_finite()||v.abs()>1_000_000.){return Err("Gray raster input outside finite bounded coordinates".into());}
        for axis in 0..2{min[axis]=min[axis].min(q[axis]);max[axis]=max[axis].max(q[axis]);}Ok(())
    };
    include(path.start)?;for segment in &path.segments{match segment{Segment::Line(to)=>include(*to)?,Segment::Cubic{control1,control2,to}=>{include(*control1)?;include(*control2)?;include(*to)?;}}}
    let b=bounds.unwrap_or([0,0,device[0]as i32,device[1]as i32]);
    let clip=[b[0].max(0)as f64,b[1].max(0)as f64,b[2].min(device[0]as i32)as f64,b[3].min(device[1]as i32)as f64];
    Ok(clip[0]<clip[2]&&clip[1]<clip[3]&&max[0]>=clip[0]-1.&&min[0]<=clip[2]+1.&&max[1]>=clip[1]-1.&&min[1]<=clip[3]+1.)
}
pub(crate) fn records(
    draws: &[Draw],
    density: f64,
    device: [u32; 2],
) -> Result<Vec<crate::sheet_image::Records>, String> {
    records_with_primitive(draws,density,device,false)
}
/// Only callers holding literal source drawLine provenance may select LinesHint.
pub(crate) fn source_line_records(draw:&Draw,density:f64,device:[u32;2])->Result<Vec<crate::sheet_image::Records>,String>{
    if !matches!(draw.path.as_slice(),[PathOp::Move(a),PathOp::Line(b)]if a!=b)||draw.stroke.is_none()||!draw.stroke_options.as_ref().is_some_and(|v|v.cap==2&&v.dash.is_empty()){return Err("Invalid source drawLine primitive admission".into());}
    records_with_primitive(std::slice::from_ref(draw),density,device,true)
}
/// Only literal source drawLine with equal endpoints may select the point primitive.
pub(crate) fn source_point_records(draw:&Draw,density:f64,device:[u32;2])->Result<Vec<crate::sheet_image::Records>,String>{
    let point=match draw.path.as_slice(){[PathOp::Move(a),PathOp::Line(b)]if a==b=>*a,_=>return Err("Invalid source point primitive".into())};
    if !density.is_finite()||density<=0.||draw.rotation_deg!=0.||draw.text.is_some()||draw.fill.is_some(){return Err("Invalid source point painter".into());}
    let(rgba,width)=draw.stroke.ok_or("Source point lacks pen")?;
    if !width.is_finite()||width<0.||width>1e6{return Err("Invalid source point pen".into());}
    let origin=draw.origin_mm.map(|q|q*density);let q=[origin[0]+point[0]*density,origin[1]+point[1]*density];
    if q.iter().any(|q|!q.is_finite()||q.abs()>1e6){return Err("Source point coordinate admission exceeded".into());}
    let effective=if width==0.{1.}else{width*density};
    let spans=if effective<=1.{let alpha=((255*((256.*effective)as u32).min(256))>>8)as u8;if alpha==0{vec![]}else{vec![ingetrazo_render::qt_gray::Span{x:q[0].floor()as i32,y:q[1].floor()as i32,len:1,coverage:alpha}]}}
    else if draw.stroke_options.as_ref().map_or(2,|v|v.cap)==0{vec![]}
    else{ingetrazo_render::qt_line::rasterize_flat([q[0]-effective/2.,q[1]],[q[0]+effective/2.,q[1]],effective,[0,0,device[0]as i32,device[1]as i32],250_000,100_000)?};
    let bounds=clip_bounds(draw,density,origin)?;let spans=clip(spans,bounds);let spans=clip(spans,Some([0,0,device[0]as i32,device[1]as i32]));let spans=mask_clip(spans,&masks(draw,density,origin)?)?;
    if spans.is_empty(){return Ok(vec![]);}Ok(vec![crate::composer_border::physical_layers(vec![(spans,EncodedRgba::from_bytes(rgba.map(|v|(v*255.).round()as u8),1.)?)])?])
}
/// Explicit original drawRect fill provenance, including uniform rotation.
pub(crate) fn source_rectangle_records(draw:&Draw,a:[f64;2],b:[f64;2],ratio:f64,density:f64,device:[u32;2])->Result<Vec<crate::sheet_image::Records>,String>{
 if draw.stroke.is_some()||draw.text.is_some()||draw.rotation_deg!=0.{return Err("Invalid source rectangle fill admission".into());}
 let color=EncodedRgba(draw.fill.ok_or("Source rectangle lacks brush")?);color.validate()?;
 let origin=draw.origin_mm.map(|v|v*density);let map=|p:[f64;2]|[p[0]*density+origin[0],p[1]*density+origin[1]];
 let spans=ingetrazo_render::qt_line::rasterize_rect_fill(map(a),map(b),ratio,[0,0,device[0]as i32,device[1]as i32],250_000,100_000)?;
 let spans=clip(spans,clip_bounds(draw,density,origin)?);let spans=mask_clip(spans,&masks(draw,density,origin)?)?;
 if spans.is_empty(){return Ok(vec![]);}Ok(vec![crate::composer_border::physical_layers(vec![(spans,color)])?])
}
fn records_with_primitive(draws:&[Draw],density:f64,device:[u32;2],line_hint:bool)->Result<Vec<crate::sheet_image::Records>,String>{
    let mut records = Vec::new();
    for draw in draws {
        if draw.rotation_deg!=0. || draw.text.is_some() || draw.even_odd {
            return Err("Physical vector text/even-odd painter remains unavailable".into());
        }
        let origin = draw.origin_mm.map(|v| v * density);
        let paths = paths(&draw.path)?;
        let bounds = clip_bounds(draw, density, origin)?;
        let masks=masks(draw,density,origin)?;
        // QColor::setAlphaF retains16-bit channels. Keep that authored solid
        // brush until QRgba64 premultiplication, then round to target bytes.
        let color = |rgba: [f32; 4]| {let color=EncodedRgba(rgba);color.validate()?;Ok::<_,String>(color)};
        if let Some(fill) = draw.fill {
            let spans = clip(
                ingetrazo_render::qt_gray::rasterize_transformed_paths(
                    &paths, density, origin, 250_000, 100_000,
                )?,
                bounds,
            );
            let spans = mask_clip(spans,&masks)?;
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
                } else if path.segments.iter().any(|s|matches!(s,Segment::Cubic{..})) {
                    let pattern=dash.iter().map(|v|v/width).collect::<Vec<_>>();
                    ingetrazo_render::qt_cosmetic::rasterize_custom_path_device(
                        &transformed(path,density,origin),closed,raster_width,cap!=0,&pattern,Some(device),100_000,
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
                let spans = mask_clip(spans,&masks)?;
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
        // QRasterPaintEngine::stroke(LinesHint): a wide, solid SquareCap pen
        // uses QRasterizer::rasterizeLine directly (16.16 area ×255), not Gray paths.
        // Physical source rules/dividers are literal two-point drawLine primitives.
        if line_hint&&cap==2&&dash.is_empty()&&paths.iter().all(|p|matches!(p.segments.as_slice(),[Segment::Line(to)]if *to!=p.start)){
            let mut clip=bounds.unwrap_or([0,0,device[0]as i32,device[1]as i32]);
            clip[0]=clip[0].max(0);clip[1]=clip[1].max(0);clip[2]=clip[2].min(device[0]as i32);clip[3]=clip[3].min(device[1]as i32);
            if clip[0]<clip[2]&&clip[1]<clip[3]{for path in &paths{let Segment::Line(to)=path.segments[0]else{unreachable!()};let map=|p:[f64;2]|[p[0]*density+origin[0],p[1]*density+origin[1]];
                let spans=ingetrazo_render::qt_line::rasterize(map(path.start),map(to),width*density,clip,250_000,100_000)?;
                let spans=mask_clip(spans,&masks)?;if !spans.is_empty(){records.push(crate::composer_border::physical_layers(vec![(spans,color(rgba)?)])?);}
            }}continue;
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
                if !dash.is_empty() && cap == 0 && options.is_some_and(|o| o.join == 2)
                    && path.segments.iter().any(|s| matches!(s, Segment::Cubic { .. }))
                {
                    outlines.extend(source_dashed_bevel(path, width, dash, density)?);
                    continue;
                }
                let mut a = DVec2::from_array(path.start);
                if closed && dash.is_empty() && path.segments.iter().any(|s|matches!(s,Segment::Cubic{..})) {
                    outlines.extend(crate::composer_cad::source_curve_stroke(path,width*0.5)?);continue;
                }
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
            // Preserve drawLines batching and local flatten arithmetic for every
            // intersecting contour. Only complete offscreen stroked outlines disappear.
            let mut refusal=None;
            outlines.retain(|path|match outline_visible(path,density,origin,bounds,device){Ok(visible)=>visible,Err(e)=>{refusal=Some(e);false}});
            if let Some(error)=refusal{return Err(error);}
            let spans=ingetrazo_render::qt_gray::rasterize_transformed_paths(&outlines,density,origin,250_000,100_000)?;
            let observation=start_observation(draw,&spans,density,origin,bounds,&masks);
            let spans=mask_clip(clip(spans,bounds),&masks)?;
            if !spans.is_empty() {
                // Diagnostic snapshots observe these exact published runs;
                // ordinary preparation does not clone or rerasterize them.
                let observed=observation.as_ref().map(|_|spans.clone());
                let record=crate::composer_border::physical_layers(vec![(spans,color(rgba)?)])?;
                if let Some(spans)=observed{finish_observation(observation,&spans,&record);}
                records.push(record);
            }
        }
    }
    Ok(records)
}

// Qt6.12 QDashStroker::processCurrentSubpath: QSubpathFlatIterator uses
// QBezier::toPolygon(0.5 / device scale), then carries position/offset across
// every flattened edge. Flat caps and bevel joins are the callout's actual pen.
fn source_dashed_bevel(
    path: &PathContour,
    width: f64,
    dash: &[f64],
    density: f64,
) -> Result<Vec<PathContour>, String> {
    if dash.len() != 2 || dash.iter().any(|d| !d.is_finite() || *d <= 0.) {
        return Err("Unsupported curved physical dash inventory".into());
    }
    let points = ingetrazo_render::qt_gray::flatten_local_path(path, density * 0.5, 100_000)?;
    let mut runs: Vec<Vec<DVec2>> = Vec::new();
    let mut run = Vec::new();
    let (mut idash, mut pos, mut offset, mut start) = (0usize, 0., 0., 0.);
    let mut steps = 0usize;
    for edge in points.windows(2) {
        let a = DVec2::from_array(edge[0]);
        let b = DVec2::from_array(edge[1]);
        let length = (b - a).length();
        let stop = start + length;
        while pos < stop {
            steps += 1;
            if steps > 100_000 {
                return Err("Physical curved dash budget exceeded".into());
            }
            let even = idash & 1 == 0;
            let dpos = (pos + dash[idash]) - (offset + start);
            let p = if dpos > length {
                b
            } else {
                a + (b - a) * (dpos / length)
            };
            if even {
                if run.is_empty() {
                    run.try_reserve(2)
                        .map_err(|_| "Physical dash allocation refused")?;
                    run.push(a + (b - a) * ((pos - start) / length));
                }
                run.try_reserve(1)
                    .map_err(|_| "Physical dash allocation refused")?;
                run.push(p);
            }
            if dpos > length {
                offset = dash[idash] - (dpos - length);
                pos = stop;
            } else {
                pos = dpos + start;
                if even && !run.is_empty() {
                    runs.try_reserve(1).map_err(|_| "Physical dash allocation refused")?;
                    runs.push(std::mem::take(&mut run));
                }
                idash = (idash + 1) % 2;
                offset = 0.;
            }
        }
        start = stop;
    }
    if !run.is_empty() {
        runs.try_reserve(1).map_err(|_| "Physical dash allocation refused")?;
        runs.push(run);
    }
    let mut out = Vec::new();
    let mut boundary_points=0usize;
    for run in runs {
        let extra = run.len().checked_mul(2).ok_or("Physical dashed outline count overflow")?;
        if out.len().checked_add(extra).is_none_or(|n| n > 250_000) {
            return Err("Physical dashed outline budget exceeded".into());
        }
        out.try_reserve(1).map_err(|_| "Physical dash outline allocation refused")?;
        // QStroker emits one forward/backward boundary per open dash.
        // Separate edge fills would quantize internal edges absent from it.
        let bound=extra.checked_mul(3).ok_or("Dash boundary size overflow")?;
        if boundary_points.checked_add(bound).is_none_or(|n|n>250_000){return Err("Dash boundary budget exceeded".into());}
        let mut boundary=Vec::new();boundary.try_reserve(bound).map_err(|_|"Dash boundary allocation refused")?;
        let mut previous:Option<(DVec2,DVec2)>=None;
        for backward in [false,true]{
            let mut first=true;
            for j in 0..run.len().saturating_sub(1){
                let (a,b)=if backward{(run[run.len()-1-j],run[run.len()-2-j])}else{(run[j],run[j+1])};
                let d=b-a;if d.x==0.&&d.y==0.{continue;}
                // QLineF normalVector/setLength retains rooted cancellation.
                // Reverse-side offsets are computed independently.
                let tip=a+DVec2::new(d.y,-d.x);let n=tip-a;let length=n.x.hypot(n.y);
                let tip=if length>0.{a+(width*0.5)*(n/length)}else{tip};let n=tip-a;
                let line=(a+n,b+n);
                if let Some(prev)=previous{
                    let fuzzy=|x:f64,y:f64|(x-y).abs()*1e12<=x.abs().min(y.abs());
                    if !(fuzzy(prev.1.x,line.0.x)&&fuzzy(prev.1.y,line.0.y)){
                        if !first{
                            let va=prev.1-prev.0;let vb=line.0-line.1;let c=prev.0-line.0;
                            let denom=va.y*vb.x-va.x*vb.y;
                            let bounded=if denom==0.||!denom.is_finite(){false}else{let na=(vb.y*c.x-vb.x*c.y)/denom;let nb=(va.x*c.y-va.y*c.x)/denom;(0. ..=1.).contains(&na)&&(0. ..=1.).contains(&nb)};
                            let shortcut=line.0-prev.1;
                            let angle=|v:DVec2|(-v.y).atan2(v.x).to_degrees().rem_euclid(360.);
                            let turn=(angle(va)-angle(shortcut)).rem_euclid(360.);
                            if bounded||(turn>90.&&!fuzzy(turn,90.)){boundary.push(a);}
                        }
                        boundary.push(line.0);
                    }
                }else{boundary.push(line.0);}
                boundary.push(line.1);previous=Some(line);first=false;
            }
        }
        boundary_points+=boundary.len();
        if let Some(first)=boundary.first().copied(){
            let mut segments=Vec::new();segments.try_reserve(boundary.len()).map_err(|_|"Dash boundary allocation refused")?;
            segments.extend(boundary.iter().skip(1).map(|p|Segment::Line(p.to_array())));segments.push(Segment::Line(first.to_array()));
            out.push(PathContour{start:first.to_array(),segments});
        }
        if out.len() > 250_000 {
            return Err("Physical dashed outline budget exceeded".into());
        }
    }
    Ok(out)
}
