//! Default local-plane source texture mapping; positioned/projected/tint stays explicit.
use super::*;
pub(super) fn tile_size(w: f64, h: f64) -> (f64, f64) {
    let sane = |x: f64| x.is_finite() && x > 1e-6 && x < 1e5;
    let (w, h) = match (sane(w), sane(h)) {
        (true, true) => (w, h),
        (true, false) => (w, w),
        (false, true) => (h, h),
        (false, false) => (1. / 0.0254, 1. / 0.0254),
    };
    (w * 0.0254, h * 0.0254)
}
fn basis(n: [f64; 3]) -> (DVec3, DVec3) {
    let mut n = DVec3::from_array(n);
    let len = n.length();
    if len > 1e-30 {
        n /= len;
    }
    let mut x = DVec3::new(-n.y, n.x, 0.);
    let len = x.length();
    if len < 1e-3 {
        if n.z > 0. {
            (DVec3::X, DVec3::Y)
        } else {
            (-DVec3::X, DVec3::Y)
        }
    } else {
        x /= len;
        (x, n.cross(x))
    }
}
fn fit(points: &[DVec3], uv: &[[f64; 2]]) -> Option<[f64; 8]> {
    let mut best = 1e-24;
    let mut pair = None;
    for i in 1..points.len() {
        for j in i + 1..points.len() {
            let a = points[i] - points[0];
            let b = points[j] - points[0];
            let area = a.cross(b).length_squared();
            if area > best {
                best = area;
                pair = Some((i, j, a, b));
            }
        }
    }
    let (i, j, a, b) = pair?;
    let g11 = a.length_squared();
    let g12 = a.dot(b);
    let g22 = b.length_squared();
    let det = g11 * g22 - g12 * g12;
    if det.abs() < 1e-18 {
        return None;
    }
    let mut out = [0.; 8];
    for k in 0..2 {
        let d1 = uv[i][k] - uv[0][k];
        let d2 = uv[j][k] - uv[0][k];
        let g = a * ((d1 * g22 - d2 * g12) / det) + b * ((d2 * g11 - d1 * g12) / det);
        out[k * 4] = g.x;
        out[k * 4 + 1] = g.y;
        out[k * 4 + 2] = g.z;
        out[k * 4 + 3] = uv[0][k] - g.dot(points[0]);
    }
    out.iter().all(|x| x.is_finite()).then_some(out)
}
fn bake_entry(
    entry: &mut serde_json::Value,
    raw: &[[f64; 3]],
    world: &[DVec3],
    normal: [f64; 3],
) -> Result<(), SkpError> {
    let Some(tex) = entry.get_mut("texture") else {
        return Ok(());
    };
    if raw.len() > 13 {
        return Err(SkpError::Unsupported(
            "large textured polygon NumPy Gram tie semantics".into(),
        ));
    }
    let sw = tex["sw"]
        .as_f64()
        .ok_or_else(|| SkpError::Invalid("Texture tile missing".into()))?
        / 0.0254;
    let sh = tex["sh"]
        .as_f64()
        .ok_or_else(|| SkpError::Invalid("Texture tile missing".into()))?
        / 0.0254;
    let (x, y) = basis(normal);
    let uv = raw
        .iter()
        .map(|v| {
            let v = DVec3::from_array(*v);
            [v.dot(x) / sw, v.dot(y) / sh]
        })
        .collect::<Vec<_>>();
    if let Some(map) = fit(world, &uv) {
        tex["uvw"] = serde_json::json!(map);
        tex["planar"] = serde_json::json!(true);
    }
    Ok(())
}
pub(super) fn bake(
    attrs: &mut HashMap<String, serde_json::Value>,
    face: &Face,
    d: &Definition,
    outer: &[usize],
    mesh: &Mesh,
    flip: bool,
) -> Result<(), SkpError> {
    if !attrs.contains_key("texture")
        && !attrs
            .get("back")
            .is_some_and(|b| b.get("texture").is_some())
    {
        return Ok(());
    }
    let mut raw = ring(d, &face.loops[0])?;
    if flip {
        raw.reverse();
    }
    let world = outer
        .iter()
        .map(|&i| mesh.vertices[i].position)
        .collect::<Vec<_>>();
    let n = face.normal.unwrap_or([0., 0., 1.]);
    let mut entry = serde_json::to_value(&*attrs).map_err(|e| SkpError::Invalid(e.to_string()))?;
    bake_entry(&mut entry, &raw, &world, n)?;
    if let Some(back) = entry.get_mut("back") {
        if back.is_object() {
            bake_entry(back, &raw, &world, n)?;
        }
    }
    *attrs = serde_json::from_value(entry).map_err(|e| SkpError::Invalid(e.to_string()))?;
    Ok(())
}

