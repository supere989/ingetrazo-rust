//! ezdxf 1.4.4 SPLINE construction and cubic-path conversion.
use super::{curves, DxfError};
use glam::DVec3;
use std::collections::BTreeMap;
fn error(s: &str) -> DxfError {
    DxfError::Parse(s.into())
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}
struct Spline {
    points: Vec<DVec3>,
    knots: Vec<f64>,
    weights: Vec<f64>,
    degree: usize,
}
impl Spline {
    fn new(
        points: Vec<DVec3>,
        degree: usize,
        mut knots: Vec<f64>,
        weights: Vec<f64>,
    ) -> Result<Self, DxfError> {
        let order = degree + 1;
        let count = points.len();
        if count > 10000 || degree > 32 {
            return Err(error("Spline admission limit exceeded"));
        }
        if degree == 0 || count < order {
            return Err(error("Invalid spline control count/order"));
        }
        if knots.is_empty() {
            knots = vec![0.; order];
            let divisor = (count - order + 1) as f64;
            knots.extend((0..count - order).map(|v| (1. + v as f64) / divisor));
            knots.extend(std::iter::repeat_n(1., order));
        } else if knots.len() != count + order {
            return Err(error("Invalid spline knot count"));
        }
        if knots.windows(2).any(|p| p[0] > p[1])
            || knots.iter().any(|k| !k.is_finite())
            || knots[0] == *knots.last().unwrap()
        {
            return Err(error("Invalid spline knots"));
        }
        if knots[0] != 0. {
            let begin = knots[0];
            let range = knots.last().unwrap() - begin;
            for k in &mut knots {
                *k = (*k - begin) / range;
            }
        }
        if !weights.is_empty() && weights.len() != count {
            return Err(error("Invalid spline weight count"));
        }
        Ok(Self {
            points,
            knots,
            weights,
            degree,
        })
    }
    fn basis(&self, mut u: f64) -> Result<(usize, Vec<f64>), DxfError> {
        let count = self.points.len();
        let max = *self.knots.last().unwrap();
        if close(u, max) {
            u = max;
        }
        let span = if u >= self.knots[count] {
            count - 1
        } else if self.knots[self.degree] == 0. {
            self.knots[self.degree..count].partition_point(|k| *k <= u) + self.degree - 1
        } else {
            let mut i = 0;
            while i < count && self.knots[i] <= u {
                i += 1;
            }
            i.saturating_sub(1)
        };
        let order = self.degree + 1;
        let mut n = vec![0.; order];
        let mut left = n.clone();
        let mut right = n.clone();
        n[0] = 1.;
        for j in 1..order {
            left[j] = u - self.knots[(span + 1).saturating_sub(j)];
            right[j] = self.knots[span + j] - u;
            let mut saved = 0.;
            for r in 0..j {
                let denominator = right[r + 1] + left[j - r];
                if denominator == 0. {
                    return Err(error("Singular spline basis"));
                }
                let temp = n[r] / denominator;
                n[r] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            n[j] = saved;
        }
        if !self.weights.is_empty() && span >= self.degree {
            let w = &self.weights[span - self.degree..=span];
            let products = n.iter().zip(w).map(|(n, w)| n * w).collect::<Vec<_>>();
            let sum = products.iter().sum::<f64>();
            if sum != 0. {
                for (n, p) in n.iter_mut().zip(products) {
                    *n = p / sum;
                }
            }
        }
        Ok((span, n))
    }
    fn point(&self, u: f64) -> Result<DVec3, DxfError> {
        let (span, n) = self.basis(u)?;
        let mut p = DVec3::ZERO;
        for (i, n) in n.into_iter().enumerate() {
            let index = (span as isize - self.degree as isize + i as isize)
                .rem_euclid(self.points.len() as isize) as usize;
            p += self.points[index] * n;
        }
        if !p.is_finite() {
            return Err(error("Spline coordinate overflow"));
        }
        Ok(p)
    }
    fn clamped(&self) -> bool {
        self.knots[..=self.degree]
            .iter()
            .all(|k| *k == self.knots[0])
            && self.knots[self.knots.len() - self.degree - 1..]
                .iter()
                .all(|k| *k == *self.knots.last().unwrap())
    }
    fn decomposition(&self) -> Result<Vec<[DVec3; 4]>, DxfError> {
        let p = self.degree;
        let m = self.points.len() - 1 + p + 1;
        let mut a = p;
        let mut b = p + 1;
        let mut bezier = self.points[..=p].to_vec();
        let mut alphas = vec![0.; self.knots.len()];
        let mut curves = Vec::new();
        while b < m {
            let mut next = vec![DVec3::ZERO; p + 1];
            let i = b;
            while b < m && close(self.knots[b + 1], self.knots[b]) {
                b += 1;
            }
            let mult = b - i + 1;
            if mult < p {
                let numer = self.knots[b] - self.knots[a];
                for j in (mult + 1..=p).rev() {
                    alphas[j - mult - 1] = numer / (self.knots[a + j] - self.knots[a]);
                }
                let r = p - mult;
                for j in 1..=r {
                    let save = r - j;
                    let s = mult + j;
                    for k in (s..=p).rev() {
                        let alpha = alphas[k - s];
                        bezier[k] = bezier[k] * alpha + bezier[k - 1] * (1. - alpha);
                    }
                    if b < m {
                        next[save] = bezier[p];
                    }
                }
            }
            curves.push(
                bezier
                    .clone()
                    .try_into()
                    .map_err(|_| error("Invalid cubic spline"))?,
            );
            if b < m {
                for i in p.saturating_sub(mult)..=p {
                    next[i] = self.points[b - p + i];
                }
                a = b;
                b += 1;
                bezier = next;
            }
        }
        Ok(curves)
    }
}
fn chord(points: &[DVec3]) -> Result<Vec<f64>, DxfError> {
    let distances = points
        .windows(2)
        .map(|p| p[0].distance(p[1]))
        .collect::<Vec<_>>();
    let total = distances.iter().sum::<f64>();
    if !total.is_finite() || total.abs() <= 1e-12 {
        return Err(error("Degenerate spline chord"));
    }
    let mut t = vec![0.];
    let mut sum = 0.;
    for d in distances.iter().take(distances.len() - 1) {
        sum += d;
        t.push(sum / total);
    }
    t.push(1.);
    Ok(t)
}
fn interpolation(points: &[DVec3]) -> Result<Vec<[DVec3; 4]>, DxfError> {
    if points.len() < 3 {
        return Ok(Vec::new());
    }
    let count = points.len() - 1;
    let mut first = Vec::with_capacity(count);
    let mut gamma = vec![0.; count];
    let mut bet = 2.;
    first.push((points[0] + points[1] * 2.) / bet);
    for j in 1..count {
        let a = if j == count - 1 { 2. } else { 1. };
        let b = if j == count - 1 { 7. } else { 4. };
        let rhs = if j == count - 1 {
            points[j] * 8. + points[j + 1]
        } else {
            (points[j] * 2. + points[j + 1]) * 2.
        };
        gamma[j] = 1. / bet;
        bet = b - a * gamma[j];
        if bet == 0. {
            return Err(error("Singular spline interpolation"));
        }
        first.push((rhs - first[j - 1] * a) / bet);
    }
    for j in (0..count - 1).rev() {
        let next = first[j + 1];
        first[j] -= next * gamma[j + 1];
    }
    Ok((0..count)
        .map(|i| {
            [
                points[i],
                first[i],
                if i == count - 1 {
                    (first[i] + points[count]) / 2.
                } else {
                    points[i + 1] * 2. - first[i + 1]
                },
                points[i + 1],
            ]
        })
        .collect())
}
/// Solve only the narrow nonzero coefficient bands; no dense O(n²) matrix.
fn solve(mut rows: Vec<BTreeMap<usize, f64>>, mut rhs: Vec<DVec3>) -> Result<Vec<DVec3>, DxfError> {
    let n = rows.len();
    let lower = rows
        .iter()
        .enumerate()
        .flat_map(|(i, r)| r.keys().map(move |&j| i.saturating_sub(j)))
        .max()
        .unwrap_or(0);
    for k in 0..n {
        let end = (k + lower + 1).min(n);
        let pivot = (k..end)
            .max_by(|&a, &b| {
                rows[a]
                    .get(&k)
                    .unwrap_or(&0.)
                    .abs()
                    .partial_cmp(&rows[b].get(&k).unwrap_or(&0.).abs())
                    .unwrap()
            })
            .unwrap();
        rows.swap(k, pivot);
        rhs.swap(k, pivot);
        let diagonal = *rows[k].get(&k).unwrap_or(&0.);
        if diagonal == 0. || !diagonal.is_finite() {
            return Err(error("Singular spline fit"));
        }
        let entries = rows[k]
            .range(k + 1..)
            .map(|(&j, &v)| (j, v))
            .collect::<Vec<_>>();
        let value = rhs[k];
        for i in k + 1..end {
            let factor = rows[i].remove(&k).unwrap_or(0.) / diagonal;
            if factor == 0. {
                continue;
            }
            for &(j, v) in &entries {
                let x = rows[i].entry(j).or_default();
                *x -= factor * v;
            }
            rhs[i] -= value * factor;
        }
    }
    let mut output = vec![DVec3::ZERO; n];
    for i in (0..n).rev() {
        let mut value = rhs[i];
        for (&j, &v) in rows[i].range(i + 1..) {
            value -= output[j] * v;
        }
        output[i] = value / rows[i][&i];
        if !output[i].is_finite() {
            return Err(error("Spline fit overflow"));
        }
    }
    Ok(output)
}
fn fit(points: Vec<DVec3>, tangents: Option<[DVec3; 2]>) -> Result<Spline, DxfError> {
    if points.len() > 10000 {
        return Err(error("Spline admission limit exceeded"));
    }
    if points.len() < 2 {
        return Err(error("Invalid spline fit count"));
    }
    let t = chord(&points)?;
    let mut knots = vec![0.; 4];
    knots.extend_from_slice(&t[1..t.len() - 1]);
    knots.extend_from_slice(&[1.; 4]);
    let count = points.len() + 2;
    let basis = Spline::new(vec![DVec3::ZERO; count], 3, knots.clone(), Vec::new())?;
    let mut rows = Vec::with_capacity(count);
    let mut rhs = Vec::with_capacity(count);
    for (i, &u) in t.iter().enumerate() {
        let (span, n) = basis.basis(u)?;
        rows.push(
            n.into_iter()
                .enumerate()
                .filter(|(_, v)| *v != 0.)
                .map(|(j, v)| (span - 3 + j, v))
                .collect::<BTreeMap<_, _>>(),
        );
        rhs.push(points[i]);
    }
    let last = count - 1;
    if let Some(tangents) = tangents {
        let total = points.windows(2).map(|p| p[0].distance(p[1])).sum::<f64>();
        let start = tangents[0] * (total / tangents[0].length());
        let end = tangents[1] * (total / tangents[1].length());
        rows.insert(1, BTreeMap::from([(0, -1.), (1, 1.)]));
        rhs.insert(1, start * (knots[4] / 3.));
        let index = rows.len() - 1;
        rows.insert(index, BTreeMap::from([(last - 1, -1.), (last, 1.)]));
        rhs.insert(index, end * ((1. - knots[knots.len() - 5]) / 3.));
    } else {
        let a = knots[4];
        let b = knots[5];
        let f = 6. / a;
        rows.insert(
            1,
            BTreeMap::from([(0, f / a), (1, -f * (a + b) / (a * b)), (2, f / b)]),
        );
        rhs.insert(1, DVec3::ZERO);
        let a = knots[knots.len() - 5];
        let b = knots[knots.len() - 6];
        let f = 6. / (1. - a);
        let index = rows.len() - 1;
        rows.insert(
            index,
            BTreeMap::from([
                (last - 2, f / (1. - b)),
                (last - 1, -f * (2. - a - b) / (1. - a) / (1. - b)),
                (last, f / (1. - a)),
            ]),
        );
        rhs.insert(index, DVec3::ZERO);
    }
    Spline::new(solve(rows, rhs)?, 3, knots, Vec::new())
}
pub(super) fn flatten(
    points: Vec<DVec3>,
    fit_points: Vec<DVec3>,
    degree: usize,
    mut knots: Vec<f64>,
    weights: Vec<f64>,
    tolerance: f64,
    tangents: Option<[DVec3; 2]>,
    scale: f64,
) -> Result<Vec<DVec3>, DxfError> {
    if tolerance > 0. && tolerance.is_finite() {
        let digits = -(tolerance.log10() as i32);
        if digits > 0 && digits <= 308 {
            for k in &mut knots {
                *k = format!("{:.*}", digits as usize, *k)
                    .parse()
                    .map_err(|_| error("Invalid rounded knot"))?;
            }
        }
    }
    let spline = if points.is_empty() {
        fit(fit_points, tangents)?
    } else {
        Spline::new(points, degree, knots, weights)?
    };
    let start = spline.point(0.)?;
    let curves = if spline.degree == 3 && spline.weights.is_empty() && spline.clamped() {
        spline.decomposition()?
    } else {
        let mut parameters = chord(&spline.points)?;
        let max = *spline.knots.last().unwrap();
        if max != 1. {
            for u in &mut parameters {
                *u *= max;
            }
        }
        for _ in 0..3 {
            let mut next = Vec::with_capacity(parameters.len() * 2 - 1);
            for pair in parameters.windows(2) {
                next.push(pair[0]);
                next.push((pair[0] + pair[1]) * 0.5);
            }
            next.push(*parameters.last().unwrap());
            parameters = next;
        }
        let samples = parameters
            .into_iter()
            .map(|u| spline.point(u))
            .collect::<Result<Vec<_>, _>>()?;
        interpolation(&samples)?
    };
    curves::bezier_path(start, curves, scale)
}
