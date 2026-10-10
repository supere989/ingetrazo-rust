// SPDX-License-Identifier: GPL-3.0-or-later AND MIT
// Ported from OpenSKP _core.py/parser.py/vff.py at 291700bc213655a00461838de71e5f206311e619.
// Copyright (c) 2024-2026 Ahsan Mehmood. MIT notice: OPENSKP-LICENSE.
use super::*;
use std::io::{Cursor, Read};
mod annotations;
const MAX_FILE: usize = 512 * 1024 * 1024;
const MAX_MODEL: usize = 256 * 1024 * 1024;
const MAX_NODES: usize = 2_000_000;
#[derive(Clone)]
struct Node<'a> {
    tag: [u8; 2],
    body: &'a [u8],
    children: Vec<Node<'a>>,
}
impl<'a> Node<'a> {
    fn child(&self, tag: [u8; 2]) -> Option<&Node<'a>> {
        self.children.iter().find(|n| n.tag == tag)
    }
}
fn uv_matrix(face: &Node<'_>, side: [u8; 2]) -> Result<Option<[f64; 9]>, SkpError> {
    let Some(info) = face.child([0xd0, 7]).and_then(|n| n.child([0xdc, 5])) else {
        return Ok(None);
    };
    let mut body = info.body;
    for tag in [
        [0xdd, 5],
        [0xb1, 0x36],
        [0xb2, 0x36],
        [0x10, 0x27],
        side,
        [0x13, 0x27],
        [0x15, 0x27],
    ] {
        let records = raw_nodes(body)?;
        let Some(node) = records.iter().find(|n| n.tag == tag) else {
            return Ok(None);
        };
        body = node.body;
    }
    if body.len() != 72 {
        return Ok(None);
    }
    let mut result = [0.; 9];
    for (value, bytes) in result.iter_mut().zip(body.chunks_exact(8)) {
        *value = f64::from_le_bytes(bytes.try_into().unwrap());
        if !value.is_finite() {
            return Err(SkpError::Invalid("Nonfinite texture mapping".into()));
        }
    }
    Ok(Some(result))
}
fn container(t: [u8; 2]) -> bool {
    matches!(
        t,
        [0xf4, 1]
            | [0xf7, 1]
            | [0xd4, 0x30]
            | [0xd5, 0x30]
            | [0xc8, 0x32]
            | [0x7c, 0x15]
            | [0x88, 0x13]
            | [0x89, 0x13]
            | [0x8a, 0x13]
            | [0x8b, 0x13]
            | [0x8c, 0x13]
            | [0x8d, 0x13]
            | [0x4c, 0x1d]
            | [0x64, 0x19]
            | [0xf9, 1]
            | [0x70, 0x17]
            | [0x71, 0x17]
            | [0xd0, 7]
            | [0xc4, 9]
            | [0x94, 0x11]
            | [0x95, 0x11]
            | [0x0f, 1]
            | [0x38, 0x4a]
            | [0xb8, 0xb]
            | [0x97, 0x13]
            | [0x2c, 0x4c]
            | [0xac, 0xd]
            | [0xae, 0xd]
            | [0xf6, 1]
            | [0xf8, 1]
            | [0x98, 0x3a]
            | [0x99, 0x3a]
            | [0x8c, 0x3c]
            | [0x90, 0x13]
            | [0x40, 0x1f]
    )
}
fn nodes<'a>(bytes: &'a [u8], depth: usize, count: &mut usize) -> Result<Vec<Node<'a>>, SkpError> {
    if depth > 128 {
        return Err(SkpError::Invalid("TLV nesting exceeds 128".into()));
    }
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < bytes.len() {
        if bytes.len() - pos < 6 {
            return Err(SkpError::Invalid("Truncated TLV header".into()));
        }
        let tag = [bytes[pos], bytes[pos + 1]];
        let size = u32::from_le_bytes(bytes[pos + 2..pos + 6].try_into().unwrap()) as usize;
        let end = pos
            .checked_add(6)
            .and_then(|n| n.checked_add(size))
            .filter(|&n| n <= bytes.len())
            .ok_or_else(|| SkpError::Invalid("TLV body exceeds container".into()))?;
        *count += 1;
        if *count > MAX_NODES {
            return Err(SkpError::Invalid("TLV record admission exceeded".into()));
        }
        let body = &bytes[pos + 6..end];
        let children = if container(tag) && !body.is_empty() {
            nodes(body, depth + 1, count)
                .map_err(|e| SkpError::Invalid(format!("Container {tag:02x?}: {e}")))?
        } else {
            Vec::new()
        };
        out.push(Node {
            tag,
            body,
            children,
        });
        pos = end;
    }
    Ok(out)
}
fn raw_nodes(b: &[u8]) -> Result<Vec<Node<'_>>, SkpError> {
    nodes(b, 0, &mut 0)
}
fn uint(b: &[u8]) -> Result<u64, SkpError> {
    if b.is_empty() || b.len() > 8 {
        return Err(SkpError::Invalid(
            "Entity integer length outside 1..8".into(),
        ));
    }
    Ok(b.iter()
        .enumerate()
        .fold(0, |a, (i, b)| a | u64::from(*b) << (8 * i)))
}
fn child_uint(n: &Node<'_>, t: [u8; 2]) -> Result<Option<u64>, SkpError> {
    n.child(t).map(|n| uint(n.body)).transpose()
}
fn info_id(body: &[u8]) -> Result<u64, SkpError> {
    if body.starts_with(&[0xde, 5]) {
        raw_nodes(body)?
            .into_iter()
            .find(|n| n.tag == [0xde, 5])
            .map(|n| uint(n.body))
            .unwrap_or_else(|| Err(SkpError::Invalid("Missing entity id".into())))
    } else {
        uint(body)
    }
}
fn entity_id(n: &Node<'_>) -> Result<u64, SkpError> {
    if let Some(c) = n.child([0xde, 5]) {
        return uint(c.body);
    }
    if let Some(c) = n.child([0xdc, 5]) {
        return info_id(c.body);
    }
    for c in &n.children {
        if let Ok(id) = entity_id(c) {
            return Ok(id);
        }
    }
    Err(SkpError::Invalid(format!(
        "Entity {:02x?} has no identity",
        n.tag
    )))
}
fn vec3(b: &[u8]) -> Result<[f64; 3], SkpError> {
    if b.len() < 24 {
        return Err(SkpError::Invalid("Truncated vector".into()));
    }
    let mut out = [0.; 3];
    for (i, v) in out.iter_mut().enumerate() {
        *v = f64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
        if !v.is_finite() {
            return Err(SkpError::Invalid("Nonfinite coordinate".into()));
        }
    }
    Ok(out)
}
fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
fn all<'a, 'b>(n: &'b Node<'a>, tag: [u8; 2], out: &mut Vec<&'b Node<'a>>) {
    if n.tag == tag {
        out.push(n);
    }
    for c in &n.children {
        all(c, tag, out);
    }
}
fn flags(n: &Node<'_>) -> Result<EntityFlags, SkpError> {
    let draw = n.child([0xd0, 7]);
    Ok(EntityFlags {
        hidden: draw
            .and_then(|n| n.child([0xd3, 7]))
            .and_then(|n| n.body.first())
            .is_some_and(|b| b & 1 != 0),
        display: draw
            .and_then(|n| n.child([0xd3, 7]))
            .and_then(|n| n.body.first())
            .copied()
            .unwrap_or(6),
        layer: draw
            .and_then(|n| n.child([0xd2, 7]))
            .map(|n| uint(n.body))
            .transpose()?,
        material: draw
            .and_then(|n| n.child([0xd1, 7]))
            .map(|n| uint(n.body))
            .transpose()?,
    })
}
fn geometry(n: &Node<'_>, d: &mut Definition) -> Result<(), SkpError> {
    match n.tag {
        [0xc4, 9] => {
            let id = entity_id(n)?;
            let p = n
                .child([0xc5, 9])
                .ok_or_else(|| SkpError::Invalid("Vertex lacks position".into()))?;
            if d.vertices.insert(id, vec3(p.body)?).is_some() {
                return Err(SkpError::Invalid("Duplicate vertex id".into()));
            }
        }
        [0xb8, 0xb] => {
            let id = entity_id(n)?;
            let a = child_uint(n, [0xb9, 0xb])?
                .ok_or_else(|| SkpError::Invalid("Edge lacks first vertex".into()))?;
            let b = child_uint(n, [0xba, 0xb])?
                .ok_or_else(|| SkpError::Invalid("Edge lacks second vertex".into()))?;
            if d.edges
                .insert(
                    id,
                    Edge {
                        vertices: [a, b],
                        flags: flags(n)?,
                        curve: child_uint(n, [0xbb, 0xb])?.filter(|n| *n != 0),
                    },
                )
                .is_some()
            {
                return Err(SkpError::Invalid("Duplicate edge identity".into()));
            }
        }
        [0xac, 0xd] => {
            let id = entity_id(n)?;
            let mut loops = Vec::new();
            if let Some(ae) = n.child([0xae, 0xd]) {
                let mut rings = Vec::new();
                all(ae, [0x94, 0x11], &mut rings);
                for ring in rings {
                    let mut co = Vec::new();
                    all(ring, [0xa0, 0xf], &mut co);
                    let mut edges = Vec::new();
                    for c in co {
                        let sub = raw_nodes(c.body)?;
                        let e = sub
                            .iter()
                            .find(|n| n.tag == [0xa1, 0xf])
                            .ok_or_else(|| SkpError::Invalid("Coedge lacks edge".into()))?;
                        let sense = sub
                            .iter()
                            .find(|n| n.tag == [0xa2, 0xf])
                            .ok_or_else(|| SkpError::Invalid("Coedge lacks sense".into()))?;
                        edges.push((uint(e.body)?, uint(sense.body)? == 0));
                    }
                    if edges.len() < 3 {
                        return Err(SkpError::Invalid(
                            "Face loop shorter than three edges".into(),
                        ));
                    }
                    loops.push(edges);
                }
            }
            if loops.is_empty() {
                return Err(SkpError::Invalid("Face lacks loops".into()));
            }
            if d.faces
                .insert(
                    id,
                    Face {
                        uv_projected: false,
                        uv_projected_back: false,
                        uv_transform: uv_matrix(n, [0x11, 0x27])?,
                        uv_transform_back: uv_matrix(n, [0x12, 0x27])?,
                        normal: n.child([0xad, 0x0d]).map(|n| vec3(n.body)).transpose()?,
                        loops,
                        flags: flags(n)?,
                        back_material: child_uint(n, [0xaf, 0xd])?,
                    },
                )
                .is_some()
            {
                return Err(SkpError::Invalid("Duplicate face identity".into()));
            }
            d.face_order.push(id);
        }
        [0x64, 0x19] => {
            let mut matrix = [0.; 12];
            let c = n
                .child([0x66, 0x19])
                .ok_or_else(|| SkpError::Invalid("Instance lacks transform".into()))?;
            if c.body.len() < 104 {
                return Err(SkpError::Invalid("Truncated instance matrix".into()));
            }
            for (i, x) in matrix.iter_mut().enumerate() {
                *x = f64::from_le_bytes(c.body[i * 8..i * 8 + 8].try_into().unwrap());
                if !x.is_finite() {
                    return Err(SkpError::Invalid("Nonfinite placement matrix".into()));
                }
            }
            d.instances.push(Instance {
                definition: child_uint(n, [0x67, 0x19])?
                    .ok_or_else(|| SkpError::Invalid("Instance lacks definition".into()))?,
                name: n
                    .child([0x65, 0x19])
                    .map(|n| text(n.body))
                    .unwrap_or_default(),
                matrix,
                flags: flags(n)?,
            });
        }
        [0x7c, 0x15] => {}
        [0x90, 0x13] => {
            d.unsupported.insert("image entity".into());
        }
        _ => {
            for c in &n.children {
                geometry(c, d)?;
            }
        }
    }
    Ok(())
}
fn inspect(n: &Node<'_>, p: &mut Payload) -> Result<(), SkpError> {
    match n.tag {
        [0x7c, 0x15] => {
            let id = entity_id(n)?;
            let mut def = Definition {
                name: n
                    .child([0x7e, 0x15])
                    .map(|n| text(n.body))
                    .unwrap_or_default(),
                ..Default::default()
            };
            if let Some(b) = n.child([0x58, 0x1b]) {
                let flags = raw_nodes(b.body)?;
                def.billboard = flags
                    .iter()
                    .find(|n| n.tag == [0x5d, 0x1b])
                    .map(|n| uint(n.body))
                    .transpose()?
                    .is_some_and(|n| n == 1);
            }
            if child_uint(n, [0x83, 0x15])? == Some(2) {
                def.unsupported.insert("image definition".into());
            }
            for c in &n.children {
                geometry(c, &mut def)?;
            }
            if p.definitions.insert(id, def).is_some() {
                return Err(SkpError::Invalid("Duplicate definition id".into()));
            }
        }
        [0xf6, 1] => {
            geometry(n, &mut p.root)?;
        }
        [0x8c, 0x3c] => {
            let id = entity_id(n)?;
            let name = n
                .child([0x8d, 0x3c])
                .map(|n| text(n.body))
                .ok_or_else(|| SkpError::Invalid("Layer name missing".into()))?;
            let visible = !n
                .child([0x8e, 0x3c])
                .and_then(|n| n.body.first())
                .is_some_and(|b| *b == 1);
            if p.layers.insert(id, LayerRecord { name, visible }).is_some() {
                return Err(SkpError::Invalid("Duplicate layer identity".into()));
            }
        }
        [0xc8, 0x32] => {
            let id = entity_id(n)?;
            let name = n
                .child([0xcc, 0x32])
                .map(|n| text(n.body))
                .ok_or_else(|| SkpError::Invalid("Material name missing".into()))?;
            if p.material_names.insert(id, name).is_some() {
                return Err(SkpError::Invalid("Duplicate material identity".into()));
            }
        }
        [7, 2] => {
            for a in raw_nodes(n.body)? {
                if a.tag == [0x60, 0x6d] {
                    for b in raw_nodes(a.body)? {
                        if b.tag == [0x61, 0x6d]
                            && raw_nodes(b.body)?.iter().any(|n| n.tag == [0x48, 0x71])
                        {
                            p.unsupported.insert("saved scenes".into());
                        }
                    }
                }
            }
        }
        _ => {}
    }
    for c in &n.children {
        inspect(c, p)?;
    }
    Ok(())
}
pub(super) fn parse(bytes: &[u8], name: &str) -> Result<Payload, SkpError> {
    if bytes.len() > MAX_FILE {
        return Err(SkpError::Invalid(
            "SKP file admission exceeds 512MiB".into(),
        ));
    }
    if !bytes.starts_with(&[0xff, 0xfe, 0xff, 0x0e]) {
        return Err(SkpError::Invalid("Not a SketchUp container".into()));
    }
    let Some(offset) = bytes[..bytes.len().min(4096)]
        .windows(4)
        .position(|s| s == b"PK\x03\x04")
    else {
        return legacy::parse(bytes, name);
    };
    let header = &bytes[..offset];
    let decoded = String::from_utf16_lossy(
        &header
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect::<Vec<_>>(),
    );
    let version = decoded
        .find('{')
        .and_then(|a| {
            decoded[a..]
                .find('}')
                .map(|b| decoded[a..=a + b].to_owned())
        })
        .unwrap_or_else(|| "unknown".into());
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes[offset..]))
        .map_err(|e| SkpError::Invalid(e.to_string()))?;
    if zip.len() > 100_000 {
        return Err(SkpError::Invalid("SKP ZIP entry count exceeded".into()));
    }
    let archive_names = zip.file_names().map(str::to_owned).collect::<Vec<_>>();
    if archive_names
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != archive_names.len()
    {
        return Err(SkpError::Invalid("Duplicate archive identity".into()));
    }
    let mut total = 0u64;
    let mut model = None;
    let mut p = Payload {
        name: name.into(),
        version,
        ..Default::default()
    };
    let mut retained_texture_bytes = 0u64;
    let mut material_folders=HashMap::new();
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| SkpError::Invalid(e.to_string()))?;
        let filename = entry.name().to_owned();
        if entry.enclosed_name().is_none() {
            return Err(SkpError::Invalid("Invalid archive path".into()));
        }
        total = total
            .checked_add(entry.size())
            .filter(|&n| n <= MAX_FILE as u64)
            .ok_or_else(|| SkpError::Invalid("Decompressed archive admission exceeded".into()))?;
        if filename == "model.dat" {
            if entry.size() > MAX_MODEL as u64 || model.is_some() {
                return Err(SkpError::Invalid("Invalid model.dat admission".into()));
            }
            let mut data = Vec::new();
            entry.take(MAX_MODEL as u64 + 1).read_to_end(&mut data)?;
            if data.len() > MAX_MODEL {
                return Err(SkpError::Invalid("model.dat expansion exceeded".into()));
            }
            model = Some(data);
        } else if filename.starts_with("materials/") && filename.ends_with("material.xml") {
            if entry.size() > 1024 * 1024 {
                return Err(SkpError::Invalid("Material XML exceeds limit".into()));
            }
            let mut xml = String::new();
            entry.read_to_string(&mut xml)?;
            let doc =
                roxmltree::Document::parse(&xml).map_err(|e| SkpError::Invalid(e.to_string()))?;
            let m = doc
                .descendants()
                .find(|n| n.tag_name().name() == "material")
                .ok_or_else(|| SkpError::Invalid("Material XML lacks recipe".into()))?;
            let mat_name = m.attribute("name").unwrap_or("unknown").to_owned();
            if let Some(folder)=filename.split('/').nth(1){material_folders.insert(folder.to_owned(),mat_name.clone());}
            let mut rgba = [0.; 4];
            for (i, attr) in ["colorRed", "colorGreen", "colorBlue"].iter().enumerate() {
                let c = m
                    .attribute(*attr)
                    .unwrap_or("128")
                    .parse::<u8>()
                    .map_err(|_| SkpError::Invalid("Material RGB invalid".into()))?;
                rgba[i] = c as f64 / 255.;
            }
            rgba[3] = if m.attribute("useTrans") == Some("1") {
                1. - m
                    .attribute("trans")
                    .unwrap_or("0")
                    .parse::<f64>()
                    .map_err(|_| SkpError::Invalid("Material transparency invalid".into()))?
                    .clamp(0., 1.)
            } else {
                1.
            };
            if !rgba[3].is_finite() {
                return Err(SkpError::Invalid("Material alpha nonfinite".into()));
            }
            drop(entry);
            let texture = if let Some(t) =
                m.descendants().find(|n| n.tag_name().name() == "texture")
            {
                if m.attribute("type") == Some("2") {
                    p.unsupported.insert("material tint/colorization".into());
                    None
                } else {
                    let folder = filename.rsplit_once('/').map(|x| x.0).unwrap_or("");
                    let filename = t.attribute("textureFilename").unwrap_or("");
                    let exact = format!("{folder}/{filename}");
                    let mut found = archive_names
                        .iter()
                        .find(|n| **n == exact && !filename.is_empty())
                        .cloned();
                    if found.is_none() {
                        found = archive_names
                            .iter()
                            .find(|n| {
                                n.starts_with(&format!("{folder}/"))
                                    && !n.to_ascii_lowercase().ends_with(".xml")
                            })
                            .cloned();
                    }
                    if found.is_none() {
                        if let Some(img) = t.descendants().find(|n| n.tag_name().name() == "image")
                        {
                            let path = img
                                .attribute("path")
                                .unwrap_or("")
                                .trim_start_matches(['.', '/']);
                            for candidate in [path.to_owned(), format!("{folder}/{path}")] {
                                if archive_names.contains(&candidate) {
                                    found = Some(candidate);
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(asset) = found {
                        let entry = zip
                            .by_name(&asset)
                            .map_err(|e| SkpError::Invalid(e.to_string()))?;
                        if entry.size() == 0 {
                            None
                        } else {
                            if entry.size() > 16 * 1024 * 1024 {
                                return Err(SkpError::Invalid(
                                    "Texture asset exceeds 16MiB admission".into(),
                                ));
                            }
                            retained_texture_bytes = retained_texture_bytes
                                .checked_add(entry.size())
                                .filter(|&bytes| bytes <= 256 * 1024 * 1024)
                                .ok_or_else(|| {
                                    SkpError::Invalid(
                                        "Texture aggregate retained bytes exceed 256MiB".into(),
                                    )
                                })?;
                            let mut data = Vec::new();
                            entry.take(16 * 1024 * 1024 + 1).read_to_end(&mut data)?;
                            if data.len() > 16 * 1024 * 1024 {
                                return Err(SkpError::Invalid(
                                    "Texture expansion exceeds admission".into(),
                                ));
                            }
                            match image::guess_format(&data) {
                                Ok(image::ImageFormat::Png | image::ImageFormat::Jpeg) => {}
                                _ => {
                                    return Err(SkpError::Unsupported(
                                        "texture image codec beyond PNG/JPEG".into(),
                                    ))
                                }
                            }
                            let reader = image::ImageReader::new(Cursor::new(&data))
                                .with_guessed_format()
                                .map_err(SkpError::Io)?;
                            let (w, h) = reader
                                .into_dimensions()
                                .map_err(|e| SkpError::Invalid(e.to_string()))?;
                            if w == 0 || h == 0 || w as u64 * h as u64 > 64 * 1024 * 1024 {
                                return Err(SkpError::Invalid(
                                    "Texture dimensions exceed admission".into(),
                                ));
                            }
                            let width = t.attribute("xScale").unwrap_or("0").parse().unwrap_or(0.);
                            let height = t.attribute("yScale").unwrap_or("0").parse().unwrap_or(0.);
                            Some(TextureRecord {
                                archive_name: asset,
                                bytes: data,
                                width,
                                height,
                            })
                        }
                    } else {
                        // Pinned source falls through to the declared RGB recipe.
                        None
                    }
                }
            } else {
                None
            };
            if p.materials
                .insert(
                    mat_name.clone(),
                    MaterialRecord {
                        color: rgba,
                        texture,
                    },
                )
                .is_some()
            {
                return Err(SkpError::Invalid("Duplicate material recipe".into()));
            }
        } else if p.back_color.is_none() && filename.starts_with("styles/") && filename.ends_with("style.xml") {
            if entry.size() > 1024 * 1024 {
                return Err(SkpError::Invalid("Style XML exceeds limit".into()));
            }
            let mut xml = String::new();
            entry.read_to_string(&mut xml)?;
            let doc =
                roxmltree::Document::parse(&xml).map_err(|e| SkpError::Invalid(e.to_string()))?;
            if let Some(item) = doc
                .descendants()
                .find(|n| n.tag_name().name() == "item" && n.attribute("id") == Some("4001"))
            {
                if let Some(v) = item
                    .descendants()
                    .find(|n| n.tag_name().name() == "variant")
                    .and_then(|n| n.text())
                {
                    if let Ok(v) = v.parse::<i64>() {
                        p.back_color = Some([
                            ((v >> 16) & 255) as f64 / 255.,
                            ((v >> 8) & 255) as f64 / 255.,
                            (v & 255) as f64 / 255.,
                        ]);
                    }
                }
            }
        }
    }
    let data = model.ok_or_else(|| SkpError::Invalid("Missing model.dat".into()))?;
    let top=nodes(&data,0,&mut 0)?;
    p.legacy_dimensions=annotations::dimensions(&data,&top)?;
    for n in &top {
        inspect(n, &mut p)?;
    }
    for name in p.material_names.values_mut(){
        if !p.materials.contains_key(name){if let Some(actual)=material_folders.get(name){*name=actual.clone();}}
    }
    p.layers.entry(1).or_insert_with(|| LayerRecord {
        name: "Layer0".into(),
        visible: true,
    });
    Ok(p)
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    fn leaf(tag: [u8; 2], body: &[u8]) -> Node<'_> {
        Node {
            tag,
            body,
            children: vec![],
        }
    }
    #[test]
    fn duplicate_record_identities_refuse() {
        let id = [1u8];
        let a = [2u8];
        let b = [3u8];
        let edge = Node {
            tag: [0xb8, 0x0b],
            body: &[],
            children: vec![
                leaf([0xde, 5], &id),
                leaf([0xb9, 0x0b], &a),
                leaf([0xba, 0x0b], &b),
            ],
        };
        let mut d = Definition::default();
        geometry(&edge, &mut d).unwrap();
        assert!(geometry(&edge, &mut d).is_err());
        let co = [0xa1, 0x0f, 1, 0, 0, 0, 1, 0xa2, 0x0f, 1, 0, 0, 0, 0];
        let face = Node {
            tag: [0xac, 0x0d],
            body: &[],
            children: vec![
                leaf([0xde, 5], &id),
                Node {
                    tag: [0xae, 0x0d],
                    body: &[],
                    children: vec![Node {
                        tag: [0x94, 0x11],
                        body: &[],
                        children: vec![
                            leaf([0xa0, 0x0f], &co),
                            leaf([0xa0, 0x0f], &co),
                            leaf([0xa0, 0x0f], &co),
                        ],
                    }],
                },
            ],
        };
        let mut d = Definition::default();
        geometry(&face, &mut d).unwrap();
        assert!(geometry(&face, &mut d).is_err());
        let mut p = Payload::default();
        let layer = Node {
            tag: [0x8c, 0x3c],
            body: &[],
            children: vec![leaf([0xde, 5], &id), leaf([0x8d, 0x3c], b"Layer")],
        };
        inspect(&layer, &mut p).unwrap();
        assert!(inspect(&layer, &mut p).is_err());
        let material = Node {
            tag: [0xc8, 0x32],
            body: &[],
            children: vec![leaf([0xde, 5], &id), leaf([0xcc, 0x32], b"Paint")],
        };
        inspect(&material, &mut p).unwrap();
        assert!(inspect(&material, &mut p).is_err());
    }
}
