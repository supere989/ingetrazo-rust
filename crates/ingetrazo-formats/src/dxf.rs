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
    let mut candidate = scene.clone();
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
            Ok(DVec3::new(
                number(10 + n, 0.)?,
                number(20 + n, 0.)?,
                number(30 + n, 0.)?,
            ))
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
                let elevation = number(38, 0.)?;
                let mut vertices = Vec::<(DVec3, f64)>::new();
                for (code, value) in attributes {
                    if *code == 10 {
                        let x = value
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .ok_or_else(|| DxfError::Parse("Invalid polyline X".into()))?;
                        vertices.push((DVec3::new(x, 0., elevation), 0.));
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
                            last.0.y = v;
                        } else {
                            last.1 = v;
                        }
                    }
                }
                if vertices.len() < 2 {
                    return Err(DxfError::Parse("Polyline requires two vertices".into()));
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
                let radius = number(40, 0.)?;
                if radius <= 0. {
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
        let mesh = if layer == "0" {
            &mut candidate.mesh
        } else {
            if !candidate.layers.iter().any(|l| l.name == layer) {
                candidate
                    .layers
                    .push(ingetrazo_core::layers::Layer::new(layer));
            }
            let index = if let Some(i) = candidate
                .groups
                .iter()
                .position(|g| g.name == layer && g.layer.as_deref() == Some(layer))
            {
                i
            } else {
                let mut group = ingetrazo_core::groups::Group::new(layer);
                group.layer = Some(layer.into());
                candidate.groups.push(group);
                candidate.groups.len() - 1
            };
            &mut candidate.groups[index].mesh
        };
        let ids = points
            .into_iter()
            .map(|p| mesh.add_vertex(p))
            .collect::<Vec<_>>();
        if face {
            let id = mesh
                .add_face(ids, vec![])
                .ok_or_else(|| DxfError::Parse("Invalid DXF face".into()))?;
            mesh.faces[id].layer = Some(layer.into());
        } else {
            let curve = mesh
                .edges
                .iter()
                .filter_map(|e| e.curve_id)
                .max()
                .map_or(0, |i| i + 1);
            for pair in ids.windows(2) {
                if let Some(id) = mesh.add_edge(pair[0], pair[1]) {
                    mesh.edges[id].layer = Some(layer.into());
                    if curved {
                        mesh.edges[id].curve_id = Some(curve);
                    }
                }
            }
        }
    }
    candidate.bump_version();
    *scene = candidate;
    Ok(())
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
