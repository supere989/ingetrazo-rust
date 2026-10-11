// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! AutoCAD DXF format export and import.

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
            mesh.faces[id].layer = Some(entity.layer.clone());
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
                    mesh.edges[id].layer = Some(entity.layer.clone());
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
            )?;
        }
        Ok(())
    }
    /// Every source DXF layer is a fresh document-root group, including layer zero.
    pub fn emit_source(self, scene: &mut Scene, stem: &str) -> Result<(), DxfError> {
        let mut layers = std::collections::BTreeMap::<String, Vec<DxfEntity>>::new();
        for entity in self.entities {
            layers.entry(entity.layer.clone()).or_default().push(entity);
        }
        if layers.is_empty() {
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
                Self::emit_entity(entity, &mut group.mesh, &mut next_curve)?;
            }
            scene.groups.push(group);
        }
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
    let mut plan = DxfPlan {
        entities: Vec::new(),
    };
    let mut section = String::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].0 == 0 && pairs[i].1 == "SECTION" {
            i += 1;
            if i >= pairs.len() || pairs[i].0 != 2 {
                return Err(DxfError::Parse("Missing section name".into()));
            }
            section = pairs[i].1.clone();
            i += 1;
            continue;
        }
        if pairs[i].0 == 0 && pairs[i].1 == "ENDSEC" {
            section.clear();
            i += 1;
            continue;
        }
        if section != "ENTITIES" || pairs[i].0 != 0 {
            i += 1;
            continue;
        }
        let kind = pairs[i].1.clone();
        i += 1;
        let start = i;
        while i < pairs.len() && pairs[i].0 != 0 {
            i += 1;
        }
        let attributes = &pairs[start..i];
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
        .contains(&kind.as_str())
        {
            continue;
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
        if number(210, 0.)?.abs() > 1e-9
            || number(220, 0.)?.abs() > 1e-9
            || (number(230, 1.)? - 1.).abs() > 1e-9
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
        match kind.as_str() {
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
                let elevation = number(38, 0.)? * scale;
                let mut vertices = Vec::<(DVec3, f64)>::new();
                for (code, value) in attributes {
                    if *code == 10 {
                        let x = value
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .ok_or_else(|| DxfError::Parse("Invalid polyline X".into()))?;
                        vertices.push((DVec3::new(x * scale, 0., elevation), 0.));
                    } else if *code == 20 || *code == 42 {
                        let v = value
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .ok_or_else(|| DxfError::Parse("Invalid polyline field".into()))?;
                        let last = vertices.last_mut().ok_or_else(|| {
                            DxfError::Parse("Polyline field without vertex".into())
                        })?;
                        if *code == 20 {
                            last.0.y = v * scale;
                        } else {
                            last.1 = v;
                        }
                    }
                }
                if vertices.len() < 2 {
                    return Err(DxfError::Parse("Polyline requires two vertices".into()));
                }
                if vertices.iter().any(|(p, _)| !p.is_finite()) {
                    return Err(DxfError::Parse("Import coordinate overflow".into()));
                }
                let closed = (number(70, 0.)? as u32) & 1 != 0;
                let segments = vertices.len() - usize::from(!closed);
                for j in 0..segments {
                    let (a, bulge) = vertices[j];
                    let b = vertices[(j + 1) % vertices.len()].0;
                    if j == 0 {
                        points.push(a);
                    }
                    if bulge.abs() < 1e-12 {
                        points.push(b);
                    } else {
                        curved = true;
                        let delta = b - a;
                        let chord = delta.length();
                        if chord < 1e-12 {
                            return Err(DxfError::Parse(
                                "Zero-length curved polyline segment".into(),
                            ));
                        }
                        let center = (a + b) / 2.
                            + DVec3::new(-delta.y, delta.x, 0.)
                                * ((1. - bulge * bulge) / (4. * bulge));
                        let radius = a.distance(center);
                        let begin = (a.y - center.y).atan2(a.x - center.x);
                        let angle = 4. * bulge.atan();
                        let step = 2. * (1. - (0.01 / radius).min(1.)).acos();
                        let count = (angle.abs() / step.max(1e-6)).ceil().max(1.) as usize;
                        if count > 1_000_000 {
                            return Err(DxfError::Parse(
                                "Curve tessellation limit exceeded".into(),
                            ));
                        }
                        for k in 1..=count {
                            let angle = begin + angle * k as f64 / count as f64;
                            points.push(if k == count {
                                b
                            } else {
                                center + DVec3::new(radius * angle.cos(), radius * angle.sin(), 0.)
                            });
                        }
                    }
                }
            }
            "CIRCLE" | "ARC" => {
                let center = point(0)?;
                let radius = number(40, 0.)? * scale;
                if !radius.is_finite() || radius <= 0. {
                    return Err(DxfError::Parse("Curve requires positive radius".into()));
                }
                let begin = if kind == "CIRCLE" {
                    0.
                } else {
                    number(50, 0.)?.to_radians()
                };
                let angle = if kind == "CIRCLE" {
                    std::f64::consts::TAU
                } else {
                    (number(51, 0.)?.to_radians() - begin).rem_euclid(std::f64::consts::TAU)
                };
                let step = 2. * (1. - (0.01 / radius).min(1.)).acos();
                let count =
                    (angle / step.max(1e-6))
                        .ceil()
                        .max(if kind == "CIRCLE" { 8. } else { 1. }) as usize;
                if count > 1_000_000 {
                    return Err(DxfError::Parse("Curve tessellation limit exceeded".into()));
                }
                for k in 0..=count {
                    let a = begin + angle * k as f64 / count as f64;
                    points.push(center + DVec3::new(radius * a.cos(), radius * a.sin(), 0.));
                }
                curved = true;
            }
            _ => {
                return Err(DxfError::Parse(format!(
                    "DXF entity {kind} is not yet supported; document unchanged"
                )))
            }
        }
        for point in &mut points {
            if source_points {
                *point = point.as_vec3().as_dvec3();
            }
            if !point.is_finite() {
                return Err(DxfError::Parse("Import coordinate overflow".into()));
            }
        }
        plan.entities.push(DxfEntity {
            layer: layer.into(),
            points,
            face,
            curved,
        });
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
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
        let invalid = source.replace("LWPOLYLINE", "INSERT");
        assert!(import_dxf(&mut std::io::Cursor::new(invalid), &mut scene).is_err());
        assert_eq!(scene, before);
        let circle = "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n0\n20\n0\n40\n1\n0\nENDSEC\n0\nEOF\n";
        let mut scene = Scene::new();
        import_dxf(&mut std::io::Cursor::new(circle), &mut scene).unwrap();
        assert!(scene.mesh.edges.len() >= 8);
        assert!(scene.mesh.edges.iter().all(|e| e.curve_id == Some(0)));
        assert!(scene
            .mesh
            .vertices
            .iter()
            .all(|v| (v.position.length() - 1.).abs() < 1e-10));
    }
}
