//! Cold-only immutable asset admission and fence-retired sampled images.
//! PNG/JPEG are currently supported; original Qt additional formats remain open.
use super::{
    resources::{Resources, TextureImage},
    VulkanContext,
};
use ash::vk;
use std::{collections::HashMap, io::Cursor, time::Instant};
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetId([u8; 32]);
#[derive(Default, Debug)]
pub struct TextureStats {
    pub hashes: u64,
    pub encoded_bytes_hashed: u64,
    pub decodes: u64,
    pub uploads: u64,
    pub hits: u64,
    pub retired: u64,
    pub hash_ms: f64,
    pub decode_ms: f64,
    pub upload_ms: f64,
}
/// Borrow prevents encoded bytes from changing between identity and admission.
pub struct EncodedAsset<'a> {
    id: AssetId,
    encoded: &'a [u8],
}
impl EncodedAsset<'_> {
    pub fn id(&self) -> AssetId {
        self.id
    }
}
pub struct DecodedTexture {
    pub extent: vk::Extent2D,
    pub rgba: Vec<u8>,
    pub hard_cutout: bool,
}
struct Entry {
    image: TextureImage,
    hard_cutout: bool,
    last_use: u64,
}
/// Candidate owns a completed transfer, but is not visible to the draw cache.
/// Build referencing descriptors first; abandon drops only the new resource.
pub struct PreparedTexture {
    id: AssetId,
    image: Option<TextureImage>,
    view: vk::ImageView,
    sampler: vk::Sampler,
    hard_cutout: bool,
}
impl PreparedTexture {
    pub fn sampled(&self) -> (vk::ImageView, vk::Sampler, bool) {
        (self.view, self.sampler, self.hard_cutout)
    }
    pub fn id(&self) -> AssetId {
        self.id
    }
}

/// Native backend owns images; CAD asset identity is platform-independent and
/// can be produced once by a persistent decoding worker. Unchanged frames use
/// existing AssetId handles and never call hash/decode/admission methods.
pub struct TextureCache {
    images: HashMap<AssetId, Entry>,
    pub stats: TextureStats,
    diagnostic_readback: bool,
}
impl Default for TextureCache {
    fn default() -> Self {
        Self {
            images: HashMap::new(),
            stats: TextureStats::default(),
            diagnostic_readback: false,
        }
    }
}
impl TextureCache {
    #[doc(hidden)]
    pub fn diagnostic_readback() -> Self {
        Self {
            diagnostic_readback: true,
            ..Self::default()
        }
    }
    #[doc(hidden)]
    pub fn readback_for_probe(
        &self,
        context: &VulkanContext,
        resources: &mut Resources,
        id: AssetId,
    ) -> Result<Vec<u8>, String> {
        let entry = self.images.get(&id).ok_or("Texture not published")?;
        resources.readback_texture_mip(context, &entry.image, 0)
    }

