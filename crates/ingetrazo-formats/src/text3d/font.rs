// SPDX-License-Identifier: GPL-3.0-or-later
//! Native Text3D outlines. Qt6.12's unscaled FreeType and QBezier sequence.
use freetype_sys as ft;
use std::{ptr, sync::Arc};
type P = [f64; 2];
const MAX_POINTS: usize = 262_144;
const MAX_RINGS: usize = 1024;
const MAX_AGGREGATE_POINTS: usize = 262_144;
// Logical cold geometry admission, separate from the bounded font resolver cache.
// This caps total flattened points across block measurement and all letters.
const MAX_PREFIX_SHAPING_BYTES: usize = 8 * 1024 * 1024;
fn admit_prefix_work(text: &str) -> Result<(), String> {
    let work = text
        .char_indices()
        .filter(|(_, c)| !whitespace(*c))
        .try_fold(0usize, |total, (offset, _)| {
            total
                .checked_add(offset)
                .ok_or("Text3D prefix work overflow")
        })?;
    if work > MAX_PREFIX_SHAPING_BYTES {
        return Err("Text3D prefix shaping work budget exceeded".into());
    }
    Ok(())
}
fn whitespace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn extend_rings(out: &mut Vec<Vec<P>>, rings: Vec<Vec<P>>, used: &mut usize) -> Result<(), String> {
    let count = rings.iter().try_fold(0usize, |n, r| {
        n.checked_add(r.len()).ok_or("Text3D point count overflow")
    })?;
    *used = used
        .checked_add(count)
        .filter(|n| *n <= MAX_AGGREGATE_POINTS)
        .ok_or("Text3D aggregate outline budget exceeded")?;
    out.try_reserve(rings.len())
        .map_err(|_| "Text3D ring publication allocation failed")?;
    out.extend(rings);
    Ok(())
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s]
}
fn div(a: P, s: f64) -> P {
    [a[0] / s, a[1] / s]
}
fn append(out: &mut Vec<P>, p: P) -> Result<(), String> {
    if out.len() >= MAX_POINTS || !p.iter().all(|x| x.is_finite()) {
        return Err("Text3D outline admission exceeded".into());
    }
    out.try_reserve(1)
        .map_err(|_| "Text3D outline allocation failed")?;
    out.push(p);
    Ok(())
}
fn cubic(out: &mut Vec<P>, c: [P; 4]) -> Result<(), String> {
    let mut stack = [([[0.; 2]; 4], 0u8); 10];
    stack[0] = (c, 9);
    let mut top = 0usize;
    loop {
        let (b, level) = stack[top];
        let dx = b[3][0] - b[0][0];
        let dy = b[3][1] - b[0][1];
        let mut l = dx.abs() + dy.abs();
        let d = if l > 1. {
            (dx * (b[0][1] - b[1][1]) - dy * (b[0][0] - b[1][0])).abs()
                + (dx * (b[0][1] - b[2][1]) - dy * (b[0][0] - b[2][0])).abs()
        } else {
            l = 1.;
            (b[0][0] - b[1][0]).abs()
                + (b[0][1] - b[1][1]).abs()
                + (b[0][0] - b[2][0]).abs()
                + (b[0][1] - b[2][1]).abs()
        };
        if d < 0.5 * l || level == 0 {
            append(out, b[3])?;
            if top == 0 {
                break;
            }
            top -= 1;
        } else {
            let a = mul(add(b[0], b[1]), 0.5);
            let c = mul(add(b[1], b[2]), 0.5);
            let d = mul(add(b[2], b[3]), 0.5);
            let e = mul(add(a, c), 0.5);
            let f = mul(add(c, d), 0.5);
            let g = mul(add(e, f), 0.5);
            stack[top] = ([g, f, d, b[3]], level - 1);
            top += 1;
            stack[top] = ([b[0], a, e, g], level - 1);
        }
    }
    Ok(())
}
struct OutlineFace {
    face: ft::FT_Face,
    library: ft::FT_Library,
    _bytes: Arc<[u8]>,
}
impl Drop for OutlineFace {
    fn drop(&mut self) {
        unsafe {
            ft::FT_Done_Face(self.face);
            ft::FT_Done_FreeType(self.library);
        }
    }
}
impl OutlineFace {
    fn new(bytes: Arc<[u8]>, index: u32) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > crate::paper_text::MAX_FONT_BYTES {
            return Err("Text3D font byte admission exceeded".into());
        }
        let mut library = ptr::null_mut();
        let mut face = ptr::null_mut();
        unsafe {
            if ft::FT_Init_FreeType(&mut library) != 0 {
                return Err("Text3D FreeType allocation failed".into());
            }
            if ft::FT_New_Memory_Face(
                library,
                bytes.as_ptr(),
                bytes.len() as _,
                index as _,
                &mut face,
            ) != 0
            {
                ft::FT_Done_FreeType(library);
                return Err("Text3D font face invalid".into());
            }
            let own = Self {
                face,
                library,
                _bytes: bytes,
            };
            let em = (*face).units_per_EM as i64;
            if em == 0 || ft::FT_Set_Char_Size(face, (em << 6) as _, (em << 6) as _, 0, 0) != 0 {
                return Err("Text3D scalable outline required".into());
            }
            Ok(own)
        }
    }
    fn rings(&mut self, glyph: u32, origin: P) -> Result<Vec<Vec<P>>, String> {
        unsafe {
            if ft::FT_Load_Glyph(self.face, glyph, ft::FT_LOAD_NO_BITMAP) != 0 {
                return Err("Text3D glyph outline load failed".into());
            }
            let slot = (*self.face).glyph;
            if (*slot).format != ft::FT_GLYPH_FORMAT_OUTLINE {
                return Err("Text3D bitmap glyph outlines unsupported".into());
            }
            let o = &(*slot).outline;
            if o.n_points < 0
                || o.n_contours < 0
                || o.n_points as usize > MAX_POINTS
                || o.n_contours as usize > MAX_RINGS
            {
                return Err("Text3D glyph point admission exceeded".into());
            }
            if o.n_points == 0 && o.n_contours == 0 {
                return Ok(Vec::new());
            }
            if o.points.is_null() || o.tags.is_null() || o.contours.is_null() {
                return Err("Text3D invalid outline pointers".into());
            }
            let points = std::slice::from_raw_parts(o.points, o.n_points as usize);
            let tags = std::slice::from_raw_parts(o.tags, o.n_points as usize);
            let ends = std::slice::from_raw_parts(o.contours, o.n_contours as usize);
            let scale = ft::FT_MulDiv(133 * 64, 1 << 10, (*self.face).units_per_EM as _);
            let p = |i: usize| -> P {
                add(
                    origin,
                    [
                        ft::FT_MulFix(points[i].x, scale) as f64 / 64.,
                        -ft::FT_MulFix(points[i].y, scale) as f64 / 64.,
                    ],
                )
            };
            let mut rings = Vec::new();
            rings
                .try_reserve(ends.len())
                .map_err(|_| "Text3D ring allocation failed")?;
            let mut i = 0isize;
            for &end in ends {
                let last = end as isize;
                if last < i || last >= o.n_points as isize {
                    return Err("Text3D malformed contour".into());
                }
                let mut start = p(i as usize);
                if tags[i as usize] & 1 == 0 {
                    start = if tags[last as usize] & 1 == 0 {
                        mul(add(p(last as usize), start), 0.5)
                    } else {
                        p(last as usize)
                    };
                    i -= 1;
                }
                let mut out = Vec::new();
                append(&mut out, start)?;
                let mut c = [[0.; 2]; 4];
                c[0] = start;
                let mut n = 1;
                while i < last {
                    i += 1;
                    c[n] = p(i as usize);
                    n += 1;
                    match tags[i as usize] & 3 {
                        2 => {
                            if n < 4 {
                                continue;
                            }
                            c[3] = mul(add(c[3], c[2]), 0.5);
                            i -= 1;
                        }
                        0 => {
                            if n < 3 {
                                continue;
                            }
                            c[3] = mul(add(c[1], c[2]), 0.5);
                            c[2] = div(add(mul(c[1], 2.), c[3]), 3.);
                            c[1] = div(add(mul(c[1], 2.), c[0]), 3.);
                            i -= 1;
                        }
                        _ => {
                            if n == 2 {
                                append(&mut out, c[1])?;
                                c[0] = c[1];
                                n = 1;
                                continue;
                            } else if n == 3 {
                                c[3] = c[2];
                                c[2] = div(add(mul(c[1], 2.), c[3]), 3.);
                                c[1] = div(add(mul(c[1], 2.), c[0]), 3.);
                            }
                        }
                    }
                    cubic(&mut out, c)?;
                    c[0] = c[3];
                    n = 1;
                }
                if n == 1 {
                    if out.last() != Some(&start) {
                        append(&mut out, start)?
                    }
                } else {
                    c[3] = start;
                    if n == 2 {
                        c[2] = div(add(mul(c[1], 2.), c[3]), 3.);
                        c[1] = div(add(mul(c[1], 2.), c[0]), 3.);
                    }
                    cubic(&mut out, c)?;
                }
                i += 1;
                if out.first() == out.last() {
                    out.pop();
                }
                for q in &mut out {
                    q[1] = -q[1];
                }
                if out.len() >= 3 {
                    rings.push(out);
                }
            }
            Ok(rings)
        }
    }
}

