//! Atomic segment drafting shared by line, rectangle and curve tools.
use crate::{mesh::Mesh, planar};
use glam::{DVec2, DVec3};
fn on_segment(p: DVec3, a: DVec3, b: DVec3) -> bool {
    let d = b - a;
    let t = (p - a).dot(d) / d.length_squared();
    t >= -1e-7 && t <= 1. + 1e-7 && (p - a - d * t).length() < 1e-6
}
fn qt_divide(v: glam::Vec3, d: f32) -> glam::Vec3 {
    glam::Vec3::new(v.x / d, v.y / d, v.z / d)
}
fn qt_length(v: glam::Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        return 0.;
    }
    let n = qt_divide(v, m);
    m * (n.x * n.x + n.y * n.y + n.z * n.z).sqrt()
}
fn qt_normalized(v: glam::Vec3) -> glam::Vec3 {
    let len = qt_length(v);
    if (len - 1.).abs() < 1e-5 {
        return v;
    }
    if len.abs() < 1e-5 {
        return glam::Vec3::ZERO;
    }
    qt_divide(v, len)
}
// Literal arrangement.coplanar_plane, confined to exactly Qt-authorable rows.
// None retains the generic native-double strict admission; positions never change.
fn source_author_plane(points:impl Iterator<Item=DVec3>+Clone)->Option<(DVec3,DVec3)>{
 if !points.clone().all(|p|p.is_finite()&&p.as_vec3().as_dvec3()==p){return None;}
 let origin=points.clone().next()?.as_vec3();let axis=points.clone().find_map(|p|{let d=p.as_vec3()-origin;(qt_length(d)>1e-6).then(||qt_normalized(d))})?;
 let n=points.clone().find_map(|p|{let c=axis.cross(p.as_vec3()-origin);(qt_length(c)>1e-6).then(||qt_normalized(c))})?;
 points.clone().all(|p|{let d=p.as_vec3()-origin;((n.x*d.x+n.y*d.y)+n.z*d.z).abs()<=1e-4}).then_some((origin.as_dvec3(),n.as_dvec3()))
}
pub(crate) fn source_on_segment(point:DVec3,a:DVec3,b:DVec3)->bool{
 let p=point.as_vec3();let a=a.as_vec3();let b=b.as_vec3();let d=b-a;let length=qt_length(d);if length<1e-4{return qt_length(p-a)<1e-4;}let q=p-a;let dot=(q.x*d.x+q.y*d.y)+q.z*d.z;let t=(dot as f64)/((length as f64)*(length as f64));t>=-1e-4&&t<=1.+1e-4&&qt_length(p-(a+d*(t as f32)))<1e-4
}
/// Original build_add_edge uses a bare AddEdgeCommand only when no extra
/// split/face/rebuild command is required and no curve already exists.
/// Such Undo removes the edge but keeps the endpoint Vertex objects.
pub fn source_bare_edge_history(before: &Mesh, after: &Mesh, a: DVec3, b: DVec3) -> bool {
    if before.edges.iter().any(|e| e.curve_id.is_some())
        || before.faces != after.faces
        || after.edges.len() < before.edges.len()
        || after.edges.len() > before.edges.len() + 1
        || after.edges[..before.edges.len()] != before.edges
        || after.vertices.len() < before.vertices.len()
        || after.vertices.len() > before.vertices.len() + 2
        || before
            .vertices
            .iter()
            .zip(&after.vertices)
            .any(|(x, y)| x.position != y.position || x.uid != y.uid)
    {
        return false;
    }
    if before.faces.is_empty() {
        return true;
    }
    // QVector3D predicates from arrangement.coplanar_plane; no copied mesh.
    let points = || {
        before
            .vertices
            .iter()
            .map(|v| v.position.as_vec3())
            .chain([a.as_vec3(), b.as_vec3()])
    };
    let origin = points().next().unwrap();
    let axis = points().find_map(|p| {
        let d = p - origin;
        (qt_length(d) > 1e-6).then(|| qt_normalized(d))
    });
    let normal = axis.and_then(|axis| {
        points().find_map(|p| {
            let n = axis.cross(p - origin);
            (qt_length(n) > 1e-6).then(|| qt_normalized(n))
        })
    });
    if normal.map_or(true, |n| points().all(|p| n.dot(p - origin).abs() <= 1e-4)) {
        return false;
    }
    // _append_face_plane_rebuild returns after the first matching face, even
    // when its populated-plane (>400) guard declines the rebuild.
    let face_normal = |id: usize| {
        let ring = &before.faces[id].vertices;
        let mut n = DVec3::ZERO;
        for i in 0..ring.len() {
            let p = before.vertices[ring[i]].position.as_vec3().as_dvec3();
            let q = before.vertices[ring[(i + 1) % ring.len()]]
                .position
                .as_vec3()
                .as_dvec3();
            n.x += (p.y - q.y) * (p.z + q.z);
            n.y += (p.z - q.z) * (p.x + q.x);
            n.z += (p.x - q.x) * (p.y + q.y);
        }
        let n = n.as_vec3();
        let length = qt_length(n);
        if ring.len() < 3 || length < 1e-9 {
            glam::Vec3::Z
        } else {
            qt_divide(n, length)
        }
    };
    for (id, face) in before.faces.iter().enumerate() {
        let Some(&first) = face.vertices.first() else {
            continue;
        };
        let origin = before.vertices[first].position.as_vec3();
        let normal = face_normal(id);
        if normal.length_squared() < 0.5 {
            continue;
        }
        if [a.as_vec3(), b.as_vec3()]
            .iter()
            .all(|p| normal.dot(*p - origin).abs() < 1e-4)
        {
            let count = before
                .faces
                .iter()
                .enumerate()
                .filter(|(i, f)| {
                    f.vertices.first().is_some_and(|&v| {
                        normal
                            .dot(before.vertices[v].position.as_vec3() - origin)
                            .abs()
                            < 1e-4
                    }) && normal.dot(face_normal(*i)).abs() > 0.999
                })
                .count();
            return count > 400;
        }
    }
    true
}