    pub fn identify<'a>(&mut self, encoded: &'a [u8]) -> EncodedAsset<'a> {
        let start = Instant::now();
        let id = AssetId(*blake3::hash(encoded).as_bytes());
        self.stats.hashes += 1;
        self.stats.encoded_bytes_hashed += encoded.len() as u64;
        self.stats.hash_ms += start.elapsed().as_secs_f64() * 1000.;
        EncodedAsset { id, encoded }
    }
    pub fn contains(&self, id: AssetId) -> bool {
        self.images.contains_key(&id)
    }
    /// Called only on changed asset admission. Decode and image transfer happen
    /// before insertion; failed admission cannot replace a published image.
    pub fn prepare(
        &mut self,
        context: &VulkanContext,
        resources: &mut Resources,
        asset: &EncodedAsset<'_>,
        max_decoded_bytes: u64,
    ) -> Result<PreparedTexture, String> {
        let id = asset.id;
        let encoded = asset.encoded;
        if let Some(entry) = self.images.get(&id) {
            self.stats.hits += 1;
            return Ok(PreparedTexture {
                id,
                image: None,
                view: entry.image.view,
                sampler: entry.image.sampler,
                hard_cutout: entry.hard_cutout,
            });
        }
        let start = Instant::now();
        let decoded = decode_png_jpeg(encoded, max_decoded_bytes)?;
        self.stats.decodes += 1;
        self.stats.decode_ms += start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let image = TextureImage::rgba8(
            context,
            resources,
            decoded.extent,
            &decoded.rgba,
            !decoded.hard_cutout,
            self.diagnostic_readback,
        )?;
        self.stats.upload_ms += start.elapsed().as_secs_f64() * 1000.;
        self.stats.uploads += 1;
        Ok(PreparedTexture {
            id,
            view: image.view,
            sampler: image.sampler,
            hard_cutout: decoded.hard_cutout,
            image: Some(image),
        })
    }
    /// Publish only after all referencing descriptor and scene candidates succeed.
    pub fn publish(&mut self, candidate: PreparedTexture) -> Result<AssetId, String> {
        if let Some(image) = candidate.image {
            self.images.entry(candidate.id).or_insert(Entry {
                image,
                hard_cutout: candidate.hard_cutout,
                last_use: 0,
            });
        }
        if !self.images.contains_key(&candidate.id) {
            return Err("Prepared cached texture was evicted before publication".into());
        }
        Ok(candidate.id)
    }
    pub fn sampled(&self, id: AssetId) -> Result<(vk::ImageView, vk::Sampler, bool), String> {
        let entry = self.images.get(&id).ok_or("Texture asset not admitted")?;
        Ok((entry.image.view, entry.image.sampler, entry.hard_cutout))
    }
    pub fn mark_used(&mut self, id: AssetId, serial: u64) -> Result<(), String> {
        let entry = self
            .images
            .get_mut(&id)
            .ok_or("Texture asset not admitted")?;
        entry.last_use = entry.last_use.max(serial);
        Ok(())
    }
    /// Eviction is allowed only after actual draw fence completion. Descriptors
    /// referencing these images must be retired by the caller on the same fence.
    pub fn collect_unused(
        &mut self,
        context: &VulkanContext,
        active: &std::collections::HashSet<AssetId>,
        fence: vk::Fence,
        completed_serial: u64,
    ) -> Result<usize, String> {
        if !unsafe { context.device.get_fence_status(fence) }.map_err(|e| e.to_string())? {
            return Ok(0);
        }
        let before = self.images.len();
        self.images
            .retain(|id, entry| active.contains(id) || entry.last_use > completed_serial);
        let count = before - self.images.len();
        self.stats.retired += count as u64;
        Ok(count)
    }
    pub fn allocation_bytes(&self) -> u64 {
        self.images.values().map(|e| e.image.allocation_bytes).sum()
    }
}
/// Bounded decode validates format and header dimensions before allocating
/// pixels. No external image conversion process or per-frame image wrapper.
pub fn decode_png_jpeg(encoded: &[u8], max_decoded_bytes: u64) -> Result<DecodedTexture, String> {
    let format = image::guess_format(encoded).map_err(|e| format!("Texture format: {e}"))?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err(format!("Native source texture decoder does not yet support {format:?}; original Qt format parity remains open"));
    }
    let dimensions = image::ImageReader::with_format(Cursor::new(encoded), format)
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    let bytes = (dimensions.0 as u64)
        .checked_mul(dimensions.1 as u64)
        .and_then(|n| n.checked_mul(4))
        .ok_or("Decoded texture size overflow")?;
    if dimensions.0 == 0 || dimensions.1 == 0 || bytes > max_decoded_bytes {
        return Err("Decoded texture exceeds admission budget; streaming required".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(encoded), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(max_decoded_bytes);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| e.to_string())?;
    let alpha = decoded.color().has_alpha();
    let rgba = decoded.into_rgba8().into_raw();
    let hard_cutout = alpha && source_cutout_coverage(&rgba, dimensions.0, dimensions.1)? < 0.6;
    Ok(DecodedTexture {
        extent: vk::Extent2D {
            width: dimensions.0,
            height: dimensions.1,
        },
        rgba,
        hard_cutout,
    })
}
/// Original QImage.scaled(64,64), FastTransformation then Alpha8: nearest
/// pixel centers, strict alpha>128 occupancy; below60% uses no-mip cutout.
pub fn source_cutout_coverage(rgba: &[u8], width: u32, height: u32) -> Result<f64, String> {
    let expected = (width as u64)
        .checked_mul(height as u64)
        .and_then(|n| n.checked_mul(4))
        .ok_or("Cutout dimensions overflow")?;
    if width == 0 || height == 0 || expected != rgba.len() as u64 {
        return Err("Invalid cutout RGBA dimensions".into());
    }
    let mut occupied = 0;
    for y in 0..64u64 {
        let sy = ((2 * y + 1) * height as u64 / 128).min(height as u64 - 1);
        for x in 0..64u64 {
            let sx = ((2 * x + 1) * width as u64 / 128).min(width as u64 - 1);
            occupied += usize::from(rgba[((sy * width as u64 + sx) * 4 + 3) as usize] > 128);
        }
    }
    Ok(occupied as f64 / 4096.)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_alpha_strict_threshold_and_sparse_coverage() {
        let mut rgba = [255, 0, 0, 128].repeat(4);
        assert_eq!(source_cutout_coverage(&rgba, 2, 2).unwrap(), 0.);
        rgba[3] = 129;
        rgba[7] = 255;
        assert_eq!(source_cutout_coverage(&rgba, 2, 2).unwrap(), 0.5);
        assert!(source_cutout_coverage(&rgba[..15], 2, 2).is_err());
    }
    #[test]
    fn decode_budget_and_format_fail_before_pixel_admission() {
        let rgba = [255, 0, 0, 255].repeat(4);
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(2, 2, rgba).unwrap())
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        assert!(decode_png_jpeg(bytes.get_ref(), 15).is_err());
        let decoded = decode_png_jpeg(bytes.get_ref(), 64).unwrap();
        assert_eq!(
            decoded.extent,
            vk::Extent2D {
                width: 2,
                height: 2
            }
        );
        assert!(!decoded.hard_cutout);
        assert!(decode_png_jpeg(b"GIF89a", 64).is_err());
    }
}
