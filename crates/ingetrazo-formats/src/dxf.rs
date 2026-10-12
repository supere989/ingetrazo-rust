// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! AutoCAD DXF format export and import.

#[path = "dxf_blocks.rs"]
mod blocks;
#[path = "dxf_curves.rs"]
mod curves;
#[path = "dxf_spline.rs"]
mod spline;
use glam::DVec3;
use ingetrazo_core::scene::Scene;
use std::io::{BufRead, BufReader, Read, Write};

#[derive(thiserror::Error, Debug)]
pub enum DxfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
}

pub fn export_dxf(scene: &Scene, writer: &mut impl Write) -> Result<(), DxfError> {
    writeln!(writer, "0\nSECTION\n2\nHEADER\n0\nENDSEC")?;
    writeln!(writer, "0\nSECTION\n2\nTABLES\n0\nENDSEC")?;
    writeln!(writer, "0\nSECTION\n2\nENTITIES")?;

    for triangle in crate::exchange::triangles(scene) {
        let [p0, p1, p2] = triangle.points;
        let layer = triangle.layer.replace(['\n', '\r'], "_");
        writeln!(writer, "0\n3DFACE\n8\n{layer}")?;
        for (i, p) in [p0, p1, p2, p2].into_iter().enumerate() {
            writeln!(
                writer,
                "{}\n{:.9}\n{}\n{:.9}\n{}\n{:.9}",
                10 + i,
                p.x,
                20 + i,
                p.y,
                30 + i,
                p.z
            )?;
        }
    }

    writeln!(writer, "0\nENDSEC")?;
    writeln!(writer, "0\nEOF")?;
    Ok(())
}

/// Parse a complete entity transaction before modifying the caller's document.
pub fn import_dxf(reader: &mut impl Read, scene: &mut Scene) -> Result<(), DxfError> {
    *scene = prepare_dxf_options(reader, scene, 1., false)?;
    Ok(())
}

/// Scale typed dimensional fields before tessellation and author only emitted points as Qt floats.
pub fn import_dxf_with_units(
    reader: &mut impl Read,
    scene: &mut Scene,
    scale: f64,
) -> Result<(), DxfError> {
    *scene = prepare_dxf_with_units(reader, scene, scale)?;
    Ok(())
}

/// Prepare the atomic source-unit import without publishing or an extra caller clone.
pub fn prepare_dxf_with_units(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
) -> Result<Scene, DxfError> {
    prepare_dxf_options(reader, scene, scale, true)
}

