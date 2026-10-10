// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! AutoCAD DXF format export and import.

use std::io::{BufRead, BufReader, Read, Write};
use glam::DVec3;
use ingetrazo_core::scene::Scene;

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
        let [p0,p1,p2]=triangle.points;
        let layer=triangle.layer.replace(['\n','\r'],"_");
        writeln!(writer,"0\n3DFACE\n8\n{layer}")?;
        for (i,p) in [p0,p1,p2,p2].into_iter().enumerate() {
            writeln!(writer,"{}\n{:.9}\n{}\n{:.9}\n{}\n{:.9}",10+i,p.x,20+i,p.y,30+i,p.z)?;
        }
    }

    writeln!(writer, "0\nENDSEC")?;
    writeln!(writer, "0\nEOF")?;
    Ok(())
}

pub fn import_dxf(reader: &mut impl Read, scene: &mut Scene) -> Result<(), DxfError> {
    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    let mut in_entities = false;

    while let Some(code_line_res) = lines.next() {
        let code_line = code_line_res?;
        let code = code_line.trim();
        let value_line = match lines.next() {
            Some(v) => v?,
            None => break,
        };
        let val = value_line.trim();

        if code == "2" && val == "ENTITIES" {
            in_entities = true;
            continue;
        }
        if code == "0" && val == "ENDSEC" {
            if in_entities {
                break;
            }
        }

        if in_entities && code == "0" && val == "3DFACE" {
            // Read 3DFACE coordinates
            let mut p0 = DVec3::ZERO;
            let mut p1 = DVec3::ZERO;
            let mut p2 = DVec3::ZERO;
            let mut p3 = DVec3::ZERO;
            let mut has_p3 = false;

            while let Some(attr_code_res) = lines.next() {
                let attr_code = attr_code_res?;
                let c = attr_code.trim();
                let attr_val = match lines.next() {
                    Some(v) => v?,
                    None => break,
                };
                let v = attr_val.trim();

                if c == "0" {
                    // Next entity starts
                    let i0 = scene.mesh.add_vertex(p0);
                    let i1 = scene.mesh.add_vertex(p1);
                    let i2 = scene.mesh.add_vertex(p2);
                    if has_p3 && p3 != p2 && p3 != p0 {
                        let i3 = scene.mesh.add_vertex(p3);
                        scene.mesh.add_face(vec![i0, i1, i2, i3], vec![]);
                    } else {
                        scene.mesh.add_face(vec![i0, i1, i2], vec![]);
                    }

                    if v == "3DFACE" {
                        // Loop to continue next 3DFACE
                        p0 = DVec3::ZERO;
                        p1 = DVec3::ZERO;
                        p2 = DVec3::ZERO;
                        p3 = DVec3::ZERO;
                        has_p3 = false;
                        continue;
                    } else {
                        break;
                    }
                }

                if let Ok(num) = v.parse::<f64>() {
                    match c {
                        "10" => p0.x = num,
                        "20" => p0.y = num,
                        "30" => p0.z = num,
                        "11" => p1.x = num,
                        "21" => p1.y = num,
                        "31" => p1.z = num,
                        "12" => p2.x = num,
                        "22" => p2.y = num,
                        "32" => p2.z = num,
                        "13" => { p3.x = num; has_p3 = true; }
                        "23" => { p3.y = num; has_p3 = true; }
                        "33" => { p3.z = num; has_p3 = true; }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}
