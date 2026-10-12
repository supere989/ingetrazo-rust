// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-compatible COLLADA front-paint export. Placements are baked into one
//! world geometry; source DAE deliberately omits backside/opacity/finish.
use crate::FormatError;
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
/// Borrowed active edit publication, identified by actual stable container UUID.
pub struct PlacementOverride<'a> {
    pub id: uuid::Uuid,
    pub mesh: &'a Mesh,
    pub children: &'a [Group],
}
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::Path,
};
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}
fn key<const N: usize>(v: [f64; N], digits: usize) -> [String; N] {
    v.map(|x| {
        if x == 0. {
            "0".into()
        } else {
            {
                let decimal = format!("{x:.digits$}");
                if decimal.parse::<f64>().ok() == Some(0.) {
                    "0".into()
                } else {
                    decimal
                }
            }
        }
    })
}
fn index<const N: usize>(
    v: [f64; N],
    digits: usize,
    rows: &mut Vec<[f64; N]>,
    map: &mut HashMap<[String; N], usize>,
) -> usize {
    let k = key(v, digits);
    if let Some(&i) = map.get(&k) {
        i
    } else {
        let i = rows.len();
        rows.push(v);
        map.insert(k, i);
        i
    }
}
fn source_z(v: [f64; 3]) -> [f64; 3] {
    [v[0], -v[2], v[1]]
}
fn name(raw: &str, i: usize, used: &mut HashSet<String>) -> String {
    let mut base = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches(['.', '_', '-'])
        .to_owned();
    if base.is_empty() {
        base = format!("mat{i}");
    }
    if base.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        base = format!("m_{base}");
    }
    let mut result = base.clone();
    let mut n = 2;
    while used.contains(&result) {
        result = format!("{base}_{n}");
        n += 1;
    }
    used.insert(result.clone());
    result
}
/// Serialize original-supported paint, world placements, normals and texture UV.
/// File sidecars are handled by save_dae; this writer never mutates the scene.
pub fn export_dae(
    scene: &Scene,
    writer: &mut impl Write,
    project_name: &str,
) -> Result<(), FormatError> {
    let prims = crate::gltf::rich::collect_for_dae(scene)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    write_collected(scene, writer, project_name, &prims)
}
fn write_collected(
    scene: &Scene,
    writer: &mut impl Write,
    project_name: &str,
    prims: &[crate::gltf::rich::Primitive],
) -> Result<(), FormatError> {
    let (mut positions, mut normals, mut uvs) = (Vec::new(), Vec::new(), Vec::new());
    let (mut pm, mut nm, mut um) = (HashMap::new(), HashMap::new(), HashMap::new());
    let mut groups = Vec::new();
    for p in prims {
        let textured = p.info["map"].is_string();
        let mut tokens = Vec::new();
        for &v in &p.indices {
            let j = v as usize;
            let ni = index(source_z(p.normals[j]), 5, &mut normals, &mut nm);
            tokens.push(index(source_z(p.positions[j]), 6, &mut positions, &mut pm));
            tokens.push(ni);
            if textured {
                tokens.push(index([p.uvs[j][0], 1. - p.uvs[j][1]], 6, &mut uvs, &mut um));
            }
        }
        groups.push((textured, tokens));
    }
    writeln!(writer,"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<COLLADA xmlns=\"http://www.collada.org/2005/11/COLLADASchema\" version=\"1.4.1\">\n<asset><contributor><authoring_tool>IngeTrazo</authoring_tool></contributor>")?;
    let now = utc_now();
    writeln!(writer, "<created>{now}</created><modified>{now}</modified>")?;
    if let Some(g) = &scene.geo_datum {
        writeln!(writer,"<coverage><geographic_location><longitude>{:.9}</longitude><latitude>{:.9}</latitude><altitude mode=\"absolute\">{:.3}</altitude></geographic_location></coverage>",g.lon,g.lat,g.alt)?;
    }
    writeln!(
        writer,
        "<unit name=\"meter\" meter=\"1\"/><up_axis>Z_UP</up_axis></asset>\n<library_images>"
    )?;
    for (i, p) in prims.iter().enumerate() {
        if let Some(map) = p.info["map"].as_str() {
            writeln!(
                writer,
                "<image id=\"img{i}\" name=\"img{i}\"><init_from>{}</init_from></image>",
                escape(map)
            )?;
        }
    }
    writeln!(writer, "</library_images><library_effects>")?;
    for (i, p) in prims.iter().enumerate() {
        write!(writer, "<effect id=\"eff{i}\"><profile_COMMON>")?;
        if p.info["map"].is_string() {
            write!(writer,"<newparam sid=\"surf{i}\"><surface type=\"2D\"><init_from>img{i}</init_from></surface></newparam><newparam sid=\"samp{i}\"><sampler2D><source>surf{i}</source></sampler2D></newparam><technique sid=\"common\"><lambert><diffuse><texture texture=\"samp{i}\" texcoord=\"UVSET0\"/></diffuse></lambert></technique>")?;
        } else {
            let c = &p.info["color"];
            write!(writer,"<technique sid=\"common\"><lambert><diffuse><color>{:.4} {:.4} {:.4} 1</color></diffuse></lambert></technique>",c[0].as_f64().unwrap_or(0.96),c[1].as_f64().unwrap_or(0.95),c[2].as_f64().unwrap_or(0.925))?;
        }
        writeln!(writer, "</profile_COMMON></effect>")?;
    }
    writeln!(writer, "</library_effects><library_materials>")?;
    let mut used = HashSet::new();
    for (i, p) in prims.iter().enumerate() {
        let n = name(p.info["mat"].as_str().unwrap_or(""), i, &mut used);
        writeln!(
            writer,
            "<material id=\"mat{i}\" name=\"{}\"><instance_effect url=\"#eff{i}\"/></material>",
            escape(&n)
        )?;
    }
    writeln!(
        writer,
        "</library_materials><library_geometries><geometry id=\"geom0\" name=\"IngeTrazo\"><mesh>"
    )?;
    fn source<const N: usize>(
        w: &mut impl Write,
        id: &str,
        rows: &[[f64; N]],
        digits: usize,
        labels: &[&str],
    ) -> Result<(), std::io::Error> {
        write!(
            w,
            "<source id=\"geom0-{id}\"><float_array id=\"geom0-{id}-array\" count=\"{}\">",
            rows.len() * N
        )?;
        for row in rows {
            for v in row {
                write!(w, "{v:.digits$} ")?;
            }
        }
        write!(w,"</float_array><technique_common><accessor source=\"#geom0-{id}-array\" count=\"{}\" stride=\"{N}\">",rows.len())?;
        for label in labels {
            write!(w, "<param name=\"{label}\" type=\"float\"/>")?;
        }
        writeln!(w, "</accessor></technique_common></source>")
    }
    source(writer, "pos", &positions, 6, &["X", "Y", "Z"])?;
    source(writer, "nrm", &normals, 5, &["X", "Y", "Z"])?;
    if !uvs.is_empty() {
        source(writer, "uv", &uvs, 6, &["S", "T"])?;
    }
    writeln!(writer,"<vertices id=\"geom0-vertices\"><input semantic=\"POSITION\" source=\"#geom0-pos\"/></vertices>")?;
    for (i, (tex, tokens)) in groups.iter().enumerate() {
        let stride = if *tex { 3 } else { 2 };
        write!(writer,"<triangles material=\"sym{i}\" count=\"{}\"><input semantic=\"VERTEX\" source=\"#geom0-vertices\" offset=\"0\"/><input semantic=\"NORMAL\" source=\"#geom0-nrm\" offset=\"1\"/>",tokens.len()/(3*stride))?;
        if *tex {
            write!(
                writer,
                "<input semantic=\"TEXCOORD\" source=\"#geom0-uv\" offset=\"2\" set=\"0\"/>"
            )?;
        }
        write!(writer, "<p>")?;
        for t in tokens {
            write!(writer, "{t} ")?;
        }
        writeln!(writer, "</p></triangles>")?;
    }
    writeln!(writer,"</mesh></geometry></library_geometries><library_visual_scenes><visual_scene id=\"scene\"><node id=\"model\" name=\"{}\"><instance_geometry url=\"#geom0\"><bind_material><technique_common>",escape(project_name))?;
    for (i, (tex, _)) in groups.iter().enumerate() {
        write!(
            writer,
            "<instance_material symbol=\"sym{i}\" target=\"#mat{i}\">"
        )?;
        if *tex {
            write!(writer,"<bind_vertex_input semantic=\"UVSET0\" input_semantic=\"TEXCOORD\" input_set=\"0\"/>")?;
        }
        writeln!(writer, "</instance_material>")?;
    }
    writeln!(writer,"</technique_common></bind_material></instance_geometry></node></visual_scene></library_visual_scenes><scene><instance_visual_scene url=\"#scene\"/></scene></COLLADA>")?;
    Ok(())
}
/// Source-compatible best-effort texture sidecar copying. Decode/resource work is
/// cold export only. Embedded IGZ bytes are borrowed until each write completes.
pub fn save_dae(scene: &Scene, path: &Path, project_name: &str) -> Result<(), FormatError> {
    save_dae_view(scene, &scene.mesh, &scene.groups, &[], path, project_name)
}
pub fn save_dae_view(
    scene: &Scene,
    loose: &Mesh,
    groups: &[Group],
    overrides: &[PlacementOverride<'_>],
    path: &Path,
    project_name: &str,
) -> Result<(), FormatError> {
    let prims = crate::gltf::rich::collect_dae_view(scene, loose, groups, overrides)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut xml = Vec::new();
    write_collected(scene, &mut xml, project_name, &prims)?;
    // Native correction: a failed XML write cannot truncate an existing export
    // or publish texture sidecars. Missing/copy-error sidecars remain source best effort.
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&xml)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }
    for p in &prims {
        if let (Some(map), Some(tex)) = (p.info["map"].as_str(), p.info.get("texture")) {
            if let Some((_, bytes)) = crate::gltf::rich::source_texture(scene, tex) {
                let destination = path.with_file_name(map);
                if destination == path {
                    continue;
                }
                let source = tex
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(Path::new);
                if source.and_then(|p| p.canonicalize().ok()) != destination.canonicalize().ok()
                    || !destination.exists()
                {
                    let _ = std::fs::write(destination, bytes.as_ref());
                }
            }
        }
    }
    Ok(())
}
fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86400) as i64;
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let remainder = seconds % 86400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}+00:00",
        remainder / 3600,
        remainder / 60 % 60,
        remainder % 60
    )
}
