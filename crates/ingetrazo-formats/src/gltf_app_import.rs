//! Original App policy kept separate from the direct reference loader.
use super::*;
use serde_json::Value;
use std::{collections::HashMap, io::Read};
pub(super) struct Input {
    pub bytes: Vec<u8>,
    pub json: Value,
}
pub(super) struct SourceFace {
    pub material: Option<usize>,
    pub uv: Option<[DVec2; 3]>,
}
pub(super) fn read_source_json(path: &Path) -> Result<Input, GltfError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(invalid("glTF encoded input admission"));
    }
    let json = if bytes.starts_with(b"glTF") {
        if bytes.len() < 20 {
            return Err(invalid("Truncated GLB header"));
        }
        let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if &bytes[16..20] != b"JSON" || n.checked_add(20).is_none_or(|n| n > bytes.len()) {
            return Err(invalid("Invalid GLB JSON chunk"));
        }
        serde_json::from_slice(&bytes[20..20 + n]).map_err(|e| invalid(e.to_string()))?
    } else {
        serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?
    };
    Ok(Input { bytes, json })
}
fn numbers<const N: usize>(v: Option<&Value>, default: [f64; N]) -> Result<[f64; N], GltfError> {
    let Some(v) = v else { return Ok(default) };
    let a = v
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| invalid("Invalid glTF numeric vector"))?;
    let mut out = [0.; N];
    for i in 0..N {
        out[i] = a[i]
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| invalid("Nonfinite glTF numeric field"))?;
    }
    Ok(out)
}
pub(super) fn node_matrix(source: &Value, index: usize) -> Result<DMat4, GltfError> {
    let n = &source["nodes"][index];
    if n.get("matrix").is_some() {
        return Ok(DMat4::from_cols_array(&numbers(
            n.get("matrix"),
            DMat4::IDENTITY.to_cols_array(),
        )?));
    }
    Ok(DMat4::from_scale_rotation_translation(
        DVec3::from_array(numbers(n.get("scale"), [1.; 3])?),
        glam::DQuat::from_array(numbers(n.get("rotation"), [0., 0., 0., 1.])?),
        DVec3::from_array(numbers(n.get("translation"), [0.; 3])?),
    ))
}
pub(super) fn transform_uv(
    source: &Value,
    material: Option<usize>,
    uv: &mut [DVec2; 3],
) -> Result<(), GltfError> {
    let m = material.map(|i| &source["materials"][i]);
    let info = m.and_then(|m| {
        m.pointer("/extensions/KHR_materials_pbrSpecularGlossiness/diffuseTexture")
            .or_else(|| m.pointer("/pbrMetallicRoughness/baseColorTexture"))
    });
    if let Some(t) = info.and_then(|i| i.pointer("/extensions/KHR_texture_transform")) {
        let o = DVec2::from_array(numbers(t.get("offset"), [0.; 2])?);
        let s = DVec2::from_array(numbers(t.get("scale"), [1.; 2])?);
        for p in uv {
            *p = *p * s + o;
        }
    }
    Ok(())
}
pub(super) fn finish(
    mesh: &mut Mesh,
    faces: &HashMap<usize, SourceFace>,
    source: &Value,
    _assets: &HashMap<String, Vec<u8>>,
) -> Result<(), GltfError> {
    if mesh.faces.is_empty() {
        return Err(invalid("glTF has no triangles"));
    }
    let bounds = mesh.vertices.iter().fold(
        (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
        |(lo, hi), v| (lo.min(v.position), hi.max(v.position)),
    );
    let size = (bounds.1 - bounds.0).max_element();
    let scale = [1., 0.0254, 0.01, 0.001]
        .into_iter()
        .find(|s| (0.2..=40.).contains(&(size * s)))
        .unwrap_or(1.);
    // Raw triangles have never been welded. Apply scale/f32 before first weld.
    let mut prepared = Mesh::new();
    prepared
        .vertices
        .try_reserve(mesh.vertices.len())
        .map_err(|_| invalid("glTF vertex allocation refused"))?;
    for (i, face) in mesh.faces.iter().enumerate() {
        if face.vertices.len() != 3 {
            return Err(invalid("glTF nontriangle"));
        }
        let points: [DVec3; 3] = std::array::from_fn(|k| {
            (mesh.vertices[face.vertices[k]].position * scale)
                .as_vec3()
                .as_dvec3()
        });
        let ids = points.map(|p| prepared.add_vertex(p));
        let Some(id) = prepared.add_face(ids.to_vec(), vec![]) else {
            continue;
        };
        let info = faces
            .get(&i)
            .ok_or_else(|| invalid("Missing glTF face provenance"))?;
        let m = info.material.map(|i| &source["materials"][i]);
        let factor = m.and_then(|m| {
            m.pointer("/extensions/KHR_materials_pbrSpecularGlossiness/diffuseFactor")
                .or_else(|| m.pointer("/pbrMetallicRoughness/baseColorFactor"))
        });
        let rgba = numbers(factor, [1.; 4])?;
        let attrs = &mut prepared.faces[id].attrs;
        if let Some(texture) = face.attrs.get("texture") {
            let uv = info
                .uv
                .ok_or_else(|| invalid("Missing glTF UV provenance"))?;
            let e1 = points[1] - points[0];
            let e2 = points[2] - points[0];
            let a = DMat3::from_cols(e1, e2, e1.cross(e2)).transpose();
            if a.determinant().abs() > 1e-18 {
                let gu = a.inverse() * DVec3::new(uv[1].x - uv[0].x, uv[2].x - uv[0].x, 0.);
                let gv = a.inverse() * DVec3::new(uv[1].y - uv[0].y, uv[2].y - uv[0].y, 0.);
                attrs.insert("texture".into(),serde_json::json!({"embed":texture["embed"],"uvw":[gu.x,gu.y,gu.z,uv[0].x-gu.dot(points[0]),gv.x,gv.y,gv.z,uv[0].y-gv.dot(points[0])],"sw":if gu.length()>1e-9{1./gu.length()}else{1.},"sh":if gv.length()>1e-9{1./gv.length()}else{1.}}));
                attrs.insert("back".into(), true.into());
            }
        } else if rgba[..3] != [1., 1., 1.] {
            attrs.insert("color".into(), serde_json::json!(&rgba[..3]));
            attrs.insert("back".into(), true.into());
        }
        if m.and_then(|m| m.get("alphaMode")).and_then(Value::as_str) == Some("BLEND") {
            attrs.insert(
                "opacity".into(),
                serde_json::json!(if rgba[3] == 1. { 0.999 } else { rgba[3] }),
            );
        }
    }
    crate::obj::obj_source_soften(&mut prepared);
    *mesh = prepared;
    Ok(())
}
