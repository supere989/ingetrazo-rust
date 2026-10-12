//! File-owned App import preparation; replacement-loader policy remains unchanged.
use super::{Payload, SkpError};
use ingetrazo_core::scene::Scene;
use std::{
    io::{Cursor, Read},
    path::Path,
};

/// Source header recognition only; parsing remains separately admitted.
pub fn recognises(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 64];
    let Ok(n) = file.read(&mut head) else {
        return false;
    };
    let head = &head[..n];
    head.starts_with(b"PK\x03\x04") || head.windows(12).any(|v| v == b"S\0k\0e\0t\0c\0h\0")
}
pub struct Plan {
    /// Private file scene, moved into the UI's admitted root publication candidate.
    pub scene: Scene,
    pub empty: bool,
    pub material_order: Vec<String>,
}
impl Plan {
    pub fn from_path(path: &Path) -> Result<Self, SkpError> {
        const LIMIT: u64 = 512 * 1024 * 1024;
        if std::fs::metadata(path)?.len() > LIMIT {
            return Err(SkpError::Invalid(
                "File exceeds bounded read admission".into(),
            ));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > LIMIT {
            return Err(SkpError::Invalid(
                "File exceeds bounded read admission".into(),
            ));
        }
        let mut payload = super::parse_skp_bytes(
            &bytes,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("SketchUp"),
        )?;
        // The original texture cache is file-owned (path, size, mtime), so an
        // edited/other file cannot overwrite a previous import's embedded owner.
        use sha2::{Digest, Sha256};
        let metadata = std::fs::metadata(path)?;
        let stamp = format!(
            "{}|{}|{}",
            path.canonicalize()?.display(),
            metadata.len(),
            metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| SkpError::Invalid(e.to_string()))?
                .as_nanos()
        );
        let namespace = format!("skp/{:x}", Sha256::digest(stamp.as_bytes()));
        for material in payload.materials.values_mut() {
            if let Some(texture) = material.texture.as_mut() {
                texture.archive_name = format!("{namespace}/{}", texture.archive_name);
            }
        }
        let empty = verified_empty(&payload, &bytes)?;
        // General replacement-loader policy still refuses saved scenes. App
        // policy admits only the records decoded above, preserving source views.
        payload.unsupported.remove("saved scenes");
        let mut scene = super::scene_from_payload_policy(&payload, empty)?;
        for (i, page) in payload.saved_scenes.iter().enumerate() {
            scene.saved_views.push(saved_view(page, &payload, i)?);
        }

        let mut material_order = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for name in payload.material_names.values() {
            if seen.insert(name) {
                material_order.push(name.clone());
            }
        }
        Ok(Self {
            scene,
            empty,
            material_order,
        })
    }
}

// Pinned _is_empty_model requires a modern ZIP, no authored entities/definitions,
// and the author's non-null uniform thumbnail. A parser-empty legacy file is not
// accepted. Decode admission is a native data-protection boundary.
fn verified_empty(p: &Payload, bytes: &[u8]) -> Result<bool, SkpError> {
    let digits = p
        .version
        .trim_start_matches('{')
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>();
    if digits
        .split('.')
        .next()
        .unwrap_or("")
        .parse::<u32>()
        .is_ok_and(|version| version < 21)
    {
        return Ok(false);
    }
    if !p.definitions.is_empty()
        || !p.root.faces.is_empty()
        || !p.root.edges.is_empty()
        || !p.root.instances.is_empty()
        || !p.legacy_dimensions.is_empty()
        || !p.legacy_texts.is_empty()
    {
        return Ok(false);
    }
    let Some(offset) = bytes.windows(4).position(|v| v == b"PK\x03\x04") else {
        return Ok(false);
    };
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes[offset..]))
        .map_err(|e| SkpError::Invalid(e.to_string()))?;
    let Ok(entry) = zip.by_name("meta/model_thumbnail.png") else {
        return Ok(false);
    };
    if entry.size() > 16 * 1024 * 1024 {
        return Err(SkpError::Invalid(
            "Thumbnail encoded admission exceeded".into(),
        ));
    }
    let mut png = Vec::new();
    entry.take(16 * 1024 * 1024 + 1).read_to_end(&mut png)?;
    if png.len() > 16 * 1024 * 1024 {
        return Err(SkpError::Invalid(
            "Thumbnail encoded admission exceeded".into(),
        ));
    }
    let reader = || image::ImageReader::with_format(Cursor::new(&png), image::ImageFormat::Png);
    let (w, h) = reader()
        .into_dimensions()
        .map_err(|e| SkpError::Invalid(e.to_string()))?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16 * 1024 * 1024 {
        return Err(SkpError::Invalid(
            "Thumbnail decoded admission exceeded".into(),
        ));
    }
    let rgba = reader()
        .decode()
        .map_err(|e| SkpError::Invalid(e.to_string()))?
        .to_rgba8();
    let first = &rgba.as_raw()[..4];
    Ok(rgba.as_raw().chunks_exact(4).all(|pixel| pixel == first))
}

fn saved_view(
    page: &super::SavedScene,
    payload: &Payload,
    index: usize,
) -> Result<ingetrazo_core::saved_views::SavedView, SkpError> {
    let eye = page.eye.map(|v| v * 0.0254);
    let target = page.target.map(|v| v * 0.0254);
    let [dx, dy, dz] = std::array::from_fn(|i| eye[i] - target[i]);
    let mut distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if distance == 0. {
        distance = 1.;
    }
    let mut pitch = (dz / distance).clamp(-1., 1.).asin();
    let limit = 89_f64.to_radians();
    let up = page.up.unwrap_or([0., 0., 1.]);
    let yaw = if pitch.abs() < limit - 1e-6 {
        dy.atan2(dx)
    } else if up[0] != 0. || up[1] != 0. {
        (-up[1]).atan2(-up[0])
    } else {
        (-90_f64).to_radians()
    };
    pitch = pitch.clamp(-limit, limit);
    let height = page.ortho_height * 0.0254;
    if page.parallel && height > 0. {
        distance = (height / 2.) / (page.fov.to_radians() / 2.).tan();
    }
    if !distance.is_finite()
        || distance <= 0.
        || !target.iter().all(|v| v.is_finite())
        || !page.fov.is_finite()
        || page.fov <= 0.
        || page.fov >= 180.
    {
        return Err(SkpError::Invalid(
            "Saved scene camera outside finite admission".into(),
        ));
    }
    let name = if page.name.is_empty() {
        format!("Scene {}", index + 1)
    } else {
        page.name.clone()
    };
    let mut view = ingetrazo_core::saved_views::SavedView::new(name);
    view.target = glam::DVec3::from_array(target);
    view.distance = distance;
    view.pitch = pitch;
    view.yaw = yaw;
    view.fov_deg = page.fov;
    view.perspective = !page.parallel;
    view.parallel = page.parallel;
    view.hidden_layers = page
        .hidden_layer_ids
        .iter()
        .filter_map(|id| payload.layers.get(id).map(|layer| layer.name.clone()))
        .collect();
    let mut raw = serde_json::json!({"name":view.name,"target":view.target,"distance":view.distance,"yaw":view.yaw,"pitch":view.pitch});
    if view.fov_deg != 45. {
        raw["fov"] = view.fov_deg.into();
    }
    if view.parallel {
        raw["parallel"] = true.into();
    }
    if !view.hidden_layers.is_empty() {
        raw["hidden_layers"] = serde_json::json!(view.hidden_layers);
    }
    view.native_baseline = crate::igz::view_model(&view);
    view.native_document = raw;
    Ok(view)
}