fn prepare_dxf_options(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
    source_points: bool,
) -> Result<Scene, DxfError> {
    let plan = parse_dxf_options(reader, scale, source_points)?;
    let mut candidate = scene.clone();
    plan.emit_legacy(&mut candidate)?;
    candidate.bump_version();
    Ok(candidate)
}
/// Parsed dimensional entities, independent of document ownership.
pub struct DxfPlan {
    entities: Vec<DxfEntity>,
    block_owners: Vec<blocks::Prototype>,
    placements: Vec<blocks::Placement>,
}
struct DxfEntity {
    layer: String,
    points: Vec<DVec3>,
    face: bool,
    curved: bool,
}
impl DxfPlan {
    fn emit_entity(
        entity: &DxfEntity,
        mesh: &mut ingetrazo_core::mesh::Mesh,
        next_curve: &mut Option<usize>,
        source_policy: bool,
    ) -> Result<(), DxfError> {
        let ids = entity
            .points
            .iter()
            .map(|p| mesh.add_vertex(*p))
            .collect::<Vec<_>>();
        if entity.face {
            let id = mesh
                .add_face(ids, vec![])
                .ok_or_else(|| DxfError::Parse("Invalid DXF face".into()))?;
            if !source_policy {
                mesh.faces[id].layer = Some(entity.layer.clone());
            }
        } else {
            let curve = if entity.curved {
                let next = next_curve.get_or_insert_with(|| {
                    mesh.edges
                        .iter()
                        .filter_map(|e| e.curve_id)
                        .max()
                        .map_or(0, |i| i + 1)
                });
                let curve = *next;
                *next += 1;
                Some(curve)
            } else {
                None
            };
            for pair in ids.windows(2) {
                if let Some(id) = mesh.add_edge(pair[0], pair[1]) {
                    if !source_policy {
                        mesh.edges[id].layer = Some(entity.layer.clone());
                    }
                    if entity.curved {
                        mesh.edges[id].curve_id = curve;
                    }
                }
            }
        }
        Ok(())
    }
    fn emit_legacy(self, scene: &mut Scene) -> Result<(), DxfError> {
        let mut curves = std::collections::BTreeMap::<String, Option<usize>>::new();
        for entity in &self.entities {
            let mesh = if entity.layer == "0" {
                &mut scene.mesh
            } else {
                if !scene.layers.iter().any(|l| l.name == entity.layer) {
                    scene
                        .layers
                        .push(ingetrazo_core::layers::Layer::new(&entity.layer));
                }
                let index = if let Some(index) = scene.groups.iter().position(|g| {
                    g.name == entity.layer && g.layer.as_deref() == Some(entity.layer.as_str())
                }) {
                    index
                } else {
                    let mut group = ingetrazo_core::groups::Group::new(&entity.layer);
                    group.layer = Some(entity.layer.clone());
                    scene.groups.push(group);
                    scene.groups.len() - 1
                };
                &mut scene.groups[index].mesh
            };
            Self::emit_entity(
                entity,
                mesh,
                curves.entry(entity.layer.clone()).or_insert(None),
                false,
            )?;
        }
        blocks::publish(self.block_owners, self.placements, scene)?;
        Ok(())
    }
    /// Every source DXF layer is a fresh document-root group, including layer zero.
    pub fn emit_source(self, scene: &mut Scene, stem: &str) -> Result<(), DxfError> {
        let mut layers = std::collections::BTreeMap::<String, Vec<DxfEntity>>::new();
        for entity in self.entities {
            layers.entry(entity.layer.clone()).or_default().push(entity);
        }
        if layers.is_empty() && self.placements.is_empty() {
            return Err(DxfError::Parse("No importable DXF geometry".into()));
        }
        for (layer, entities) in layers {
            let mut group =
                ingetrazo_core::groups::Group::new(if layer == "0" { stem } else { &layer });
            if layer != "0" {
                group.layer = Some(layer.clone());
                if !scene.layers.iter().any(|l| l.name == layer) {
                    scene
                        .layers
                        .push(ingetrazo_core::layers::Layer::new(&layer));
                }
            }
            let mut next_curve = Some(0);
            for entity in &entities {
                Self::emit_entity(entity, &mut group.mesh, &mut next_curve, true)?;
            }
            crate::obj::source_group_kind(&mut group, false);
            scene.groups.push(group);
        }
        blocks::publish(self.block_owners, self.placements, scene)?;
        scene.bump_version();
        Ok(())
    }
}
pub fn parse_dxf_with_units(reader: &mut impl Read, scale: f64) -> Result<DxfPlan, DxfError> {
    parse_dxf_options(reader, scale, true)
}
fn parse_dxf_options(
    reader: &mut impl Read,
    scale: f64,
    source_points: bool,
) -> Result<DxfPlan, DxfError> {
    if !scale.is_finite() || scale <= 0. {
        return Err(DxfError::Parse("Invalid import scale".into()));
    }

    let lines = BufReader::new(reader)
        .lines()
        .collect::<Result<Vec<_>, _>>()?;
    if lines.len() % 2 != 0 {
        return Err(DxfError::Parse("Incomplete DXF code/value pair".into()));
    }
    let pairs = lines
        .chunks_exact(2)
        .map(|p| {
            Ok((
                p[0].trim()
                    .parse::<i32>()
                    .map_err(|_| DxfError::Parse("Invalid DXF group code".into()))?,
                p[1].trim().to_owned(),
            ))
        })
        .collect::<Result<Vec<_>, DxfError>>()?;
    blocks::parse(&pairs, scale, source_points)
}

