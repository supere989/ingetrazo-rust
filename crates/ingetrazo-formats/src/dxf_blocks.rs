//! Typed DXF definition scan and source component placement ownership.
use super::{DxfEntity, DxfError, DxfPlan};
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
use std::collections::{HashMap, HashSet};
type Pair = (i32, String);
#[derive(Clone, Copy)]
struct Record {
    begin: usize,
    attrs_end: usize,
    end: usize,
}
struct Definition {
    base: DVec3,
    records: Vec<Record>,
}
pub(super) struct Prototype {
    name: String,
    mesh: Mesh,
}
pub(super) struct Placement {
    name: String,
    matrix: DMat4,
    layer: String,
}
fn invalid(s: &str) -> DxfError {
    DxfError::Parse(s.into())
}
fn value<'a>(a: &'a [Pair], code: i32, default: &'a str) -> &'a str {
    a.iter()
        .find(|(c, _)| *c == code)
        .map_or(default, |(_, s)| s)
}
fn number(a: &[Pair], code: i32, default: f64) -> Result<f64, DxfError> {
    a.iter()
        .find(|(c, _)| *c == code)
        .map_or(Ok(default), |(_, v)| {
            v.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| invalid("Invalid block numeric field"))
        })
}
fn point(a: &[Pair], n: i32) -> Result<DVec3, DxfError> {
    Ok(DVec3::new(
        number(a, 10 + n, 0.)?,
        number(a, 20 + n, 0.)?,
        number(a, 30 + n, 0.)?,
    ))
}
fn record(pairs: &[Pair], begin: usize) -> Record {
    let mut end = begin + 1;
    while end < pairs.len() && pairs[end].0 != 0 {
        end += 1;
    }
    let attrs_end = end;
    if pairs[begin].1 == "POLYLINE" {
        while end < pairs.len() && pairs[end].0 == 0 && pairs[end].1 == "VERTEX" {
            end += 1;
            while end < pairs.len() && pairs[end].0 != 0 {
                end += 1;
            }
        }
        if end < pairs.len() && pairs[end].0 == 0 && pairs[end].1 == "SEQEND" {
            end += 1;
            while end < pairs.len() && pairs[end].0 != 0 {
                end += 1;
            }
        }
    }
    if pairs[begin].1 == "INSERT"
        && pairs[begin + 1..attrs_end]
            .iter()
            .any(|(c, v)| *c == 66 && v.parse::<i32>().ok().is_some_and(|v| v != 0))
    {
        while end < pairs.len() && pairs[end].0 == 0 && pairs[end].1 == "ATTRIB" {
            end += 1;
            while end < pairs.len() && pairs[end].0 != 0 {
                end += 1;
            }
        }
        if end < pairs.len() && pairs[end].0 == 0 && pairs[end].1 == "SEQEND" {
            end += 1;
            while end < pairs.len() && pairs[end].0 != 0 {
                end += 1;
            }
        }
    }
    Record {
        begin,
        attrs_end,
        end,
    }
}
fn attrs(p: &[Pair], r: Record) -> &[Pair] {
    &p[r.begin + 1..r.attrs_end]
}
fn matrix(a: &[Pair], base: DVec3, source_points: bool) -> Result<DMat4, DxfError> {
    let normal = DVec3::new(
        number(a, 210, 0.)?,
        number(a, 220, 0.)?,
        number(a, 230, 1.)?,
    );
    let ocs = super::curves::ocs(normal)?;
    let angle = number(a, 50, 0.)?.to_radians();
    let c = libm::cos(angle);
    let s = libm::sin(angle);
    // ezdxf UCS columns scaled first, then WCS rotation about extrusion.
    let rotate = |v: DVec3| {
        if angle == 0. {
            return v;
        }
        let n = ocs.z;
        let one = 1. - c;
        let xyz = DVec3::new(
            (one * n.x * n.x + c) * v.x
                + (one * n.x * n.y - s * n.z) * v.y
                + (one * n.x * n.z + s * n.y) * v.z,
            (one * n.x * n.y + s * n.z) * v.x
                + (one * n.y * n.y + c) * v.y
                + (one * n.y * n.z - s * n.x) * v.z,
            (one * n.x * n.z - s * n.y) * v.x
                + (one * n.y * n.z + s * n.x) * v.y
                + (one * n.z * n.z + c) * v.z,
        );
        if source_points {
            // INSERT.matrix44 multiplies UCS by a homogeneous axis-rotation
            // matrix. Each direction row includes the fourth 0*0 term;
            // omitting it changes signed zero for rotated negative scales.
            DVec3::new(xyz.x + 0. * 0., xyz.y + 0. * 0., xyz.z + 0. * 0.)
        } else { xyz }
    };
    let x = rotate(ocs.x * number(a, 41, 1.)?);
    let y = rotate(ocs.y * number(a, 42, 1.)?);
    let z = rotate(ocs.z * number(a, 43, 1.)?);
    let offset = ocs.map(point(a, 0)?) - (x * base.x + y * base.y + z * base.z);
    let m = DMat4::from_cols(x.extend(0.), y.extend(0.), z.extend(0.), offset.extend(1.));
    if !m.is_finite() {
        return Err(invalid("Block transform overflow"));
    }
    Ok(m)
}
fn polyline(p: &[Pair], r: Record, scale: f64, m: DMat4) -> Result<Vec<DxfEntity>, DxfError> {
    let a = attrs(p, r);
    let legacy = p[r.begin].1 == "POLYLINE";
    let flags = number(a, 70, 0.)? as u32;
    if legacy && flags & (16 | 64) != 0 {
        return Ok(Vec::new());
    }
    let layer = value(a, 8, "0").to_owned();
    let mut vertices = Vec::<(DVec3, f64)>::new();
    if legacy {
        let mut i = r.attrs_end;
        while i < r.end && p[i].1 == "VERTEX" {
            let v = record(p, i);
            let fields = attrs(p, v);
            vertices.push((point(fields, 0)?, number(fields, 42, 0.)?));
            i = v.end;
        }
    } else {
        for (c, v) in a {
            if *c == 10 {
                vertices.push((
                    DVec3::new(
                        v.parse::<f64>()
                            .map_err(|_| invalid("Invalid block polyline X"))?,
                        0.,
                        0.,
                    ),
                    0.,
                ));
            } else if *c == 20 || *c == 42 {
                let n = v
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("Invalid block polyline field"))?;
                let last = vertices
                    .last_mut()
                    .ok_or_else(|| invalid("Block polyline field without vertex"))?;
                if *c == 20 {
                    last.0.y = n;
                } else {
                    last.1 = n;
                }
            }
        }
    }
    if vertices.len() < 2 {
        return Ok(Vec::new());
    }
    if vertices
        .iter()
        .any(|(v, b)| !v.is_finite() || !b.is_finite())
    {
        return Err(invalid("Block polyline coordinate overflow"));
    }
    let closed = flags & 1 != 0;
    if legacy && flags & 8 != 0 {
        let mut points = vertices
            .iter()
            .map(|(v, _)| m.transform_point3(*v) * scale)
            .collect::<Vec<_>>();
        if closed && points.first() != points.last() {
            points.push(points[0]);
        }
        return Ok(vec![DxfEntity {
            layer,
            points,
            face: false,
            curved: vertices.iter().any(|(_, b)| b.abs() > 1e-12),
        }]);
    }
    let elevation = number(
        a,
        if legacy { 30 } else { 38 },
        if legacy { vertices[0].0.z } else { 0. },
    )?;
    let normal = DVec3::new(
        number(a, 210, 0.)?,
        number(a, 220, 0.)?,
        number(a, 230, 1.)?,
    );
    let old = super::curves::ocs(normal)?;
    let x = m.transform_vector3(old.x);
    let y = m.transform_vector3(old.y);
    let xx = x.length_squared();
    let yy = y.length_squared();
    let uniform = (xx - yy).abs() <= (1e-9 * xx.abs().max(yy.abs())).max(1e-9);
    let has_arc = vertices.iter().any(|(_, b)| *b != 0.);
    if uniform || !has_arc {
        let n = x.cross(y);
        let length = n.length();
        if length == 0. || !length.is_finite() {
            return Err(invalid("Invalid block polyline extrusion"));
        }
        let n = n / length;
        let new = super::curves::ocs(n)?;
        let mut mapped = Vec::with_capacity(vertices.len());
        for (v, b) in &vertices {
            let world = m.transform_point3(old.map(DVec3::new(v.x, v.y, elevation)));
            mapped.push((
                DVec3::new(new.x.dot(world), new.y.dot(world), new.z.dot(world)),
                *b,
            ));
        }
        let points = super::curves::polyline(&mapped, closed, n, mapped[0].0.z, scale)?;
        return Ok(vec![DxfEntity {
            layer,
            points,
            face: false,
            curved: if legacy {
                vertices.iter().any(|(_, b)| b.abs() > 1e-12)
            } else {
                has_arc
            },
        }]);
    }
    // virtual_entities splits nonuniform bulges into ordered ARC/LINE objects;
    // each ARC gets its own curve owner, straight pieces remain straight.
    let count = vertices.len() - 1 + usize::from(closed);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let (a, b) = vertices[i];
        let (end, _) = vertices[(i + 1) % vertices.len()];
        let a = DVec3::new(a.x, a.y, elevation);
        let end = DVec3::new(end.x, end.y, elevation);
        if b == 0. {
            out.push(DxfEntity {
                layer: layer.clone(),
                points: vec![
                    m.transform_point3(old.map(a)) * scale,
                    m.transform_point3(old.map(end)) * scale,
                ],
                face: false,
                curved: false,
            });
        } else {
            let radius = libm::hypot(end.x - a.x, end.y - a.y) * (1. + b * b) / 4. / b;
            let angle = libm::atan2(end.y - a.y, end.x - a.x)
                + (std::f64::consts::PI / 2. - libm::atan(b) * 2.);
            let center = DVec3::new(
                a.x + radius * libm::cos(angle),
                a.y + radius * libm::sin(angle),
                elevation,
            );
            let mut start = libm::atan2(a.y - center.y, a.x - center.x);
            let mut finish = libm::atan2(end.y - center.y, end.x - center.x);
            if b < 0. {
                std::mem::swap(&mut start, &mut finish);
            }
            let points = super::curves::transformed_circle(
                center,
                radius.abs(),
                normal,
                start.to_degrees().to_radians(),
                finish.to_degrees().to_radians(),
                m,
                scale,
            )?;
            if !points.is_empty() {
                out.push(DxfEntity {
                    layer: layer.clone(),
                    points,
                    face: false,
                    curved: true,
                });
            }
        }
    }
    Ok(out)
}
fn emitted(p: &[Pair], r: Record, scale: f64, m: DMat4) -> Result<Option<DxfEntity>, DxfError> {
    let kind = p[r.begin].1.as_str();
    let a = attrs(p, r);
    let mut cursor = r.attrs_end;
    if kind == "INSERT" || kind == "MINSERT" {
        return Ok(None);
    }
    // Curves must be transformed as analytic entities before adaptive flattening.
    if m != DMat4::IDENTITY && matches!(kind, "CIRCLE" | "ARC" | "ELLIPSE") {
        let center = point(a, 0)?;
        let normal = DVec3::new(
            number(a, 210, 0.)?,
            number(a, 220, 0.)?,
            number(a, 230, 1.)?,
        );
        let points = if kind == "ELLIPSE" {
            super::curves::transformed_ellipse(
                center,
                point(a, 1)?,
                normal,
                number(a, 40, 1.)?,
                number(a, 41, 0.)?,
                number(a, 42, std::f64::consts::TAU)?,
                m,
                scale,
            )?
        } else {
            super::curves::transformed_circle(
                center,
                number(a, 40, 0.)?,
                normal,
                if kind == "CIRCLE" {
                    0.
                } else {
                    number(a, 50, 0.)?.to_radians()
                },
                if kind == "CIRCLE" {
                    std::f64::consts::TAU
                } else {
                    number(a, 51, 360.)?.to_radians()
                },
                m,
                scale,
            )?
        };
        return Ok(Some(DxfEntity {
            layer: value(a, 8, "0").into(),
            points,
            face: false,
            curved: true,
        }));
    }
    let Some(mut entity) = super::parse_entity(
        kind,
        a,
        p,
        &mut cursor,
        if kind == "SPLINE" || m == DMat4::IDENTITY {
            scale
        } else {
            1.
        },
        false,
        if kind == "SPLINE" { m } else { DMat4::IDENTITY },
    )?
    else {
        return Ok(None);
    };
    if m != DMat4::IDENTITY && kind != "SPLINE" {
        if entity.curved {
            return Err(invalid(
                "Nested block bulged polyline transform requires virtual-entity admission",
            ));
        }
        for v in &mut entity.points {
            *v = m.transform_point3(*v) * scale;
            if !v.is_finite() {
                return Err(invalid("Block coordinate overflow"));
            }
        }
    }
    Ok(Some(entity))
}
fn contents(
    p: &[Pair],
    defs: &HashMap<String, Definition>,
    records: &[Record],
    parent: DMat4,
    depth: usize,
    scale: f64,
    source_points: bool,
    out: &mut Vec<DxfEntity>,
) -> Result<(), DxfError> {
    for &r in records {
        if p[r.begin].1 == "INSERT" || p[r.begin].1 == "MINSERT" {
            if depth >= 8 {
                continue;
            }
            let a = attrs(p, r);
            if let Some(def) = defs.get(&value(a, 2, "").to_uppercase()) {
                let local = matrix(a, def.base, source_points)?;
                contents(p, defs, &def.records, parent * local, depth + 1, scale, source_points, out)?;
            }
            continue;
        }
        if parent != DMat4::IDENTITY && matches!(p[r.begin].1.as_str(), "POLYLINE" | "LWPOLYLINE") {
            for mut e in polyline(p, r, scale, parent)? {
                e.layer = "0".into();
                out.push(e);
            }
            continue;
        }
        if let Some(mut e) = emitted(p, r, scale, parent)? {
            e.layer = "0".into();
            out.push(e);
        }
    }
    Ok(())
}
fn build(entities: &[DxfEntity]) -> Result<Mesh, DxfError> {
    let mut mesh = Mesh::new();
    let mut curve = Some(0);
    for face in [false, true] {
        for e in entities.iter().filter(|e| e.face == face) {
            DxfPlan::emit_entity(e, &mut mesh, &mut curve, true)?;
        }
    }
    Ok(mesh)
}
pub(super) fn parse(p: &[Pair], scale: f64, source_points: bool) -> Result<DxfPlan, DxfError> {
    let mut defs = HashMap::<String, Definition>::new();
    let mut model = Vec::new();
    let mut section = "";
    let mut block = None;
    let mut i = 0;
    while i < p.len() {
        if p[i].0 != 0 {
            i += 1;
            continue;
        }
        let r = record(p, i);
        let kind = p[i].1.as_str();
        let a = attrs(p, r);
        match kind {
            "SECTION" => section = value(a, 2, ""),
            "ENDSEC" => {
                section = "";
                block = None;
            }
            "BLOCK" if section == "BLOCKS" => {
                let name = value(a, 2, "").to_uppercase();
                let def = Definition {
                    base: point(a, 0)?,
                    records: Vec::new(),
                };
                defs.insert(name.clone(), def);
                block = Some(name);
            }
            "ENDBLK" => block = None,
            _ if section == "BLOCKS" => {
                if let Some(name) = &block {
                    defs.get_mut(name).unwrap().records.push(r);
                }
            }
            _ if section == "ENTITIES" => model.push(r),
            _ => {}
        }
        i = r.end;
    }
    let mut entities = Vec::new();
    let mut placements = Vec::new();
    let mut names = HashSet::new();
    let mut prototypes = Vec::new();
    for r in model {
        if p[r.begin].1 == "INSERT" {
            let a = attrs(p, r);
            let name = value(a, 2, "").to_owned();
            let def = defs.get(&name.to_uppercase());
            placements.push(Placement {
                name: name.clone(),
                matrix: matrix(a, def.map_or(DVec3::ZERO, |d| d.base), source_points)?,
                layer: value(a, 8, "0").into(),
            });
            if names.insert(name.clone()) {
                if let Some(def) = def {
                    let mut rows = Vec::new();
                    contents(p, &defs, &def.records, DMat4::IDENTITY, 0, scale, source_points, &mut rows)?;
                    if source_points {
                        for e in &mut rows {
                            for v in &mut e.points {
                                *v = v.as_vec3().as_dvec3();
                                if !v.is_finite() {
                                    return Err(invalid("Block Qt coordinate overflow"));
                                }
                            }
                        }
                    }
                    let mesh = if source_points { DxfPlan::build_source_mesh(&rows)? } else { build(&rows)? };
                    if !mesh.edges.is_empty() || !mesh.faces.is_empty() {
                        prototypes.push(Prototype { name, mesh });
                    }
                }
            }
        } else if let Some(e) = emitted(p, r, scale, DMat4::IDENTITY)? {
            entities.push(e);
        }
    }
    if entities.is_empty() && prototypes.is_empty() {
        return Err(invalid("No importable DXF geometry"));
    }
    // Measure all ordinary points and INSERT translations, including absent/empty
    // definitions, before either Qt narrowing or offset subtraction.
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for v in entities.iter().flat_map(|e| &e.points).copied().chain(
        placements
            .iter()
            .map(|p| p.matrix.w_axis.truncate() * scale),
    ) {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let offset = if source_points
        && lo.x < hi.x
        && lo.abs().max_element().max(hi.abs().max_element()) > 100000.
    {
        DVec3::new(lo.x.trunc(), lo.y.trunc(), lo.z.trunc())
    } else {
        DVec3::ZERO
    };
    for e in &mut entities {
        for v in &mut e.points {
            *v -= offset;
            if source_points {
                *v = v.as_vec3().as_dvec3();
            }
            if !v.is_finite() {
                return Err(invalid("DXF coordinate overflow"));
            }
        }
    }
    for p in &mut placements {
        p.matrix.w_axis.x = p.matrix.w_axis.x * scale - offset.x;
        p.matrix.w_axis.y = p.matrix.w_axis.y * scale - offset.y;
        p.matrix.w_axis.z = p.matrix.w_axis.z * scale - offset.z;
        if source_points {
            p.matrix = p.matrix.as_mat4().as_dmat4();
        }
        if !p.matrix.is_finite() {
            return Err(invalid("DXF placement overflow"));
        }
    }
    Ok(DxfPlan {
        entities,
        block_owners: prototypes,
        placements,
    })
}
pub(super) fn publish(
    prototypes: Vec<Prototype>,
    placements: Vec<Placement>,
    scene: &mut Scene,
) -> Result<(), DxfError> {
    let mut owners = HashMap::new();
    for proto in prototypes {
        let id = uuid::Uuid::new_v4();
        scene.component_definitions.insert(
            id,
            ingetrazo_core::components::ComponentDefinition {
                mesh_share: Some(id),
                mesh: proto.mesh.clone(),
                children: Vec::new(),
            },
        );
        scene.mesh_shares.insert(id, proto.mesh);
        owners.insert(proto.name, id);
    }
    for p in placements {
        if let Some(id) = owners.get(&p.name) {
            let mut g = Group::with_mesh(p.name, scene.mesh_shares[id].clone());
            g.transform = p.matrix;
            g.mesh_share = Some(*id);
            g.component_definition = Some(*id);
            crate::obj::source_group_kind(&mut g, true);
            if p.layer != "0" {
                if !scene.layers.iter().any(|l| l.name == p.layer) {
                    scene
                        .layers
                        .push(ingetrazo_core::layers::Layer::new(&p.layer));
                }
                g.layer = Some(p.layer);
            }
            scene.groups.push(g);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const BLOCK:&str="0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nCase\n10\n0\n20\n0\n30\n0\n0\nLINE\n10\n0\n20\n0\n30\n0\n11\n2\n21\n1\n31\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n";
    #[test]
    fn definition_lookup_is_folded_but_prototype_ownership_is_exact() {
        let text=format!("{BLOCK}0\nINSERT\n2\nCase\n10\n0\n0\nINSERT\n2\ncase\n10\n4\n0\nINSERT\n2\nCase\n10\n8\n0\nENDSEC\n0\nEOF\n");
        let plan = super::super::parse_dxf_with_units(&mut text.as_bytes(), 1.).unwrap();
        assert_eq!(plan.block_owners.len(), 2);
        assert_eq!(plan.placements.len(), 3);
        let mut scene = Scene::new();
        plan.emit_source(&mut scene, "drawing").unwrap();
        assert_eq!(scene.groups[0].mesh_share, scene.groups[2].mesh_share);
        assert_ne!(scene.groups[0].mesh_share, scene.groups[1].mesh_share);
        assert_eq!(scene.mesh_shares.len(), 2);
    }
    #[test]
    fn owned_attribute_sequence_is_not_modelspace_geometry() {
        let text=format!("{BLOCK}0\nINSERT\n2\nCase\n66\n1\n10\n3\n0\nATTRIB\n2\nTAG\n1\nopaque text\n10\n3\n0\nSEQEND\n0\nENDSEC\n0\nEOF\n");
        let plan = super::super::parse_dxf_with_units(&mut text.as_bytes(), 1.).unwrap();
        assert_eq!(plan.placements.len(), 1);
        assert!(plan.entities.is_empty());
    }
    #[test]
    fn qt_placement_overflow_preserves_native_document() {
        let text = format!("{BLOCK}0\nINSERT\n2\nCase\n41\n1e308\n0\nENDSEC\n0\nEOF\n");
        let mut scene = Scene::new();
        scene
            .mesh
            .add_vertex(DVec3::new(1. + 2f64.powi(-40), 2., 3.));
        let old = scene.clone();
        assert!(super::super::import_dxf_with_units(&mut text.as_bytes(), &mut scene, 1.).is_err());
        assert_eq!(scene, old);
    }
}
