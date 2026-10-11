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
    pub bc7_uploads: u64,
    pub compression_attempts: u64,
    pub compression_ms: f64,
    pub uploaded_bytes: u64,
    pub exact_saved_bytes: u64,
    pub hits: u64,
    pub retired: u64,
    pub hash_ms: f64,
    pub decode_ms: f64,
    pub upload_ms: f64,
    pub orientation_bytes: u64,
    pub orientation_ms: f64,
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
    pending: Option<Vec<u8>>,
}
impl PreparedTexture {
    pub fn sampled(&self) -> (vk::ImageView, vk::Sampler, bool) {
        (self.view, self.sampler, self.hard_cutout)
    }
    pub fn id(&self) -> AssetId {
        self.id
    }
    pub(crate) fn pending_bytes(&self)->usize{self.pending.as_ref().map_or(0,Vec::len)}
}

/// Native backend owns images; CAD asset identity is platform-independent and
/// can be produced once by a persistent decoding worker. Unchanged frames use
/// existing AssetId handles and never call hash/decode/admission methods.
pub struct TextureCache {
    images: HashMap<AssetId, Entry>,
    pub stats: TextureStats,
    diagnostic_readback: bool,
    publication_entry_limit: Option<usize>,
}
impl Default for TextureCache {
    fn default() -> Self {
        Self {
            images: HashMap::new(),
            stats: TextureStats::default(),
            diagnostic_readback: false,
            publication_entry_limit: None,
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
        resources.readback_texture_decoded_for_probe(context, &entry.image)
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
        let mut candidate=self.prepare_deferred(context,resources,asset,max_decoded_bytes)?;
        self.complete_many(context,resources,std::iter::once(&mut candidate))?;
        Ok(candidate)
    }
    pub(crate) fn prepare_deferred(&mut self,context:&VulkanContext,resources:&mut Resources,asset:&EncodedAsset<'_>,max_decoded_bytes:u64)->Result<PreparedTexture,String>{
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
                pending: None,
            });
        }
        let start = Instant::now();
        let mut decoded = decode_png_jpeg(encoded, max_decoded_bytes)?;
        self.stats.decodes += 1;
        self.stats.decode_ms += start.elapsed().as_secs_f64() * 1000.;
        // Original QOpenGLTexture uploads QImage.mirrored(), with positive V.
        // Inverting V instead is not equivalent under fixed-point filtering.
        // Cold in-place row swaps preserve one shared AssetId/GPU image for faces
        // and reference planes; warm draws perform no orientation work.
        let orientation_start = Instant::now();
        mirror_rgba_rows(&mut decoded.rgba, decoded.extent.width as usize, decoded.extent.height as usize);
        self.stats.orientation_bytes += decoded.rgba.len() as u64;
        self.stats.orientation_ms += orientation_start.elapsed().as_secs_f64() * 1000.;
        // The original hard-cutout path has no mips. General/mipped images
        // retain existing GPU mip generation until exact BC mip chains exist.
        let compression_start=Instant::now();
        let compressed=if decoded.hard_cutout && TextureImage::supports_bc7(context, decoded.extent, self.diagnostic_readback) {
            self.stats.compression_attempts+=1;
            ingetrazo_render::block_compression::exact_bc7(decoded.extent.width,decoded.extent.height,&decoded.rgba)?
        } else { None };
        self.stats.compression_ms+=compression_start.elapsed().as_secs_f64()*1000.;
        let format=if compressed.is_some(){ingetrazo_render::texture_layout::Format::Bc7}else{ingetrazo_render::texture_layout::Format::Rgba8};
        let data=compressed.unwrap_or(decoded.rgba);
        let image=TextureImage::allocate_color(context,resources,decoded.extent,data.len()as u64,format,!decoded.hard_cutout,self.diagnostic_readback)?;
        Ok(PreparedTexture {
            id,
            view: image.view,
            sampler: image.sampler,
            hard_cutout: decoded.hard_cutout,
            image: Some(image),
            pending: Some(data),
        })
    }
    pub(crate) fn complete_many<'a>(&mut self,context:&VulkanContext,resources:&mut Resources,candidates:impl Iterator<Item=&'a mut PreparedTexture>)->Result<(),String>{
        let mut admitted=Vec::new();
        for candidate in candidates{if candidate.pending.is_some(){admitted.try_reserve(1).map_err(|_|"Image batch candidate admission")?;admitted.push(candidate);}}
        if admitted.is_empty(){return Ok(());}
        let mut candidates=admitted;
        let mut inputs=Vec::new();inputs.try_reserve_exact(candidates.len()).map_err(|_|"Image batch input admission")?;
        for c in &candidates{if let Some(data)=&c.pending{inputs.push((c.image.as_ref().expect("pending image owner"),data.as_slice()));}}
        let start=Instant::now();
        resources.upload_texture_batch(context,&inputs)?;
        self.stats.upload_ms+=start.elapsed().as_secs_f64()*1000.;
        for candidate in &mut candidates{if let Some(data)=candidate.pending.take(){
            self.stats.uploads+=1;self.stats.uploaded_bytes+=data.len()as u64;
            let image=candidate.image.as_ref().expect("pending image owner");
            if image.format==ingetrazo_render::texture_layout::Format::Bc7{self.stats.bc7_uploads+=1;self.stats.exact_saved_bytes+=image.extent.width as u64*image.extent.height as u64*4-data.len()as u64;}
        }}
        Ok(())
    }
    /// Publish only after all referencing descriptor and scene candidates succeed.
    pub fn set_publication_entry_limit(&mut self,limit:Option<usize>){self.publication_entry_limit=limit;}
    pub(crate) fn preflight_publication<'a>(&mut self,candidates:impl Iterator<Item=&'a PreparedTexture>)->Result<(),String>{
        let mut count=0usize;
        for candidate in candidates {
            if candidate.pending.is_some(){return Err("Texture upload has not completed".into());}
            if candidate.image.is_none()&&!self.images.contains_key(&candidate.id){
                return Err("Prepared cached texture was evicted before publication".into());
            }
            if candidate.image.is_some(){count=count.checked_add(1).ok_or("Texture publication count overflow")?;}
        }
        if self.publication_entry_limit.is_some_and(|limit|self.images.len().checked_add(count).is_none_or(|n|n>limit)){return Err("Texture publication retained entry admission".into());}
        self.images.try_reserve(count).map_err(|_|"Texture publication map admission".into())
    }
    pub(crate) fn publish_admitted(&mut self,candidate:PreparedTexture)->AssetId{
        if let Some(image)=candidate.image{self.images.entry(candidate.id).or_insert(Entry{image,hard_cutout:candidate.hard_cutout,last_use:0});}
        debug_assert!(self.images.contains_key(&candidate.id));candidate.id
    }
    pub fn publish(&mut self,candidate:PreparedTexture)->Result<AssetId,String>{
        self.preflight_publication(std::iter::once(&candidate))?;
        Ok(self.publish_admitted(candidate))
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
    pub(crate) fn collect_unused_completed(&mut self,active:&std::collections::HashSet<AssetId>,completed_serial:u64){
        let before=self.images.len();
        self.images.retain(|id,entry|active.contains(id)||entry.last_use>completed_serial);
        self.stats.retired+=(before-self.images.len()) as u64;
    }
    pub fn published_entries(&self)->usize{self.images.len()}
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
    if format == image::ImageFormat::Jpeg {
        // QImage uses the integer slow IDCT/fancy chroma path. Use the same
        // source-compatible decoder as shaded means: image's default JPEG
        // decoder produces different texels, before any GPU filtering occurs.
        // The caller already owns the encoded slice. Bound
        // JPEG coefficient/output/scratch work before pixel allocation.
        let decoded=ingetrazo_render::source_texture_decode::decode(encoded,
            ingetrazo_render::source_texture_decode::Limits {
                encoded_bytes:encoded.len(),decoded_work_bytes:max_decoded_bytes,
                ..Default::default()
            })?;
        return Ok(DecodedTexture {
            extent:vk::Extent2D{width:decoded.width,height:decoded.height},
            rgba:decoded.rgba,hard_cutout:false,
        });
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


// Cold-only source QImage.mirrored() ABI. No second image allocation.
fn mirror_rgba_rows(pixels: &mut [u8], width: usize, height: usize) {
    let stride=width*4;
    debug_assert_eq!(pixels.len(),stride*height);
    for y in 0..height/2 {
        let (top,bottom)=pixels.split_at_mut((height-1-y)*stride);
        top[y*stride..(y+1)*stride].swap_with_slice(&mut bottom[..stride]);
    }
}
#[cfg(test)]
mod source_orientation_tests {
    use super::mirror_rgba_rows;
    #[test]
    fn odd_height_preserves_center_and_rgba_channels_without_reallocation() {
        let mut p=(0..24u8).collect::<Vec<_>>();let pointer=p.as_ptr();
        mirror_rgba_rows(&mut p,2,3);
        assert_eq!(p,[16,17,18,19,20,21,22,23,8,9,10,11,12,13,14,15,0,1,2,3,4,5,6,7]);
        assert_eq!(p.as_ptr(),pointer);
    }
}


#[cfg(test)]
mod publication_admission_tests{
 use super::*;
 #[test]fn missing_cached_identity_refuses_before_mutation_and_empty_retry_succeeds(){
  let mut cache=TextureCache::default();
  let candidate=PreparedTexture{id:AssetId([17;32]),image:None,view:vk::ImageView::null(),sampler:vk::Sampler::null(),hard_cutout:false};
  assert!(cache.preflight_publication(std::iter::once(&candidate)).is_err());assert!(cache.images.is_empty());
  cache.preflight_publication(std::iter::empty()).unwrap();assert!(cache.images.is_empty());
 }
}
