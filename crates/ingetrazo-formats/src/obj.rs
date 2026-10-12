// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Wavefront OBJ format export and import.

use glam::DVec3;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use ingetrazo_core::scene::Scene;
#[path = "obj_material.rs"]
mod source_material;
#[path = "obj_geometry.rs"]
mod obj_geometry;

pub(crate) fn obj_source_soften(mesh:&mut ingetrazo_core::mesh::Mesh){obj_geometry::soften(mesh,true);}

pub(crate) fn stitch_dxf_import(mesh:&mut ingetrazo_core::mesh::Mesh,first_new:usize,seed:&[DVec3])->Result<(),String>{obj_geometry::stitch(mesh,first_new,seed)}

/// Shared original editable-import cleanup; file parsers prepare only their own geometry.
pub(crate) fn stitch_source_import(mesh:&mut ingetrazo_core::mesh::Mesh,first_new:usize,seed:&[DVec3])->Result<(),String>{obj_geometry::editable(mesh,first_new,seed)}

#[derive(thiserror::Error, Debug)]
pub enum ObjError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error at line {0}: {1}")]
    Parse(usize, String),
}

/// Export scene (loose mesh + groups) into an OBJ writer.
pub fn export_obj(scene: &Scene, writer: &mut impl Write) -> Result<(), ObjError> {
    writeln!(writer, "# IngeTrazo OBJ Export")?;

    let triangles = crate::exchange::triangles(scene);
    for triangle in &triangles {
        for p in triangle.points {
            writeln!(writer, "v {:.6} {:.6} {:.6}", p.x, p.y, p.z)?;
        }
    }
    for triangle in &triangles {
        let [a, b, c] = triangle.points;
        let n = (b - a).cross(c - a).normalize_or_zero();
        writeln!(writer, "vn {:.6} {:.6} {:.6}", n.x, n.y, n.z)?;
    }
    for (i, _) in triangles.iter().enumerate() {
        let v = 3 * i + 1;
        let n = i + 1;
        writeln!(writer, "f {v}//{n} {}//{n} {}//{n}", v + 1, v + 2)?;
    }

    Ok(())
}

/// Import an OBJ stream into a scene's mesh.
pub fn import_obj(reader: &mut impl Read, scene: &mut Scene) -> Result<(), ObjError> {
    *scene = prepare_obj_options(reader, scene, 1., false)?;
    Ok(())
}

/// Source Import confirmation: scale authored points then construct Qt-float positions.
/// Existing unscaled callers retain their native f64 policy.
pub fn import_obj_with_units(
    reader: &mut impl Read,
    scene: &mut Scene,
    scale: f64,
) -> Result<(), ObjError> {
    *scene = prepare_obj_with_units(reader, scene, scale)?;
    Ok(())
}

/// Prepare the atomic source-unit import without publishing or an extra caller clone.
pub fn prepare_obj_with_units(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
) -> Result<Scene, ObjError> {
    prepare_obj_options(reader, scene, scale, true)
}

fn prepare_obj_options(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
    source_points: bool,
) -> Result<Scene, ObjError> {
    if !scale.is_finite() || scale <= 0. {
        return Err(ObjError::Parse(0, "Invalid import scale".into()));
    }
    let plan = parse_obj_options(reader, scene, scale, source_points, false, None)?;
    let mut candidate = scene.clone();
    plan.emit_legacy(&mut candidate)?;
    candidate.bump_version();
    Ok(candidate)
}

