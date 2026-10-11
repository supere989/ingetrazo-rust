//! Literal original OBJ material subset; all file IO is cold plan preparation.
use super::ObjError;
use glam::DVec3;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Default)]
pub(super) struct Recipe {
    pub color: Option<[f64; 3]>,
    pub map: Option<PathBuf>,
    painted: bool,
}
pub(super) fn parse(path: &Path) -> Result<HashMap<String, Recipe>, ObjError> {
    let mut result = HashMap::new();
    if !path.exists() {
        return Ok(result);
    }
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut current = None;
    for (line_number, line) in text.lines().enumerate() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() {
            continue;
        }
        if parts[0] == "newmtl" {
            let name = parts
                .get(1)
                .ok_or_else(|| ObjError::Parse(line_number + 1, "MTL newmtl has no name".into()))?;
            result.insert((*name).to_owned(), Recipe::default());
            current = Some((*name).to_owned());
            continue;
        }
        let Some(recipe) = current.as_ref().and_then(|name| result.get_mut(name)) else {
            continue;
        };
        if parts[0] == "Kd" && parts.len() >= 4 {
            let mut color = [0_f64; 3];
            for i in 0..3 {
                color[i] = parts[i + 1]
                    .parse()
                    .map_err(|_| ObjError::Parse(line_number + 1, "Invalid MTL Kd".into()))?;
                if !color[i].is_finite() {
                    return Err(ObjError::Parse(line_number + 1, "Nonfinite MTL Kd".into()));
                }
            }
            recipe.painted = color
                .iter()
                .zip([0.96, 0.95, 0.925])
                .any(|(c, d)| format!("{c:.4}") != format!("{d:.4}"));
            recipe.color = Some(color);
        } else if parts[0] == "map_Kd" && parts.len() >= 2 {
            recipe.map = resolve(path, &parts[1..]);
        }
    }
    Ok(result)
}
fn resolve(mtl: &Path, tokens: &[&str]) -> Option<PathBuf> {
    let mut i = 0;
    while i < tokens.len() && tokens[i].starts_with('-') {
        i += 1;
        while i < tokens.len() && tokens[i].parse::<f64>().is_ok() {
            i += 1;
        }
    }
    let value = tokens[i..].join(" ");
    let value = value.trim().trim_matches('"');
    if value.is_empty() {
        return None;
    }
    let parent = mtl.parent().unwrap_or_else(|| Path::new("."));
    let portable = value.replace('\\', "/");
    let name = portable.rsplit('/').next().unwrap_or("");
    [parent.join(value), parent.join(name)]
        .into_iter()
        .find(|p| p.is_file())
}
/// Capture bytes only for actually used recipes, once per resolved file.
pub(super) fn attrs(
    recipe: Option<&Recipe>,
    points: &[DVec3],
    uvs: &[Option<glam::DVec2>],
    assets: &mut HashMap<String, Vec<u8>>,
    paths: &mut HashMap<PathBuf, String>,
) -> Result<HashMap<String, serde_json::Value>, ObjError> {
    let mut attrs = HashMap::new();
    let Some(recipe) = recipe else {
        return Ok(attrs);
    };
    if let Some(path) = &recipe.map {
        let key = if let Some(key) = paths.get(path) {
            key.clone()
        } else {
            let resolved = path.canonicalize()?;
            if let Some(key) = paths.get(&resolved).cloned() {
                paths.insert(path.clone(), key.clone());
                key
            } else {
                let bytes = std::fs::read(path)?;
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let key = crate::igz::prepared_texture_member(&bytes, &name);
                if assets.get(&key).is_some_and(|old| old != &bytes) {
                    return Err(ObjError::Parse(
                        0,
                        "Conflicting prepared texture member".into(),
                    ));
                }
                assets.entry(key.clone()).or_insert(bytes);
                paths.insert(resolved, key.clone());
                paths.insert(path.clone(), key.clone());
                key
            }
        };
        let mut tex = serde_json::json!({"path":key,"sw":1.,"sh":1.});
        if uvs.iter().all(Option::is_some) {
            let uv = uvs
                .iter()
                .map(|p| p.unwrap().to_array())
                .collect::<Vec<_>>();
            if let Some(uvw) = fit(points, &uv)? {
                let glu = uvw[0].hypot(uvw[1]).hypot(uvw[2]);
                let glv = uvw[4].hypot(uvw[5]).hypot(uvw[6]);
                tex["uvw"] = serde_json::json!(uvw);
                tex["sw"] = serde_json::json!(if glu > 1e-9 { 1. / glu } else { 1. });
                tex["sh"] = serde_json::json!(if glv > 1e-9 { 1. / glv } else { 1. });
            }
        }
        attrs.insert("texture".into(), tex);
        attrs.insert("back".into(), serde_json::json!(true));
    } else if let Some(color) = recipe.color {
        if recipe.painted {
            attrs.insert("color".into(), serde_json::json!(color));
            attrs.insert("back".into(), serde_json::json!(true));
        }
    }
    Ok(attrs)
}
fn fit(points: &[DVec3], uvs: &[[f64; 2]]) -> Result<Option<[f64; 8]>, ObjError> {
    if points.len() < 3 || uvs.len() < points.len() {
        return Ok(None);
    }
    let p = points[0].to_array();
    let d = points[1..]
        .iter()
        .map(|q| {
            let q = q.to_array();
            [q[0] - p[0], q[1] - p[1], q[2] - p[2]]
        })
        .collect::<Vec<_>>();
    let (mut best, mut pair) = (1e-24, None);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    if d.len() > 12 {
        best = f64::NEG_INFINITY;
        for i in 0..d.len() {
            for j in 0..d.len() {
                let aa = dot(d[i], d[i]);
                let bb = dot(d[j], d[j]);
                let ab = dot(d[i], d[j]);
                let size = aa * bb - ab * ab;
                if size > best {
                    best = size;
                    pair = Some((i, j));
                }
            }
        }
        if best <= 1e-24 {
            return Ok(None);
        }
    } else {
        for i in 0..d.len() {
            for j in i + 1..d.len() {
                let a = d[i];
                let b = d[j];
                let c = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                let size = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
                if size > best {
                    best = size;
                    pair = Some((i, j));
                }
            }
        }
    }
    let Some((i, j)) = pair else { return Ok(None) };
    let a = d[i];
    let b = d[j];
    let g11 = dot(a, a);
    let g12 = dot(a, b);
    let g22 = dot(b, b);
    let det = g11 * g22 - g12 * g12;
    if det.abs() < 1e-18 {
        return Ok(None);
    }
    let mut out = [0.; 8];
    for k in 0..2 {
        let d1 = uvs[i + 1][k] - uvs[0][k];
        let d2 = uvs[j + 1][k] - uvs[0][k];
        let aa = (d1 * g22 - d2 * g12) / det;
        let bb = (d2 * g11 - d1 * g12) / det;
        for v in 0..3 {
            out[k * 4 + v] = a[v] * aa + b[v] * bb;
        }
        out[k * 4 + 3] =
            uvs[0][k] - (out[k * 4] * p[0] + out[k * 4 + 1] * p[1] + out[k * 4 + 2] * p[2]);
    }
    if !out.iter().all(|v| v.is_finite()) {
        return Err(ObjError::Parse(
            0,
            "Texture affine fit overflow; document unchanged".into(),
        ));
    }
    Ok(Some(out))
}
