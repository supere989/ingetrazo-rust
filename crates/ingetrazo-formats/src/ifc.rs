// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! IFC4 (Industry Foundation Classes) STEP ISO-10303-21 Export.
//!
//! Provides native open BIM export for structural, architectural, and quantity takeoff tools.
//! Exports spatial hierarchy (Project -> Site -> Building -> Storey), faceted B-Rep representations,
//! IFC element classifications (IfcWall, IfcSlab, IfcColumn...), and BaseQuantities.

use crate::FormatError;
#[cfg(test)]
use glam::DVec3;
use ingetrazo_core::scene::Scene;
use std::collections::HashMap;
use std::io::Write;

const GUID_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// Generates a 22-character IFC compressed GUID from a UUID.
pub fn ifc_guid() -> String {
    let u = uuid::Uuid::new_v4();
    let bytes = u.as_bytes();
    let mut num: u128 = 0;
    for &b in bytes {
        num = (num << 8) | (b as u128);
    }

    let mut out = Vec::with_capacity(22);
    for i in (0..22).rev() {
        let idx = ((num >> (6 * i)) & 0x3F) as usize;
        out.push(GUID_CHARS[idx]);
    }
    String::from_utf8(out).unwrap_or_else(|_| "0000000000000000000000".to_string())
}

fn step_str(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        if c == '\'' {
            out.push_str("''");
        } else if c.is_ascii() && !c.is_control() && c != '\\' {
            out.push(c);
        } else {
            out.push_str("\\X2\\");
            for u in c.encode_utf16(&mut [0; 2]) {
                out.push_str(&format!("{u:04X}"));
            }
            out.push_str("\\X0\\");
        }
    }
    out.push('\'');
    out
}

fn step_float(v: f64) -> String {
    let s = format!("{v:.6}");
    if s == "-0.000000" {
        "0.000000".into()
    } else {
        s
    }
}

pub struct StepWriter {
    pub lines: Vec<String>,
    next_id: usize,
}

impl StepWriter {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            next_id: 1,
        }
    }

    pub fn add(&mut self, entity: &str, attrs: &[&str]) -> usize {
        let eid = self.next_id;
        self.next_id += 1;
        self.lines
            .push(format!("#{eid}={entity}({});", attrs.join(",")));
        eid
    }
}