/// Parsed file-owned geometry; choosing an owner does not clone or mutate a Scene.
pub struct ObjPlan {
    declarations: Vec<Option<String>>,
    entities: Vec<ObjEntity>,
    pieces: Vec<((String, String), Vec<usize>)>,
    face_count: usize,
    assets: std::collections::HashMap<String, Vec<u8>>,
}
struct ObjEntity {
    declaration: Option<usize>,
    points: Vec<DVec3>,
    attrs: std::collections::HashMap<String, serde_json::Value>,
    face: bool,
    line: usize,
}
impl ObjPlan {
    pub fn targets_document_root(&self) -> bool {
        self.pieces.len() > 1 || self.face_count > 400
    }
    fn emit_entity(
        entity: &mut ObjEntity,
        mesh: &mut ingetrazo_core::mesh::Mesh,
    ) -> Result<(), ObjError> {
        let ids = entity
            .points
            .iter()
            .map(|p| mesh.add_vertex(*p))
            .collect::<Vec<_>>();
        if entity.face {
            let id = mesh
                .add_face(ids, vec![])
                .ok_or_else(|| ObjError::Parse(entity.line, "Degenerate face".into()))?;
            mesh.faces[id].attrs = std::mem::take(&mut entity.attrs);
        } else {
            for pair in ids.windows(2) {
                mesh.add_edge(pair[0], pair[1]);
            }
        }
        Ok(())
    }
    fn emit_legacy(mut self, scene: &mut Scene) -> Result<(), ObjError> {
        let start = scene.groups.len();
        for name in &self.declarations {
            if let Some(name) = name {
                scene
                    .groups
                    .push(ingetrazo_core::groups::Group::new(name.clone()));
            }
        }
        for entity in &mut self.entities {
            let mesh = if let Some(index) = entity.declaration {
                &mut scene.groups[start + index].mesh
            } else {
                &mut scene.mesh
            };
            Self::emit_entity(entity, mesh)?;
        }
        Ok(())
    }
    /// Source application grouping policy. Unscaled legacy format callers use emit_legacy.
    pub fn emit_source(mut self, scene: &mut Scene, stem: &str) -> Result<(), ObjError> {
        for (key, bytes) in self.assets.drain() {
            if scene
                .embedded_textures
                .get(&key)
                .is_some_and(|old| old != &bytes)
            {
                return Err(ObjError::Parse(
                    0,
                    "Prepared texture conflicts with document asset".into(),
                ));
            }
            scene.embedded_textures.entry(key).or_insert(bytes);
        }
        if self.pieces.len() > 1 {
            let mut container = ingetrazo_core::groups::Group::new(stem);
            for (number, (piece, indices)) in self.pieces.iter().enumerate() {
                let name = source_piece_name(piece, number + 1);
                let mut child = ingetrazo_core::groups::Group::new(name);
                child.mesh=obj_geometry::reference(indices.iter().map(|&i|(&self.entities[i].points[..],&self.entities[i].attrs))).map_err(|e|ObjError::Parse(0,e))?;
                if !child.mesh.faces.is_empty() { container.children.push(child); }
            }
            source_group_kind(&mut container, true);
            scene.groups.push(container);
        } else if self.face_count > 400 {
            let mesh=obj_geometry::reference(self.entities.iter().filter(|e|e.face).map(|e|(&e.points[..],&e.attrs))).map_err(|e|ObjError::Parse(0,e))?;
            let name = ingetrazo_core::groups::next_source_group_name()
                .map_err(|e| ObjError::Parse(0, e.into()))?;
            let mut group = ingetrazo_core::groups::Group::with_mesh(name, mesh);
            source_group_kind(&mut group, false);
            scene.groups.push(group);
        } else {
            let first_new=scene.mesh.faces.len();
            let seed=self.entities.iter().filter(|e|e.face).flat_map(|e|e.points.iter().copied()).collect::<Vec<_>>();
            for entity in self.entities.iter_mut().filter(|e| e.face) {
                Self::emit_entity(entity, &mut scene.mesh)?;
            }
            obj_geometry::editable(&mut scene.mesh,first_new,&seed).map_err(|e|ObjError::Parse(0,e))?;
        }
        scene.bump_version();
        Ok(())
    }
}
pub(crate) fn source_group_kind(group: &mut ingetrazo_core::groups::Group, placed: bool) {
    let identity = serde_json::json!(glam::DMat4::IDENTITY.to_cols_array());
    group.native_document = serde_json::json!({"component":true,"xform":if placed {identity.clone()}else{serde_json::Value::Null}});
    group.native_baseline = serde_json::json!({"component":false,"xform":if placed {serde_json::Value::Null}else{identity}});
}
fn source_piece_name(piece: &(String, String), number: usize) -> String {
    let name = if piece.1.is_empty() {
        &piece.0
    } else {
        &piece.1
    };
    let mut name = name.trim().to_owned();
    // Blender suffix cleanup is kept in this naming layer, not in owner identity.
    if let Some((head, tail)) = name.rsplit_once('_') {
        let word = tail.split('.').next().unwrap_or("");
        let suffix = tail.strip_prefix(word).unwrap_or("");
        if !head.is_empty()
            && word.chars().next().is_some_and(char::is_uppercase)
            && word.chars().all(|c| c.is_ascii_alphabetic())
            && (suffix.is_empty()
                || suffix
                    .strip_prefix('.')
                    .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())))
        {
            name = head.to_owned();
        }
    }
    let fallback = name.clone();
    loop {
        let Some((head, tail)) = name.rsplit_once('.') else {
            break;
        };
        if tail.len() != 3 || !tail.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        name = head.to_owned();
    }
    if name.trim().is_empty() {
        name = fallback;
    }
    let name = name.trim();
    if name.is_empty() {
        format!("Part {number}")
    } else if !name.chars().any(char::is_alphabetic) {
        format!("Part {name}")
    } else {
        name.to_owned()
    }
}
pub fn parse_obj_with_units(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
) -> Result<ObjPlan, ObjError> {
    if !scale.is_finite() || scale <= 0. {
        return Err(ObjError::Parse(0, "Invalid import scale".into()));
    }
    parse_obj_options(reader, scene, scale, true, true, None)
}
/// Path-aware source Import; sidecars and used images are prepared before owner selection.
// Unicode decimal blocks pinned to the unchanged original Python runtime.
// Python int accepts Nd digits and interior single underscores, independently
// of machine index range. Numeric overflow is a missing UV, not invalid syntax.
fn source_decimal_digit(ch:char)->Option<u32> {
    const ZEROES: &[u32] = &[0x30,0x660,0x6f0,0x7c0,0x966,0x9e6,0xa66,0xae6,0xb66,0xbe6,0xc66,0xce6,0xd66,0xde6,0xe50,0xed0,0xf20,0x1040,0x1090,0x17e0,0x1810,0x1946,0x19d0,0x1a80,0x1a90,0x1b50,0x1bb0,0x1c40,0x1c50,0xa620,0xa8d0,0xa900,0xa9d0,0xa9f0,0xaa50,0xabf0,0xff10,0x104a0,0x10d30,0x11066,0x110f0,0x11136,0x111d0,0x112f0,0x11450,0x114d0,0x11650,0x116c0,0x11730,0x118e0,0x11950,0x11c50,0x11d50,0x11da0,0x11f50,0x16a60,0x16ac0,0x16b50,0x1d7ce,0x1d7d8,0x1d7e2,0x1d7ec,0x1d7f6,0x1e140,0x1e2f0,0x1e4f0,0x1e950,0x1fbf0];
    let n=ch as u32;ZEROES.iter().find_map(|zero|n.checked_sub(*zero).filter(|d|*d<10))
}
fn source_smoothing_token(raw:&str)->Result<Option<String>,()> {
    const OTHER_DIGITS:&[(u32,u32)]=&[(0xb2,0xb3),(0xb9,0xb9),(0x1369,0x1371),(0x19da,0x19da),(0x2070,0x2070),(0x2074,0x2079),(0x2080,0x2089),(0x2460,0x2468),(0x2474,0x247c),(0x2488,0x2490),(0x24ea,0x24ea),(0x24f5,0x24fd),(0x24ff,0x24ff),(0x2776,0x277e),(0x2780,0x2788),(0x278a,0x2792),(0x10a40,0x10a43),(0x10e60,0x10e68),(0x11052,0x1105a),(0x1f100,0x1f10a)];
    if raw.is_empty() || !raw.chars().all(|c|source_decimal_digit(c).is_some()||OTHER_DIGITS.iter().any(|&(a,b)|(a..=b).contains(&(c as u32)))) {return Ok(None);}
    let mut digits=String::with_capacity(raw.len());let mut count=0;
    for c in raw.chars(){count+=1;if count>4300{return Err(());}let d=source_decimal_digit(c).ok_or(())?;digits.push((b'0'+d as u8)as char);}
    let normalized=digits.trim_start_matches('0');if normalized.is_empty(){Ok(None)}else{Ok(Some(normalized.to_owned()))}
}
fn source_uv_integer(raw: &str) -> Result<Option<isize>, ()> {
    let (negative, digits) = match raw.as_bytes().first() {
        Some(b'-') => (true, &raw[1..]), Some(b'+') => (false, &raw[1..]), _ => (false, raw),
    };
    let mut last_digit = false;
    let mut magnitude = Some(0_u128);
    let mut digit_count = 0;
    for ch in digits.chars() {
        if ch == '_' {
            if !last_digit { return Err(()); }
            last_digit = false;
            continue;
        }
        let digit = source_decimal_digit(ch).ok_or(())?;
        digit_count += 1;
        // Pinned Python's default decimal conversion limit.
        if digit_count > 4300 { return Err(()); }
        magnitude = magnitude.and_then(|v| v.checked_mul(10)?.checked_add(digit as u128));
        last_digit = true;
    }
    if !last_digit { return Err(()); }
    Ok(magnitude.and_then(|n| {
        if negative {
            if n == isize::MAX as u128 + 1 { Some(isize::MIN) }
            else { isize::try_from(n).ok().map(|v| -v) }
        } else { isize::try_from(n).ok() }
    }))
}

