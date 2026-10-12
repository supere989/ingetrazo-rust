//! ezdxf 1.4.4 make_path -> cubic Bezier -> adaptive flattening law.
//! Work in authored DXF coordinates until the source's final scale operation.
use super::DxfError;
use glam::DVec3;
use std::f64::consts::{PI, TAU};
const MAX_POINTS: usize = 1_000_000;
fn error(s: &str) -> DxfError {
    DxfError::Parse(s.into())
}
fn close_scalar(a: f64, b: f64, rel: f64, abs: f64) -> bool {
    (a - b).abs() <= (rel * a.abs().max(b.abs())).max(abs)
}
fn close(a: DVec3, b: DVec3, rel: f64, abs: f64) -> bool {
    a.to_array()
        .into_iter()
        .zip(b.to_array())
        .all(|(a, b)| close_scalar(a, b, rel, abs))
}
#[derive(Clone, Copy)]
pub(super) struct Ocs {
    pub x: DVec3,
    pub y: DVec3,
    pub z: DVec3,
}
impl Ocs {
    pub fn map(self, p: DVec3) -> DVec3 {
        self.x * p.x + self.y * p.y + self.z * p.z
    }
}
pub(super) fn ocs(normal: DVec3) -> Result<Ocs, DxfError> {
    let len = normal.length();
    if !len.is_finite() || len == 0. {
        return Err(error("Invalid curve extrusion"));
    }
    let z = normal / len;
    let a = if z.x.abs() < 1. / 64. && z.y.abs() < 1. / 64. {
        DVec3::Y.cross(z)
    } else {
        DVec3::Z.cross(z)
    };
    let x = a / a.length();
    Ok(Ocs {
        x,
        y: z.cross(x),
        z,
    })
}
fn span(start: f64, end: f64) -> f64 {
    if close_scalar(start, end, 1e-9, 1e-15) {
        return 0.;
    }
    let start = start.rem_euclid(TAU);
    if close_scalar(start, end.rem_euclid(TAU), 1e-9, 1e-15) {
        return TAU;
    }
    let mut end = if close_scalar(end, TAU, 1e-9, 1e-15) {
        end
    } else {
        end.rem_euclid(TAU)
    };
    if end < start {
        end += TAU;
    }
    end - start
}
fn ellipse_controls(
    center: DVec3,
    major: DVec3,
    minor: DVec3,
    start: f64,
    end: f64,
) -> Vec<[DVec3; 4]> {
    let delta = span(start, end);
    if delta.abs() < 1e-9 {
        return Vec::new();
    }
    let count = (delta / PI * 2.).ceil().max(1.) as usize;
    let step = delta / count as f64;
    let tangent = (4. / 3.) * libm::tan(step / 4.);
    let mut angle = start.rem_euclid(TAU);
    let mut last = DVec3::new(libm::cos(angle), libm::sin(angle), 0.);
    let transform = |p: DVec3| center + major * p.x + minor * p.y;
    let mut curves = Vec::with_capacity(count);
    for _ in 0..count {
        let first = last;
        angle += step;
        last = DVec3::new(libm::cos(angle), libm::sin(angle), 0.);
        let c1 = first + DVec3::new(-first.y * tangent, first.x * tangent, 0.);
        let c2 = last + DVec3::new(last.y * tangent, -last.x * tangent, 0.);
        curves.push([
            transform(first),
            transform(c1),
            transform(c2),
            transform(last),
        ]);
    }
    curves
}
#[derive(Clone, Copy)]
enum Command {
    Line(DVec3),
    Cubic([DVec3; 3]),
}
struct Path {
    start: DVec3,
    end: DVec3,
    commands: Vec<Command>,
}
impl Path {
    fn new(start: DVec3) -> Self {
        Self {
            start,
            end: start,
            commands: Vec::new(),
        }
    }
    fn line(&mut self, p: DVec3) {
        self.commands.push(Command::Line(p));
        self.end = p;
    }
    fn beziers(&mut self, mut curves: Vec<[DVec3; 4]>) {
        if curves.is_empty() {
            return;
        }
        if close(self.end, curves.last().unwrap()[3], 1e-9, 1e-12) {
            curves.reverse();
            for c in &mut curves {
                c.reverse();
            }
        }
        for [a, b, c, d] in curves {
            if !close(a, self.end, 1e-9, 1e-12) {
                self.line(a);
            }
            if close(a, b, 1e-15, 0.) && close(d, c, 1e-15, 0.) {
                self.line(d);
            } else {
                self.commands.push(Command::Cubic([b, c, d]));
                self.end = d;
            }
        }
    }
    fn transform(&mut self, ocs: Ocs, elevation: f64) {
        let map = |p: DVec3| ocs.map(DVec3::new(p.x, p.y, elevation));
        self.start = map(self.start);
        self.end = map(self.end);
        for cmd in &mut self.commands {
            match cmd {
                Command::Line(p) => *p = map(*p),
                Command::Cubic(cp) => {
                    for p in cp {
                        *p = map(*p)
                    }
                }
            }
        }
    }
    fn flatten(self, scale: f64) -> Result<Vec<DVec3>, DxfError> {
        if self.commands.is_empty() {
            return Ok(Vec::new());
        }
        let distance = (0.01 / scale).max(1e-6);
        let mut out = vec![self.start];
        let mut start = self.start;
        for cmd in self.commands {
            match cmd {
                Command::Line(end) => out.push(end),
                Command::Cubic([b, c, end]) => {
                    flatten_cubic([start, b, c, end], distance, &mut out)?
                }
            }
            start = match cmd {
                Command::Line(p) => p,
                Command::Cubic(cp) => cp[2],
            };
            if out.len() > MAX_POINTS {
                return Err(error("Curve tessellation limit exceeded"));
            }
        }
        for p in &mut out {
            *p *= scale;
            if !p.is_finite() {
                return Err(error("Import coordinate overflow"));
            }
        }
        Ok(out)
    }
}
fn flatten_cubic(cp: [DVec3; 4], distance: f64, out: &mut Vec<DVec3>) -> Result<(), DxfError> {
    // ezdxf subtracts the first point before Bernstein evaluation, then adds
    // the offset last. Do not evaluate a direct weighted world-coordinate sum.
    let offset = cp[0];
    let p1 = cp[1] - offset;
    let p2 = cp[2] - offset;
    let p3 = cp[3] - offset;
    let point = |t: f64| {
        let t2 = t * t;
        let u = 1. - t;
        p1 * (3. * u * u * t) + p2 * (3. * u * t2) + p3 * (t2 * t) + offset
    };
    let mut t0 = 0.;
    let mut start = cp[0];
    let mut stack = Vec::new();
    while t0 < 1. {
        let mut t1 = t0 + 0.125;
        let mut end = if close_scalar(t1, 1., 1e-9, 0.) {
            t1 = 1.;
            cp[3]
        } else {
            point(t1)
        };
        loop {
            let mid = (t0 + t1) * 0.5;
            let p = point(mid);
            let check = start + (end - start) * 0.5;
            if !p.is_finite() {
                return Err(error("Curve coordinate overflow"));
            }
            if check.distance(p) < distance {
                out.push(end);
                if out.len() > MAX_POINTS {
                    return Err(error("Curve tessellation limit exceeded"));
                }
                t0 = t1;
                start = end;
                if let Some((next, p)) = stack.pop() {
                    t1 = next;
                    end = p;
                } else {
                    break;
                }
            } else {
                if stack.len() >= 64 || mid == t0 || mid == t1 {
                    return Err(error("Curve flattening limit exceeded"));
                }
                stack.push((t1, end));
                t1 = mid;
                end = p;
            }
        }
    }
    Ok(())
}
pub(super) fn ellipse(
    center: DVec3,
    major: DVec3,
    normal: DVec3,
    ratio: f64,
    start: f64,
    end: f64,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    let cross = normal.cross(major);
    let len = cross.length();
    if !len.is_finite() || len == 0. || !ratio.is_finite() {
        return Err(error("Invalid ellipse axes"));
    }
    let minor = cross * (major.length() * ratio / len);
    let first = center + major * libm::cos(start) + minor * libm::sin(start);
    let mut path = Path::new(first);
    path.beziers(ellipse_controls(center, major, minor, start, end));
    path.flatten(scale)
}
/// Preserve Circle/Arc.transform's uniform OCS law; only nonuniform
/// transformed XY lengths use the source Ellipse.from_arc fallback.
pub(super) fn transformed_circle(
    center: DVec3,
    radius: f64,
    normal: DVec3,
    start: f64,
    end: f64,
    m: glam::DMat4,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    let old = ocs(normal)?;
    let x = m.transform_vector3(old.x);
    let y = m.transform_vector3(old.y);
    if !close_scalar(x.length_squared(), y.length_squared(), 1e-9, 1e-9) {
        return transformed_ellipse(
            old.map(center),
            old.x * radius.abs(),
            normal,
            1.,
            start,
            end,
            m,
            scale,
        );
    }
    let n = x.cross(y);
    let len = n.length();
    if len == 0. || !len.is_finite() {
        return Err(error("Invalid transformed circle axes"));
    }
    let n = n / len;
    let new = ocs(n)?;
    let local = |p: DVec3| DVec3::new(new.x.dot(p), new.y.dot(p), new.z.dot(p));
    let center = local(m.transform_point3(old.map(center)));
    let radius = m
        .transform_vector3(old.map(DVec3::new(radius, 0., 0.)))
        .length();
    let old_span = span(start, end);
    // Arc.transform guards full360 before calling transform_ccw_arc_angles;
    // preserve its authored seam too (not the unused helper's rotated seam).
    let (start, end) = if close_scalar(old_span, TAU, 1e-9, 0.) {
        (start, end)
    } else {
        let angle = |a: f64| {
            let d = local(m.transform_vector3(old.map(DVec3::new(libm::cos(a), libm::sin(a), 0.))));
            libm::atan2(d.y, d.x)
        };
        let mut s = angle(start);
        let mut e = angle(end);
        let semicircle = close_scalar(old_span, PI, 1e-9, 0.);
        let old_check = if semicircle { 1. } else { old_span };
        let new_check = span(s, if semicircle { angle(start + 1.) } else { e });
        if !close_scalar(old_check, new_check, 1e-8, 0.) {
            std::mem::swap(&mut s, &mut e);
        }
        // Arc's DXF degrees are stored before make_path converts back to radians.
        (s.to_degrees().to_radians(), e.to_degrees().to_radians())
    };
    circle(center, radius, n, start, end, scale)
}
/// ezdxf ConstructionEllipse.transform: transformed analytic axes/parameters
/// precede make_path and sagitta subdivision (not transformed sample points).
pub(super) fn transformed_ellipse(
    center: DVec3,
    major: DVec3,
    normal: DVec3,
    ratio: f64,
    mut start: f64,
    mut end: f64,
    m: glam::DMat4,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    let normalized = |v: DVec3, length: f64| -> Result<DVec3, DxfError> {
        let n = v.length();
        if n == 0. || !n.is_finite() {
            Err(error("Invalid transformed ellipse axes"))
        } else {
            Ok(v * (length / n))
        }
    };
    let original_minor = normalized(normal.cross(major), major.length() * ratio)?;
    let transformed_center = m.transform_point3(center);
    let mut a = m.transform_vector3(major);
    let mut b = m.transform_vector3(original_minor);
    let adjust = normalized(a, 1.)?.dot(normalized(b, 1.)?).abs() > 1e-6;
    let authored_ratio = ratio;
    let mut ratio;
    if adjust {
        let q = a;
        let p = if a.z.abs() <= 1e-9 && b.z.abs() <= 1e-9 {
            DVec3::new(b.y, -b.x, 0.)
        } else {
            normalized(a.cross(b).cross(b), b.length())?
        };
        let d = p + (q - p) * 0.5;
        let rv = normalized(q - p, d.length())?;
        let aa = d - rv;
        let bb = d + rv;
        let major_len = (aa - q).length();
        let minor_len = (bb - q).length();
        if major_len == 0. || minor_len == 0. {
            return Err(error("Invalid transformed ellipse axes"));
        }
        ratio = minor_len / major_len;
        a = normalized(bb, major_len)?;
        b = normalized(aa, minor_len)?;
    } else {
        ratio = b.length() / a.length();
    }
    let extrusion = normalized(a.cross(b), 1.)?;
    if !adjust {
        b = normalized(extrusion.cross(a), a.length() * ratio)?;
    }
    if adjust && !close_scalar(start, end, 1e-9, 1e-9) {
        let old_span = (end - start).rem_euclid(TAU);
        let x = normalized(a, 1.)?;
        let y = normalized(b, 1.)?;
        // The old vertex uses the authored ratio, not the transformed ratio.
        let old_vertex = |t: f64| {
            center
                + normalized(major, 1.).unwrap() * (libm::cos(t) * major.length())
                + normalized(original_minor, 1.).unwrap()
                    * (libm::sin(t) * major.length() * authored_ratio)
        };
        let param = |p: DVec3| libm::atan2(y.dot(p) / ratio, x.dot(p)).rem_euclid(TAU);
        let old_start = start;
        let old_end = end;
        start = param(m.transform_point3(old_vertex(start)) - transformed_center);
        end = param(m.transform_point3(old_vertex(end)) - transformed_center);
        let swap = if !close_scalar(old_span, PI, 1e-9, 1e-9) {
            !close_scalar(old_span, (end - start).rem_euclid(TAU), 1e-9, 1e-9)
        } else {
            let mid = |s: f64, mut e: f64| {
                if e < s {
                    e += TAU;
                }
                (s + e) / 2.
            };
            let old = m.transform_point3(old_vertex(mid(old_start, old_end)));
            let t = mid(start, end);
            let new = transformed_center
                + normalized(a, 1.)? * (libm::cos(t) * a.length())
                + normalized(b, 1.)? * (libm::sin(t) * a.length() * ratio);
            !close(old, new, 1e-9, 1e-9)
        };
        if swap {
            std::mem::swap(&mut start, &mut end);
        }
    }
    if ratio > 1. {
        a = normalized(extrusion.cross(a), a.length() * ratio)?;
        ratio = 1. / ratio;
        if !(close_scalar(start, 0., 1e-9, 0.) && close_scalar(end, TAU, 1e-9, 0.)) {
            start -= PI / 2.;
            end -= PI / 2.;
        }
    }
    start = start.rem_euclid(TAU);
    end = end.rem_euclid(TAU);
    if close_scalar(start, end, 1e-9, 0.) {
        start = 0.;
        end = TAU;
    }
    ellipse(transformed_center, a, extrusion, ratio, start, end, scale)
}
pub(super) fn circle(
    center: DVec3,
    radius: f64,
    normal: DVec3,
    start: f64,
    end: f64,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    if radius.abs() <= 1e-12 {
        return Ok(Vec::new());
    }
    let ocs = ocs(normal)?;
    ellipse(
        ocs.map(center),
        ocs.map(DVec3::new(radius.abs(), 0., 0.)),
        normal,
        1.,
        start,
        end,
        scale,
    )
}
pub(super) fn polyline(
    vertices: &[(DVec3, f64)],
    closed: bool,
    normal: DVec3,
    elevation: f64,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    if vertices.len() < 2 {
        return Ok(Vec::new());
    }
    let mut path = Path::new(DVec3::new(vertices[0].0.x, vertices[0].0.y, 0.));
    let segments = vertices.len() - 1
        + usize::from(closed && !close(vertices[0].0, vertices.last().unwrap().0, 1e-10, 0.));
    for i in 0..segments {
        let a = vertices[i % vertices.len()].0;
        let b = vertices[(i + 1) % vertices.len()].0;
        let bulge = vertices[i % vertices.len()].1;
        if bulge.abs() < 1e-6 {
            path.line(DVec3::new(b.x, b.y, 0.));
            continue;
        }
        if close(
            DVec3::new(a.x, a.y, 0.),
            DVec3::new(b.x, b.y, 0.),
            1e-10,
            0.,
        ) {
            continue;
        }
        let radius = libm::hypot(b.x - a.x, b.y - a.y) * (1. + bulge * bulge) / 4. / bulge;
        let angle = libm::atan2(b.y - a.y, b.x - a.x) + (PI / 2. - libm::atan(bulge) * 2.);
        let center = DVec3::new(
            a.x + radius * libm::cos(angle),
            a.y + radius * libm::sin(angle),
            0.,
        );
        let angle_a = libm::atan2(a.y - center.y, a.x - center.x);
        let angle_b = libm::atan2(b.y - center.y, b.x - center.x);
        let (start, end) = if bulge < 0. {
            (angle_b, angle_a)
        } else {
            (angle_a, angle_b)
        };
        let start = start.rem_euclid(TAU);
        let mut end = end.rem_euclid(TAU);
        if start > end {
            end += TAU;
        }
        // Source converts radians to degrees and back through from_arc.
        let start = start.to_degrees().to_radians();
        let end = end.to_degrees().to_radians();
        let mut curves = ellipse_controls(
            center,
            DVec3::X * radius.abs(),
            DVec3::Y * radius.abs(),
            start,
            end,
        );
        if !curves.is_empty() && close(curves[0][0], DVec3::new(b.x, b.y, 0.), 1e-10, 0.) {
            curves.reverse();
            for c in &mut curves {
                c.reverse();
            }
        }
        path.beziers(curves);
    }
    path.transform(ocs(normal)?, elevation);
    path.flatten(scale)
}