/// Admit neutral payload assets before constructing any scene resources.
pub(super) fn admit_assets(payload: &Payload) -> Result<(), SkpError> {
    let mut identities = std::collections::HashMap::<&str, &[u8]>::new();
    let mut retained_bytes = 0usize;
    for texture in payload
        .materials
        .values()
        .filter_map(|m| m.texture.as_ref())
    {
        retained_bytes = retained_bytes
            .checked_add(texture.bytes.len())
            .ok_or_else(|| SkpError::Invalid("Texture aggregate admission overflow".into()))?;
        if retained_bytes > 256 * 1024 * 1024 {
            return Err(SkpError::Invalid(
                "Texture aggregate retained bytes exceed 256MiB".into(),
            ));
        }
        if texture.archive_name.is_empty() || texture.bytes.len() > 16 * 1024 * 1024 {
            return Err(SkpError::Invalid(
                "Texture identity or byte admission failed".into(),
            ));
        }
        if let Some(previous) = identities.insert(&texture.archive_name, &texture.bytes) {
            if previous != texture.bytes.as_slice() {
                return Err(SkpError::Invalid(
                    "Conflicting embedded texture identity".into(),
                ));
            }
            continue;
        }
        let format = image::guess_format(&texture.bytes)
            .map_err(|_| SkpError::Invalid("Texture image header invalid".into()))?;
        if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
            return Err(SkpError::Unsupported("Texture image codec".into()));
        }
        let (width, height) =
            image::ImageReader::with_format(std::io::Cursor::new(&texture.bytes), format)
                .into_dimensions()
                .map_err(|_| SkpError::Invalid("Texture dimensions invalid".into()))?;
        if width == 0 || height == 0 || width as u64 * height as u64 > 64 * 1024 * 1024 {
            return Err(SkpError::Invalid("Texture pixel admission failed".into()));
        }
        image::ImageReader::with_format(std::io::Cursor::new(&texture.bytes), format)
            .decode()
            .map_err(|_| SkpError::Invalid("Texture pixel payload invalid".into()))?;
    }
    Ok(())
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    fn image() -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }
    fn material(bytes: Vec<u8>) -> MaterialRecord {
        MaterialRecord {
            color: [1.; 4],
            texture: Some(TextureRecord {
                archive_name: "materials/Tile/tile.png".into(),
                bytes,
                width: 10.,
                height: 20.,
            }),
        }
    }
    #[test]
    fn assets_admit_equal_identity_and_refuse_conflicting_or_invalid_bytes() {
        let mut payload = Payload::default();
        payload.materials.insert("one".into(), material(image()));
        payload
            .materials
            .insert("two".into(), payload.materials["one"].clone());
        assert!(admit_assets(&payload).is_ok());
        payload
            .materials
            .get_mut("two")
            .unwrap()
            .texture
            .as_mut()
            .unwrap()
            .bytes
            .push(1);
        assert!(
            matches!(admit_assets(&payload), Err(SkpError::Invalid(s)) if s.contains("Conflicting"))
        );
        payload.materials.remove("two");
        let mut truncated = image();
        truncated.truncate(41);
        payload
            .materials
            .get_mut("one")
            .unwrap()
            .texture
            .as_mut()
            .unwrap()
            .bytes = truncated;
        assert!(
            matches!(admit_assets(&payload), Err(SkpError::Invalid(s)) if s.contains("payload"))
        );
        payload.materials.remove("two");
        payload
            .materials
            .get_mut("one")
            .unwrap()
            .texture
            .as_mut()
            .unwrap()
            .bytes = vec![0; 24];
        assert!(
            matches!(admit_assets(&payload), Err(SkpError::Invalid(s)) if s.contains("header"))
        );
    }
    #[test]
    fn tile_fallback_and_normal_basis_match_source() {
        assert_eq!(tile_size(0., 0.), (1., 1.));
        assert_eq!(tile_size(1., 1e-231), (0.0254, 0.0254));
        assert_eq!(basis([0., 0., -1.]), (-DVec3::X, DVec3::Y));
        assert_eq!(basis([0., -1., 0.]), (DVec3::X, DVec3::Z));
    }
}