fn parse_entity(
    kind: &str,
    attributes: &[(i32, String)],
    pairs: &[(i32, String)],
    cursor: &mut usize,
    scale: f64,
    source_points: bool,
    spline_transform: glam::DMat4,
) -> Result<Option<DxfEntity>, DxfError> {
    if kind == "POLYLINE" {
        let (layer, mut points, curved) = legacy_polyline(attributes, pairs, cursor, scale)?;
        if points.is_empty() {
            return Ok(None);
        }
        for p in &mut points {
            if source_points {
                *p = p.as_vec3().as_dvec3();
            }
            if !p.is_finite() {
                return Err(DxfError::Parse("Import coordinate overflow".into()));
            }
        }
        return Ok(Some(DxfEntity {
            layer,
            points,
            face: false,
            curved,
        }));
    }
    // Preserve the baseline's deliberate omissions, rather than treating
    // annotations as broken modelling geometry.
    if [
        "TEXT",
        "MTEXT",
        "DIMENSION",
        "HATCH",
        "POINT",
        "ATTDEF",
        "VIEWPORT",
        "XLINE",
        "RAY",
    ]
    .contains(&kind)
    {
        return Ok(None);
    }
    let layer = attributes
        .iter()
        .find(|(c, _)| *c == 8)
        .map_or("0", |(_, v)| v.as_str());
    let number = |code, default| -> Result<f64, DxfError> {
        attributes
            .iter()
            .find(|(c, _)| *c == code)
            .map_or(Ok(default), |(_, v)| {
                v.parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| DxfError::Parse(format!("Invalid numeric field {code}")))
            })
    };
    if !matches!(kind, "LWPOLYLINE" | "CIRCLE" | "ARC" | "ELLIPSE" | "SPLINE")
        && (number(210, 0.)?.abs() > 1e-9
            || number(220, 0.)?.abs() > 1e-9
            || (number(230, 1.)? - 1.).abs() > 1e-9)
    {
        return Err(DxfError::Parse(
            "Non-Z DXF entity coordinate systems require OCS conversion".into(),
        ));
    }
    let point = |n| -> Result<DVec3, DxfError> {
        let point = DVec3::new(
            number(10 + n, 0.)?,
            number(20 + n, 0.)?,
            number(30 + n, 0.)?,
        ) * scale;
        if !point.is_finite() {
            return Err(DxfError::Parse("Import coordinate overflow".into()));
        }
        Ok(point)
    };
    let mut points = Vec::new();
    let mut face = false;
    let mut curved = false;
    match kind {
        "3DFACE" | "SOLID" | "TRACE" => {
            points = vec![point(0)?, point(1)?, point(2)?];
            let fourth = point(3)?;
            if attributes.iter().any(|(c, _)| *c == 13)
                && fourth != points[2]
                && fourth != points[0]
            {
                points.push(fourth);
            }
            if kind != "3DFACE" && points.len() == 4 {
                points.swap(2, 3);
            }
            face = true;
        }
        "LINE" => {
            points = vec![point(0)?, point(1)?];
        }
        "LWPOLYLINE" => {
            let elevation = number(38, 0.)?;
            let mut vertices = Vec::<(DVec3, f64)>::new();
            for (code, value) in attributes {
                if *code == 10 {
                    let x = value
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| DxfError::Parse("Invalid polyline X".into()))?;
                    vertices.push((DVec3::new(x, 0., 0.), 0.));
                } else if *code == 20 || *code == 42 {
                    let v = value
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| DxfError::Parse("Invalid polyline field".into()))?;
                    let last = vertices
                        .last_mut()
                        .ok_or_else(|| DxfError::Parse("Polyline field without vertex".into()))?;
                    if *code == 20 {
                        last.0.y = v;
                    } else {
                        last.1 = v;
                    }
                }
            }
            curved = vertices.iter().any(|(_, b)| *b != 0.);
            let normal = DVec3::new(number(210, 0.)?, number(220, 0.)?, number(230, 1.)?);
            points = curves::polyline(
                &vertices,
                (number(70, 0.)? as u32) & 1 != 0,
                normal,
                elevation,
                scale,
            )?;
        }
        "CIRCLE" | "ARC" => {
            let center = DVec3::new(number(10, 0.)?, number(20, 0.)?, number(30, 0.)?);
            let normal = DVec3::new(number(210, 0.)?, number(220, 0.)?, number(230, 1.)?);
            let start = if kind == "CIRCLE" {
                0.
            } else {
                number(50, 0.)?.to_radians()
            };
            let end = if kind == "CIRCLE" {
                std::f64::consts::TAU
            } else {
                number(51, 360.)?.to_radians()
            };
            points = curves::circle(center, number(40, 0.)?, normal, start, end, scale)?;
            curved = true;
        }
        "ELLIPSE" => {
            let center = DVec3::new(number(10, 0.)?, number(20, 0.)?, number(30, 0.)?);
            let major = DVec3::new(number(11, 1.)?, number(21, 0.)?, number(31, 0.)?);
            let normal = DVec3::new(number(210, 0.)?, number(220, 0.)?, number(230, 1.)?);
            points = curves::ellipse(
                center,
                major,
                normal,
                number(40, 1.)?,
                number(41, 0.)?,
                number(42, std::f64::consts::TAU)?,
                scale,
            )?;
            curved = true;
        }
        "SPLINE" => {
            let mut controls = Vec::<DVec3>::new();
            let mut fits = Vec::<DVec3>::new();
            let mut knots = Vec::new();
            let mut weights = Vec::new();
            for (code, value) in attributes {
                let scalar = || {
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| DxfError::Parse("Invalid spline numeric field".into()))
                };
                match *code {
                    10 => controls.push(DVec3::new(scalar()?, 0., 0.)),
                    20 | 30 => {
                        let p = controls.last_mut().ok_or_else(|| {
                            DxfError::Parse("Spline coordinate without control".into())
                        })?;
                        if *code == 20 {
                            p.y = scalar()?;
                        } else {
                            p.z = scalar()?;
                        }
                    }
                    11 => fits.push(DVec3::new(scalar()?, 0., 0.)),
                    21 | 31 => {
                        let p = fits.last_mut().ok_or_else(|| {
                            DxfError::Parse("Spline coordinate without fit point".into())
                        })?;
                        if *code == 21 {
                            p.y = scalar()?;
                        } else {
                            p.z = scalar()?;
                        }
                    }
                    40 => knots.push(scalar()?),
                    41 => weights.push(scalar()?),
                    _ => {}
                }
            }
            if spline_transform != glam::DMat4::IDENTITY {
                for p in controls.iter_mut().chain(&mut fits) {
                    *p = spline_transform.transform_point3(*p);
                    if !p.is_finite() {
                        return Err(DxfError::Parse("Block spline coordinate overflow".into()));
                    }
                }
            }
            let mut tangents = if attributes.iter().any(|(c, _)| *c == 12)
                && attributes.iter().any(|(c, _)| *c == 13)
            {
                Some([
                    DVec3::new(number(12, 0.)?, number(22, 0.)?, number(32, 0.)?),
                    DVec3::new(number(13, 0.)?, number(23, 0.)?, number(33, 0.)?),
                ])
            } else {
                None
            };
            if spline_transform != glam::DMat4::IDENTITY {
                if let Some(tangents) = &mut tangents {
                    for tangent in tangents {
                        *tangent = spline_transform.transform_vector3(*tangent);
                        if !tangent.is_finite() {
                            return Err(DxfError::Parse("Block spline tangent overflow".into()));
                        }
                    }
                }
            }
            match spline::flatten(
                controls,
                fits,
                number(71, 3.)? as usize,
                knots,
                weights,
                number(42, 1e-10)?,
                tangents,
                scale,
            ) {
                Ok(p) => points = p,
                // Original _entity_segments catches invalid make_path
                // construction and skips just that entity. Admission limits
                // and nonfinite output still refuse the native transaction.
                Err(DxfError::Parse(message))
                    if message.starts_with("Invalid spline")
                        || message.starts_with("Singular spline")
                        || message.starts_with("Degenerate spline") =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            }
            curved = true;
        }
        _ => {
            return Err(DxfError::Parse(format!(
                "DXF entity {kind} is not yet supported; document unchanged"
            )))
        }
    }
    if points.len() < 2 && !face {
        return Ok(None);
    }
    for point in &mut points {
        if source_points {
            *point = point.as_vec3().as_dvec3();
        }
        if !point.is_finite() {
            return Err(DxfError::Parse("Import coordinate overflow".into()));
        }
    }
    Ok(Some(DxfEntity {
        layer: layer.into(),
        points,
        face,
        curved,
    }))
}

