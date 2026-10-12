//! Bounded Qt6.12 qt_curves_for_arc / qt_find_ellipse_coords algebra.
//! https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qstroker.cpp
//! https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qpainterpath.cpp
use ingetrazo_formats::sheet_pdf::PathOp;
const K: f64 = 0.5522847498;
fn t_for_angle(angle: f64) -> f64 {
    if angle.abs() <= 1e-12 {
        return 0.;
    }
    if (angle - 90.).abs() <= angle.abs().min(90.) * 1e-12 {
        return 1.;
    }
    let r = angle.to_radians();
    let (co, si) = (r.cos(), r.sin());
    let mut tc = angle / 90.;
    for _ in 0..2 {
        tc -= ((((2. - 3. * K) * tc + 3. * (K - 1.)) * tc) * tc + 1. - co)
            / (((6. - 9. * K) * tc + 6. * (K - 1.)) * tc);
    }
    let mut ts = tc;
    for _ in 0..2 {
        ts -= ((((3. * K - 2.) * ts - 6. * K + 3.) * ts + 3. * K) * ts - si)
            / (((9. * K - 6.) * ts + 12. * K - 6.) * ts + 3. * K);
    }
    0.5 * (tc + ts)
}
fn ellipse_point(r: [f64; 4], angle: f64) -> [f64; 2] {
    let theta = angle - 360. * (angle / 360.).floor();
    let mut t = theta / 90.;
    let q = t as i32;
    t -= f64::from(q);
    t = t_for_angle(90. * t);
    if q & 1 != 0 {
        t = 1. - t;
    }
    let mt = 1. - t;
    let mut b = mt * mt;
    let mut c = t * t;
    let d = c * t;
    let a = b * mt;
    b *= 3. * t;
    c *= 3. * mt;
    let mut p = [a + b + c * K, d + c + b * K];
    if q == 1 || q == 2 {
        p[0] = -p[0];
    }
    if q == 0 || q == 1 {
        p[1] = -p[1];
    }
    [
        r[0] + r[2] / 2. + r[2] / 2. * p[0],
        r[1] + r[3] / 2. + r[3] / 2. * p[1],
    ]
}
fn split(p: &mut [[f64; 2]; 4], t: f64) -> [[f64; 2]; 4] {
    let mut left = [[0.; 2]; 4];
    for a in 0..2 {
        left[0][a] = p[0][a];
        left[1][a] = p[0][a] + t * (p[1][a] - p[0][a]);
        left[2][a] = p[1][a] + t * (p[2][a] - p[1][a]);
        p[2][a] += t * (p[3][a] - p[2][a]);
        p[1][a] = left[2][a] + t * (p[2][a] - left[2][a]);
        left[2][a] = left[1][a] + t * (left[2][a] - left[1][a]);
        left[3][a] = left[2][a] + t * (p[1][a] - left[2][a]);
        p[0][a] = left[3][a];
    }
    left
}
fn interval(mut p: [[f64; 2]; 4], a: f64, b: f64) -> [[f64; 2]; 4] {
    if a == 0. && b == 1. {
        return p;
    }
    split(&mut p, a);
    split(&mut p, (b - a) / (1. - a))
}
pub(super) fn arc(r: [f64; 4], start: f64, sweep: f64) -> Result<Vec<PathOp>, String> {
    if r.iter()
        .chain([start, sweep].iter())
        .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err("Invalid annotation arc".into());
    }
    if r[2] == 0. && r[3] == 0. {
        return Ok(vec![]);
    }
    let [x, y, w, h] = r;
    let (w2, h2) = (w / 2., h / 2.);
    let (wk, hk) = (w2 * K, h2 * K);
    let points = [
        [x + w, y + h2],
        [x + w, y + h2 + hk],
        [x + w2 + wk, y + h],
        [x + w2, y + h],
        [x + w2 - wk, y + h],
        [x, y + h2 + hk],
        [x, y + h2],
        [x, y + h2 - hk],
        [x + w2 - wk, y],
        [x + w2, y],
        [x + w2 + wk, y],
        [x + w, y + h2 - hk],
        [x + w, y + h2],
    ];
    let sweep = sweep.clamp(-360., 360.);
    if start == 0. && sweep.abs() == 360. {
        let indices: Vec<usize> = if sweep > 0. {
            (0..12).rev().collect()
        } else {
            (1..13).collect()
        };
        let mut out = vec![PathOp::Move(if sweep > 0. {
            points[12]
        } else {
            points[0]
        })];
        for chunk in indices.chunks_exact(3) {
            let [a, b, c] = [points[chunk[0]], points[chunk[1]], points[chunk[2]]];
            out.push(PathOp::Cubic([a[0], a[1], b[0], b[1], c[0], c[1]]));
        }
        return Ok(out);
    }
    let (mut first, mut last) = (
        (start / 90.).floor() as i32,
        ((start + sweep) / 90.).floor() as i32,
    );
    let (mut ta, mut tb) = (
        (start - f64::from(first) * 90.) / 90.,
        (start + sweep - f64::from(last) * 90.) / 90.,
    );
    let step = if sweep > 0. { 1 } else { -1 };
    if step < 0 {
        ta = 1. - ta;
        tb = 1. - tb;
    }
    if (ta - 1.).abs() <= 1e-12 {
        ta = 0.;
        first += step;
    }
    if tb.abs() <= 1e-12 {
        tb = 1.;
        last -= step;
    }
    ta = t_for_angle(ta * 90.);
    tb = t_for_angle(tb * 90.);
    let end = last + step;
    let mut out = vec![PathOp::Move(ellipse_point(r, start))];
    if first == end || first == last && (ta - tb).abs() <= ta.abs().min(tb.abs()) * 1e-12 {
        return Ok(out);
    }
    let mut i = first;
    let mut count = 0;
    while i != end {
        count += 1;
        if count > 5 {
            return Err("Annotation arc subdivision exceeded bound".into());
        }
        let j = 3 * (3 - i.rem_euclid(4)) as usize;
        let mut c = if step > 0. {
            [points[j + 3], points[j + 2], points[j + 1], points[j]]
        } else {
            [points[j], points[j + 1], points[j + 2], points[j + 3]]
        };
        if i == first {
            if i == last && (tb - 1.).abs() > 1e-12 {
                c = interval(c, ta, tb);
            } else if ta.abs() > 1e-12 {
                c = interval(c, ta, 1.);
            }
        } else if i == last && (tb - 1.).abs() > 1e-12 {
            c = interval(c, 0., tb);
        }
        out.push(PathOp::Cubic([
            c[1][0], c[1][1], c[2][0], c[2][1], c[3][0], c[3][1],
        ]));
        i += step;
    }
    if let Some(PathOp::Cubic(p)) = out.last_mut() {
        let e = ellipse_point(r, start + sweep);
        p[4] = e[0];
        p[5] = e[1];
    }
    Ok(out)
}