fn area(r: &[P]) -> f64 {
    let mut a = 0.;
    for i in 0..r.len() {
        let p = r[i];
        let q = r[(i + 1) % r.len()];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.
}
fn contains(p: P, r: &[P]) -> bool {
    let mut inside = false;
    for i in 0..r.len() {
        let a = r[i];
        let b = r[(i + 1) % r.len()];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let t = (p[1] - a[1]) / (b[1] - a[1]);
            if p[0] < a[0] + t * (b[0] - a[0]) {
                inside = !inside
            }
        }
    }
    inside
}
fn normal(m: &ingetrazo_core::mesh::Mesh, id: usize) -> glam::Vec3 {
    let r = &m.faces[id].vertices;
    let mut n = glam::DVec3::ZERO;
    for i in 0..r.len() {
        let a = m.vertices[r[i]].position;
        let b = m.vertices[r[(i + 1) % r.len()]].position;
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    let n = n.as_vec3();
    let length = n.length();
    if length < 1e-9 {
        glam::Vec3::Z
    } else {
        n / length
    }
}
fn solid(
    rings: &[Vec<P>],
    scale: f64,
    ymin: f64,
    thickness: f64,
) -> Result<ingetrazo_core::mesh::Mesh, String> {
    use ingetrazo_core::mesh::{Mesh, MeshVertex};
    use std::collections::HashMap;
    let mut mesh = Mesh::default();
    let mut registry: HashMap<[i64; 3], usize> = HashMap::new();
    let total: usize = rings.iter().map(Vec::len).sum();
    if rings.len() > MAX_RINGS || total > MAX_POINTS {
        return Err("Text3D aggregate point budget exceeded".into());
    }
    mesh.vertices
        .try_reserve(
            total
                .checked_mul(2)
                .ok_or("Text3D vertex budget overflow")?,
        )
        .map_err(|_| "Text3D mesh allocation failed")?;
    mesh.edges
        .try_reserve(total * 3)
        .map_err(|_| "Text3D edge allocation failed")?;
    mesh.faces
        .try_reserve(total + 2 * rings.len())
        .map_err(|_| "Text3D face allocation failed")?;
    registry
        .try_reserve(total * 2)
        .map_err(|_| "Text3D weld allocation failed")?;
    let depth: Vec<_> = rings
        .iter()
        .enumerate()
        .map(|(i, r)| {
            rings
                .iter()
                .enumerate()
                .filter(|(j, q)| *j != i && contains(r[0], q))
                .count()
        })
        .collect();
    let mut face = |loops: Vec<Vec<(P, f64)>>, reverse: bool| -> Result<(), String> {
        let mut ids = Vec::new();
        ids.try_reserve(loops.len())
            .map_err(|_| "Text3D loop allocation failed")?;
        for mut r in loops {
            if reverse {
                r.reverse()
            }
            let mut ring = Vec::new();
            ring.try_reserve(r.len())
                .map_err(|_| "Text3D vertex loop allocation failed")?;
            for (p, side) in r {
                let pos = glam::DVec3::new(
                    (p[0] * scale) as f32 as f64,
                    side as f32 as f64,
                    ((p[1] - ymin) * scale) as f32 as f64,
                );
                if !pos.is_finite() {
                    return Err("Text3D transformed coordinate overflow".into());
                }
                let raw = [pos.x * 10000., pos.y * 10000., pos.z * 10000.];
                if raw.iter().any(|x| x.abs() >= i64::MAX as f64 - 2.) {
                    return Err("Text3D weld coordinate admission exceeded".into());
                }
                let key = raw.map(f64::round_ties_even).map(|x| x as i64);
                let mut found = registry.get(&key).copied();
                if found.is_none() {
                    'neighbor: for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                if dx == 0 && dy == 0 && dz == 0 {
                                    continue;
                                }
                                if let Some(&id) =
                                    registry.get(&[key[0] + dx, key[1] + dy, key[2] + dz])
                                {
                                    if ((mesh.vertices[id].position.as_vec3() - pos.as_vec3())
                                        .length_squared()
                                        as f64)
                                        < 1e-8
                                    {
                                        found = Some(id);
                                        break 'neighbor;
                                    }
                                }
                            }
                        }
                    }
                }
                let id = found.unwrap_or_else(|| {
                    let id = mesh.vertices.len();
                    mesh.vertices.push(MeshVertex::new(pos));
                    registry.insert(key, id);
                    id
                });
                ring.push(id);
            }
            ids.push(ring);
        }
        let outer = ids.remove(0);
        mesh.add_face(outer, ids).ok_or("Text3D malformed face")?;
        Ok(())
    };
    for i in 0..rings.len() {
        if depth[i] % 2 != 0 {
            continue;
        }
        let mut outlines = Vec::new();
        let mut outer = rings[i].clone();
        if area(&outer) < 0. {
            outer.reverse()
        }
        outlines.push(outer);
        for j in 0..rings.len() {
            if depth[j] % 2 == 0 || depth[j] != depth[i] + 1 {
                continue;
            }
            let owner = (0..rings.len()).find(|&k| {
                depth[k] % 2 == 0 && depth[k] + 1 == depth[j] && contains(rings[j][0], &rings[k])
            });
            if owner == Some(i) {
                let mut hole = rings[j].clone();
                if area(&hole) >= 0. {
                    hole.reverse()
                }
                outlines.push(hole);
            }
        }
        let front = outlines
            .iter()
            .map(|r| r.iter().map(|&p| (p, 0.)).collect())
            .collect();
        face(front, false)?;
        if thickness <= 1e-9 {
            continue;
        }
        let back = outlines
            .iter()
            .map(|r| r.iter().map(|&p| (p, thickness)).collect())
            .collect();
        face(back, true)?;
        for r in outlines {
            for k in 0..r.len() {
                let a = r[k];
                let b = r[(k + 1) % r.len()];
                face(
                    vec![vec![(a, 0.), (a, thickness), (b, thickness), (b, 0.)]],
                    false,
                )?;
            }
        }
    }
    drop(face);
    for e in 0..mesh.edges.len() {
        if mesh.edges[e].faces.len() != 2 {
            continue;
        }
        let a = normal(&mesh, mesh.edges[e].faces[0]);
        let b = normal(&mesh, mesh.edges[e].faces[1]);
        if a.y.abs() < 0.5 && b.y.abs() < 0.5 && a.dot(b) as f64 > 0.85 {
            mesh.edges[e].soft = true
        }
    }
    mesh.rebuild_spatial_index();
    Ok(mesh)
}
pub(super) fn letters(
    params: &super::Params,
) -> Result<Vec<(String, ingetrazo_core::mesh::Mesh)>, String> {
    if params.text.len() > crate::paper_text::MAX_TEXT_BYTES
        || !params.height.is_finite()
        || params.height <= 0.
        || !params.thickness.is_finite()
        || params.thickness < 0.
    {
        return Err("Text3D parameter admission refused".into());
    }
    admit_prefix_work(&params.text)?;
    if params.text.is_empty() {
        return Ok(Vec::new());
    }
    let mut resolver = crate::paper_font_resolver::Resolver::new()?;
    let matched = resolver.resolve(crate::paper_font_resolver::Request {
        family: params.font.clone(),
        bold: params.bold,
        italic: params.italic,
        required_chars: params.text.chars().filter(|c| !whitespace(*c)).collect(),
    })?;
    let bytes = matched.font.shared_bytes().clone();
    let face_index = matched.font.key().face_index;
    let font =
        ttf_parser::Face::parse(&bytes, face_index).map_err(|_| "Text3D font metadata invalid")?;
    if (params.bold && !font.is_bold()) || (params.italic && !font.is_italic()) {
        return Err("Text3D synthesized font outline policy remains unsupported".into());
    }
    if params
        .text
        .chars()
        .filter(|c| !whitespace(*c))
        .any(|c| font.glyph_index(c).is_none())
    {
        return Err("Text3D mixed-family fallback outline policy remains unsupported".into());
    }
    let shaping = crate::paper_text::PaperFont::new_at_pixel_size(
        bytes.clone(),
        face_index,
        Some(matched.policy),
        133,
    )?;
    let mut outline = OutlineFace::new(bytes, face_index)?;
    let mut used = 0usize;
    let mut full = Vec::new();
    let mut x = 0.;
    for g in shaping.shape_run(&params.text, None)? {
        let rr = outline.rings(
            g.id,
            [x + g.offset[0] as f64 / 64., g.offset[1] as f64 / 64.],
        )?;
        extend_rings(&mut full, rr, &mut used)?;
        x += g.advance[0] as f64 / 64.;
    }
    if full.is_empty() {
        return Ok(Vec::new());
    }
    let ymin = full
        .iter()
        .flatten()
        .map(|p| p[1])
        .fold(f64::INFINITY, f64::min);
    let ymax = full
        .iter()
        .flatten()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let scale = params.height / (ymax - ymin).max(1e-9);
    let mut result = Vec::new();
    result
        .try_reserve(params.text.chars().count())
        .map_err(|_| "Text3D letter allocation failed")?;
    for (i, ch) in params.text.char_indices() {
        if whitespace(ch) {
            continue;
        }
        let prefix = shaping.shape_run(&params.text[..i], None)?;
        let mut x = prefix.iter().map(|g| g.advance[0] as i64).sum::<i64>() as f64 / 64.;
        let mut rr = Vec::new();
        for g in shaping.shape_run(&ch.to_string(), None)? {
            extend_rings(
                &mut rr,
                outline.rings(
                    g.id,
                    [x + g.offset[0] as f64 / 64., g.offset[1] as f64 / 64.],
                )?,
                &mut used,
            )?;
            x += g.advance[0] as f64 / 64.;
        }
        if rr.is_empty() {
            continue;
        }
        let mesh = solid(&rr, scale, ymin, params.thickness)?;
        if !mesh.faces.is_empty() {
            result.push((ch.to_string(), mesh));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_and_coordinate_admission() {
        assert!(admit_prefix_work(&"a".repeat(4096)).is_ok());
        assert!(admit_prefix_work(&"a".repeat(4097)).is_err());
        assert!(admit_prefix_work(&" ".repeat(16_384)).is_ok());
        let mut used = MAX_AGGREGATE_POINTS;
        let mut rings = Vec::new();
        assert!(extend_rings(&mut rings, vec![vec![[0., 0.]]], &mut used).is_err());
        assert!(rings.is_empty());
        assert!(solid(&[vec![[0., 0.], [1., 0.], [0., 1.]]], f64::MAX, 0., 1.).is_err());
        let mut out = Vec::new();
        assert!(append(&mut out, [f64::NAN, 0.]).is_err());
        assert!(out.is_empty());
    }
    #[test]
    fn cubic_depth_bound_and_flat() {
        let mut out = vec![[0., 0.]];
        cubic(&mut out, [[0., 0.], [0., 100.], [100., 100.], [100., 0.]]).unwrap();
        assert_eq!(out.last(), Some(&[100., 0.]));
        assert!(out.len() <= 513);
        let flat = solid(&[vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]], 1., 0., 0.).unwrap();
        assert_eq!(flat.faces.len(), 1);
        assert_eq!(flat.edges.len(), 4);
        assert!(flat.vertices.iter().all(|v| v.position.y == 0.));
    }
}