/// Legacy POLYLINE owns a VERTEX sequence, terminated by SEQEND. Parse it once
/// into the existing immutable entity plan; no document geometry is touched.
fn legacy_polyline(
    header: &[(i32, String)],
    pairs: &[(i32, String)],
    cursor: &mut usize,
    scale: f64,
) -> Result<(String, Vec<DVec3>, bool), DxfError> {
    fn number(attrs: &[(i32, String)], code: i32, default: f64) -> Result<f64, DxfError> {
        attrs
            .iter()
            .find(|(c, _)| *c == code)
            .map_or(Ok(default), |(_, v)| {
                v.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| DxfError::Parse("Invalid legacy polyline field".into()))
            })
    }
    let flags = number(header, 70, 0.)? as u32;
    let layer = header
        .iter()
        .find(|(c, _)| *c == 8)
        .map_or("0", |(_, v)| v.as_str())
        .to_owned();
    let mut points = Vec::new();
    let mut first_z = None;
    let mut ended = false;
    while *cursor < pairs.len() {
        if pairs[*cursor].0 != 0 {
            return Err(DxfError::Parse("Malformed POLYLINE sequence".into()));
        }
        let kind = &pairs[*cursor].1;
        if kind == "SEQEND" {
            *cursor += 1;
            while *cursor < pairs.len() && pairs[*cursor].0 != 0 {
                *cursor += 1;
            }
            ended = true;
            break;
        }
        if kind != "VERTEX" {
            break;
        }
        *cursor += 1;
        let begin = *cursor;
        while *cursor < pairs.len() && pairs[*cursor].0 != 0 {
            *cursor += 1;
        }
        let attrs = &pairs[begin..*cursor];
        // Source make_path skips polygon/polyface meshes, not their records.
        if flags & (16 | 64) != 0 {
            continue;
        }
        let bulge = number(attrs, 42, 0.)?;
        let p = DVec3::new(
            number(attrs, 10, 0.)?,
            number(attrs, 20, 0.)?,
            number(attrs, 30, 0.)?,
        );
        first_z.get_or_insert(p.z);
        points.push((p, bulge));
    }
    if !ended {
        return Err(DxfError::Parse("POLYLINE missing SEQEND".into()));
    }
    if flags & (16 | 64) != 0 {
        return Ok((layer, Vec::new(), false));
    }
    if points.len() < 2 {
        return Ok((layer, Vec::new(), false));
    }
    let curved = points.iter().any(|(_, b)| b.abs() > 1e-12);
    let points = if flags & 8 == 0 {
        let elevation = number(header, 30, first_z.unwrap_or(0.))?;
        let normal = DVec3::new(
            number(header, 210, 0.)?,
            number(header, 220, 0.)?,
            number(header, 230, 1.)?,
        );
        curves::polyline(&points, flags & 1 != 0, normal, elevation, scale)?
    } else {
        let mut points = points
            .into_iter()
            .map(|(p, _)| p * scale)
            .collect::<Vec<_>>();
        if points.iter().any(|p| !p.is_finite()) {
            return Err(DxfError::Parse("Import coordinate overflow".into()));
        }
        if flags & 1 != 0 && points.first() != points.last() {
            points.push(points[0]);
        }
        points
    };
    Ok((layer, points, curved))
}

