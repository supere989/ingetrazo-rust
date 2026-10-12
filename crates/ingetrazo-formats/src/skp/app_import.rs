//! File-owned App import preparation; replacement-loader policy remains unchanged.
use super::{Payload, SkpError};
use ingetrazo_core::scene::Scene;
use std::{
    io::{Cursor, Read},
    path::Path,
};

pub struct Plan {
    /// Private file scene, moved into the UI's admitted root publication candidate.
    pub scene: Scene,
    pub empty: bool,
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
        let payload = super::parse_skp_bytes(
            &bytes,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("SketchUp"),
        )?;
        let empty = verified_empty(&payload, &bytes)?;
        let scene = super::scene_from_payload_policy(&payload, empty)?;
        Ok(Self { scene, empty })
    }
}

// Pinned _is_empty_model requires a modern ZIP, no authored entities/definitions,
// and the author's non-null uniform thumbnail. A parser-empty legacy file is not
// accepted. Decode admission is a native data-protection boundary.
fn verified_empty(p: &Payload, bytes: &[u8]) -> Result<bool, SkpError> {
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
    let Ok(mut entry) = zip.by_name("meta/model_thumbnail.png") else {
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