pub fn parse_obj_import(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
    path: &Path,
) -> Result<ObjPlan, ObjError> {
    if !scale.is_finite() || scale <= 0. {
        return Err(ObjError::Parse(0, "Invalid import scale".into()));
    }
    parse_obj_options(reader, scene, scale, true, true, Some(path))
}
fn parse_obj_options(
    reader: &mut impl Read,
    scene: &Scene,
    scale: f64,
    source_points: bool,
    source_policy: bool,
    source_path: Option<&Path>,
) -> Result<ObjPlan, ObjError> {
    let buf_reader = BufReader::new(reader);
    let mut file_verts: Vec<DVec3> = Vec::new();
    let mut container = None;
    let mut plan = ObjPlan {
        declarations: Vec::new(),
        entities: Vec::new(),
        pieces: Vec::new(),
        face_count: 0,
        assets: std::collections::HashMap::new(),
    };
    let mut piece_indices = std::collections::BTreeMap::new();
    let mut object = String::new();
    let mut group = String::new();
    let mut material: Option<String> = None;
    let mut source_recipes = std::collections::HashMap::new();
    let mut current_recipe = None;
    let mut asset_paths = std::collections::HashMap::new();
    let mut file_uv = Vec::<glam::DVec2>::new();
    let mut smooth = 0_u64;
    let mut smoothing_tokens=std::collections::HashMap::<String,u64>::new();

    for (line_num, line_res) in buf_reader.lines().enumerate() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => continue,
        };

        if source_policy && cmd == "l" {
            continue;
        }
        if source_policy && cmd == "mtllib" {
            let path = source_path.ok_or_else(|| {
                ObjError::Parse(
                    line_num + 1,
                    "Material sidecar requires an OBJ source path".into(),
                )
            })?;
            let name = parts
                .next()
                .ok_or_else(|| ObjError::Parse(line_num + 1, "mtllib has no filename".into()))?;
            if Path::new(name).components().count() != 1 || name == "." || name == ".." {
                return Err(ObjError::Parse(
                    line_num + 1,
                    "MTL filename must be beside OBJ".into(),
                ));
            }
            source_recipes = source_material::parse(&path.with_file_name(name))?;
            continue;
        }
        match cmd {
            "v" => {
                let x: f64 = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| ObjError::Parse(line_num + 1, "Invalid vertex X".to_string()))?;
                let y: f64 = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| ObjError::Parse(line_num + 1, "Invalid vertex Y".to_string()))?;
                let z: f64 = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| ObjError::Parse(line_num + 1, "Invalid vertex Z".to_string()))?;
                let point = DVec3::new(x, y, z) * scale;
                let point = if source_points {
                    point.as_vec3().as_dvec3()
                } else {
                    point
                };
                if !point.is_finite() {
                    return Err(ObjError::Parse(line_num + 1, "Nonfinite vertex".into()));
                }
                file_verts.push(point);
            }
            "vt" => {
                if source_policy {
                    let values = parts.collect::<Vec<_>>();
                    let parsed = values.first().and_then(|v| v.parse::<f64>().ok()).zip(
                        if values.len() > 1 {
                            values[1].parse::<f64>().ok()
                        } else {
                            Some(0.)
                        },
                    );
                    let (u, v) = parsed.unwrap_or((0., 0.));
                    if !u.is_finite() || !v.is_finite() {
                        return Err(ObjError::Parse(line_num + 1, "Nonfinite texture UV".into()));
                    }
                    file_uv.push(glam::DVec2::new(u, v));
                    continue;
                }
                let u = parts
                    .next()
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or_else(|| ObjError::Parse(line_num + 1, "Invalid texture U".into()))?;
                let v = parts
                    .next()
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or_else(|| ObjError::Parse(line_num + 1, "Invalid texture V".into()))?;
                if !u.is_finite() || !v.is_finite() {
                    return Err(ObjError::Parse(line_num + 1, "Nonfinite texture UV".into()));
                }
                file_uv.push(glam::DVec2::new(u, v));
            }
            "o" | "g" => {
                let name = parts.collect::<Vec<_>>().join(" ");
                if cmd == "o" {
                    object = name.clone();
                    group.clear();
                } else {
                    group = name.clone();
                }
                if name.is_empty() {
                    container = None;
                } else {
                    let index = plan.declarations.len();
                    plan.declarations.push(Some(name));
                    container = Some(index);
                }
            }
            "s" if source_policy => {
                let raw=parts.next().unwrap_or("off");
                let token=source_smoothing_token(raw).map_err(|_|ObjError::Parse(line_num+1,"Invalid smoothing group integer".into()))?;
                smooth=if let Some(token)=token {
                    let next=u64::try_from(smoothing_tokens.len()).ok().and_then(|v|v.checked_add(1)).ok_or_else(||ObjError::Parse(line_num+1,"Smoothing identity exhausted".into()))?;
                    *smoothing_tokens.entry(token).or_insert(next)
                }else{0};
            }
            "usemtl" => {
                if source_policy {
                    let name = parts.next().ok_or_else(|| {
                        ObjError::Parse(line_num + 1, "usemtl has no name".into())
                    })?;
                    current_recipe = source_recipes.get(name).cloned();
                } else {
                    material = Some(parts.collect::<Vec<_>>().join(" "));
                }
            }
            "f" | "l" => {
                // Original f parsing skips the whole face on a noninteger UV
                // token. Numeric out-of-range indices instead keep planar paint.
                if source_policy && cmd == "f" && parts.clone().any(|token| {
                    token.split('/').nth(1).filter(|raw| !raw.is_empty())
                        .is_some_and(|raw| source_uv_integer(raw).is_err())
                }) {
                    continue;
                }
                let mut points = Vec::new();
                let mut uvs = Vec::new();
                for token in parts {
                    let raw = token.split('/').next().unwrap_or("");
                    let index = raw.parse::<isize>().map_err(|_| {
                        ObjError::Parse(line_num + 1, "Invalid vertex index".into())
                    })?;
                    let index = if index > 0 {
                        index - 1
                    } else if index < 0 {
                        file_verts.len() as isize + index
                    } else {
                        -1
                    };
                    let point = usize::try_from(index)
                        .ok()
                        .and_then(|i| file_verts.get(i))
                        .copied()
                        .ok_or_else(|| {
                            ObjError::Parse(
                                line_num + 1,
                                "Vertex index outside source array".into(),
                            )
                        })?;
                    points.push(point);
                    let uv = token
                        .split('/')
                        .nth(1)
                        .filter(|v| !v.is_empty())
                        .map(|raw| {
                            let i = if source_policy {
                                source_uv_integer(raw).map_err(|_| ObjError::Parse(line_num + 1, "Invalid UV index".into()))?
                                    .ok_or_else(|| ObjError::Parse(line_num + 1, "UV index outside source array".into()))?
                            } else {
                                raw.parse::<isize>().map_err(|_| ObjError::Parse(line_num + 1, "Invalid UV index".into()))?
                            };
                            let index = if i > 0 {
                                i - 1
                            } else if i < 0 {
                                file_uv.len() as isize + i
                            } else {
                                -1
                            };
                            usize::try_from(index)
                                .ok()
                                .and_then(|i| file_uv.get(i))
                                .copied()
                                .ok_or_else(|| {
                                    ObjError::Parse(
                                        line_num + 1,
                                        "UV index outside source array".into(),
                                    )
                                })
                        })
                        .transpose();
                    let uv = if source_policy {
                        uv.unwrap_or(None)
                    } else {
                        uv?
                    };
                    uvs.push(uv);
                }
                if points.len() < if cmd == "f" { 3 } else { 2 } {
                    return Err(ObjError::Parse(
                        line_num + 1,
                        "Insufficient entity vertices".into(),
                    ));
                }
                let mut attrs = material
                    .as_ref()
                    .filter(|_| !source_policy)
                    .map(|name| {
                        let mut attrs = std::collections::HashMap::new();
                        attrs.insert("mat".into(), serde_json::json!(name));
                        if let Some(mat) = scene.materials.get(name) {
                            attrs.insert(
                                "color".into(),
                                serde_json::json!([mat.color[0], mat.color[1], mat.color[2]]),
                            );
                            attrs.insert("opacity".into(), serde_json::json!(mat.opacity));
                            if let Some(path) = &mat.texture_path {
                                attrs.insert("texture".into(), serde_json::json!({"path":path}));
                            }
                        }
                        attrs
                    })
                    .unwrap_or_default();
                if source_policy && cmd == "f" {
                    attrs = source_material::attrs(
                        current_recipe.as_ref(),
                        &points,
                        &uvs,
                        &mut plan.assets,
                        &mut asset_paths,
                    )?;
                }
                if source_policy && cmd == "f" && smooth != 0 { attrs.insert("_s".into(), serde_json::Value::from(smooth)); }
                if !source_policy && cmd == "f" && attrs.contains_key("texture") {
                    if uvs.iter().all(Option::is_some) {
                        let e1 = points[1] - points[0];
                        let e2 = points[2] - points[0];
                        let n = e1.cross(e2);
                        let matrix = glam::DMat3::from_cols(e1, e2, n).transpose();
                        if matrix.determinant().abs() < 1e-18 {
                            return Err(ObjError::Parse(
                                line_num + 1,
                                "Cannot fit degenerate texture plane".into(),
                            ));
                        }
                        let uv = uvs.iter().map(|v| v.unwrap()).collect::<Vec<_>>();
                        let gu =
                            matrix.inverse() * DVec3::new(uv[1].x - uv[0].x, uv[2].x - uv[0].x, 0.);
                        let gv =
                            matrix.inverse() * DVec3::new(uv[1].y - uv[0].y, uv[2].y - uv[0].y, 0.);
                        let cu = uv[0].x - gu.dot(points[0]);
                        let cv = uv[0].y - gv.dot(points[0]);
                        for (p, uv) in points.iter().zip(&uv) {
                            if (gu.dot(*p) + cu - uv.x).abs() > 1e-6
                                || (gv.dot(*p) + cv - uv.y).abs() > 1e-6
                            {
                                return Err(ObjError::Parse(
                                    line_num + 1,
                                    "Nonaffine polygon UVs require triangle faces".into(),
                                ));
                            }
                        }
                        attrs.get_mut("texture").unwrap()["uvw"] =
                            serde_json::json!([gu.x, gu.y, gu.z, cu, gv.x, gv.y, gv.z, cv]);
                    } else {
                        attrs.insert("texture".into(), serde_json::Value::Null);
                    }
                }
                if cmd == "f" {
                    let key = (object.clone(), group.clone());
                    let piece = *piece_indices.entry(key.clone()).or_insert_with(|| {
                        let index = plan.pieces.len();
                        plan.pieces.push((key, Vec::new()));
                        index
                    });
                    plan.pieces[piece].1.push(plan.entities.len());
                    plan.face_count += 1;
                }
                plan.entities.push(ObjEntity {
                    declaration: container,
                    points,
                    attrs,
                    face: cmd == "f",
                    line: line_num + 1,
                });
            }
            _ => {}
        }
    }

    Ok(plan)
}