pub fn export_ifc(
    scene: &Scene,
    writer: &mut impl Write,
    project_name: &str,
) -> Result<usize, FormatError> {
    let mut w = StepWriter::new();

    // 1. Spatial Structure & Context
    let origin = w.add("IFCCARTESIANPOINT", &["(0.,0.,0.)"]);
    let axis = w.add("IFCAXIS2PLACEMENT3D", &[&format!("#{origin}"), "$", "$"]);
    let context = w.add(
        "IFCGEOMETRICREPRESENTATIONCONTEXT",
        &["$", "'Model'", "3", "1.E-5", &format!("#{axis}"), "$"],
    );

    let u_len = w.add("IFCSIUNIT", &["*", ".LENGTHUNIT.", "$", ".METRE."]);
    let u_area = w.add("IFCSIUNIT", &["*", ".AREAUNIT.", "$", ".SQUARE_METRE."]);
    let u_vol = w.add("IFCSIUNIT", &["*", ".VOLUMEUNIT.", "$", ".CUBIC_METRE."]);
    let units_list = format!("(#{u_len},#{u_area},#{u_vol})");
    let unit_assign = w.add("IFCUNITASSIGNMENT", &[&units_list]);

    let proj_guid = ifc_guid();
    let project = w.add(
        "IFCPROJECT",
        &[
            &step_str(&proj_guid),
            "$",
            &step_str(project_name),
            "$",
            "$",
            "$",
            "$",
            &format!("(#{context})"),
            &format!("#{unit_assign}"),
        ],
    );

    let placement = w.add("IFCLOCALPLACEMENT", &["$", &format!("#{axis}")]);

    let site_guid = ifc_guid();
    let site = w.add(
        "IFCSITE",
        &[
            &step_str(&site_guid),
            "$",
            "'Default Site'",
            "$",
            "$",
            &format!("#{placement}"),
            "$",
            "$",
            ".ELEMENT.",
            "$",
            "$",
            "$",
            "$",
            "$",
        ],
    );

    let bld_guid = ifc_guid();
    let building = w.add(
        "IFCBUILDING",
        &[
            &step_str(&bld_guid),
            "$",
            "'Building 1'",
            "$",
            "$",
            &format!("#{placement}"),
            "$",
            "$",
            ".ELEMENT.",
            "$",
            "$",
            "$",
        ],
    );

    let storey_guid = ifc_guid();
    let storey = w.add(
        "IFCBUILDINGSTOREY",
        &[
            &step_str(&storey_guid),
            "$",
            "'Storey 1'",
            "$",
            "$",
            &format!("#{placement}"),
            "$",
            "$",
            ".ELEMENT.",
            "0.",
        ],
    );

    // RelAggregates
    let rel_proj_site = ifc_guid();
    w.add(
        "IFCRELAGGREGATES",
        &[
            &step_str(&rel_proj_site),
            "$",
            "$",
            "$",
            &format!("#{project}"),
            &format!("(#{site})"),
        ],
    );
    let rel_site_bld = ifc_guid();
    w.add(
        "IFCRELAGGREGATES",
        &[
            &step_str(&rel_site_bld),
            "$",
            "$",
            "$",
            &format!("#{site}"),
            &format!("(#{building})"),
        ],
    );
    let rel_bld_str = ifc_guid();
    w.add(
        "IFCRELAGGREGATES",
        &[
            &step_str(&rel_bld_str),
            "$",
            "$",
            "$",
            &format!("#{building}"),
            &format!("(#{storey})"),
        ],
    );

    // Tagged objects retain their polygon and hole rings. Compute world geometry
    // once per object; the same geometry drives its shape and quantities.
    let objects = ingetrazo_core::bim::collect_objects(scene).map_err(invalid)?;
    let mut element_ids = Vec::new();
    let mut space_ids = Vec::new();
    for obj in objects {
        if obj.faces.is_empty() {
            continue;
        }
        let measured = ingetrazo_core::bim::object_takeoff(&obj).map_err(invalid)?;
        let q = measured.quantities;
        let mut points = HashMap::<String, usize>::new();
        let mut faces = Vec::new();
        for &id in &obj.faces {
            let f = &obj.mesh.faces[id];
            let mut bounds = Vec::new();
            for (n, ring) in std::iter::once(&f.vertices)
                .chain(f.holes.iter())
                .enumerate()
            {
                let mut ids = Vec::new();
                for &v in ring {
                    let p = obj.mesh.vertices[v].position;
                    let coords = format!(
                        "({},{},{})",
                        step_float(p.x),
                        step_float(p.y),
                        step_float(p.z)
                    );
                    let id = *points
                        .entry(coords.clone())
                        .or_insert_with(|| w.add("IFCCARTESIANPOINT", &[&coords]));
                    ids.push(id);
                }
                if ids
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != ids.len()
                {
                    return Err(invalid(
                        "IFC six-decimal precision collapses distinct ring corners",
                    ));
                }
                let poly = w.add("IFCPOLYLOOP", &[&refs(&ids)]);
                let kind = if n == 0 {
                    "IFCFACEOUTERBOUND"
                } else {
                    "IFCFACEBOUND"
                };
                bounds.push(w.add(kind, &[&format!("#{poly}"), ".T."]));
            }
            faces.push(w.add("IFCFACE", &[&refs(&bounds)]));
        }
        let closed = measured.volume.is_some();
        let shell = w.add(
            if closed {
                "IFCCLOSEDSHELL"
            } else {
                "IFCOPENSHELL"
            },
            &[&refs(&faces)],
        );
        let item = if closed {
            w.add("IFCFACETEDBREP", &[&format!("#{shell}")])
        } else {
            w.add("IFCSHELLBASEDSURFACEMODEL", &[&refs(&[shell])])
        };
        let rep = w.add(
            "IFCSHAPEREPRESENTATION",
            &[
                &format!("#{context}"),
                "'Body'",
                if closed { "'Brep'" } else { "'SurfaceModel'" },
                &refs(&[item]),
            ],
        );
        let pds = w.add("IFCPRODUCTDEFINITIONSHAPE", &["$", "$", &refs(&[rep])]);
        let class = if ingetrazo_core::bim::IFC_CLASSES.contains(&obj.class.as_str()) {
            obj.class.as_str()
        } else {
            "IfcBuildingElementProxy"
        };
        let count = match class {
            "IfcPile" => 10,
            "IfcDoor" | "IfcWindow" => 13,
            "IfcSpace" => 11,
            _ => 9,
        };
        let name = if obj.name.is_empty() {
            class
        } else {
            &obj.name
        };
        let mut attrs = vec![
            step_str(&ifc_guid()),
            "$".into(),
            step_str(name),
            "$".into(),
            "$".into(),
            format!("#{placement}"),
            format!("#{pds}"),
            "'IngeTrazo'".into(),
        ];
        if matches!(class, "IfcDoor" | "IfcWindow") {
            for name in ["Height", "Width"] {
                attrs.push(
                    q.entries
                        .iter()
                        .find(|e| e.kind == "length" && e.name == name)
                        .map(|e| step_float(e.value))
                        .unwrap_or_else(|| "$".into()),
                );
            }
        }
        attrs.resize(count, "$".into());
        let elem = w.add(
            &class.to_ascii_uppercase(),
            &attrs.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        if class == "IfcSpace" {
            space_ids.push(elem);
        } else {
            element_ids.push(elem);
        }
        let mut quantities = Vec::new();
        for e in &q.entries {
            if !e.value.is_finite() {
                return Err(invalid("Nonfinite IFC quantity"));
            }
            let kind = match e.kind {
                "length" => "IFCQUANTITYLENGTH",
                "area" => "IFCQUANTITYAREA",
                _ => "IFCQUANTITYVOLUME",
            };
            quantities.push(w.add(
                kind,
                &[&step_str(e.name), "$", "$", &step_float(e.value), "$"],
            ));
        }
        if !quantities.is_empty() {
            let eq = w.add(
                "IFCELEMENTQUANTITY",
                &[
                    &step_str(&ifc_guid()),
                    "$",
                    &step_str(&q.set),
                    "$",
                    "$",
                    &refs(&quantities),
                ],
            );
            w.add(
                "IFCRELDEFINESBYPROPERTIES",
                &[
                    &step_str(&ifc_guid()),
                    "$",
                    "$",
                    "$",
                    &refs(&[elem]),
                    &format!("#{eq}"),
                ],
            );
        }
    }
    if element_ids.is_empty() && space_ids.is_empty() {
        return Err(invalid("No tagged BIM objects to export"));
    }
    if !element_ids.is_empty() {
        w.add(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE",
            &[
                &step_str(&ifc_guid()),
                "$",
                "$",
                "$",
                &refs(&element_ids),
                &format!("#{storey}"),
            ],
        );
    }
    if !space_ids.is_empty() {
        w.add(
            "IFCRELAGGREGATES",
            &[
                &step_str(&ifc_guid()),
                "$",
                "$",
                "$",
                &format!("#{storey}"),
                &refs(&space_ids),
            ],
        );
    }
    // Header timestamp is optional in IFC4; no fabricated export date.
    writeln!(
        writer,
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');"
    )?;
    writeln!(
        writer,
        "FILE_NAME({},'',('IngeTrazo'),('IngeTrazo'),'IngeTrazo Rust','IngeTrazo','');",
        step_str(project_name)
    )?;
    writeln!(writer, "FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;")?;
    for line in w.lines {
        writeln!(writer, "{line}")?;
    }
    writeln!(writer, "ENDSEC;\nEND-ISO-10303-21;")?;
    Ok(element_ids.len() + space_ids.len())
}

fn invalid(message: impl Into<String>) -> FormatError {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into()
}
fn refs(ids: &[usize]) -> String {
    format!(
        "({})",
        ids.iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Stage beside the destination; failures before rename preserve its bytes.
/// A directory sync error after rename reports that publication has occurred.
pub fn save_ifc(
    scene: &Scene,
    path: &std::path::Path,
    project: &str,
) -> Result<usize, FormatError> {
    let mut count = 0;
    write_atomic(path, |f| {
        count = export_ifc(scene, f, project)?;
        Ok(())
    })?;
    Ok(count)
}
pub fn save_quantities_csv(scene: &Scene, path: &std::path::Path) -> Result<usize, FormatError> {
    let rows = ingetrazo_core::bim::takeoff(scene).map_err(invalid)?;
    let csv = ingetrazo_core::bim::quantities_csv(&rows);
    write_atomic(path, |f| {
        f.write_all(csv.as_bytes())?;
        Ok(())
    })?;
    Ok(rows.len())
}
fn write_atomic(
    path: &std::path::Path,
    write: impl FnOnce(&mut std::fs::File) -> Result<(), FormatError>,
) -> Result<(), FormatError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let temp = parent.join(format!(".ingetrazo-bim-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        write(&mut file)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("BIM file replaced; parent directory durability sync failed: {e}"),
                )
            })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ifc_guid_length_and_validity() {
        let guid = ifc_guid();
        assert_eq!(guid.len(), 22);
        for b in guid.bytes() {
            assert!(GUID_CHARS.contains(&b));
        }
    }

    #[test]
    fn test_ifc_export_basic_structure() {
        let mut scene = Scene::new();
        let pts = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(3.0, 0.2, 0.0),
            DVec3::new(0.0, 0.2, 0.0),
        ];
        let v0 = scene.mesh.add_vertex(pts[0]);
        let v1 = scene.mesh.add_vertex(pts[1]);
        let v2 = scene.mesh.add_vertex(pts[2]);
        let v3 = scene.mesh.add_vertex(pts[3]);
        let id = scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]).unwrap();
        scene.mesh.faces[id].attrs.insert(
            "ifc".into(),
            serde_json::json!({"id":1,"class":"IfcSlab","name":"Slab"}),
        );

        let mut output = Vec::new();
        let count = export_ifc(&scene, &mut output, "TestHouse").unwrap();
        assert!(count >= 1);

        let ifc_str = String::from_utf8(output).unwrap();
        assert!(ifc_str.starts_with("ISO-10303-21;"));
        assert!(ifc_str.contains("FILE_SCHEMA(('IFC4'));"));
        assert!(ifc_str.contains("IFCPROJECT"));
        assert!(ifc_str.contains("IFCSITE"));
        assert!(ifc_str.contains("IFCBUILDING"));
        assert!(ifc_str.contains("IFCBUILDINGSTOREY"));
        assert!(ifc_str.contains("IFCSHELLBASEDSURFACEMODEL"));
        assert!(!ifc_str.contains("IFCFACETEDBREP"));
        assert!(ifc_str.contains("Qto_SlabBaseQuantities"));
        assert!(ifc_str.ends_with("END-ISO-10303-21;\n"));
    }
}
#[cfg(test)]
mod refusal_tests {
    use super::*;
    #[test]
    fn bim_export_refusal_preserves_existing_file_and_removes_stage() {
        let dir = std::env::temp_dir().join(format!("ifc-refusal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("existing.ifc");
        std::fs::write(&path, b"original").unwrap();
        assert!(crate::save_file(&Scene::new(), &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let mut s = Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| s.mesh.add_vertex(p));
        let f = s.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        s.mesh.faces[f]
            .attrs
            .insert("ifc".into(), serde_json::json!({"id":1,"class":"IfcSlab"}));
        s.mesh.vertices[0].position.x = f64::NAN;
        assert!(save_ifc(&s, &path, "invalid").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bim_overflow_and_quantized_corner_refusals_preserve_destination() {
        let dir = std::env::temp_dir().join(format!("ifc-finite-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("kept.ifc");
        std::fs::write(&path, b"kept").unwrap();
        for points in [
            [
                DVec3::ZERO,
                DVec3::new(1e20, 0., 0.),
                DVec3::new(0., 1e20, 0.),
            ],
            [DVec3::ZERO, DVec3::new(0.0000004, 0., 0.), DVec3::Y],
        ] {
            let mut s = Scene::new();
            let ids = points.map(|p| s.mesh.add_vertex(p));
            // Explicit vertex identities bypass modelling weld for the precision refusal control.
            let ids = if ids[0] == ids[1] {
                s.mesh
                    .vertices
                    .push(ingetrazo_core::mesh::MeshVertex::new(points[1]));
                [ids[0], s.mesh.vertices.len() - 1, ids[2]]
            } else {
                ids
            };
            let f = s.mesh.add_face(ids.to_vec(), vec![]).unwrap();
            s.mesh.faces[f]
                .attrs
                .insert("ifc".into(), serde_json::json!({"id":1,"class":"IfcWall"}));
            assert!(save_ifc(&s, &path, "invalid").is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"kept");
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