pub(super) fn bezier_path(
    start: DVec3,
    curves: Vec<[DVec3; 4]>,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    let mut path = Path::new(start);
    path.beziers(curves);
    path.flatten(scale)
}

#[cfg(test)]
mod math_diagnostic {
    use super::*;
    fn trace(portable: bool) -> serde_json::Value {
        let sin: fn(f64) -> f64 = if portable { libm::sin } else { f64::sin };
        let cos: fn(f64) -> f64 = if portable { libm::cos } else { f64::cos };
        let atan: fn(f64) -> f64 = if portable { libm::atan } else { f64::atan };
        let atan2: fn(f64, f64) -> f64 = if portable { libm::atan2 } else { f64::atan2 };
        let radius = 1.25;
        let angle = PI / 2. - atan(0.5) * 2.;
        let center = DVec3::new(radius * cos(angle), radius * sin(angle), 0.);
        let raw_start = atan2(-center.y, -center.x);
        let raw_end = atan2(-center.y, 2. - center.x);
        let start = raw_start.rem_euclid(TAU);
        let mut end = raw_end.rem_euclid(TAU);
        if start > end {
            end += TAU;
        }
        let ellipse_start = start.to_degrees().to_radians();
        let ellipse_end = end.to_degrees().to_radians();
        let delta = span(ellipse_start, ellipse_end);
        let count = (delta / PI * 2.).ceil().max(1.) as usize;
        let step = delta / count as f64;
        let mut final_angle = ellipse_start.rem_euclid(TAU);
        for _ in 0..count {
            final_angle += step;
        }
        let last_cp = if portable {
            ellipse_controls(
                center,
                DVec3::X * radius,
                DVec3::Y * radius,
                ellipse_start,
                ellipse_end,
            )
            .last()
            .unwrap()[3]
        } else {
            center + DVec3::X * (radius * cos(final_angle)) + DVec3::Y * (radius * sin(final_angle))
        };
        serde_json::json!({"center":[center.x,center.y],"radius":radius,"raw_start":raw_start,"raw_end":raw_end,"normalized_start":start,"normalized_end":end,"ellipse_start":ellipse_start,"ellipse_end":ellipse_end,"span":delta,"step":step,"final_accumulated_angle":final_angle,"sin_final":sin(final_angle),"cos_final":cos(final_angle),"last_cp":last_cp.to_array()})
    }
    #[test]
    fn portable_bulge_intermediates_match_source() {
        let portable = trace(true);
        let native = trace(false);
        println!(
            "{}",
            serde_json::json!({"portable_curve_math":portable,"platform_curve_math":native})
        );
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/dxf-curves/lw-positive-math-source.json"
        ))
        .unwrap();
        assert_eq!(portable, source);
    }
}