/// Save OBJ with a material sidecar and self-contained texture assets.
/// All source assets are read before touching the destination.
pub fn save_obj(scene: &Scene, path: &Path) -> Result<(), ObjError> {
    let triangles = crate::exchange::triangles(scene);
    let mut obj = Vec::new();
    let mut mtl = Vec::new();
    let mut assets = Vec::new();
    let stem = path.file_stem().and_then(|v| v.to_str()).unwrap_or("model");
    let mtl_path = path.with_extension("mtl");
    writeln!(
        obj,
        "# IngeTrazo OBJ Export\nmtllib {}",
        mtl_path.file_name().unwrap().to_string_lossy()
    )?;
    for triangle in &triangles {
        for p in triangle.points {
            writeln!(obj, "v {:.6} {:.6} {:.6}", p.x, p.y, p.z)?;
        }
    }
    let mut uv_indices = Vec::new();
    let mut uv_count = 0;
    let mut recipes = std::collections::HashMap::<String, String>::new();
    let mut material_names = Vec::new();
    for t in &triangles {
        let named = t.attrs.get("mat").and_then(|v| v.as_str());
        let registered = named.and_then(|name| scene.materials.get(name));
        let color = t.attrs.get("color").and_then(|v| v.as_array());
        let rgba = std::array::from_fn::<_, 4, _>(|j| {
            color
                .and_then(|c| c.get(j))
                .and_then(|v| v.as_f64())
                .unwrap_or_else(|| {
                    registered.map_or([0.96, 0.95, 0.925, 1.][j], |m| m.color[j] as f64)
                })
        });
        let opacity = t
            .attrs
            .get("opacity")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| registered.map_or(rgba[3], |m| m.opacity as f64));
        let label = named
            .unwrap_or("paint")
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let texture = t.attrs.get("texture").cloned().or_else(|| {
            registered.and_then(|m| {
                m.texture_embed
                    .as_ref()
                    .map(|e| serde_json::json!({"embed":e}))
                    .or_else(|| {
                        m.texture_path
                            .as_ref()
                            .map(|p| serde_json::json!({"path":p}))
                    })
            })
        });
        let key = serde_json::json!([named, rgba, opacity, texture]).to_string();
        let new_material = !recipes.contains_key(&key);
        let material = if let Some(name) = recipes.get(&key) {
            name.clone()
        } else {
            let mut name = if named.is_some() {
                label.clone()
            } else {
                format!("paint{}", recipes.len())
            };
            if recipes.values().any(|existing| existing == &name) {
                name = format!("{name}_{}", recipes.len());
            }
            recipes.insert(key, name.clone());
            name
        };
        material_names.push(material.clone());
        if new_material {
            writeln!(
                mtl,
                "newmtl {material}\nKd {:.6} {:.6} {:.6}\nd {:.6}",
                rgba[0], rgba[1], rgba[2], opacity
            )?;
        }
        // An explicit null texture suppresses registry fallback, as when an
        // imported OBJ face has no UVs despite a textured material.
        if let Some(tex) = texture.filter(|value| !value.is_null()) {
            let embed = tex.get("embed").and_then(|v| v.as_str());
            let source = tex.get("path").and_then(|v| v.as_str());
            let bytes = if let Some(data) = embed.and_then(|e| scene.embedded_textures.get(e)) {
                data.clone()
            } else if let Some(source) = source {
                std::fs::read(source)?
            } else {
                return Err(ObjError::Parse(
                    0,
                    "Texture has no readable embedded asset or path".into(),
                ));
            };
            let extension = Path::new(source.or(embed).unwrap_or("texture.png"))
                .extension()
                .and_then(|e| e.to_str())
                .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or("png");
            let filename = format!(
                "{}_texture_{}.{}",
                stem.replace(['\n', '\r'], "_"),
                uuid::Uuid::new_v4(),
                extension
            );
            if new_material {
                writeln!(mtl, "map_Kd {filename}")?;
                assets.push((filename, bytes));
            }
            let explicit = tex
                .get("uvw")
                .and_then(|v| v.as_array())
                .filter(|a| a.len() == 8);
            let [a, b, c] = t.points;
            let normal = (b - a).cross(c - a).normalize_or_zero();
            let (u, v) = uv_basis(normal);
            let angle = tex
                .get("rot")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.)
                .to_radians();
            let (u, v) = (
                u * angle.cos() + v * angle.sin(),
                v * angle.cos() - u * angle.sin(),
            );
            let dimension = |key| {
                tex.get(key)
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.abs() > 1e-9)
                    .unwrap_or(1.)
            };
            let first = uv_count + 1;
            for k in 0..3 {
                let (uu, vv) = if let Some(map) = explicit {
                    let p = t.local_points[k];
                    let n = |j: usize| map[j].as_f64().unwrap_or(0.);
                    (
                        DVec3::new(n(0), n(1), n(2)).dot(p) + n(3),
                        DVec3::new(n(4), n(5), n(6)).dot(p) + n(7),
                    )
                } else {
                    (
                        u.dot(t.points[k]) / dimension("sw"),
                        v.dot(t.points[k]) / dimension("sh"),
                    )
                };
                if !uu.is_finite() || !vv.is_finite() {
                    return Err(ObjError::Parse(0, "Nonfinite texture coordinates".into()));
                }
                writeln!(obj, "vt {:.9} {:.9}", uu, vv)?;
                uv_count += 1;
            }
            uv_indices.push(Some(first));
        } else {
            uv_indices.push(None);
        }
        if new_material {
            writeln!(mtl)?;
        }
    }
    for (i, _) in triangles.iter().enumerate() {
        writeln!(obj, "usemtl {}", material_names[i])?;
        let first = i * 3 + 1;
        if let Some(uv) = uv_indices[i] {
            writeln!(
                obj,
                "f {first}/{uv} {}/{} {}/{}",
                first + 1,
                uv + 1,
                first + 2,
                uv + 2
            )?;
        } else {
            writeln!(obj, "f {first} {} {}", first + 1, first + 2)?;
        }
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    for (name, data) in assets {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(name))?;
        file.write_all(&data)?;
    }
    std::fs::write(mtl_path, mtl)?;
    std::fs::write(path, obj)?;
    Ok(())
}