fn source_project(ring: &[DVec3], point: DVec3) -> DVec2 {
    source_project_mode(ring, point, false)
}
fn source_project_mode(ring: &[DVec3], point: DVec3, arrangement: bool) -> DVec2 {
    let ring: Vec<_> = ring.iter().map(|p| p.as_vec3().as_dvec3()).collect();
    let n = crate::math::newell_vector(&ring).as_vec3();
    let len = qt_length(n);
    let n = if len > 1e-9 {
        qt_divide(n, len)
    } else {
        glam::Vec3::ZERO
    };
    let n = if arrangement {
        let l = qt_length(n);
        if l > 1e-9 {
            qt_divide(n, l)
        } else {
            n
        }
    } else {
        n
    };
    let n = qt_normalized(n);
    let mut u = glam::Vec3::X - n * n.x;
    if qt_length(u) < 0.1 {
        u = glam::Vec3::Y - n * n.y;
    }
    let u = qt_normalized(u);
    let v = qt_normalized(n.cross(u));
    let p = point.as_vec3() - ring[0].as_vec3();
    DVec2::new(p.dot(u) as f64, p.dot(v) as f64)
}
fn source_drawing_point(point: DVec3, original: &Mesh, segments: &[(DVec3, DVec3)]) -> DVec3 {
    if let Some(anchor) = original
        .vertices
        .iter()
        .map(|v| v.position)
        .chain(segments.iter().flat_map(|&(a, b)| [a, b]))
        .find(|p| p.distance(point) <= 1e-4)
    {
        return anchor.as_vec3().as_dvec3();
    }
    for &(a, b) in segments {
        if !on_segment(point, a, b) {
            continue;
        }
        for e in &original.edges {
            let c = original.vertices[e.v0].position;
            let d = original.vertices[e.v1].position;
            if !on_segment(point, c, d) {
                continue;
            }
            let a = a.as_vec3();
            let b = b.as_vec3();
            let c = c.as_vec3();
            let d = d.as_vec3();
            let d1 = b - a;
            let d2 = d - c;
            let l1 = qt_length(d1) as f64;
            let l2 = qt_length(d2) as f64;
            if l1 < 1e-9 || l2 < 1e-9 {
                continue;
            }
            let u1 = d1.as_dvec3() / l1;
            let u2 = d2.as_dvec3() / l2;
            let w = (a - c).as_dvec3();
            let aa = u1.dot(u1);
            let bb = u1.dot(u2);
            let cc = u2.dot(u2);
            let dd = u1.dot(w);
            let ee = u2.dot(w);
            let den = aa * cc - bb * bb;
            if den < 1e-12 {
                continue;
            }
            let s = (bb * ee - cc * dd) / den;
            let t = (aa * ee - bb * dd) / den;
            let p = (a.as_dvec3() + u1 * s + c.as_dvec3() + u2 * t) * 0.5;
            return p.as_vec3().as_dvec3();
        }
    }
    point.as_vec3().as_dvec3()
}
fn coverage_triangles(mesh: &Mesh, id: usize) -> Result<Vec<(DVec3, DVec3, DVec3)>, String> {
    let rings = planar::rings(mesh, &mesh.faces[id]);
    let n = mesh.face_normal(id);
    let mut u = DVec3::X - n * n.x;
    if u.length() < 0.1 {
        u = DVec3::Y - n * n.y;
    }
    u = u.normalize_or_zero();
    let v = n.cross(u).normalize_or_zero();
    let points: Vec<_> = rings.iter().flatten().copied().collect();
    let projected: Vec<_> = points
        .iter()
        .map(|p| DVec2::new(p.dot(u), p.dot(v)))
        .collect();
    if rings.len() == 1
        && crate::triangulate::is_convex(&projected.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
    {
        return Ok((1..points.len() - 1)
            .map(|i| (points[0], points[i], points[i + 1]))
            .collect());
    }
    let mut offset = rings[0].len();
    let mut holes = vec![];
    for ring in rings.iter().skip(1) {
        holes.push(offset);
        offset += ring.len();
    }
    Ok(crate::drafting_earcut::earcut(&projected, &holes)?
        .into_iter()
        .map(|[a, b, c]| (points[a], points[b], points[c]))
        .collect())
}
// Source find_cycles_through uses two breadth-first minimal cycles through a
// new edge, excluding the first cycle's interior nodes for its second search.
fn minimal_cycles(mesh: &Mesh, edges: &[(usize, usize)], a: usize, b: usize) -> Vec<Vec<usize>> {
    use std::collections::VecDeque;
    let search = |excluded: &[usize]| {
        let mut parent = vec![None; mesh.vertices.len()];
        let mut seen = vec![false; parent.len()];
        let mut queue = VecDeque::from([b]);
        seen[b] = true;
        while let Some(u) = queue.pop_front() {
            if u == a {
                break;
            }
            for &(x, y) in edges {
                if (x == a && y == b)
                    || (x == b && y == a)
                    || excluded.contains(&x)
                    || excluded.contains(&y)
                {
                    continue;
                }
                let v = if x == u {
                    y
                } else if y == u {
                    x
                } else {
                    continue;
                };
                if !seen[v] {
                    seen[v] = true;
                    parent[v] = Some(u);
                    queue.push_back(v);
                }
            }
        }
        if !seen[a] {
            return None;
        }
        let mut path = vec![a];
        let mut u = a;
        while let Some(v) = parent[u] {
            path.push(v);
            u = v;
        }
        (path.len() >= 3 && path.len() <= 32).then_some(path)
    };
    let Some(first) = search(&[]) else {
        return vec![];
    };
    let excluded: Vec<_> = first
        .iter()
        .copied()
        .filter(|&v| v != a && v != b)
        .collect();
    let second = search(&excluded);
    let mut result = vec![first];
    if let Some(second) = second {
        result.push(second);
    }
    result
}
fn strictly_inside(p: DVec2, ring: &[DVec2]) -> bool {
    if (0..ring.len()).any(|i| {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let d = b - a;
        let dot = (p - a).dot(d);
        d.perp_dot(p - a).abs() <= 1e-7 && dot >= -1e-7 && dot <= d.length_squared() + 1e-7
    }) {
        return false;
    }
    planar::inside_ring(p, ring)
}
fn chord_declaration(mesh: &Mesh, a: usize, b: usize) -> Option<(usize, Vec<usize>, Vec<usize>)> {
    let pa = mesh.vertices[a].position;
    let pb = mesh.vertices[b].position;
    for (i, face) in mesh.faces.iter().enumerate() {
        if !face.holes.is_empty() {
            continue;
        }
        let points: Vec<_> = face
            .vertices
            .iter()
            .map(|&v| mesh.vertices[v].position)
            .collect();
        let Ok(frame) = planar::Frame::new(&points) else {
            continue;
        };
        if !frame.on_plane(pa) || !frame.on_plane(pb) {
            continue;
        }
        if ![pa, pb].iter().all(|&p| {
            (0..points.len()).any(|j| on_segment(p, points[j], points[(j + 1) % points.len()]))
        }) {
            continue;
        }
        let mut augmented = vec![];
        for j in 0..face.vertices.len() {
            let first = face.vertices[j];
            let last = face.vertices[(j + 1) % face.vertices.len()];
            augmented.push(first);
            let mut stations: Vec<_> = [a, b]
                .into_iter()
                .filter(|&id| {
                    id != first
                        && id != last
                        && on_segment(
                            mesh.vertices[id].position,
                            mesh.vertices[first].position,
                            mesh.vertices[last].position,
                        )
                })
                .collect();
            stations.sort_by(|&x, &y| {
                mesh.vertices[x]
                    .position
                    .distance_squared(mesh.vertices[first].position)
                    .total_cmp(
                        &mesh.vertices[y]
                            .position
                            .distance_squared(mesh.vertices[first].position),
                    )
            });
            augmented.extend(stations);
        }
        let ia = augmented.iter().position(|&v| v == a)?;
        let ib = augmented.iter().position(|&v| v == b)?;
        let (ia, ib) = if ia < ib { (ia, ib) } else { (ib, ia) };
        if ib - ia <= 1 || (ia == 0 && ib == augmented.len() - 1) {
            continue;
        }
        let projected: Vec<_> = points.iter().map(|&p| source_project(&points, p)).collect();
        if !strictly_inside(source_project(&points, (pa + pb) * 0.5), &projected) {
            continue;
        }
        let first = augmented[ia..=ib].to_vec();
        let second = augmented[ib..]
            .iter()
            .chain(&augmented[..=ia])
            .copied()
            .collect();
        return Some((i, first, second));
    }
    None
}
fn carve_cycle_declaration(
    mesh: &mut Mesh,
    cycle: &[usize],
    authored: &mut Vec<bool>,
) -> Result<Option<std::collections::HashMap<String, serde_json::Value>>, String> {
    let points: Vec<_> = cycle.iter().map(|&v| mesh.vertices[v].position).collect();
    for i in 0..mesh.faces.len() {
        let face = mesh.faces[i].clone();
        let loops = planar::rings(mesh, &face);
        let frame = planar::Frame::new(&loops[0])?;
        if !points.iter().all(|&p| frame.on_plane(p)) {
            continue;
        }
        let outer: Vec<_> = loops[0].iter().map(|&p| frame.project(p)).collect();
        let ring: Vec<_> = points.iter().map(|&p| frame.project(p)).collect();
        let boundary = |p: DVec3| {
            (0..loops[0].len())
                .any(|j| on_segment(p, loops[0][j], loops[0][(j + 1) % loops[0].len()]))
        };
        if !points.iter().any(|&p| boundary(p))
            || !points
                .iter()
                .all(|&p| boundary(p) || planar::inside_ring(frame.project(p), &outer))
        {
            continue;
        }
        let segments: Vec<_> = [&outer, &ring]
            .iter()
            .flat_map(|r| (0..r.len()).map(|j| (r[j], r[(j + 1) % r.len()])))
            .collect();
        let cells = planar::arrange_2d(&segments)?;
        let mut remainder: Vec<_> = cells
            .into_iter()
            .filter(|(r, _)| {
                let p = planar::sample(r);
                planar::inside_ring(p, &outer) && !planar::inside_ring(p, &ring)
            })
            .map(|(outer, holes)| {
                let mut loops = vec![outer
                    .iter()
                    .map(|&p| frame.unproject(p))
                    .collect::<Vec<_>>()];
                loops.extend(
                    holes
                        .iter()
                        .map(|h| h.iter().map(|&p| frame.unproject(p)).collect()),
                );
                loops
            })
            .collect();
        if remainder.is_empty() {
            continue;
        }
        let mut straddle = false;
        for hole in loops.iter().skip(1) {
            let owner = remainder.iter().position(|r| {
                hole.iter().all(|&p| {
                    strictly_inside(
                        frame.project(p),
                        &r[0].iter().map(|&p| frame.project(p)).collect::<Vec<_>>(),
                    )
                })
            });
            if let Some(k) = owner {
                remainder[k].push(hole.clone());
            } else {
                straddle = true;
                break;
            }
        }
        if straddle {
            continue;
        }
        mesh.faces.remove(i);
        authored.remove(i);
        for loops in remainder {
            planar::add_face(mesh, &loops, &face)?;
            authored.push(false);
        }
        let mut attrs = face.attrs;
        attrs.retain(|k, _| matches!(k.as_str(), "color" | "mat" | "texture" | "opacity"));
        return Ok(Some(attrs));
    }
    Ok(None)
}
// Keep the pre-arrangement AddFace declarations separate from the resulting
// mesh: source authoring can transiently punch overlapping hole loops before
// its coverage rebuild. Those declarations determine paint and covered cells.
fn hole_cycle_declarations(
    mesh: &Mesh,
    original: &Mesh,
    segments: &[(DVec3, DVec3)],
    plane: Option<(DVec3,DVec3)>,
    active: Option<&serde_json::Value>,
    tag: &mut Option<serde_json::Value>,
    source_id: Option<&Result<i64,String>>,
) -> Result<Mesh, String> {
    let mut declared = mesh.clone();
    for vertex in &mut declared.vertices {
        vertex.position = source_drawing_point(vertex.position, original, segments);
    }
    declared.faces.clear();
    for e in &mut declared.edges {
        e.faces.clear();
    }
    let on_plane = |p:DVec3| if let Some((origin,normal))=plane {(p-origin).dot(normal).abs()<1e-6} else {planar::Frame::new(&planar::rings(mesh, &mesh.faces[0])[0]).is_ok_and(|f|f.on_plane(p))};
    for face in &original.faces {
        let loops = planar::rings(original, face);
        if loops.iter().flatten().all(|&p| on_plane(p)) {
            planar::add_face(&mut declared, &loops, face)?;
        }
    }
    // Explicit draw-command identity. Re-emitted source/remainder declarations
    // are false; only chord halves and authored cycles are auto commands.
    let mut authored=vec![false;declared.faces.len()];
    let mut planned_faces: Vec<_> = declared.faces.iter().map(|f| f.vertices.clone()).collect();
    let drawn = |a: DVec3, b: DVec3| {
        segments
            .iter()
            .any(|&(s, t)| on_segment(a, s, t) && on_segment(b, s, t))
    };
    let mut graph: Vec<_> = mesh
        .edges
        .iter()
        .filter(|e| !drawn(mesh.vertices[e.v0].position, mesh.vertices[e.v1].position))
        .map(|e| (e.v0, e.v1))
        .collect();
    for &(start, end) in segments {
        let d = end - start;
        if d.length_squared() < 1e-18 {
            continue;
        }
        let mut pieces: Vec<_> = mesh
            .edges
            .iter()
            .filter(|e| {
                on_segment(mesh.vertices[e.v0].position, start, end)
                    && on_segment(mesh.vertices[e.v1].position, start, end)
            })
            .map(|e| {
                let a = e.v0;
                let b = e.v1;
                if (mesh.vertices[a].position - start).dot(d)
                    < (mesh.vertices[b].position - start).dot(d)
                {
                    (a, b)
                } else {
                    (b, a)
                }
            })
            .collect();
        pieces.sort_by(|&(a, _), &(b, _)| {
            (mesh.vertices[a].position - start)
                .dot(d)
                .total_cmp(&(mesh.vertices[b].position - start).dot(d))
        });
        for (a, b) in pieces {
            if !graph
                .iter()
                .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            {
                graph.push((a, b));
            }
            // Source chord splitting replaces the mother's outer loop with
            // two painted halves. Holed mothers are excluded by the
            // source chord path and proceed through cycle authoring instead.
            if let Some((id, left, right)) = chord_declaration(&declared, a, b) {
                let source = declared.faces.remove(id);
                authored.remove(id);
                for ring in [left, right] {
                    let id = declared
                        .add_face(ring, vec![])
                        .ok_or("Invalid authored chord")?;
                    declared.faces[id].attrs = source.attrs.clone();
                    declared.faces[id].layer = source.layer.clone();
                    authored.push(true);
                }
                continue;
            }
            for mut cycle in minimal_cycles(mesh, &graph, a, b) {
                let mut keys = cycle.clone();
                keys.sort_unstable();
                if planned_faces.iter().any(|f| {
                    let mut old = f.clone();
                    old.sort_unstable();
                    old == keys
                }) {
                    continue;
                }
                let points: Vec<_> = cycle
                    .iter()
                    .map(|&v| declared.vertices[v].position)
                    .collect();
                let Ok(frame) = planar::Frame::new(&points) else {
                    continue;
                };
                if points.iter().any(|&p| !frame.on_plane(p)) {
                    continue;
                }
                if frame.normal.z.abs() > 0.5 && frame.normal.z < 0. {
                    cycle[1..].reverse();
                }
                let mother = declared
                    .faces
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| {
                        let rings = planar::rings(&declared, f);
                        let Ok(frame) = planar::Frame::new(&rings[0]) else {
                            return false;
                        };
                        let projected: Vec<Vec<DVec2>> = rings
                            .iter()
                            .map(|r| r.iter().map(|&p| source_project(&rings[0], p)).collect())
                            .collect();
                        points.iter().all(|&p| {
                            frame.on_plane(p)
                                && strictly_inside(source_project(&rings[0], p), &projected[0])
                                && !projected
                                    .iter()
                                    .skip(1)
                                    .any(|h| strictly_inside(source_project(&rings[0], p), h))
                        })
                    })
                    .min_by_key(|(_, f)| f.vertices.len())
                    .map(|(i, _)| i);
                let mut attrs = std::collections::HashMap::new();
                if let Some(i) = mother {
                    attrs = declared.faces[i].attrs.clone();
                    attrs.retain(|k, _| {
                        matches!(k.as_str(), "color" | "mat" | "texture" | "opacity")
                    });
                    declared.faces[i].holes.push(cycle.clone());
                }
                planned_faces.push(cycle.clone());
                if mother.is_none() {
                    if let Some(paint) = carve_cycle_declaration(&mut declared, &cycle, &mut authored)? {
                        attrs = paint;
                    }
                }
                let id = declared
                    .add_face(cycle, vec![])
                    .ok_or("Invalid authored cycle")?;
                declared.faces[id].attrs = attrs;
                authored.push(true);
            }
        }
    }
    if authored.len()!=declared.faces.len(){return Err("Drawing declaration identity was lost".into());}
    if let Some(active)=active {
        if authored.iter().any(|a|*a) {
            let stamp=draw_tag(original,active,tag,source_id)?;
            for (face,is_auto) in declared.faces.iter_mut().zip(authored) {
                if is_auto && !face.attrs.get("ifc").is_some_and(tag_truthy) {face.attrs.insert("ifc".into(),stamp.clone());}
            }
        }
    }
    Ok(declared)
}
fn tag_truthy(v:&serde_json::Value)->bool {crate::bim::json_truthy(v)}
fn draw_tag(mesh:&Mesh,active:&serde_json::Value,allocated:&mut Option<serde_json::Value>,source_id:Option<&Result<i64,String>>)->Result<serde_json::Value,String>{
    if let Some(tag)=allocated {return Ok(tag.clone());}
    let mut tag=active.as_object().ok_or("Active BIM tag must be an object")?.clone();
    let id=match source_id{Some(id)=>id.clone()?,None=>crate::bim::next_draw_object_id(mesh)?};
    tag.insert("id".into(),id.into());let tag=serde_json::Value::Object(tag);*allocated=Some(tag.clone());Ok(tag)
}
// AddFaceCommand reintroduces full-length polygon sides after the segment
// planner has noded them. Its Qt-projected arrangement may discover additional
// stations where a rounded full side meets one of those rounded subsegments.
fn explicit_source_stations(
    mesh: &Mesh,
    original: &Mesh,
    segments: &[(DVec3, DVec3)],
    ring: &[DVec3],
) -> Vec<DVec3> {
    let sources: Vec<_> = mesh
        .edges
        .iter()
        .map(|e| (mesh.vertices[e.v0].position, mesh.vertices[e.v1].position))
        .chain((0..ring.len()).map(|i| (ring[i], ring[(i + 1) % ring.len()])))
        .collect();
    let mut stations = vec![];
    for face in &original.faces {
        let loops = planar::rings(original, face);
        let Ok(frame) = planar::Frame::new(&loops[0]) else {
            continue;
        };
        if !ring.iter().all(|&p| frame.on_plane(p)) {
            continue;
        }
        // Cache frame projections before the pairwise arrangement pass.
        let projected: Vec<_> = sources
            .iter()
            .filter(|&&(a, b)| frame.on_plane(a) && frame.on_plane(b))
            .map(|&(a, b)| {
                (
                    a,
                    b,
                    source_project_mode(
                        &loops[0],
                        source_drawing_point(a, original, segments),
                        true,
                    ),
                    source_project_mode(
                        &loops[0],
                        source_drawing_point(b, original, segments),
                        true,
                    ),
                )
            })
            .collect();
        for &(a, b, aa, bb) in &projected {
            let r = bb - aa;
            for &(_, _, cc, dd) in &projected {
                let delta = cc - aa;
                let s = dd - cc;
                let det = r.perp_dot(s);
                if det.abs() <= 1e-9 {
                    continue;
                }
                let t = delta.perp_dot(s) / det;
                let u = delta.perp_dot(r) / det;
                if t > 1e-7 && t < 1. - 1e-7 && u >= -1e-7 && u <= 1. + 1e-7 {
                    stations.push(a + (b - a) * t);
                }
            }
        }
    }
    stations
}
fn resplit_curves(mesh: &mut Mesh) -> Result<(), String> {
    let mut ids = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for (i, e) in mesh.edges.iter().enumerate() {
        if let Some(id) = e.curve_id {
            ids.entry(id).or_default().push(i);
        }
    }
    let mut next = mesh
        .edges
        .iter()
        .filter_map(|e| e.curve_id)
        .max()
        .unwrap_or(0);
    for (id, edges) in ids {
        let breaks = |v: usize| {
            let incident = &mesh.vertices[v].edges;
            let degree = incident.iter().filter(|e| edges.contains(e)).count();
            degree != 2 || incident.len() != degree
        };
        let mut seeds: Vec<_> = edges
            .iter()
            .copied()
            .filter(|&i| breaks(mesh.edges[i].v0) || breaks(mesh.edges[i].v1))
            .collect();
        seeds.extend(&edges);
        let mut unvisited: std::collections::BTreeSet<_> = edges.iter().copied().collect();
        let mut chains = vec![];
        for seed in seeds {
            if !unvisited.contains(&seed) {
                continue;
            }
            let edge = &mesh.edges[seed];
            let mut vertex = if breaks(edge.v0) {
                edge.v0
            } else if breaks(edge.v1) {
                edge.v1
            } else {
                edge.v0
            };
            let mut current = seed;
            let mut chain = vec![];
            while unvisited.remove(&current) {
                chain.push(current);
                let edge = &mesh.edges[current];
                vertex = if edge.v0 == vertex { edge.v1 } else { edge.v0 };
                if breaks(vertex) {
                    break;
                }
                let Some(edge) = mesh.vertices[vertex]
                    .edges
                    .iter()
                    .find(|e| unvisited.contains(e))
                else {
                    break;
                };
                current = *edge;
            }
            chains.push(chain);
        }
        if chains.len() > 1 {
            for (i, chain) in chains.iter().enumerate() {
                let cid = if i == 0 {
                    id
                } else {
                    next = next.checked_add(1).ok_or("Curve identifier overflow")?;
                    next
                };
                for &edge in chain {
                    mesh.edges[edge].curve_id = Some(cid);
                }
            }
        }
    }
    Ok(())
}
/// Add ordered drawing segments, optionally owning an explicit face or curve.
/// Only spatial crossings split; generated faces retain mother attributes.
pub fn add_segments(
    mesh: &Mesh,
    segments: &[(DVec3, DVec3)],
    explicit: Option<&[DVec3]>,
    curve: Option<usize>,
) -> Result<Mesh, String> { add_segments_tagged(mesh,segments,explicit,curve,None) }
// Source topology.segment_intersection: QVector3D differences/lengths,
// double unit-direction closest points, scale-aware gap, final Qt point.
fn source_segment_intersection(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> Option<DVec3> {
    let (a, b, c, d) = (a.as_vec3(), b.as_vec3(), c.as_vec3(), d.as_vec3());
    let d1 = b - a;
    let d2 = d - c;
    let l1 = qt_length(d1) as f64;
    let l2 = qt_length(d2) as f64;
    if l1 < 1e-4 || l2 < 1e-4 {
        return None;
    }
    let u = d1.as_dvec3() / l1;
    let v = d2.as_dvec3() / l2;
    let w = (a - c).as_dvec3();
    let dot = |x: DVec3, y: DVec3| x.x * y.x + x.y * y.y + x.z * y.z;
    let aa = dot(u, u);
    let ab = dot(u, v);
    let bb = dot(v, v);
    let ad = dot(u, w);
    let bd = dot(v, w);
    let denominator = aa * bb - ab * ab;
    if denominator < 1e-12 {
        return None;
    }
    let along_a = (ab * bd - bb * ad) / denominator;
    let along_b = (aa * bd - ab * ad) / denominator;
    if !(-1e-4 / l1..=1. + 1e-4 / l1).contains(&(along_a / l1))
        || !(-1e-4 / l2..=1. + 1e-4 / l2).contains(&(along_b / l2))
    {
        return None;
    }
    let p = a.as_dvec3() + u * along_a;
    let q = c.as_dvec3() + v * along_b;
    if (p - q).length() > 1e-4f64.max(1e-7 * l1.max(l2)) {
        return None;
    }
    Some(((p + q) * 0.5).as_vec3().as_dvec3())
}
fn source_same_position(a: DVec3, b: DVec3) -> bool {
    // Float32×10000 is exact in float64 (at most 38 significant bits).
    // Keep float keys instead of saturating integer casts for large coordinates.
    let key = |p: DVec3| {
        p.as_vec3()
            .to_array()
            .map(|v| ((v as f64) * 10000.).round_ties_even())
    };
    key(a) == key(b)
}
fn source_distance(a: DVec3, b: DVec3) -> f32 {
    qt_length(a.as_vec3() - b.as_vec3())
}
// Pinned edits.plan_edge_commands executes existing-edge replacements before
// new segment pieces. Keep record ownership/order here, not by sorting the
// final coordinates (which loses duplicate/equal object distinctions).
fn append_straight_segment(
    mesh: &mut Mesh,
    a: DVec3,
    b: DVec3,
) -> Result<Vec<(DVec3, DVec3)>, String> {
    let direction = b.as_vec3() - a.as_vec3();
    let mut cuts = vec![];
    let mut points = vec![];
    for (id, e) in mesh.edges.iter().enumerate() {
        let c = mesh.vertices[e.v0].position;
        let d = mesh.vertices[e.v1].position;
        let Some(mut p) = source_segment_intersection(a, b, c, d) else {
            continue;
        };
        for anchor in [c, d, a, b] {
            if source_distance(p, anchor) <= 1e-4 {
                p = anchor;
                break;
            }
        }
        if !source_same_position(p, a) && !source_same_position(p, b) {
            points.push(p);
        }
        if !source_same_position(p, c) && !source_same_position(p, d) {
            cuts.push((id, p));
        }
    }
    // Remove each old row then append its subedges in command order. Endpoint
    // vertices survive removal, and the first subedge authors the intersection.
    let replacements = cuts
        .into_iter()
        .map(|(id, p)| (mesh.edges[id].clone(), p))
        .collect::<Vec<_>>();
    let removed = replacements
        .iter()
        .map(|(edge, _)| edge.history_id)
        .collect::<std::collections::HashSet<_>>();
    if !removed.is_empty() {
        mesh.edges
            .retain(|edge| !removed.contains(&edge.history_id));
        mesh.rebuild_incidence();
    }
    for (old_edge, p) in replacements {
        let old_edge = &old_edge;
        let vertex = mesh.add_vertex(p);
        for (x, y) in [(old_edge.v0, vertex), (vertex, old_edge.v1)] {
            // The split planner emits no AddEdgeCommand for a subedge
            // already present in its simulated graph: do not stamp it.
            if mesh.find_edge(x, y).is_some() {
                continue;
            }
            let edge = mesh.add_edge(x, y).ok_or("Drawing subedge collapsed")?;
            // AddEdgeCommand stamps only specified soft/curve flags. A
            // welded existing subedge keeps its own hidden/layer/opaque data;
            // a newly constructed subedge keeps constructor defaults.
            if old_edge.soft {
                mesh.edges[edge].soft = true;
            }
            if let Some(curve) = old_edge.curve_id {
                mesh.edges[edge].curve_id = Some(curve);
            }
        }
        for f in &mut mesh.faces {
            for ring in std::iter::once(&mut f.vertices).chain(&mut f.holes) {
                let mut refined = vec![];
                for i in 0..ring.len() {
                    let x = ring[i];
                    let y = ring[(i + 1) % ring.len()];
                    refined.push(x);
                    if (x == old_edge.v0 && y == old_edge.v1)
                        || (x == old_edge.v1 && y == old_edge.v0)
                    {
                        refined.push(vertex);
                    }
                }
                if *ring != refined {
                    f.history_id = crate::mesh::MeshFace::new(vec![]).history_id;
                }
                *ring = refined;
            }
        }
        mesh.rebuild_incidence();
    }
    let mut unique = vec![];
    for p in points {
        if source_distance(p, a) <= 1e-4 || source_distance(p, b) <= 1e-4 {
            continue;
        }
        if !unique.iter().any(|&q| source_distance(p, q) <= 1e-4) {
            unique.push(p);
        }
    }
    unique.sort_by(|&x, &y| {
        let station = |p: DVec3| (p.as_vec3() - a.as_vec3()).dot(direction);
        station(x).total_cmp(&station(y))
    });
    let chain = std::iter::once(a)
        .chain(unique)
        .chain(std::iter::once(b))
        .collect::<Vec<_>>();
    let pieces = chain.windows(2).map(|p| (p[0], p[1])).collect::<Vec<_>>();
    for &(x, y) in &pieces {
        let x = mesh.add_vertex(x);
        let y = mesh.add_vertex(y);
        mesh.add_edge(x, y).ok_or("Drawing edge collapsed")?;
    }
    Ok(pieces)
}

/// Active tags stamp the draw plan's own declarations before coverage rebuild.
/// Existing/re-emitted regions are never selected by index/count differences.
pub fn add_segments_tagged(mesh:&Mesh,segments:&[(DVec3,DVec3)],explicit:Option<&[DVec3]>,curve:Option<usize>,active:Option<&serde_json::Value>)->Result<Mesh,String>{
    add_segments_tagged_with_id(mesh,segments,explicit,curve,active,None)
}
pub fn add_segments_tagged_with_id(mesh:&Mesh,segments:&[(DVec3,DVec3)],explicit:Option<&[DVec3]>,curve:Option<usize>,active:Option<&serde_json::Value>,source_id:Option<&Result<i64,String>>)->Result<Mesh,String>{
    let active=active.filter(|v|tag_truthy(v));let mut allocated_tag=None;
    planar::validate_mesh(mesh)?;
    if segments
        .iter()
        .any(|(a, b)| !a.is_finite() || !b.is_finite())
        || explicit.is_some_and(|r| r.iter().any(|p| !p.is_finite()))
    {
        return Err("Drawing coordinates must be finite".into());
    }
    let mut paint_only = explicit.is_some_and(|ring| {
        mesh.faces.iter().any(|face| {
            let loops = planar::rings(mesh, face);
            let Ok(frame) = planar::Frame::new(&loops[0]) else {
                return false;
            };
            ring.iter().all(|&p| {
                frame.on_plane(p)
                    && (planar::inside_rings(
                        frame.project(p),
                        &loops
                            .iter()
                            .map(|r| r.iter().map(|&p| frame.project(p)).collect())
                            .collect::<Vec<_>>(),
                    ) || loops
                        .iter()
                        .any(|r| (0..r.len()).any(|i| on_segment(p, r[i], r[(i + 1) % r.len()]))))
            })
        })
    });
    let mut out = mesh.clone();
    let reuse_straight=curve.is_none()&&!mesh.edges.iter().any(|e|e.curve_id.is_some());
    let mut authored_pieces=vec![];
    if reuse_straight {
        for &(a,b) in segments {if a.distance(b)>=1e-9{authored_pieces.extend(append_straight_segment(&mut out,a,b)?);}}
    }else{
        let mut targets=vec![];
        for &(a,b) in segments {if a.distance(b)<1e-9{continue;}let va=out.add_vertex(a);let vb=out.add_vertex(b);targets.push(out.add_edge(va,vb).ok_or("Drawing edge collapsed")?);}
        out=planar::split_edge_intersections(&out,&targets)?;
    }
    // Containment concerns the whole footprint, not only its corners. A band
    // whose corners lie on the mother may cross a declared hole. Source
    // AddFaceCommand refuses that carve, leaving the mother's attributes on
    // the covered pieces instead of stamping paint-only authored metadata.
    if paint_only {
        paint_only = explicit.is_some_and(|ring| {
            mesh.faces.iter().any(|face| {
                let loops = planar::rings(mesh, face);
                let Ok(frame) = planar::Frame::new(&loops[0]) else {
                    return false;
                };
                let projected: Vec<_> = loops
                    .iter()
                    .map(|r| r.iter().map(|&p| frame.project(p)).collect())
                    .collect();
                let contains = |p: DVec3| {
                    frame.on_plane(p)
                        && (planar::inside_rings(frame.project(p), &projected)
                            || loops.iter().any(|r| {
                                (0..r.len()).any(|i| on_segment(p, r[i], r[(i + 1) % r.len()]))
                            }))
                };
                ring.iter().all(|&p| contains(p))
                    && out
                        .edges
                        .iter()
                        .filter(|e| {
                            let a = out.vertices[e.v0].position;
                            let b = out.vertices[e.v1].position;
                            (0..ring.len()).any(|i| {
                                on_segment(a, ring[i], ring[(i + 1) % ring.len()])
                                    && on_segment(b, ring[i], ring[(i + 1) % ring.len()])
                            })
                        })
                        .all(|e| {
                            contains(
                                (out.vertices[e.v0].position + out.vertices[e.v1].position) * 0.5,
                            )
                        })
            })
        });
    }
    if let Some(ring) = explicit {
        let ids = ring.iter().map(|&p| out.add_vertex(p)).collect();
        let id=out.add_face(ids, vec![]).ok_or("Invalid drawing face")?;
        if let Some(active)=active {out.faces[id].attrs.insert("ifc".into(),draw_tag(mesh,active,&mut allocated_tag,source_id)?);}
        out = planar::stitch_boundaries(&out)?;
    }
    if let Some(id) = curve {
        for e in &mut out.edges {
            if e.curve_id.is_some() {
                continue;
            }
            let a = out.vertices[e.v0].position;
            let b = out.vertices[e.v1].position;
            if segments
                .iter()
                .any(|&(s, t)| on_segment(a, s, t) && on_segment(b, s, t))
            {
                e.curve_id = Some(id);
            }
        }
    }
    let source_rows=curve.is_some()&&mesh.vertices.iter().all(|v|v.position.as_vec3().as_dvec3()==v.position)&&segments.iter().flat_map(|&(a,b)|[a,b]).all(|p|p.as_vec3().as_dvec3()==p);
    let source_flat=source_rows.then(||source_author_plane(out.vertices.iter().map(|v|v.position))).flatten();
    let source_input=source_rows.then(||source_author_plane(segments.iter().flat_map(|&(a,b)|[a,b]))).flatten();
    let source_virgin=source_flat.is_none()&&explicit.is_some()&&source_input.is_some_and(|(p,n)|!mesh.edges.iter().any(|e|[e.v0,e.v1].iter().all(|&i|{let d=mesh.vertices[i].position.as_vec3()-p.as_vec3();let n=n.as_vec3();((d.x*n.x+d.y*n.y)+d.z*n.z).abs()<1e-4})));

    let source_rebuild_frame=source_flat.map(|(origin,normal)|{
        let (origin,n)=if let Some(face)=mesh.faces.iter().find(|face|face.vertices.len()>=3){let points=planar::rings(mesh,face);let n=crate::math::newell_vector(&points[0]).as_vec3();let length=qt_length(n);(points[0][0],if length>1e-9{qt_divide(n,length)}else{glam::Vec3::Z})}else{(origin,normal.as_vec3())};
        let n=qt_normalized(n);let reference=if n.x.abs()>0.9{glam::Vec3::Y}else{glam::Vec3::X};let u=qt_normalized(reference-n*n.dot(reference));let v=qt_normalized(n.cross(u));(origin,n.as_dvec3(),u.as_dvec3(),v.as_dvec3())
    });
    let mut planes: Vec<(DVec3, DVec3)> = vec![];
    let mut remember = |point: DVec3, normal: DVec3| {
        if normal.length_squared() > 0.5
            && !planes
                .iter()
                .any(|&(p, n)| n.dot(normal).abs() > 1. - 1e-7 && (point - p).dot(n).abs() < 1e-6)
        {
            planes.push((point, normal));
        }
    };
    if source_virgin {}else if let Some((origin,normal))=source_flat {remember(origin,normal);}else{
    // A cycle through a new subedge must leave it through an incident edge.
    // Those neighbors enumerate its possible planes without scanning arbitrary
    // vertices throughout unrelated 3D content.
    for &(a, b) in segments {
        if a.distance(b) < 1e-9 {
            continue;
        }
        for edge in &out.edges {
            if !on_segment(out.vertices[edge.v0].position, a, b)
                || !on_segment(out.vertices[edge.v1].position, a, b)
            {
                continue;
            }
            for endpoint in [edge.v0, edge.v1] {
                for &neighbor in &out.vertices[endpoint].edges {
                    let e = &out.edges[neighbor];
                    let other = if e.v0 == endpoint { e.v1 } else { e.v0 };
                    remember(
                        a,
                        (b - a)
                            .cross(out.vertices[other].position - a)
                            .normalize_or_zero(),
                    );
                }
            }
        }
        // An interior chord can remain disconnected from the mother's boundary.
        for (id, face) in out.faces.iter().enumerate() {
            let p = out.vertices[face.vertices[0]].position;
            let n = out.face_normal(id);
            if (a - p).dot(n).abs() < 1e-6 && (b - p).dot(n).abs() < 1e-6 {
                remember(p, n);
            }
        }
    }
    // Source curve drafting performs one global arrangement on whole-flat
    // drawings, including disconnected old curves and deleted bounded cells.
    if let Some(first) = out.vertices.first() {
        let origin = first.position;
        if let Some(axis) = out
            .vertices
            .iter()
            .map(|v| v.position - origin)
            .find(|d| d.length_squared() > 1e-12)
        {
            if let Some(normal) = out
                .vertices
                .iter()
                .map(|v| axis.cross(v.position - origin).normalize_or_zero())
                .find(|n| n.length_squared() > 0.5)
            {
                if out
                    .vertices
                    .iter()
                    .all(|v| (v.position - origin).dot(normal).abs() < 1e-6)
                {
                    remember(origin, normal);
                }
            }
        }
    }
    }
    drop(remember);
    if planes.len() > 4096 {
        return Err("Drawing meets too many distinct planes for an interactive edit".into());
    }
    let old_curves = mesh.edges.iter().any(|e| e.curve_id.is_some());
    let flat_rebuild = (!mesh.faces.is_empty() || old_curves)
        && planes.iter().any(|&(p, n)| {
            out.vertices
                .iter()
                .all(|v| (v.position - p).dot(n).abs() < 1e-6)
        });
    let input_points: Vec<_> = segments.iter().flat_map(|&(a, b)| [a, b]).collect();
    let input_plane = input_points.first().and_then(|&p| {
        let axis = input_points
            .iter()
            .map(|&q| q - p)
            .find(|d| d.length_squared() > 1e-12)?;
        let normal = input_points
            .iter()
            .map(|&q| axis.cross(q - p).normalize_or_zero())
            .find(|n| n.length_squared() > 0.5)?;
        input_points
            .iter()
            .all(|&q| (q - p).dot(normal).abs() < 1e-6)
            .then_some((p, normal))
    });
    let scoped_curve_rebuild = old_curves
        && input_plane.is_some_and(|(p, n)| {
            mesh.edges.iter().any(|e| {
                e.curve_id.is_some()
                    && [e.v0, e.v1]
                        .iter()
                        .all(|&v| (mesh.vertices[v].position - p).dot(n).abs() < 1e-4)
            })
        });
    for (point, normal) in planes {
        // The explicit footprint supplies coverage; it does not auto-face
        // unrelated cycles. Contained bands split their mother even in 3D.
        let tolerance=if source_flat.is_some(){1e-4}else{1e-6};
        let flat = out
            .vertices
            .iter()
            .all(|v| (v.position - point).dot(normal).abs() <= tolerance);
        let affected: Vec<_> = out
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                planar::rings(&out, f)
                    .iter()
                    .flatten()
                    .all(|p| (*p - point).dot(normal).abs() <= tolerance)
            })
            .map(|(i, _)| i)
            .collect();
        let mut plane = Mesh::new();
        for &i in &affected {
            planar::add_face(
                &mut plane,
                &planar::rings(&out, &out.faces[i]),
                &out.faces[i],
            )?;
        }
        for e in &out.edges {
            let a = out.vertices[e.v0].position;
            let b = out.vertices[e.v1].position;
            if (a - point).dot(normal).abs() <= tolerance && (b - point).dot(normal).abs() <= tolerance {
                let a = plane.add_vertex(a);
                let b = plane.add_vertex(b);
                plane.add_edge(a, b);
            }
        }
        if plane.edges.len() < 3 {
            continue;
        }
        let mut rebuilt = if let Some((origin,normal,u,v))=source_rebuild_frame{planar::rebuild_source_drawing_faces(&plane,origin,normal,u,v)?}else{planar::rebuild_drawing_faces(&plane)?};
        // Source commit_arc disables auto-face planning on a flat drawing.
        // Rebuild-created curve cells therefore carry no newly active tag.
        // commit_arc places the busy-plane rebuild in extra BEFORE AutoTag.
        // Its face commands have been replaced by then and are no longer live.
        let source_curve_plane=curve.is_some()&&!flat&&!affected.is_empty();
        let declaration_active=if curve.is_some()&&(flat||source_curve_plane){None}else{active};
        let declarations = if explicit.is_none() && (declaration_active.is_some() || source_curve_plane || plane.faces.iter().any(|f| !f.holes.is_empty()))
        {
            hole_cycle_declarations(&plane, mesh, segments,Some((point,normal)),declaration_active,&mut allocated_tag,source_id)?
        } else {
            plane.clone()
        };
        if affected.is_empty() && rebuilt.faces.is_empty() {
            continue;
        }
        let mut result = Mesh::new();
        if reuse_straight {
            result.vertices=out.vertices.clone();result.edges=out.edges.clone();
            result.rebuild_spatial_index();result.rebuild_incidence();
        }
        for (i, f) in out.faces.iter().enumerate() {
            if !affected.contains(&i) {
                planar::add_face(&mut result, &planar::rings(&out, f), f)?;
            }
        }
        let old_triangles: Vec<_> = (0..declarations.faces.len())
            .map(|i| coverage_triangles(&declarations, i))
            .collect::<Result<_, _>>()?;
        for f in &rebuilt.faces {
            let mut loops=planar::rings(&rebuilt,f);
            if source_curve_plane {
                // Original _walk_faces begins each loop at the first inserted
                // graph vertex. Ring start controls the first fan probe, so a
                // collinear start must not select the adjacent cell's attrs.
                for ring in &mut loops {
                    let start=(0..ring.len()).min_by_key(|&i|plane.vertices.iter().position(|v|v.position.distance_squared(ring[i])<1e-18).unwrap_or(usize::MAX)).unwrap_or(0);
                    ring.rotate_left(start);
                }
            }
            if reuse_straight {
                // A newly closed cycle starts at the authored subedge's first
                // endpoint (topology.find_smallest_cycle_through); a scoped
                // existing-face rebuild starts at the first graph insertion.
                for ring in &mut loops {
                    let start=if explicit.is_none()&&affected.is_empty(){
                        authored_pieces.iter().find_map(|&(a,b)| (0..ring.len()).find(|&i|ring[i].distance_squared(a)<1e-18&&(ring[(i+1)%ring.len()].distance_squared(b)<1e-18||ring[(i+ring.len()-1)%ring.len()].distance_squared(b)<1e-18)))
                    }else{out.edges.iter().filter(|e|[e.v0,e.v1].iter().all(|&v|(out.vertices[v].position-point).dot(normal).abs()<1e-6)).flat_map(|e|[e.v0,e.v1]).find_map(|v|ring.iter().position(|p|p.distance_squared(out.vertices[v].position)<1e-18))};
                    if let Some(start)=start {ring.rotate_left(start);}
                }
            }
            let frame = planar::Frame::new(&loops[0])?;
            let sample = frame.unproject(planar::sample(
                &loops[0]
                    .iter()
                    .map(|&p| frame.project(p))
                    .collect::<Vec<_>>(),
            ));
            let mut cell = Mesh::new();
            let cell_id = planar::add_face(&mut cell, &loops, f)?;
            let probe = coverage_triangles(&cell, cell_id)?
                .first()
                .map(|t| (t.0 + t.1 + t.2) / 3.)
                .unwrap_or(sample);
            let covering = declarations
                .faces
                .iter()
                .enumerate()
                .find(|(i, _)| {
                    old_triangles[*i].iter().any(|t| {
                        let a = frame.project(t.0);
                        let b = frame.project(t.1);
                        let c = frame.project(t.2);
                        let p = frame.project(probe);
                        let cross = |x: DVec2, y: DVec2| x.perp_dot(y);
                        if cross(b - a, c - a).abs() < 1e-12 {
                            return false;
                        }
                        let signs = [
                            cross(b - a, p - a),
                            cross(c - b, p - b),
                            cross(a - c, p - c),
                        ];
                        signs.iter().all(|&x| x >= -1e-8) || signs.iter().all(|&x| x <= 1e-8)
                    })
                })
                .map(|(_, f)| f);
            let covered = covering.is_some();
            let touched = loops.iter().any(|ring| {
                (0..ring.len()).any(|i| {
                    segments.iter().any(|&(a, b)| {
                        on_segment(ring[i], a, b) && on_segment(ring[(i + 1) % ring.len()], a, b)
                    })
                })
            });
            let full_curve = flat && out.edges.iter().any(|e| e.curve_id.is_some());
            if covered || touched && explicit.is_none() || full_curve {
                let mut source = covering.unwrap_or(f).clone();
                if paint_only
                    && explicit.is_some_and(|ring| {
                        ring.iter().all(|p| (*p - point).dot(normal).abs() < 1e-6)
                            && planar::inside_ring(
                                frame.project(sample),
                                &ring.iter().map(|&p| frame.project(p)).collect::<Vec<_>>(),
                            )
                    })
                {
                    // AddFaceCommand's newly authored rectangle inherits only
                    // PAINT_KEYS. Opaque metadata belongs to the mother and
                    // remains on her remainder pieces, unlike a chord split.
                    source.attrs.retain(|key, _| {
                        matches!(key.as_str(), "color" | "mat" | "texture" | "opacity")
                    });
                    if let Some(active)=active {source.attrs.insert("ifc".into(),draw_tag(mesh,active,&mut allocated_tag,source_id)?);}
                }
                if reuse_straight {source.history_id=crate::mesh::MeshFace::new(vec![]).history_id;}
                planar::add_face(&mut result, &loops, &source)?;
            }
        }
        if source_rebuild_frame.is_some(){planar::preserve_source_edges(&mut result,&out);}else{planar::preserve_edges(&mut result, &out);}
        out = result;
    }
    if let Some(ring) = explicit {
        for point in explicit_source_stations(&out, mesh, segments, ring) {
            out.add_vertex(point);
        }
        out = planar::stitch_boundaries(&out)?;
    }
    if flat_rebuild || scoped_curve_rebuild {
        resplit_curves(&mut out)?;
    }
    if source_flat.is_some()&&flat_rebuild{
        let mut used=vec![false;out.vertices.len()];for e in &out.edges{used[e.v0]=true;used[e.v1]=true;}for f in &out.faces{for &v in f.vertices.iter().chain(f.holes.iter().flatten()){used[v]=true;}}
        if used.iter().any(|u|!*u){let mut map=vec![0;used.len()];let mut i=0;let mut next=0;out.vertices.retain(|_|{let keep=used[i];if keep{map[i]=next;next+=1;}i+=1;keep});for e in &mut out.edges{e.v0=map[e.v0];e.v1=map[e.v1];}for f in &mut out.faces{for v in f.vertices.iter_mut().chain(f.holes.iter_mut().flatten()){*v=map[*v];}}out.rebuild_spatial_index();out.rebuild_incidence();}
    }
    out.preserve_vertex_uids_from(mesh);
    planar::validate_mesh(&out)?;
    if planar::equivalent(mesh, &out) {
        Ok(mesh.clone())
    } else {
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square() -> Mesh {
        let mut m = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| m.add_vertex(p));
        let f = m.add_face(ids.to_vec(), vec![]).unwrap();
        m.faces[f]
            .attrs
            .insert("paint".into(), serde_json::json!("brick"));
        m
    }
    #[test]
    fn diagonal_splits_painted_mother_into_two_halves() {
        let m = square();
        let out = add_segments(&m, &[(DVec3::ZERO, DVec3::X + DVec3::Y)], None, None).unwrap();
        assert_eq!(out.faces.len(), 2);
        assert!(out.faces.iter().all(|f| f.attrs == m.faces[0].attrs));
        assert!(
            (out.faces
                .iter()
                .enumerate()
                .map(|(i, _)| out.face_area(i))
                .sum::<f64>()
                - 1.)
                .abs()
                < 1e-9
        );
    }
    #[test]
    fn authored_rectangle_band_inherits_paint_but_not_mother_metadata() {
        let mut m = square();
        m.faces[0]
            .attrs
            .insert("color".into(), serde_json::json!([1., 0., 0.]));
        m.faces[0]
            .attrs
            .insert("vendor".into(), serde_json::json!(71));
        let ring = [
            DVec3::new(0.25, 0., 0.),
            DVec3::new(0.5, 0., 0.),
            DVec3::new(0.5, 1., 0.),
            DVec3::new(0.25, 1., 0.),
        ];
        let segments = (0..4)
            .map(|i| (ring[i], ring[(i + 1) % 4]))
            .collect::<Vec<_>>();
        let out = add_segments(&m, &segments, Some(&ring), None).unwrap();
        assert_eq!(out.faces.len(), 3);
        assert_eq!(
            out.faces
                .iter()
                .filter(|f| f.attrs.contains_key("vendor"))
                .count(),
            2
        );
        assert!(out
            .faces
            .iter()
            .all(|f| f.attrs.get("color") == m.faces[0].attrs.get("color")));
    }
    #[test]
    fn faceless_diagonal_closes_both_sides() {
        let mut m = square();
        m.faces.clear();
        for e in &mut m.edges {
            e.faces.clear();
        }
        let out = add_segments(&m, &[(DVec3::ZERO, DVec3::X + DVec3::Y)], None, None).unwrap();
        assert_eq!(out.faces.len(), 2);
    }
    #[test]
    fn drawing_elsewhere_does_not_resurrect_deleted_region() {
        let mut m = square();
        m.faces.clear();
        for e in &mut m.edges {
            e.faces.clear();
        }
        let out = add_segments(
            &m,
            &[(DVec3::new(3., 0., 0.), DVec3::new(4., 0., 0.))],
            None,
            None,
        )
        .unwrap();
        assert!(out.faces.is_empty());
    }
    #[test]
    fn original_old_disconnected_curve_rebuilds_deleted_flat_region() {
        let mut m = square();
        m.faces.clear();
        for e in &mut m.edges {
            e.faces.clear();
        }
        for v in &mut m.vertices {
            v.position.x += 10.;
        }
        let a = m.add_vertex(DVec3::ZERO);
        let b = m.add_vertex(DVec3::X);
        let c = m.add_vertex(DVec3::X + DVec3::Y);
        for (a, b) in [(a, b), (b, c)] {
            let id = m.add_edge(a, b).unwrap();
            m.edges[id].curve_id = Some(71);
        }
        let out = add_segments(
            &m,
            &[(DVec3::new(20., 0., 0.), DVec3::new(21., 0., 0.))],
            None,
            None,
        )
        .unwrap();
        assert_eq!(out.faces.len(), 1);
        assert!((out.face_area(0) - 1.).abs() < 1e-9);
    }
    #[test]
    fn unrelated_spatial_vertices_do_not_supply_drafting_planes() {
        let mut m = Mesh::new();
        for i in 0..5000 {
            m.add_vertex(DVec3::new(
                i as f64 + 10.,
                (i * i) as f64 + 10.,
                (i % 17) as f64 + 1.,
            ));
        }
        let uid = m.vertices[4999].uid;
        let out = add_segments(&m, &[(DVec3::ZERO, DVec3::X)], None, None).unwrap();
        assert_eq!(out.edges.len(), 1);
        assert!(out.faces.is_empty());
        assert!(out.vertices.iter().any(|v| v.uid == uid));
    }
    #[test]
    fn rectangle_crossing_mother_hole_does_not_claim_paint_only_containment() {
        let mut mesh = Mesh::new();
        let outer = [(0., 0.), (4., 0.), (4., 4.), (0., 4.)]
            .map(|(x, y)| mesh.add_vertex(DVec3::new(x, y, 0.)));
        let hole = [(1., 1.), (1., 3.), (3., 3.), (3., 1.)]
            .map(|(x, y)| mesh.add_vertex(DVec3::new(x, y, 0.)));
        let f = mesh.add_face(outer.to_vec(), vec![hole.to_vec()]).unwrap();
        mesh.faces[f]
            .attrs
            .insert("vendor".into(), serde_json::json!(71));
        mesh.faces[f]
            .attrs
            .insert("color".into(), serde_json::json!([1, 0, 0]));
        let ring = [
            DVec3::new(0.5, 0., 0.),
            DVec3::new(2., 0., 0.),
            DVec3::new(2., 4., 0.),
            DVec3::new(0.5, 4., 0.),
        ];
        let segments = (0..4)
            .map(|i| (ring[i], ring[(i + 1) % 4]))
            .collect::<Vec<_>>();
        let out = add_segments(&mesh, &segments, Some(&ring), None).unwrap();
        assert_eq!(out.faces.len(), 4);
        let painted: Vec<_> = out
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.attrs.get("color").is_some())
            .collect();
        assert_eq!(painted.len(), 3);
        assert!(painted
            .iter()
            .all(|(_, f)| f.attrs.get("vendor") == Some(&serde_json::json!(71))));
        assert!((painted.iter().map(|(i, _)| out.face_area(*i)).sum::<f64>() - 12.).abs() < 1e-9);
        assert!(out
            .faces
            .iter()
            .enumerate()
            .any(|(i, f)| f.attrs.is_empty() && (out.face_area(i) - 2.).abs() < 1e-9));
    }
    fn holed_surface(size: f64, holes: &[[(f64, f64); 4]]) -> Mesh {
        let mut mesh = Mesh::new();
        let outer = [(0., 0.), (size, 0.), (size, size), (0., size)]
            .map(|(x, y)| mesh.add_vertex(DVec3::new(x, y, 0.)));
        let holes = holes
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|&(x, y)| mesh.add_vertex(DVec3::new(x, y, 0.)))
                    .collect()
            })
            .collect();
        let id = mesh.add_face(outer.to_vec(), holes).unwrap();
        mesh.faces[id]
            .attrs
            .insert("vendor".into(), serde_json::json!(71));
        mesh.faces[id]
            .attrs
            .insert("color".into(), serde_json::json!([1, 0, 0]));
        mesh
    }
    #[test]
    fn line_authors_touched_hole_cells_and_leaves_unrelated_void_empty() {
        let mesh = holed_surface(
            8.,
            &[
                [(1., 1.), (1., 3.), (3., 3.), (3., 1.)],
                [(5., 5.), (5., 7.), (7., 7.), (7., 5.)],
            ],
        );
        let uid = mesh.vertices[0].uid;
        let out = add_segments(
            &mesh,
            &[(DVec3::new(0., 2., 0.), DVec3::new(8., 2., 0.))],
            None,
            None,
        )
        .unwrap();
        assert_eq!(out.faces.len(), 4);
        assert!(
            (out.faces
                .iter()
                .enumerate()
                .map(|(i, _)| out.face_area(i))
                .sum::<f64>()
                - 60.)
                .abs()
                < 1e-8
        );
        assert!(out
            .vertices
            .iter()
            .any(|v| v.uid == uid && v.position == DVec3::ZERO));
        for face in &out.faces {
            let rings = planar::rings(&out, face);
            let frame = planar::Frame::new(&rings[0]).unwrap();
            let projected = rings
                .iter()
                .map(|r| r.iter().map(|&p| frame.project(p)).collect())
                .collect::<Vec<_>>();
            assert!(!planar::inside_rings(
                frame.project(DVec3::new(6., 6., 0.)),
                &projected
            ));
        }
        assert!(out
            .faces
            .iter()
            .any(|f| f.attrs.contains_key("color") && !f.attrs.contains_key("vendor")));
        assert!(out.faces.iter().any(|f| f.attrs.contains_key("vendor")));
    }
    #[test]
    fn curve_contours_break_at_hard_edge_junctions_but_keep_plain_stations() {
        let mesh = holed_surface(4., &[[(1., 1.), (1., 3.), (3., 3.), (3., 1.)]]);
        let out = add_segments(
            &mesh,
            &[
                (DVec3::new(0., 2., 0.), DVec3::new(2., 2., 0.)),
                (DVec3::new(2., 2., 0.), DVec3::new(4., 2., 0.)),
            ],
            None,
            Some(99),
        )
        .unwrap();
        let groups = out
            .edges
            .iter()
            .filter_map(|e| e.curve_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(groups.len(), 3);
        let center = out
            .vertices
            .iter()
            .position(|v| v.position.distance(DVec3::new(2., 2., 0.)) < 1e-7)
            .unwrap();
        let center_ids = out.vertices[center]
            .edges
            .iter()
            .filter_map(|&e| out.edges[e].curve_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(center_ids.len(), 1);
    }
    #[test]
    fn chord_outside_a_hole_matches_source_auto_face_carve_and_coverage() {
        let mesh = holed_surface(4., &[[(1., 1.), (1., 3.), (3., 3.), (3., 1.)]]);
        let out = add_segments(
            &mesh,
            &[(DVec3::new(0., 0.5, 0.), DVec3::new(4., 0.5, 0.))],
            None,
            None,
        )
        .unwrap();
        assert_eq!(out.faces.len(), 3);
        assert!(
            (out.faces
                .iter()
                .enumerate()
                .map(|(i, _)| out.face_area(i))
                .sum::<f64>()
                - 16.)
                .abs()
                < 1e-8
        );
        assert!(out
            .faces
            .iter()
            .enumerate()
            .any(|(i, f)| (out.face_area(i) - 2.).abs() < 1e-8
                && f.attrs.contains_key("color")
                && !f.attrs.contains_key("vendor")));
        assert!(out
            .faces
            .iter()
            .enumerate()
            .any(|(i, f)| (out.face_area(i) - 4.).abs() < 1e-8 && f.attrs.is_empty()));
        assert!(out
            .faces
            .iter()
            .any(|f| f.holes.len() == 1 && f.attrs.contains_key("vendor")));
    }
    #[test]
    fn nonfinite_batch_is_atomic() {
        let m = square();
        assert!(add_segments(
            &m,
            &[(DVec3::ZERO, DVec3::new(f64::NAN, 1., 0.))],
            None,
            None
        )
        .is_err());
        assert_eq!(m.faces.len(), 1);
    }
}

#[cfg(test)]
mod bim_draw_tests {
 use super::*;
 fn ring(x:f64,y:f64,w:f64,h:f64)->Vec<DVec3>{vec![DVec3::new(x,y,0.),DVec3::new(x+w,y,0.),DVec3::new(x+w,y+h,0.),DVec3::new(x,y+h,0.)]}
 fn rect(m:&Mesh,p:&[DVec3],active:Option<&serde_json::Value>)->Result<Mesh,String>{let segments=(0..p.len()).map(|i|(p[i],p[(i+1)%p.len()])).collect::<Vec<_>>();add_segments_tagged(m,&segments,Some(p),None,active)}
 fn active()->serde_json::Value{serde_json::json!({"class":"IfcWall","name":"Muros N1"})}
 #[test]fn each_explicit_trace_gets_its_own_object(){let tag=active();let a=rect(&Mesh::new(),&ring(0.,0.,4.,0.15),Some(&tag)).unwrap();let b=rect(&a,&ring(0.,5.,4.,0.15),Some(&tag)).unwrap();let ids:std::collections::BTreeSet<_>=b.faces.iter().map(|f|f.attrs["ifc"]["id"].as_i64().unwrap()).collect();assert_eq!(ids,std::collections::BTreeSet::from([1,2]));}
 #[test]fn contained_authored_face_does_not_tag_mother_remainder(){let mut m=Mesh::new();let p=ring(0.,0.,6.,6.);let v=p.into_iter().map(|p|m.add_vertex(p)).collect();m.add_face(v,vec![]).unwrap();let o=rect(&m,&ring(2.,2.,1.,1.),Some(&active())).unwrap();let tagged:Vec<_>=o.faces.iter().enumerate().filter(|(_,f)|f.attrs.contains_key("ifc")).collect();assert_eq!(tagged.len(),1);assert!((o.face_area(tagged[0].0)-1.).abs()<1e-9);assert!(o.faces.iter().any(|f|!f.attrs.contains_key("ifc")));}
 #[test]fn line_cycle_and_chord_are_explicit_auto_declarations(){let tag=active();let p=ring(0.,0.,2.,2.);let mut m=Mesh::new();for i in 0..4{m=add_segments_tagged(&m,&[(p[i],p[(i+1)%4])],None,None,Some(&tag)).unwrap();}assert_eq!(m.faces.len(),1);assert_eq!(m.faces[0].attrs["ifc"]["id"],1);let o=add_segments_tagged(&m,&[(DVec3::new(0.,1.,0.),DVec3::new(2.,1.,0.))],None,None,Some(&tag)).unwrap();assert_eq!(o.faces.len(),2);assert!(o.faces.iter().all(|f|f.attrs["ifc"]["id"]==1));}
 #[test]fn exhaustion_refuses_without_mutating_input(){let mut m=rect(&Mesh::new(),&ring(0.,0.,1.,1.),None).unwrap();m.faces[0].attrs.insert("ifc".into(),serde_json::json!({"id":i64::MAX,"class":"IfcWall"}));let before=m.clone();assert!(rect(&m,&ring(5.,0.,1.,1.),Some(&active())).unwrap_err().contains("identity exhausted"));assert_eq!(m,before);}
}

#[cfg(test)]mod segmented_curve_metadata_tests {
 use super::*;
 #[test]fn nonflat_segmented_chord_authors_paint_only_region(){
  let mut m=Mesh::new();let ids=[DVec3::new(0.,0.,0.),DVec3::new(4.,0.,0.),DVec3::new(4.,4.,0.),DVec3::new(0.,4.,0.)].map(|p|m.add_vertex(p));let f=m.add_face(ids.to_vec(),vec![]).unwrap();m.faces[f].attrs.insert("vendor".into(),71.into());m.faces[f].attrs.insert("color".into(),serde_json::json!([1,0,0]));let a=m.add_vertex(DVec3::new(10.,0.,4.));let b=m.add_vertex(DVec3::new(11.,0.,4.));m.add_edge(a,b);
  let segments=[(DVec3::new(0.,2.,0.),DVec3::new(2.,2.,0.)),(DVec3::new(2.,2.,0.),DVec3::new(4.,2.,0.))];let tag=serde_json::json!({"class":"IfcWall","name":"context"});
  let out=add_segments_tagged(&m,&segments,None,Some(99),Some(&tag)).unwrap();
  assert_eq!(out.faces.len(),2);let lower=out.faces.iter().find(|f|f.vertices.iter().map(|&i|out.vertices[i].position.y).sum::<f64>()/(f.vertices.len()as f64)<2.).unwrap();
  assert!(!lower.attrs.contains_key("vendor"));assert_eq!(lower.attrs["color"],serde_json::json!([1,0,0]));assert!(!lower.attrs.contains_key("ifc"));
 }
}

#[cfg(test)]
mod source_shape_plane_tests {
 use super::*;
 #[test]
 fn source_plane_admission_does_not_downcast_native_double_or_nonplanar_rows(){
  let n=qt_normalized(glam::Vec3::ONE);let u=qt_normalized(glam::Vec3::X-n*n.x);let v=qt_normalized(n.cross(u));
  let points=(0..24).map(|i|{let a=std::f64::consts::TAU*i as f64/24.;(u*(a.cos() as f32)+v*(a.sin() as f32)).as_dvec3()}).collect::<Vec<_>>();
  assert!(source_author_plane(points.iter().copied()).is_some());
  let mut nonplanar=points.clone();nonplanar[3]+=(n*0.001).as_dvec3();assert!(source_author_plane(nonplanar.iter().copied()).is_none());
  let mut native=points.clone();native[0].x+=1e-10;let preserved=native[0].x.to_bits();assert!(source_author_plane(native.iter().copied()).is_none());assert_eq!(native[0].x.to_bits(),preserved);
 }
}
