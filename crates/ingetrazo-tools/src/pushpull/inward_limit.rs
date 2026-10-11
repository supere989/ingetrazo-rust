// SPDX-License-Identifier: GPL-3.0-or-later
// Source: pinned tools/pushpull.py _compute_inward_limit and imported Qt helpers.
use glam::DVec3;

pub(super) fn compute(
    mesh: &ingetrazo_core::mesh::Mesh,
    face_id: usize,
    anchor: DVec3,
) -> Option<f64> {
    use glam::{DVec2, Vec3};
    fn divide(v: Vec3, d: f32) -> Vec3 {
        Vec3::new(v.x / d, v.y / d, v.z / d)
    }
    fn length(v: Vec3) -> f32 {
        let m = v.abs().max_element();
        if m == 0. {
            0.
        } else {
            let n = divide(v, m);
            m * (n.x * n.x + n.y * n.y + n.z * n.z).sqrt()
        }
    }
    fn normalized(v: Vec3) -> Vec3 {
        let l = length(v);
        if (l - 1.).abs() < 1e-5 {
            v
        } else if l.abs() < 1e-5 {
            Vec3::ZERO
        } else {
            divide(v, l)
        }
    }
    fn basis(n: Vec3) -> (Vec3, Vec3) {
        let n = normalized(n);
        let r = if Vec3::X.dot(n).abs() > 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let u = normalized(r - n * r.dot(n));
        (u, normalized(n.cross(u)))
    }
    fn inside(p: DVec2, poly: &[DVec2]) -> bool {
        let mut yes = false;
        let mut j = poly.len() - 1;
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[j];
            if (a.y > p.y) != (b.y > p.y)
                && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y + 1e-30) + a.x
            {
                yes = !yes;
            }
            j = i;
        }
        yes
    }
    fn strict(p: DVec2, poly: &[DVec2]) -> bool {
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            let d = b - a;
            let q = p - a;
            if (d.x * q.y - d.y * q.x).abs() <= 1e-7
                && q.dot(d) >= -1e-7
                && q.dot(d) <= d.length_squared() + 1e-7
            {
                return false;
            }
        }
        inside(p, poly)
    }
    fn interior(poly: &[DVec2]) -> DVec2 {
        let c = poly.iter().copied().sum::<DVec2>() / poly.len() as f64;
        if inside(c, poly) {
            return c;
        }
        let p = (poly[0] + poly[1]) * 0.5 * 0.999 + c * 0.001;
        if inside(p, poly) {
            return p;
        }
        let mut ys = poly.iter().map(|p| p.y).collect::<Vec<_>>();
        ys.sort_by(f64::total_cmp);
        ys.dedup();
        let mut best = None;
        for band in ys.windows(2) {
            if band[1] - band[0] < 1e-9 {
                continue;
            }
            let y = (band[0] + band[1]) * 0.5;
            let mut xs = vec![];
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                if (a.y > y) != (b.y > y) {
                    xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            xs.sort_by(f64::total_cmp);
            for span in xs.chunks_exact(2) {
                let w = (span[1] - span[0]).min(band[1] - band[0]);
                if best.is_none_or(|(width, _)| w > width) {
                    best = Some((w, DVec2::new((span[0] + span[1]) * 0.5, y)));
                }
            }
        }
        best.map_or(p, |(_, p)| p)
    }
    fn hit(q: Vec3, d: Vec3, t: (Vec3, Vec3, Vec3)) -> Option<f64> {
        let e1 = t.1 - t.0;
        let e2 = t.2 - t.0;
        let p = d.cross(e2);
        let det = e1.dot(p) as f64;
        if det.abs() < 1e-7 {
            return None;
        }
        let inv = 1. / det;
        let tv = q - t.0;
        let u = tv.dot(p) as f64 * inv;
        if !(-1e-7..=1. + 1e-7).contains(&u) {
            return None;
        }
        let cross = tv.cross(e1);
        let v = d.dot(cross) as f64 * inv;
        if v < -1e-7 || u + v > 1. + 1e-7 {
            return None;
        }
        let t = e2.dot(cross) as f64 * inv;
        (t > 1e-6).then_some(t)
    }
    if mesh.vertices.iter().any(|v| !v.position.is_finite())
        || mesh.faces.iter().any(|f| {
            std::iter::once(&f.vertices)
                .chain(&f.holes)
                .any(|r| r.len() < 3 || r.iter().any(|&v| v >= mesh.vertices.len()))
        })
        || mesh
            .edges
            .iter()
            .any(|e| e.faces.iter().any(|&f| f >= mesh.faces.len()))
    {
        return None;
    }
    let face = mesh.faces.get(face_id)?;
    if face.vertices.len() < 3 {
        return None;
    }
    let rings = std::iter::once(&face.vertices).chain(&face.holes);
    let on_segment = |a: DVec3, p: DVec3, q: DVec3| {
        let a = a.as_vec3();
        let p = p.as_vec3();
        let d = q.as_vec3() - p;
        let length = d.length();
        if length < 1e-4 {
            return (a - p).length() < 1e-4;
        }
        let t = d.dot(a - p) / (length * length);
        t >= -1e-4 && t <= 1. + 1e-4 && (a - (p + d * t)).length() < 1e-4
    };
    // classify_push_edge: every authored rim has another backing face.
    if !rings.clone().all(|r| {
        (0..r.len()).all(|i| {
            if mesh
                .find_edge(r[i], r[(i + 1) % r.len()])
                .is_some_and(|e| mesh.edges[e].faces.iter().any(|&f| f != face_id))
            {
                return true;
            }
            let a = mesh.vertices[r[i]].position;
            let b = mesh.vertices[r[(i + 1) % r.len()]].position;
            mesh.faces.iter().enumerate().any(|(id, g)| {
                id != face_id
                    && std::iter::once(&g.vertices).chain(&g.holes).any(|r| {
                        (0..r.len()).any(|j| {
                            let p = mesh.vertices[r[j]].position;
                            let q = mesh.vertices[r[(j + 1) % r.len()]].position;
                            on_segment(a, p, q) && on_segment(b, p, q)
                        })
                    })
            })
        })
    }) {
        return None;
    }
    let positions = mesh
        .vertices
        .iter()
        .map(|v| v.position.as_vec3())
        .collect::<Vec<_>>();
    let normals = mesh
        .faces
        .iter()
        .map(|f| {
            let points = f
                .vertices
                .iter()
                .map(|&v| positions[v].as_dvec3())
                .collect::<Vec<_>>();
            let n = ingetrazo_core::math::newell_vector(&points).as_vec3();
            let l = length(n);
            normalized(if l < 1e-9 { Vec3::Z } else { divide(n, l) })
        })
        .collect::<Vec<_>>();
    let n = normals[face_id];
    let direction = -n;
    let anchor = anchor.as_vec3();
    let (u, w) = basis(n);
    let project = |p: Vec3| DVec2::new(p.dot(u) as f64, p.dot(w) as f64);
    let base = face
        .vertices
        .iter()
        .map(|&v| project(positions[v]))
        .collect::<Vec<_>>();
    let mut limit: Option<f64> = None;
    for (id, g) in mesh.faces.iter().enumerate() {
        if id == face_id || normals[id].dot(n).abs() < 0.999 {
            continue;
        }
        let centroid = (g
            .vertices
            .iter()
            .map(|&v| positions[v].as_dvec3())
            .sum::<DVec3>()
            / g.vertices.len() as f64)
            .as_vec3();
        let dist = (anchor - centroid).dot(n) as f64;
        if dist <= 1e-6 {
            continue;
        }
        // loop_inside_face uses its own axes and strict outer containment,
        // deliberately ignoring holes under the original default policy.
        let gn = normals[id];
        let origin = positions[g.vertices[0]];
        let gn = normalized(gn);
        let mut gu = Vec3::X - gn * Vec3::X.dot(gn);
        if gu.length() < 0.1 {
            gu = Vec3::Y - gn * Vec3::Y.dot(gn);
        }
        let gu = normalized(gu);
        let gw = normalized(gn.cross(gu));
        let gp = |p: Vec3| {
            let q = p - origin;
            DVec2::new(q.dot(gu) as f64, q.dot(gw) as f64)
        };
        let outer = g
            .vertices
            .iter()
            .map(|&v| gp(positions[v]))
            .collect::<Vec<_>>();
        let contained = face.vertices.iter().all(|&v| {
            let p = positions[v] - n * dist as f32;
            (gn.dot(p - origin) as f64).abs() <= 1e-3 && strict(gp(p), &outer)
        });
        if contained {
            continue;
        }
        let back = g
            .vertices
            .iter()
            .map(|&v| project(positions[v]))
            .collect::<Vec<_>>();
        if !inside(interior(&base), &back)
            && !inside(interior(&back), &base)
            && !base.iter().any(|&p| inside(p, &back))
            && !back.iter().any(|&p| inside(p, &base))
        {
            continue;
        }
        limit = Some(limit.map_or(dist, |old| old.min(dist)));
    }
    for ring in rings {
        for i in 0..ring.len() {
            if let Some(edge) = mesh.find_edge(ring[i], ring[(i + 1) % ring.len()]) {
                for &id in &mesh.edges[edge].faces {
                    if id != face_id
                        && !mesh.faces[id].interior
                        && normals[id].dot(n).abs() <= 0.999
                        && direction.dot(normals[id]) > 1e-6
                    {
                        return Some(0.);
                    }
                }
            }
        }
    }
    let origin = positions[face.vertices[0]];
    let poly = face
        .vertices
        .iter()
        .map(|&v| project(positions[v] - origin))
        .collect::<Vec<_>>();
    let ip = interior(&poly);
    let ip3 = origin + u * ip.x as f32 + w * ip.y as f32;
    let mut samples = Vec::with_capacity(1 + 2 * face.vertices.len());
    samples.push(ip3);
    for (i, &v) in face.vertices.iter().enumerate() {
        let p = positions[v];
        samples.push(p + (ip3 - p) * 1e-3);
        let mid = (p + positions[face.vertices[(i + 1) % face.vertices.len()]]) * 0.5;
        samples.push(mid + (ip3 - mid) * 1e-3);
    }
    let mut lateral: Option<f64> = None;
    for (id, g) in mesh.faces.iter().enumerate() {
        if id == face_id || g.interior || normals[id].dot(n).abs() > 0.999 {
            continue;
        }
        for (a, b, c) in mesh.triangulate_face(id) {
            for &q in &samples {
                if let Some(t) =
                    hit(q, direction, (a.as_vec3(), b.as_vec3(), c.as_vec3())).filter(|&t| t > 1e-4)
                {
                    lateral = Some(lateral.map_or(t, |old| old.min(t)));
                }
            }
        }
    }
    if let Some(lateral) = lateral {
        let lateral = (lateral - 1e-3).max(0.);
        limit = Some(limit.map_or(lateral, |old| old.min(lateral)));
    }
    limit
}