fn uv_basis(normal: DVec3) -> (DVec3, DVec3) {
    let n = normal.normalize_or_zero();
    let length = n.x.hypot(n.y);
    if length < 1e-3 {
        (
            DVec3::new(if n.z >= 0. { 1. } else { -1. }, 0., 0.),
            DVec3::Y,
        )
    } else {
        let u = DVec3::new(-n.y / length, n.x / length, 0.);
        (u, n.cross(u))
    }
}

pub fn load_obj(path: &Path) -> Result<Scene, ObjError> {
    let text = std::fs::read_to_string(path)?;
    let mut scene = Scene::new();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for line in text.lines() {
        if let Some(names) = line.trim().strip_prefix("mtllib ") {
            let joined = parent.join(names.trim());
            let paths = if joined.exists() {
                vec![joined]
            } else {
                names
                    .split_whitespace()
                    .map(|name| parent.join(name))
                    .collect()
            };
            for mtl in paths {
                load_mtl(&mut scene, &mtl)?;
            }
        }
    }
    import_obj(&mut std::io::Cursor::new(text), &mut scene)?;
    Ok(scene)
}

fn load_mtl(scene: &mut Scene, path: &Path) -> Result<(), ObjError> {
    use ingetrazo_core::materials::Material;
    let text = std::fs::read_to_string(path)?;
    let mut current: Option<String> = None;
    for (line_number, line) in text.lines().enumerate() {
        let mut parts = line.trim().split_whitespace();
        let Some(command) = parts.next() else {
            continue;
        };
        if command == "newmtl" {
            let name = parts.collect::<Vec<_>>().join(" ");
            scene
                .materials
                .insert(name.clone(), Material::new(&name, [0.96, 0.95, 0.925, 1.]));
            current = Some(name);
            continue;
        }
        let Some(material) = current
            .as_ref()
            .and_then(|name| scene.materials.get_mut(name))
        else {
            continue;
        };
        match command {
            "Kd" => {
                for i in 0..3 {
                    let value = parts
                        .next()
                        .and_then(|p| p.parse::<f32>().ok())
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| {
                            ObjError::Parse(line_number + 1, "Invalid MTL diffuse color".into())
                        })?;
                    material.color[i] = value;
                }
            }
            "d" | "Tr" => {
                let value = parts
                    .next()
                    .and_then(|p| p.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| {
                        ObjError::Parse(line_number + 1, "Invalid MTL opacity".into())
                    })?;
                material.opacity = if command == "Tr" { 1. - value } else { value };
                material.color[3] = material.opacity;
            }
            "map_Kd" => {
                let name = parts.collect::<Vec<_>>().join(" ");
                if name.starts_with('-') {
                    return Err(ObjError::Parse(
                        line_number + 1,
                        "MTL texture options are not yet supported".into(),
                    ));
                }
                material.texture_path = Some(
                    path.parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(name)
                        .to_string_lossy()
                        .into(),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod asset_tests {
    use super::*;
    use ingetrazo_core::{groups::Group, materials::Material, mesh::Mesh};
    #[test]
    fn material_texture_bytes_and_local_affine_uv_survive_nested_export() {
        let directory = std::env::temp_dir().join(format!("obj-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let mut mesh = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        let face = mesh.add_face(ids.to_vec(), vec![]).unwrap();
        mesh.faces[face]
            .attrs
            .insert("mat".into(), serde_json::json!("Blue Paint"));
        mesh.faces[face].attrs.insert(
            "texture".into(),
            serde_json::json!({"embed":"images/test.png","uvw":[1.,0.,0.,0.25,0.,1.,0.,0.5]}),
        );
        let mut group = Group::with_mesh("mapped", mesh);
        group.transform = glam::DMat4::from_translation(DVec3::new(10., 20., 30.));
        let mut scene = Scene::new();
        scene.groups.push(group);
        scene
            .embedded_textures
            .insert("images/test.png".into(), vec![1, 2, 3, 4]);
        scene.materials.insert(
            "Blue Paint".into(),
            Material::new("Blue Paint", [0.1, 0.2, 0.8, 0.6]),
        );
        let path = directory.join("model.obj");
        save_obj(&scene, &path).unwrap();
        let imported = load_obj(&path).unwrap();
        assert_eq!(imported.mesh.faces.len(), 1);
        let attrs = &imported.mesh.faces[0].attrs;
        assert_eq!(attrs["mat"], serde_json::json!("Blue_Paint"));
        let map = attrs["texture"]["uvw"].as_array().unwrap();
        for (p, expected) in [
            (DVec3::new(10., 20., 30.), [0.25, 0.5]),
            (DVec3::new(11., 20., 30.), [1.25, 0.5]),
            (DVec3::new(10., 21., 30.), [0.25, 1.5]),
        ] {
            let value = |i: usize| map[i].as_f64().unwrap();
            assert!(
                (DVec3::new(value(0), value(1), value(2)).dot(p) + value(3) - expected[0]).abs()
                    < 1e-8
            );
            assert!(
                (DVec3::new(value(4), value(5), value(6)).dot(p) + value(7) - expected[1]).abs()
                    < 1e-8
            );
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("vt 0.250000000 0.500000000"));
        assert!(text.contains("mtllib model.mtl"));
        assert!(text.contains("usemtl Blue_Paint"));
        let mtl = std::fs::read_to_string(directory.join("model.mtl")).unwrap();
        assert!(mtl.contains("Kd 0.100000 0.200000 0.800000"));
        assert!(mtl.contains("d 0.600000"));
        let image = mtl.lines().find_map(|l| l.strip_prefix("map_Kd ")).unwrap();
        assert_eq!(
            std::fs::read(directory.join(image)).unwrap(),
            vec![1, 2, 3, 4]
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn missing_texture_fails_before_destination_or_sidecar_mutation() {
        let directory = std::env::temp_dir().join(format!("obj-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let mut scene = Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| scene.mesh.add_vertex(p));
        let f = scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        scene.mesh.faces[f]
            .attrs
            .insert("texture".into(), serde_json::json!({"embed":"missing"}));
        let path = directory.join("model.obj");
        std::fs::write(&path, b"original").unwrap();
        std::fs::write(path.with_extension("mtl"), b"material-original").unwrap();
        assert!(save_obj(&scene, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(
            std::fs::read(path.with_extension("mtl")).unwrap(),
            b"material-original"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    #[test]
    fn bad_indices_or_nonfinite_vertices_never_partially_modify_scene() {
        let mut scene = Scene::new();
        scene.mesh.add_vertex(DVec3::new(7., 8., 9.));
        let before = scene.clone();
        for source in [
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 9\n",
            "v NaN 0 0\n",
            "v 0 0 0\nv 1 0 0\nf 0 1 2\n",
        ] {
            assert!(import_obj(&mut std::io::Cursor::new(source), &mut scene).is_err());
            assert_eq!(scene, before);
        }
    }
    #[test]
    fn named_objects_negative_indices_and_linework_are_retained() {
        let source = "v 0 0 0\nv 1 0 0\nv 0 1 0\ng seat\nf -3 -2 -1\ng leg\nl 1 2 3\n";
        let mut scene = Scene::new();
        import_obj(&mut std::io::Cursor::new(source), &mut scene).unwrap();
        assert_eq!(
            scene
                .groups
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            vec!["seat", "leg"]
        );
        assert_eq!(scene.groups[0].mesh.faces.len(), 1);
        assert_eq!(scene.groups[1].mesh.edges.len(), 2);
        assert!(scene.mesh.faces.is_empty());
    }
}
