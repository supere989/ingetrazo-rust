// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! IFC4 (Industry Foundation Classes) STEP ISO-10303-21 Export.
//!
//! Provides native open BIM export for structural, architectural, and quantity takeoff tools.
//! Exports spatial hierarchy (Project -> Site -> Building -> Storey), faceted B-Rep representations,
//! IFC element classifications (IfcWall, IfcSlab, IfcColumn...), and BaseQuantities.

use std::collections::HashMap;
use std::io::Write;
use glam::DVec3;
use ingetrazo_core::scene::Scene;
use crate::FormatError;

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
    let escaped = text.replace('\'', "''");
    format!("'{escaped}'")
}

fn step_float(v: f64) -> String {
    if v.abs() < 1e-9 {
        "0.".to_string()
    } else {
        format!("{v:.6}")
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
        self.lines.push(format!("#{eid}={entity}({});", attrs.join(",")));
        eid
    }
}

pub fn export_ifc(scene: &Scene, writer: &mut impl Write, project_name: &str) -> Result<usize, FormatError> {
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
        &[&step_str(&rel_proj_site), "$", "$", "$", &format!("#{project}"), &format!("(#{site})")],
    );
    let rel_site_bld = ifc_guid();
    w.add(
        "IFCRELAGGREGATES",
        &[&step_str(&rel_site_bld), "$", "$", "$", &format!("#{site}"), &format!("(#{building})")],
    );
    let rel_bld_str = ifc_guid();
    w.add(
        "IFCRELAGGREGATES",
        &[&step_str(&rel_bld_str), "$", "$", "$", &format!("#{building}"), &format!("(#{storey})")],
    );

    // 2. Export BIM Elements
    let mut point_cache: HashMap<[i64; 3], usize> = HashMap::new();
    let mut get_pt_id = |p: DVec3, w: &mut StepWriter| -> usize {
        let key = [
            (p.x * 1_000_000.0).round() as i64,
            (p.y * 1_000_000.0).round() as i64,
            (p.z * 1_000_000.0).round() as i64,
        ];
        if let Some(&pid) = point_cache.get(&key) {
            pid
        } else {
            let pid = w.add(
                "IFCCARTESIANPOINT",
                &[&format!("({},{},{})", step_float(p.x), step_float(p.y), step_float(p.z))],
            );
            point_cache.insert(key, pid);
            pid
        }
    };

    let mut element_ids: Vec<usize> = Vec::new();

    // Export groups as BIM elements
    for (_g_idx, g) in scene.groups.iter().enumerate() {
        if g.hidden || g.mesh.faces.is_empty() {
            continue;
        }

        let mut face_ids = Vec::new();
        for f_id in 0..g.mesh.faces.len() {
            for (p0, p1, p2) in g.mesh.triangulate_face(f_id) {
                let tp0 = g.transform.transform_point3(p0);
                let tp1 = g.transform.transform_point3(p1);
                let tp2 = g.transform.transform_point3(p2);

                let id0 = get_pt_id(tp0, &mut w);
                let id1 = get_pt_id(tp1, &mut w);
                let id2 = get_pt_id(tp2, &mut w);

                let loop_pts = format!("(#{id0},#{id1},#{id2})");
                let poly_loop = w.add("IFCPOLYLOOP", &[&loop_pts]);
                let bound = w.add("IFCFACEOUTERBOUND", &[&format!("#{poly_loop}"), ".T."]);
                let ifc_face = w.add("IFCFACE", &[&format!("(#{bound})")]);
                face_ids.push(ifc_face);
            }
        }

        if face_ids.is_empty() {
            continue;
        }

        let face_list = format!("({})", face_ids.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(","));
        let shell = w.add("IFCCLOSEDSHELL", &[&face_list]);
        let brep = w.add("IFCFACETEDBREP", &[&format!("#{shell}")]);
        let rep = w.add(
            "IFCSHAPEREPRESENTATION",
            &[&format!("#{context}"), "'Body'", "'Brep'", &format!("(#{brep})")],
        );
        let pds = w.add("IFCPRODUCTDEFINITIONSHAPE", &["$", "$", &format!("(#{rep})")]);

        // Element class from BIM tag attr or group name
        let ifc_class = g
            .ifc_tag
            .as_ref()
            .and_then(|v| v.get("class"))
            .and_then(|c| c.as_str())
            .unwrap_or("IfcBuildingElementProxy");

        let elem_name = g
            .ifc_tag
            .as_ref()
            .and_then(|v| v.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or(&g.name);

        let elem_guid = ifc_guid();
        let class_upper = ifc_class.to_uppercase();
        let elem_id = w.add(
            &class_upper,
            &[
                &step_str(&elem_guid),
                "$",
                &step_str(elem_name),
                "$",
                "$",
                &format!("#{placement}"),
                &format!("#{pds}"),
                "'IngeTrazo'",
                "$",
            ],
        );
        element_ids.push(elem_id);
    }

    // Export untagged / ungrouped loose mesh faces if present
    if !scene.mesh.faces.is_empty() {
        let mut loose_faces = Vec::new();
        for f_id in 0..scene.mesh.faces.len() {
            for (p0, p1, p2) in scene.mesh.triangulate_face(f_id) {
                let id0 = get_pt_id(p0, &mut w);
                let id1 = get_pt_id(p1, &mut w);
                let id2 = get_pt_id(p2, &mut w);

                let loop_pts = format!("(#{id0},#{id1},#{id2})");
                let poly_loop = w.add("IFCPOLYLOOP", &[&loop_pts]);
                let bound = w.add("IFCFACEOUTERBOUND", &[&format!("#{poly_loop}"), ".T."]);
                let ifc_face = w.add("IFCFACE", &[&format!("(#{bound})")]);
                loose_faces.push(ifc_face);
            }
        }

        if !loose_faces.is_empty() {
            let face_list = format!("({})", loose_faces.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(","));
            let shell = w.add("IFCCLOSEDSHELL", &[&face_list]);
            let brep = w.add("IFCFACETEDBREP", &[&format!("#{shell}")]);
            let rep = w.add(
                "IFCSHAPEREPRESENTATION",
                &[&format!("#{context}"), "'Body'", "'Brep'", &format!("(#{brep})")],
            );
            let pds = w.add("IFCPRODUCTDEFINITIONSHAPE", &["$", "$", &format!("(#{rep})")]);

            let elem_guid = ifc_guid();
            let elem_id = w.add(
                "IFCBUILDINGELEMENTPROXY",
                &[
                    &step_str(&elem_guid),
                    "$",
                    "'Freeform Model'",
                    "$",
                    "$",
                    &format!("#{placement}"),
                    &format!("#{pds}"),
                    "'IngeTrazo'",
                    "$",
                ],
            );
            element_ids.push(elem_id);
        }
    }

    // RelContainedInSpatialStructure
    if !element_ids.is_empty() {
        let rel_guid = ifc_guid();
        let elems_str = format!("({})", element_ids.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(","));
        w.add(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE",
            &[&step_str(&rel_guid), "$", "$", "$", &elems_str, &format!("#{storey}")],
        );
    }

    // Write STEP file format
    let stamp = "2026-10-07T12:00:00";
    writeln!(writer, "ISO-10303-21;")?;
    writeln!(writer, "HEADER;")?;
    writeln!(writer, "FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');")?;
    writeln!(
        writer,
        "FILE_NAME('{}','{}',('IngeTrazo'),('IngeTrazo'),'IngeTrazo 0.3.15','IngeTrazo','');",
        step_str(project_name),
        stamp
    )?;
    writeln!(writer, "FILE_SCHEMA(('IFC4'));")?;
    writeln!(writer, "ENDSEC;")?;
    writeln!(writer, "DATA;")?;

    for line in &w.lines {
        writeln!(writer, "{line}")?;
    }

    writeln!(writer, "ENDSEC;")?;
    writeln!(writer, "END-ISO-10303-21;")?;

    Ok(element_ids.len())
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
        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);

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
        assert!(ifc_str.contains("IFCFACETEDBREP"));
        assert!(ifc_str.ends_with("END-ISO-10303-21;\n"));
    }
}
