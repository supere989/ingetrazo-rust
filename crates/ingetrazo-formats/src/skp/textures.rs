//! Local-plane, positioned/projected mappings and bounded legacy source tinting.
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
fn positioned(
    raw: &[[f64; 3]],
    normal: [f64; 3],
    matrix: [f64; 9],
    sw: f64,
    sh: f64,
    projected: bool,
) -> Option<Vec<[f64; 2]>> {
    let m = [&matrix[0..3], &matrix[3..6], &matrix[6..9]];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-15 {
        return None;
    }
    let inv = [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / det,
        ],
    ];
    let (x, y) = basis(normal);
    raw.iter()
        .map(|p| {
            let p = DVec3::from_array(*p);
            let (x2, y2) = if projected {
                (p.x, p.y)
            } else {
                (p.dot(x), p.dot(y))
            };
            let u = x2 * inv[0][0] + y2 * inv[1][0] + inv[2][0];
            let v = x2 * inv[0][1] + y2 * inv[1][1] + inv[2][1];
            let q = x2 * inv[0][2] + y2 * inv[1][2] + inv[2][2];
            if q.abs() < 1e-12 {
                None
            } else {
                Some([u / q / sw, v / q / sh])
            }
        })
        .collect()
}
// Python round uses decimal rounding of the binary value, rather than scaling
// first (which can create new ties). Rust's fixed precision formatter does likewise.
fn decimal_key(value: f64, digits: usize) -> u64 {
    let rounded = format!("{value:.digits$}")
        .parse::<f64>()
        .expect("finite decimal");
    if rounded == 0. {
        0
    } else {
        rounded.to_bits()
    }
}
pub(super) fn projected_faces(p: &Payload, d: &Definition) -> Result<HashSet<u64>, SkpError> {
    let mut groups = BTreeMap::<(u64, [u64; 9]), Vec<(u64, &Face)>>::new();
    let order = if d.face_order.is_empty() {
        d.faces.keys().copied().collect::<Vec<_>>()
    } else {
        d.face_order.clone()
    };
    if order.len() != d.faces.len()
        || order.iter().copied().collect::<HashSet<_>>().len() != order.len()
    {
        return Err(SkpError::Invalid("Invalid face ordering metadata".into()));
    }
    for id in order {
        let f = d
            .faces
            .get(&id)
            .ok_or_else(|| SkpError::Invalid("Unknown ordered face".into()))?;
        let (Some(material), Some(matrix)) = (f.flags.material, f.uv_transform) else {
            continue;
        };
        if p.material_names
            .get(&material)
            .and_then(|n| p.materials.get(n))
            .is_none_or(|m| m.texture.is_none())
        {
            continue;
        }
        groups
            .entry((material, matrix.map(|v| decimal_key(v, 1))))
            .or_default()
            .push((id, f));
    }
    let mut result = HashSet::new();
    for ((material, _), faces) in groups {
        if faces.len() < 8 {
            continue;
        }
        let tex = p
            .material_names
            .get(&material)
            .and_then(|n| p.materials.get(n))
            .and_then(|m| m.texture.as_ref())
            .unwrap();
        let (sw, sh) = tile_size(tex.width, tex.height);
        let discontinuity = |projected| -> Result<Option<f64>, SkpError> {
            let mut at = BTreeMap::<[u64; 3], BTreeSet<[u64; 2]>>::new();
            for (_, face) in faces.iter().take(80) {
                let raw = ring(d, &face.loops[0])?;
                let Some(uv) = positioned(
                    &raw,
                    face.normal.unwrap_or([0., 0., 1.]),
                    face.uv_transform.unwrap(),
                    sw / 0.0254,
                    sh / 0.0254,
                    projected,
                ) else {
                    return Ok(None);
                };
                for (point, uv) in raw.iter().zip(uv) {
                    at.entry(point.map(|v| decimal_key(v, 1)))
                        .or_default()
                        .insert(uv.map(|v| decimal_key(v, 2)));
                }
            }
            if at.is_empty() {
                Ok(None)
            } else {
                Ok(Some(
                    at.values().filter(|v| v.len() > 1).count() as f64 / at.len() as f64,
                ))
            }
        };
        if matches!((discontinuity(false)?,discontinuity(true)?),(Some(local),Some(plan)) if local>0.3 && plan<0.1)
        {
            result.extend(faces.iter().map(|(id, _)| *id));
        }
    }
    Ok(result)
}
fn bake_entry(
    entry: &mut serde_json::Value,
    raw: &[[f64; 3]],
    world: &[DVec3],
    normal: [f64; 3],
    matrix: Option<[f64; 9]>,
    projected: bool,
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
    let uv = if let Some(matrix) = matrix {
        let Some(uv) = positioned(raw, normal, matrix, sw, sh, projected) else {
            return Ok(());
        };
        uv
    } else {
        let (x, y) = basis(normal);
        raw.iter()
            .map(|v| {
                let v = DVec3::from_array(*v);
                [v.dot(x) / sw, v.dot(y) / sh]
            })
            .collect()
    };
    if let Some(map) = fit(world, &uv) {
        tex["uvw"] = serde_json::json!(map);
        if matrix.is_none() {
            tex["planar"] = serde_json::json!(true);
        }
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
    projected: bool,
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
    bake_entry(
        &mut entry,
        &raw,
        &world,
        n,
        if flip {
            face.uv_transform_back
        } else {
            face.uv_transform
        },
        if flip {
            face.uv_projected_back
        } else {
            face.uv_projected || projected
        },
    )?;
    if let Some(back) = entry.get_mut("back") {
        if back.is_object() {
            bake_entry(
                back,
                &raw,
                &world,
                n,
                face.uv_transform_back,
                face.uv_projected_back,
            )?;
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
    fn positioned_homogeneous_and_singular_source_boundaries() {
        let raw = [[0., 0., 0.], [10., 0., 0.], [10., 10., 0.]];
        let identity = [1., 0., 0., 0., 1., 0., 0., 0., 1.];
        assert_eq!(
            positioned(&raw, [0., 0., 1.], identity, 10., 20., false).unwrap(),
            vec![[0., 0.], [1., 0.], [1., 0.5]]
        );
        assert!(positioned(&raw, [0., 0., 1.], [0.; 9], 10., 20., false).is_none());
        let horizon = [1., 0., -0.1, 0., 1., 0., 0., 0., 1.];
        // Inverse q=1+.1*x; x=-10 is the projective horizon.
        assert!(positioned(&[[-10., 0., 0.]], [0., 0., 1.], horizon, 10., 20., false).is_none());
        assert_eq!(decimal_key(-0.01, 1), decimal_key(0., 1));
        assert_eq!(f64::from_bits(decimal_key(2.675, 2)), 2.67);
    }
    #[test]
    fn tile_fallback_and_normal_basis_match_source() {
        assert_eq!(tile_size(0., 0.), (1., 1.));
        assert_eq!(tile_size(1., 1e-231), (0.0254, 0.0254));
        assert_eq!(basis([0., 0., -1.]), (-DVec3::X, DVec3::Y));
        assert_eq!(basis([0., -1., 0.]), (DVec3::X, DVec3::Z));
    }
}

// Frozen core.texture.colorize_image: HLS in float64, visibility mean,
// half-up channel quantization, alpha unchanged. No fitted colour table.
fn hls(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lum = (max + min) / 2.;
    let delta = max - min;
    if delta <= 1e-12 {
        return [0., lum, 0.];
    }
    let den = if lum <= 0.5 {
        max + min
    } else {
        2. - max - min
    };
    let sat = if den > 1e-12 { delta / den } else { 0. };
    let rc = (max - r) / delta;
    let gc = (max - g) / delta;
    let bc = (max - b) / delta;
    let hue = if max == r {
        bc - gc
    } else if max == g {
        2. + rc - bc
    } else {
        4. + gc - rc
    };
    [(hue / 6.).rem_euclid(1.), lum, sat]
}
fn from_hls([h, l, s]: [f64; 3]) -> [f64; 3] {
    let m2 = if l <= 0.5 {
        l * (1. + s)
    } else {
        l + s - l * s
    };
    let m1 = 2. * l - m2;
    let channel = |h: f64| {
        let h = h.rem_euclid(1.);
        if h < 1. / 6. {
            m1 + (m2 - m1) * h * 6.
        } else if h < 0.5 {
            m2
        } else if h < 2. / 3. {
            m1 + (m2 - m1) * (2. / 3. - h) * 6.
        } else {
            m1
        }
    };
    [channel(h + 1. / 3.), channel(h), channel(h - 1. / 3.)]
}
// Source default FastTransformation scales to 16x16 before alpha-weighted
// guard. This remains independently checked against actual QImage sampling.
fn needs_tint(image: &image::RgbaImage, target: [u8; 3]) -> bool {
    let mut sum = [0_u64; 3];
    let mut alpha = 0_u64;
    for y in 0..16_u64 {
        for x in 0..16_u64 {
            let sx = ((2 * x + 1) * image.width() as u64 / 32).min(image.width() as u64 - 1) as u32;
            let sy =
                ((2 * y + 1) * image.height() as u64 / 32).min(image.height() as u64 - 1) as u32;
            let p = image.get_pixel(sx, sy).0;
            for c in 0..3 {
                sum[c] += p[c] as u64 * p[3] as u64;
            }
            alpha += p[3] as u64;
        }
    }
    alpha != 0 && (0..3).any(|c| (sum[c] as f64 / alpha as f64 - target[c] as f64).abs() > 24.)
}
pub(super) fn tint_legacy(
    texture: &TextureRecord,
    target: [u8; 3],
) -> Result<Option<Vec<u8>>, SkpError> {
    let format = image::guess_format(&texture.bytes)
        .map_err(|_| SkpError::Invalid("Legacy tint image header".into()))?;
    let reader = || image::ImageReader::with_format(std::io::Cursor::new(&texture.bytes), format);
    let (w, h) = reader()
        .into_dimensions()
        .map_err(|_| SkpError::Invalid("Legacy tint dimensions".into()))?;
    if w == 0 || h == 0 || w as u64 * h as u64 > 64 * 1024 * 1024 {
        return Err(SkpError::Invalid("Legacy tint pixel admission".into()));
    }
    let mut decoded = reader()
        .decode()
        .map_err(|_| SkpError::Invalid("Legacy tint pixel payload".into()))?
        .into_rgba8();
    if !needs_tint(&decoded, target) {
        return Ok(None);
    }
    // Pixel storage is reused in-place; no float image copies retained.
    // Two passes avoid a per-pixel float64 array as in the reference.
    let mut visible = false;
    let mut sum = [0.; 3];
    let mut n = 0usize;
    for p in decoded.pixels() {
        if p[3] != 0 {
            visible = true;
            break;
        }
    }
    for p in decoded.pixels().filter(|p| !visible || p[3] != 0) {
        for c in 0..3 {
            sum[c] += p[c] as f64 / 255.;
        }
        n += 1;
    }
    let base = hls(sum.map(|c| c / n as f64));
    let goal = hls(target.map(|c| c as f64 / 255.));
    for p in decoded.pixels_mut() {
        let source = hls([p[0] as f64 / 255., p[1] as f64 / 255., p[2] as f64 / 255.]);
        let rgb = from_hls([
            goal[0],
            (source[1] + (goal[1] - base[1])).clamp(0., 1.),
            goal[2],
        ]);
        for c in 0..3 {
            p[c] = (rgb[c] * 255. + 0.5).clamp(0., 255.) as u8;
        }
    }
    struct Output(Vec<u8>);
    impl std::io::Write for Output {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            if self
                .0
                .len()
                .checked_add(data.len())
                .is_none_or(|n| n > 16 * 1024 * 1024)
            {
                return Err(std::io::Error::other("Tint encoded byte admission"));
            }
            self.0.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = Output(Vec::new());
    use image::ImageEncoder;
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(decoded.as_raw(), w, h, image::ExtendedColorType::Rgba8)
        .map_err(|_| SkpError::Invalid("Legacy tint PNG encode/admission".into()))?;
    Ok(Some(bytes.0))
}
