//! Cold, bounded bounds of retained original PhotoMesh NPZ vertices.
use glam::DVec3;
use ingetrazo_core::scene::Scene;
use std::io::{Cursor, Read};
pub fn visible_bounds(scene: &Scene) -> Result<Option<(DVec3, DVec3)>, String> {
    let Some(meta) = scene
        .native_document
        .get("photo_mesh")
        .filter(|m| !m.is_null())
    else {
        return Ok(None);
    };
    if meta.get("visible").and_then(|v| v.as_bool()) == Some(false) {
        return Ok(None);
    }
    let key = meta
        .get("geometry")
        .and_then(|v| v.as_str())
        .ok_or("Survey geometry reference is missing")?;
    let bytes = scene
        .embedded_textures
        .get(key)
        .ok_or("Survey geometry resource is missing")?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("Survey geometry exceeds bounds budget".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Survey geometry archive: {e}"))?;
    let mut file = archive
        .by_name("vertices.npy")
        .map_err(|e| format!("Survey vertices: {e}"))?;
    if file.size() > 256 * 1024 * 1024 {
        return Err("Survey vertices exceed bounds budget".into());
    }
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic[..6] != b"\x93NUMPY" || !(1..=3).contains(&magic[6]) || magic[7] != 0 {
        return Err("Unsupported survey vertex header".into());
    }
    let lenbytes = if magic[6] == 1 { 2 } else { 4 };
    let header_len = if lenbytes == 2 {
        let mut x = [0; 2];
        file.read_exact(&mut x).map_err(|e| e.to_string())?;
        u16::from_le_bytes(x) as usize
    } else {
        let mut x = [0; 4];
        file.read_exact(&mut x).map_err(|e| e.to_string())?;
        u32::from_le_bytes(x) as usize
    };
    if header_len > 4096 {
        return Err("Survey vertex header exceeds budget".into());
    }
    let mut header = [0u8; 4096];
    file.read_exact(&mut header[..header_len])
        .map_err(|e| e.to_string())?;
    let h = std::str::from_utf8(&header[..header_len]).map_err(|e| e.to_string())?;
    if !h.contains("'descr': '<f4'") || !h.contains("'fortran_order': False") {
        return Err("Unsupported survey vertex layout".into());
    }
    let shape = h
        .split("'shape':")
        .nth(1)
        .and_then(|s| s.split('(').nth(1))
        .and_then(|s| s.split(')').next())
        .ok_or("Missing survey vertex shape")?;
    let mut dims = shape.split(',').map(str::trim).filter(|s| !s.is_empty());
    let count = dims
        .next()
        .ok_or("Missing survey count")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    if dims.next() != Some("3") || dims.next().is_some() {
        return Err("Unsupported survey vertex shape".into());
    }
    let payload = count.checked_mul(12).ok_or("Survey vertex size overflow")?;
    if payload > 256 * 1024 * 1024 || file.size() != ((8 + lenbytes + header_len + payload) as u64)
    {
        return Err("Survey vertex size mismatch".into());
    }
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = -lo;
    let mut row = [0; 12];
    for _ in 0..count {
        file.read_exact(&mut row).map_err(|e| e.to_string())?;
        let p = DVec3::from_array(std::array::from_fn(|i| {
            f32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap()) as f64
        }));
        if !p.is_finite() {
            return Err("Survey contains nonfinite vertices".into());
        }
        lo = lo.min(p);
        hi = hi.max(p);
    }
    Ok((count != 0).then_some((lo, hi)))
}