#[cfg(test)]
mod tests {
    #[test]
    fn oversized_spline_degree_preserves_document() {
        let input="0\nSECTION\n2\nENTITIES\n0\nSPLINE\n71\n1e308\n10\n0\n20\n0\n10\n1\n20\n1\n10\n2\n20\n0\n10\n3\n20\n1\n0\nENDSEC\n0\nEOF\n";
        let mut scene = Scene::new();
        scene
            .mesh
            .add_vertex(DVec3::new(1. + 2f64.powi(-40), 2., 3.));
        let before = scene.clone();
        assert!(import_dxf(&mut std::io::Cursor::new(input), &mut scene).is_err());
        assert_eq!(scene, before);
    }
    #[test]
    fn exact_source_spline_paths() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/dxf-splines/source-paths.json"
        ))
        .unwrap();
        for row in source.as_array().unwrap() {
            let mut input = row["dxf"].as_str().unwrap().as_bytes();
            let plan = super::parse_dxf_options(&mut input, 1., true).unwrap();
            let actual=plan.entities.iter().map(|e|serde_json::json!({"points":e.points.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"curved":e.curved})).collect::<Vec<_>>();
            assert_eq!(
                serde_json::json!(actual),
                row["entities"],
                "{}",
                row["case"]
            );
        }
    }
    #[test]
    fn exact_source_curve_paths() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/dxf-curves/source-paths.json"
        ))
        .unwrap();
        for row in source.as_array().unwrap() {
            let mut input = row["dxf"].as_str().unwrap().as_bytes();
            let plan = super::parse_dxf_options(&mut input, 1., true).unwrap();
            assert_eq!(plan.entities.len(), 1, "{}", row["case"]);
            assert_eq!(
                serde_json::json!(plan.entities[0]
                    .points
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>()),
                row["points"],
                "{}",
                row["case"]
            );
        }
    }
    use super::*;
    #[test]
    fn linework_layers_bulges_and_invalid_entities_are_transactional() {
        let source="0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nSITE\n10\n0\n20\n0\n11\n2\n21\n0\n0\nLWPOLYLINE\n8\nSITE\n70\n0\n10\n0\n20\n0\n10\n0\n20\n3\n10\n2\n20\n3\n0\nENDSEC\n0\nEOF\n";
        let mut scene = Scene::new();
        import_dxf(&mut std::io::Cursor::new(source), &mut scene).unwrap();
        assert_eq!(scene.groups[0].name, "SITE");
        assert_eq!(scene.groups[0].mesh.edges.len(), 3);
        assert!(scene.groups[0]
            .mesh
            .edges
            .iter()
            .all(|e| e.layer.as_deref() == Some("SITE")));
        let before = scene.clone();
        // INSERT is supported; malformed coordinates keep this a rejection
        // transaction after the preceding valid LINE has been parsed.
        let invalid = source.replace("20\n3\n", "20\nNaN\n");
        assert!(matches!(
            import_dxf(&mut std::io::Cursor::new(invalid), &mut scene),
            Err(DxfError::Parse(message)) if message == "Invalid polyline field"
        ));
        assert_eq!(scene, before);
        let circle = "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n0\n20\n0\n40\n1\n0\nENDSEC\n0\nEOF\n";
        let mut scene = Scene::new();
        import_dxf(&mut std::io::Cursor::new(circle), &mut scene).unwrap();
        assert!(scene.mesh.edges.len() >= 8);
        assert!(scene.mesh.edges.iter().all(|e| e.curve_id == Some(0)));
        // Source uses cubic-Bezier circle approximation, whose intermediate
        // samples intentionally do not all lie on the analytic circle.
        // Exact source samples are covered by exact_source_curve_paths.
        assert_eq!(scene.mesh.edges.len(), 32);
        assert_eq!(scene.mesh.vertices[0].position, DVec3::X);
    }
}
