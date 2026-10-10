// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! COLLADA 1.4.1 (.dae) Export.
//!
//! Provides open XML interchange format for CAD/3D software (SketchUp, Blender, etc.).
//! Writes Z-up, metric units, faceted geometry, materials, and real-world geolocation.

use std::io::Write;
use glam::DVec3;
use ingetrazo_core::scene::Scene;
use crate::FormatError;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn export_dae(scene: &Scene, writer: &mut impl Write, project_name: &str) -> Result<(), FormatError> {
    let mut positions: Vec<DVec3> = Vec::new();
    let mut normals: Vec<DVec3> = Vec::new();
    let mut triangles: Vec<(usize, usize, usize, usize)> = Vec::new(); // (p0, p1, p2, normal_idx)

    // Helper to get or insert position
    let mut pos_map: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    let mut get_pos_idx = |p: DVec3, pos_list: &mut Vec<DVec3>| -> usize {
        let key = [
            (p.x * 1_000_000.0).round() as i64,
            (p.y * 1_000_000.0).round() as i64,
            (p.z * 1_000_000.0).round() as i64,
        ];
        if let Some(&idx) = pos_map.get(&key) {
            idx
        } else {
            let idx = pos_list.len();
            pos_list.push(p);
            pos_map.insert(key, idx);
            idx
        }
    };

    // Helper to get or insert normal
    let mut norm_map: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    let mut get_norm_idx = |n: DVec3, norm_list: &mut Vec<DVec3>| -> usize {
        let key = [
            (n.x * 100_000.0).round() as i64,
            (n.y * 100_000.0).round() as i64,
            (n.z * 100_000.0).round() as i64,
        ];
        if let Some(&idx) = norm_map.get(&key) {
            idx
        } else {
            let idx = norm_list.len();
            norm_list.push(n);
            norm_map.insert(key, idx);
            idx
        }
    };

    // Share the visible, nested, hole-aware world geometry contract with OBJ/STL.
    for triangle in crate::exchange::triangles(scene) {
        let [p0,p1,p2]=triangle.points;
        let n=(p1-p0).cross(p2-p0).normalize_or_zero();
        let ni=get_norm_idx(n,&mut normals);
        let i0=get_pos_idx(p0,&mut positions);
        let i1=get_pos_idx(p1,&mut positions);
        let i2=get_pos_idx(p2,&mut positions);
        triangles.push((i0,i1,i2,ni));
    }

    // Begin XML generation
    writeln!(writer, "<?xml version=\"1.0\" encoding=\"utf-8\"?>")?;
    writeln!(writer, "<COLLADA xmlns=\"http://www.collada.org/2005/11/COLLADASchema\" version=\"1.4.1\">")?;

    // <asset>
    writeln!(writer, "  <asset>")?;
    writeln!(writer, "    <contributor>")?;
    writeln!(writer, "      <authoring_tool>IngeTrazo 0.3.15 (Rust)</authoring_tool>")?;
    writeln!(writer, "    </contributor>")?;
    writeln!(writer, "    <unit name=\"meter\" meter=\"1.0\"/>")?;
    writeln!(writer, "    <up_axis>Z_UP</up_axis>")?;

    if let Some(ref datum) = scene.geo_datum {
        writeln!(writer, "    <coverage>")?;
        writeln!(writer, "      <geographic_location>")?;
        writeln!(writer, "        <longitude>{:.9}</longitude>", datum.lon)?;
        writeln!(writer, "        <latitude>{:.9}</latitude>", datum.lat)?;
        writeln!(writer, "        <altitude mode=\"absolute\">{:.3}</altitude>", datum.alt)?;
        writeln!(writer, "      </geographic_location>")?;
        writeln!(writer, "    </coverage>")?;
    }
    writeln!(writer, "  </asset>")?;

    // <library_materials> & <library_effects>
    writeln!(writer, "  <library_materials>")?;
    writeln!(writer, "    <material id=\"Material_Default\" name=\"Default_Material\">")?;
    writeln!(writer, "      <instance_effect url=\"#Effect_Default\"/>")?;
    writeln!(writer, "    </material>")?;
    writeln!(writer, "  </library_materials>")?;

    writeln!(writer, "  <library_effects>")?;
    writeln!(writer, "    <effect id=\"Effect_Default\">")?;
    writeln!(writer, "      <profile_COMMON>")?;
    writeln!(writer, "        <technique sid=\"common\">")?;
    writeln!(writer, "          <lambert>")?;
    writeln!(writer, "            <diffuse>")?;
    writeln!(writer, "              <color>0.88 0.86 0.82 1.0</color>")?;
    writeln!(writer, "            </diffuse>")?;
    writeln!(writer, "          </lambert>")?;
    writeln!(writer, "        </technique>")?;
    writeln!(writer, "      </profile_COMMON>")?;
    writeln!(writer, "    </effect>")?;
    writeln!(writer, "  </library_effects>")?;

    // <library_geometries>
    writeln!(writer, "  <library_geometries>")?;
    writeln!(writer, "    <geometry id=\"Geometry_IngeTrazo\" name=\"{}\">", xml_escape(project_name))?;
    writeln!(writer, "      <mesh>")?;

    // Positions array
    let pos_str = positions
        .iter()
        .map(|p| format!("{:.6} {:.6} {:.6}", p.x, p.y, p.z))
        .collect::<Vec<_>>()
        .join(" ");

    writeln!(writer, "        <source id=\"Geometry_IngeTrazo-positions\">")?;
    writeln!(
        writer,
        "          <float_array id=\"Geometry_IngeTrazo-positions-array\" count=\"{}\">{}</float_array>",
        positions.len() * 3,
        pos_str
    )?;
    writeln!(writer, "          <technique_common>")?;
    writeln!(
        writer,
        "            <accessor source=\"#Geometry_IngeTrazo-positions-array\" count=\"{}\" stride=\"3\">",
        positions.len()
    )?;
    writeln!(writer, "              <param name=\"X\" type=\"float\"/>")?;
    writeln!(writer, "              <param name=\"Y\" type=\"float\"/>")?;
    writeln!(writer, "              <param name=\"Z\" type=\"float\"/>")?;
    writeln!(writer, "            </accessor>")?;
    writeln!(writer, "          </technique_common>")?;
    writeln!(writer, "        </source>")?;

    // Normals array
    let norm_str = normals
        .iter()
        .map(|n| format!("{:.5} {:.5} {:.5}", n.x, n.y, n.z))
        .collect::<Vec<_>>()
        .join(" ");

    writeln!(writer, "        <source id=\"Geometry_IngeTrazo-normals\">")?;
    writeln!(
        writer,
        "          <float_array id=\"Geometry_IngeTrazo-normals-array\" count=\"{}\">{}</float_array>",
        normals.len() * 3,
        norm_str
    )?;
    writeln!(writer, "          <technique_common>")?;
    writeln!(
        writer,
        "            <accessor source=\"#Geometry_IngeTrazo-normals-array\" count=\"{}\" stride=\"3\">",
        normals.len()
    )?;
    writeln!(writer, "              <param name=\"X\" type=\"float\"/>")?;
    writeln!(writer, "              <param name=\"Y\" type=\"float\"/>")?;
    writeln!(writer, "              <param name=\"Z\" type=\"float\"/>")?;
    writeln!(writer, "            </accessor>")?;
    writeln!(writer, "          </technique_common>")?;
    writeln!(writer, "        </source>")?;

    writeln!(writer, "        <vertices id=\"Geometry_IngeTrazo-vertices\">")?;
    writeln!(writer, "          <input semantic=\"POSITION\" source=\"#Geometry_IngeTrazo-positions\"/>")?;
    writeln!(writer, "        </vertices>")?;

    // <triangles>
    let mut p_indices: Vec<String> = Vec::with_capacity(triangles.len() * 6);
    for (i0, i1, i2, ni) in &triangles {
        p_indices.push(format!("{i0} {ni} {i1} {ni} {i2} {ni}"));
    }

    writeln!(
        writer,
        "        <triangles count=\"{}\" material=\"Material_Default\">",
        triangles.len()
    )?;
    writeln!(writer, "          <input offset=\"0\" semantic=\"VERTEX\" source=\"#Geometry_IngeTrazo-vertices\"/>")?;
    writeln!(writer, "          <input offset=\"1\" semantic=\"NORMAL\" source=\"#Geometry_IngeTrazo-normals\"/>")?;
    writeln!(writer, "          <p>{}</p>", p_indices.join(" "))?;
    writeln!(writer, "        </triangles>")?;

    writeln!(writer, "      </mesh>")?;
    writeln!(writer, "    </geometry>")?;
    writeln!(writer, "  </library_geometries>")?;

    // <library_visual_scenes>
    writeln!(writer, "  <library_visual_scenes>")?;
    writeln!(writer, "    <visual_scene id=\"VisualScene_Root\" name=\"IngeTrazoScene\">")?;
    writeln!(writer, "      <node id=\"ModelNode\" name=\"Model\">")?;
    writeln!(writer, "        <instance_geometry url=\"#Geometry_IngeTrazo\">")?;
    writeln!(writer, "          <bind_material>")?;
    writeln!(writer, "            <technique_common>")?;
    writeln!(writer, "              <instance_material symbol=\"Material_Default\" target=\"#Material_Default\"/>")?;
    writeln!(writer, "            </technique_common>")?;
    writeln!(writer, "          </bind_material>")?;
    writeln!(writer, "        </instance_geometry>")?;
    writeln!(writer, "      </node>")?;
    writeln!(writer, "    </visual_scene>")?;
    writeln!(writer, "  </library_visual_scenes>")?;

    // <scene>
    writeln!(writer, "  <scene>")?;
    writeln!(writer, "    <instance_visual_scene url=\"#VisualScene_Root\"/>")?;
    writeln!(writer, "  </scene>")?;
    writeln!(writer, "</COLLADA>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dae_export_basic_cube() {
        let mut scene = Scene::new();
        let pts = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];
        let v0 = scene.mesh.add_vertex(pts[0]);
        let v1 = scene.mesh.add_vertex(pts[1]);
        let v2 = scene.mesh.add_vertex(pts[2]);
        let v3 = scene.mesh.add_vertex(pts[3]);
        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);

        let mut output = Vec::new();
        export_dae(&scene, &mut output, "TestCube").unwrap();
        let xml = String::from_utf8(output).unwrap();

        assert!(xml.contains("<COLLADA"));
        assert!(xml.contains("<up_axis>Z_UP</up_axis>"));
        assert!(xml.contains("<unit name=\"meter\" meter=\"1.0\"/>"));
        assert!(xml.contains("Geometry_IngeTrazo-positions"));
        assert!(xml.contains("<triangles count=\"2\""));
        assert!(xml.ends_with("</COLLADA>\n"));
    }
}
