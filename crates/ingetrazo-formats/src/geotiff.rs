//! Source GeoTIFF header and band decoder. Pixels are reference assets, never mesh.
use ingetrazo_core::georef;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
#[derive(Clone, Copy)]
pub struct Limits {
    pub memory_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 1024 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Info {
    pub path: PathBuf,
    pub width: usize,
    pub height: usize,
    pub samples: usize,
    pub bits: Vec<u16>,
    pub compression: u16,
    pub photometric: u16,
    pub planar: u16,
    pub tiled: bool,
    pub tile_w: usize,
    pub tile_h: usize,
    pub offsets: Vec<u64>,
    pub bytecounts: Vec<u64>,
    pub predictor: u16,
    pub jpeg_tables: Vec<u8>,
    pub extra_samples: Vec<u16>,
    pub sample_format: Vec<u16>,
    pub big: bool,
    pub little_endian: bool,
    pub transform: Option<[f64; 6]>,
    pub epsg: Option<u16>,
    pub zone: Option<i32>,
    pub northern: Option<bool>,
    pub geographic: bool,
    pub citation: String,
    pub nodata: Option<String>,
}
impl Info {
    pub fn georeferenced(&self) -> bool {
        self.transform.is_some() && (self.geographic || self.zone.is_some())
    }
    pub fn pixel_to_map(&self, col: f64, row: f64) -> Result<(f64, f64), String> {
        let [a, b, c, d, e, f] = self.transform.ok_or("Missing GeoTIFF map transform")?;
        Ok((a * col + b * row + c, d * col + e * row + f))
    }
    pub fn corners_geodetic(&self) -> Result<[(f64, f64); 4], String> {
        if !self.georeferenced() {
            return Err("GeoTIFF has no supported coordinate reference".into());
        }
        let mut result = [(0., 0.); 4];
        for (i, (col, row)) in [
            (0., self.height as f64),
            (self.width as f64, self.height as f64),
            (self.width as f64, 0.),
            (0., 0.),
        ]
        .into_iter()
        .enumerate()
        {
            let (x, y) = self.pixel_to_map(col, row)?;
            result[i] = if self.geographic {
                (y, x)
            } else {
                georef::utm_inverse(x, y, self.zone.unwrap(), self.northern.unwrap_or(false))
            };
        }
        if result.iter().any(|(lat, lon)| {
            !lat.is_finite() || !lon.is_finite() || lat.abs() > 90. || lon.abs() > 180.
        }) {
            return Err("GeoTIFF map coordinates are not finite WGS84 coordinates".into());
        }
        Ok(result)
    }
    pub fn unsupported_reason(&self) -> Option<String> {
        if !matches!(self.compression, 1 | 5 | 7 | 8 | 32773 | 32946) {
            return Some(format!("compression scheme {}", self.compression));
        }
        if let Some(b) = self
            .bits
            .iter()
            .take(self.samples)
            .find(|b| !matches!(**b, 8 | 16))
        {
            return Some(format!("{}-bit samples", b));
        }
        if self
            .sample_format
            .iter()
            .take(self.samples)
            .any(|v| !matches!(*v, 1 | 4))
        {
            return Some("floating-point samples (an elevation model, not a picture)".into());
        }
        if !matches!(self.photometric, 0 | 1 | 2 | 6) {
            return Some(format!("photometric interpretation {}", self.photometric));
        }
        if !matches!(self.samples, 1..=4) && self.photometric != 2 {
            return Some(format!("{} samples per pixel", self.samples));
        }
        if self.compression == 7 && (self.bits.first() != Some(&8) || self.planar != 1) {
            return Some("JPEG-compressed samples in this layout".into());
        }
        None
    }
}
#[derive(Clone, Copy)]
enum Number {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
}
impl Number {
    fn u64(self) -> Result<u64, String> {
        match self {
            Self::Unsigned(v) => Ok(v),
            Self::Signed(v) => u64::try_from(v).map_err(|_| "Negative TIFF integer".into()),
            Self::Float(v) if v.is_finite() && v >= 0. && v < u64::MAX as f64 => Ok(v as u64),
            _ => Err("Invalid TIFF integer".into()),
        }
    }
    fn f64(self) -> f64 {
        match self {
            Self::Unsigned(v) => v as f64,
            Self::Signed(v) => v as f64,
            Self::Float(v) => v,
        }
    }
}
enum Tag {
    Numbers(Vec<Number>),
    Bytes(Vec<u8>),
    Text(String),
}
fn product(a: usize, b: usize) -> Result<usize, String> {
    a.checked_mul(b)
        .ok_or_else(|| "GeoTIFF size overflow".into())
}
fn allocated<T: Clone>(
    n: usize,
    value: T,
    resident: usize,
    limit: usize,
) -> Result<Vec<T>, String> {
    let bytes = product(n, std::mem::size_of::<T>())?;
    if resident.checked_add(bytes).is_none_or(|n| n > limit) {
        return Err("GeoTIFF CPU memory admission exceeded".into());
    }
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| "GeoTIFF allocation refused")?;
    if resident
        .checked_add(product(v.capacity(), std::mem::size_of::<T>())?)
        .is_none_or(|n| n > limit)
    {
        return Err("GeoTIFF allocation capacity exceeds CPU admission".into());
    }
    v.resize(n, value);
    Ok(v)
}
fn value(bytes: &[u8], little: bool) -> u64 {
    if little {
        bytes
            .iter()
            .enumerate()
            .fold(0, |v, (i, b)| v | (u64::from(*b) << (8 * i)))
    } else {
        bytes.iter().fold(0, |v, b| (v << 8) | u64::from(*b))
    }
}
fn known(tag: u16) -> bool {
    matches!(
        tag,
        256 | 257
            | 258
            | 259
            | 262
            | 273
            | 277
            | 278
            | 279
            | 284
            | 317
            | 322
            | 323
            | 324
            | 325
            | 338
            | 339
            | 347
            | 33550
            | 33922
            | 34264
            | 34735
            | 34736
            | 34737
            | 42113
    )
}
fn numbers(tags: &HashMap<u16, Tag>, key: u16) -> &[Number] {
    match tags.get(&key) {
        Some(Tag::Numbers(n)) => n,
        _ => &[],
    }
}
fn text(tags: &HashMap<u16, Tag>, key: u16) -> &str {
    match tags.get(&key) {
        Some(Tag::Text(n)) => n,
        _ => "",
    }
}
fn first(tags: &HashMap<u16, Tag>, key: u16, default: u64) -> Result<u64, String> {
    numbers(tags, key).first().map_or(Ok(default), |n| n.u64())
}
fn ints(tags: &HashMap<u16, Tag>, key: u16) -> Result<Vec<u64>, String> {
    numbers(tags, key).iter().map(|n| n.u64()).collect()
}
fn shorts(tags: &HashMap<u16, Tag>, key: u16) -> Result<Vec<u16>, String> {
    numbers(tags, key)
        .iter()
        .map(|n| u16::try_from(n.u64()?).map_err(|_| "TIFF short overflow".into()))
        .collect()
}
pub fn read_info(path: &Path, limits: Limits) -> Result<Info, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    read_header(&mut f, path, limits)
}
pub fn read_header<R: Read + Seek>(f: &mut R, path: &Path, limits: Limits) -> Result<Info, String> {
    let length = f.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut head = [0u8; 16];
    let n = f.read(&mut head).map_err(|e| e.to_string())?;
    if n < 8 || !matches!(&head[..2], b"II" | b"MM") {
        return Err("not a TIFF file".into());
    }
    let le = &head[..2] == b"II";
    let magic = value(&head[2..4], le);
    let big = match magic {
        42 => false,
        43 if n >= 16 => true,
        _ => return Err("not a TIFF file".into()),
    };
    if big && (value(&head[4..6], le) != 8 || value(&head[6..8], le) != 0) {
        return Err("Unsupported BigTIFF offset layout".into());
    }
    let ifd = value(if big { &head[8..16] } else { &head[4..8] }, le);
    if ifd >= length {
        return Err("TIFF directory outside file".into());
    }
    f.seek(SeekFrom::Start(ifd)).map_err(|e| e.to_string())?;
    let count_width = if big { 8 } else { 2 };
    let mut count = [0u8; 8];
    f.read_exact(&mut count[..count_width])
        .map_err(|e| e.to_string())?;
    let count = usize::try_from(value(&count[..count_width], le))
        .map_err(|_| "TIFF directory count overflow")?;
    let entry_size = if big { 20 } else { 12 };
    let raw_size = product(count, entry_size)?;
    if ifd
        .checked_add(count_width as u64)
        .and_then(|n| n.checked_add(raw_size as u64))
        .is_none_or(|n| n > length)
    {
        return Err("Truncated TIFF directory".into());
    }
    let mut raw = allocated(raw_size, 0u8, 0, limits.memory_bytes)?;
    f.read_exact(&mut raw).map_err(|e| e.to_string())?;
    let mut resident = raw.capacity();
    let mut tags = HashMap::new();
    tags.try_reserve(count.min(32))
        .map_err(|_| "TIFF tag allocation refused")?;
    for entry in raw.chunks_exact(entry_size) {
        let tag = value(&entry[..2], le) as u16;
        if !known(tag) {
            continue;
        }
        let typ = value(&entry[2..4], le) as u16;
        let count = usize::try_from(value(if big { &entry[4..12] } else { &entry[4..8] }, le))
            .map_err(|_| "TIFF tag count overflow")?;
        let size = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 | 16 | 17 | 18 => 8,
            _ => continue,
        };
        let bytes = product(count, size)?;
        let inline = if big { 8 } else { 4 };
        let val = if big { &entry[12..20] } else { &entry[8..12] };
        let mut data = allocated(bytes, 0u8, resident, limits.memory_bytes)?;
        if bytes <= inline {
            data.copy_from_slice(&val[..bytes]);
        } else {
            let offset = value(val, le);
            if offset.checked_add(bytes as u64).is_none_or(|n| n > length) {
                continue;
            }
            f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
            f.read_exact(&mut data).map_err(|e| e.to_string())?;
        }
        let parsed = if typ == 2 {
            Tag::Text(
                data.iter()
                    .map(|b| char::from(*b))
                    .collect::<String>()
                    .trim_end_matches('\0')
                    .to_string(),
            )
        } else if typ == 7 {
            Tag::Bytes(data)
        } else {
            let mut values = allocated(
                count,
                Number::Unsigned(0),
                resident + data.capacity(),
                limits.memory_bytes,
            )?;
            for (i, out) in values.iter_mut().enumerate() {
                let chunk = &data[i * size..(i + 1) * size];
                let u = value(chunk, le);
                *out = match typ {
                    5 | 10 => {
                        let a = value(&chunk[..4], le);
                        let b = value(&chunk[4..], le);
                        let (a, b) = if typ == 10 {
                            (a as u32 as i32 as f64, b as u32 as i32 as f64)
                        } else {
                            (a as f64, b as f64)
                        };
                        Number::Float(if b != 0. { a / b } else { 0. })
                    }
                    6 => Number::Signed(u as u8 as i8 as i64),
                    8 => Number::Signed(u as u16 as i16 as i64),
                    9 => Number::Signed(u as u32 as i32 as i64),
                    17 => Number::Signed(u as i64),
                    11 => Number::Float(f32::from_bits(u as u32) as f64),
                    12 => Number::Float(f64::from_bits(u)),
                    _ => Number::Unsigned(u),
                };
            }
            Tag::Numbers(values)
        };
        resident = resident
            .checked_add(match &parsed {
                Tag::Numbers(v) => v.capacity() * std::mem::size_of::<Number>(),
                Tag::Bytes(v) => v.capacity(),
                Tag::Text(v) => v.capacity(),
            })
            .ok_or("TIFF header budget overflow")?;
        if resident > limits.memory_bytes {
            return Err("TIFF header admission exceeded".into());
        }
        tags.insert(tag, parsed);
    }
    let width = usize::try_from(first(&tags, 256, 0)?).map_err(|_| "TIFF width overflow")?;
    let height = usize::try_from(first(&tags, 257, 0)?).map_err(|_| "TIFF height overflow")?;
    if width == 0 || height == 0 {
        return Err("the TIFF has no image size".into());
    }
    let samples =
        usize::try_from(first(&tags, 277, 1)?).map_err(|_| "TIFF sample count overflow")?;
    if samples == 0 {
        return Err("Zero TIFF sample count".into());
    }
    let mut bits = shorts(&tags, 258)?;
    if bits.is_empty() {
        bits.push(1);
    }
    if samples > bits.len() {
        if product(samples, 2)? > limits.memory_bytes {
            return Err("TIFF bits admission exceeded".into());
        }
        bits.resize(samples, *bits.last().unwrap());
    }
    let tiled = tags.contains_key(&324);
    let tile_w = if tiled {
        first(&tags, 322, 0)? as usize
    } else {
        width
    };
    let rows = first(&tags, 278, height as u64)?;
    let tile_h = if tiled {
        first(&tags, 323, 0)? as usize
    } else if rows == 0 {
        height
    } else {
        (rows as usize).min(height)
    };
    if tile_w == 0 || tile_h == 0 {
        return Err("Zero TIFF block extent".into());
    }
    let offsets = ints(&tags, if tiled { 324 } else { 273 })?;
    let bytecounts = ints(&tags, if tiled { 325 } else { 279 })?;
    if offsets.is_empty() {
        return Err("the TIFF has no image data".into());
    }
    if bytecounts.is_empty() {
        return Err("the TIFF has no byte counts".into());
    }
    let mut info = Info {
        path: path.to_owned(),
        width,
        height,
        samples,
        bits,
        compression: first(&tags, 259, 1)? as u16,
        photometric: first(&tags, 262, if samples >= 3 { 2 } else { 1 })? as u16,
        planar: first(&tags, 284, 1)? as u16,
        tiled,
        tile_w,
        tile_h,
        offsets,
        bytecounts,
        predictor: first(&tags, 317, 1)? as u16,
        jpeg_tables: match tags.remove(&347) {
            Some(Tag::Bytes(b)) => b,
            _ => vec![],
        },
        extra_samples: shorts(&tags, 338)?,
        sample_format: shorts(&tags, 339)?,
        big,
        little_endian: le,
        transform: None,
        epsg: None,
        zone: None,
        northern: None,
        geographic: false,
        citation: String::new(),
        nodata: tags.get(&42113).map(|_| text(&tags, 42113).to_string()),
    };
    placement(&mut info, &tags)?;
    Ok(info)
}
fn placement(info: &mut Info, tags: &HashMap<u16, Tag>) -> Result<(), String> {
    let matrix = numbers(tags, 34264);
    let scale = numbers(tags, 33550);
    let tie = numbers(tags, 33922);
    let mut transform = if matrix.len() >= 16 {
        Some([
            matrix[0].f64(),
            matrix[1].f64(),
            matrix[3].f64(),
            matrix[4].f64(),
            matrix[5].f64(),
            matrix[7].f64(),
        ])
    } else if scale.len() >= 2 && tie.len() >= 6 {
        let sx = scale[0].f64();
        let sy = scale[1].f64();
        Some([
            sx,
            0.,
            tie[3].f64() - tie[0].f64() * sx,
            0.,
            -sy,
            tie[4].f64() + tie[1].f64() * sy,
        ])
    } else {
        None
    };
    let keys = numbers(tags, 34735);
    let ascii = text(tags, 34737);
    let (mut model, mut raster, mut epsg, mut geographic) = (None, 1, None, None);
    let mut citation = String::new();
    if keys.len() >= 4 {
        for key in keys[4..].chunks_exact(4).take(keys[3].u64()? as usize) {
            let (k, loc, count, val) = (key[0].u64()?, key[1].u64()?, key[2].u64()?, key[3].u64()?);
            match k {
                1024 => model = Some(val),
                1025 => raster = val,
                3072 => epsg = Some(val),
                2048 => geographic = Some(val),
                1026 | 3073 if loc == 34737 => {
                    let part = ascii
                        .chars()
                        .skip(val as usize)
                        .take(count as usize)
                        .collect::<String>();
                    citation.push_str(part.trim_end_matches('|'));
                    citation.push('|');
                }
                _ => {}
            }
        }
    }
    info.citation = citation.trim_matches('|').to_string();
    info.epsg = epsg
        .filter(|v| *v != 0 && *v != 32767)
        .and_then(|v| u16::try_from(v).ok());
    if let Some(t) = &mut transform {
        if raster == 2 {
            t[2] = t[2] - 0.5 * t[0] - 0.5 * t[1];
            t[5] = t[5] - 0.5 * t[3] - 0.5 * t[4];
        }
        if t.iter().any(|v| !v.is_finite()) {
            return Err("Nonfinite TIFF map transform".into());
        }
    }
    info.transform = transform;
    if let Some(e) = epsg {
        if (32601..=32660).contains(&e) {
            info.zone = Some((e - 32600) as i32);
            info.northern = Some(true);
        } else if (32701..=32760).contains(&e) {
            info.zone = Some((e - 32700) as i32);
            info.northern = Some(false);
        }
    }
    if info.zone.is_none() {
        let lower = ascii.to_ascii_lowercase();
        for (at, _) in lower.match_indices("utm") {
            let mut p = &lower[at + 3..];
            p = p.trim_start();
            if !p.starts_with("zone") {
                continue;
            }
            p = p[4..].trim_start();
            let n = p.bytes().take_while(u8::is_ascii_digit).count().min(2);
            if n == 0 {
                continue;
            }
            let zone = p[..n].parse::<i32>().ok();
            let h = p[n..].trim_start().chars().next();
            if matches!(h, Some('n' | 's')) {
                info.zone = zone;
                info.northern = Some(h == Some('n'));
                break;
            }
        }
    }
    if info.zone.is_none() {
        info.geographic = model == Some(2)
            || (epsg.is_none()
                && geographic.is_some()
                && transform.is_some_and(|t| t[2].abs() <= 180. && t[5].abs() <= 90.));
    }
    Ok(())
}
#[derive(serde::Serialize)]
pub struct DecodeStats {
    pub bands: usize,
    pub peak_logical_bytes: usize,
    pub largest_band_bytes: usize,
    pub largest_block_bytes: usize,
    pub full_resolution_fallback: bool,
}
pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub factor: usize,
    pub rgba: Vec<u8>,
    pub stats: DecodeStats,
}
fn ceil(n: usize, d: usize) -> usize {
    n / d + usize::from(n % d != 0)
}
fn info_bytes(info: &Info) -> Result<usize, String> {
    let n = info
        .offsets
        .capacity()
        .checked_add(info.bytecounts.capacity())
        .ok_or("TIFF metadata overflow")?;
    product(n, 8)?
        .checked_add(info.jpeg_tables.capacity())
        .and_then(|v| v.checked_add(std::mem::size_of::<Info>()))
        .ok_or_else(|| "TIFF metadata overflow".into())
}
pub fn read_rgba(
    info: &Info,
    max_px: usize,
    limits: Limits,
    progress: impl FnMut(f64) -> bool,
) -> Result<Decoded, String> {
    let mut f = File::open(&info.path).map_err(|e| e.to_string())?;
    decode(&mut f, info, max_px, limits, progress)
}
pub fn decode<R: Read + Seek>(
    f: &mut R,
    info: &Info,
    max_px: usize,
    limits: Limits,
    mut progress: impl FnMut(f64) -> bool,
) -> Result<Decoded, String> {
    if let Some(reason) = info.unsupported_reason() {
        return Err(format!("{reason} is not supported"));
    }
    if max_px == 0 {
        return Err("GeoTIFF texture ceiling must be positive".into());
    }
    if info.samples == 2 && !matches!(info.photometric, 0 | 1) {
        return Err("RGB TIFF has fewer than three samples".into());
    }
    let mut factor = 1usize;
    while info.width.max(info.height) as f64 / factor as f64 > max_px as f64 {
        factor = factor
            .checked_mul(2)
            .ok_or("TIFF reduction factor overflow")?;
    }
    if info.tiled && factor > 1 {
        while info.tile_w % factor != 0 || info.tile_h % factor != 0 {
            factor = factor
                .checked_mul(2)
                .filter(|v| *v <= info.tile_w.min(info.tile_h))
                .ok_or("TIFF tiles cannot reduce at the requested ceiling")?;
        }
    }
    let (out_w, out_h) = (ceil(info.width, factor), ceil(info.height, factor));
    let meta = info_bytes(info)?;
    let out_len = product(product(out_w, out_h)?, 4)?;
    let mut out = allocated(out_len, 0u8, meta, limits.memory_bytes)?;
    let mut resident = meta + out.capacity();
    let sum_len = product(out_w, 4)?;
    let mut sums = allocated(sum_len, 0u64, resident, limits.memory_bytes)?;
    resident += sums.capacity() * 8;
    let mut last = allocated(sum_len, 0u64, resident, limits.memory_bytes)?;
    resident += last.capacity() * 8;
    let bands = ceil(info.height, info.tile_h);
    let mut stats = DecodeStats {
        bands,
        peak_logical_bytes: resident,
        largest_band_bytes: 0,
        largest_block_bytes: 0,
        full_resolution_fallback: false,
    };
    let mut output_row = 0usize;
    let mut count = 0usize;
    for band_index in 0..bands {
        let rows = (info.height - band_index * info.tile_h).min(info.tile_h);
        let band_len = product(product(rows, info.width)?, 4)?;
        let mut band = allocated(band_len, 0u8, resident, limits.memory_bytes)?;
        let live = resident + band.capacity();
        stats.largest_band_bytes = stats.largest_band_bytes.max(band.capacity());
        stats.peak_logical_bytes = stats.peak_logical_bytes.max(live);
        decode_band(
            f, info, band_index, rows, &mut band, live, limits, &mut stats,
        )?;
        for row in band.chunks_exact(info.width * 4) {
            last.fill(0);
            for x in 0..out_w * factor {
                let source = (x.min(info.width - 1)) * 4;
                let target = (x / factor) * 4;
                for c in 0..4 {
                    last[target + c] += u64::from(row[source + c]);
                }
            }
            for (sum, value) in sums.iter_mut().zip(&last) {
                *sum += *value;
            }
            count += 1;
            if count == factor {
                let divisor = product(factor, factor)? as u64;
                for (c, sum) in sums.iter().enumerate() {
                    out[output_row * sum_len + c] = (sum / divisor) as u8;
                }
                output_row += 1;
                count = 0;
                sums.fill(0);
            }
        }
        if !progress((band_index + 1) as f64 / bands as f64) {
            return Err("cancelled".into());
        }
    }
    if count > 0 {
        let missing = factor - count;
        let divisor = product(factor, factor)? as u64;
        for (c, sum) in sums.iter().enumerate() {
            out[output_row * sum_len + c] = ((sum + last[c] * missing as u64) / divisor) as u8;
        }
        output_row += 1;
    }
    if output_row != out_h {
        return Err("TIFF reduction row inventory mismatch".into());
    }
    Ok(Decoded {
        width: out_w,
        height: out_h,
        factor,
        rgba: out,
        stats,
    })
}
fn decode_band<R: Read + Seek>(
    f: &mut R,
    info: &Info,
    band: usize,
    rows: usize,
    rgba: &mut [u8],
    resident: usize,
    limits: Limits,
    stats: &mut DecodeStats,
) -> Result<(), String> {
    let planes = if info.planar == 2 { info.samples } else { 1 };
    let per = if info.planar == 2 { 1 } else { info.samples };
    let tiles_x = if info.tiled {
        ceil(info.width, info.tile_w)
    } else {
        1
    };
    let per_plane = product(tiles_x, ceil(info.height, info.tile_h))?;
    if info.offsets.len() < product(planes, per_plane)?
        || info.bytecounts.len() < product(planes, per_plane)?
    {
        return Err("Incomplete TIFF block inventory".into());
    }
    for plane in 0..planes {
        for tx in 0..tiles_x {
            let index = plane * per_plane + band * tiles_x + tx;
            let count = usize::try_from(info.bytecounts[index])
                .map_err(|_| "TIFF encoded block overflow")?;
            let mut encoded = allocated(count, 0u8, resident, limits.memory_bytes)?;
            f.seek(SeekFrom::Start(info.offsets[index]))
                .map_err(|e| e.to_string())?;
            f.read_exact(&mut encoded).map_err(|e| e.to_string())?;
            let x0 = tx * info.tile_w;
            let cols = (info.width - x0).min(info.tile_w);
            let block = decode_block(&encoded, info, per, resident + encoded.capacity(), limits)?;
            let live = resident + encoded.capacity() + block.capacity();
            stats.largest_block_bytes = stats
                .largest_block_bytes
                .max(encoded.capacity() + block.capacity());
            stats.peak_logical_bytes = stats.peak_logical_bytes.max(live);
            if live > limits.memory_bytes {
                return Err("TIFF decoded capacity exceeds CPU admission".into());
            }
            for y in 0..rows {
                for x in 0..cols {
                    let pixel = &block[(y * info.tile_w + x) * per..][..per];
                    let dest = &mut rgba[(y * info.width + x0 + x) * 4..][..4];
                    if info.planar == 2 {
                        if plane < 4 {
                            dest[plane] = pixel[0];
                        }
                    } else {
                        sample_rgba(pixel, info.photometric, dest);
                    }
                }
            }
        }
    }
    if info.planar == 2 {
        for p in rgba.chunks_exact_mut(4) {
            match info.samples {
                1 => {
                    let g = if info.photometric == 0 {
                        255 - p[0]
                    } else {
                        p[0]
                    };
                    p.copy_from_slice(&[g, g, g, 255]);
                }
                2 if matches!(info.photometric, 0 | 1) => {
                    let g = if info.photometric == 0 {
                        255 - p[0]
                    } else {
                        p[0]
                    };
                    let alpha = p[1];
                    p.copy_from_slice(&[g, g, g, alpha]);
                }
                3 => p[3] = 255,
                _ => {}
            }
        }
    }
    Ok(())
}
fn sample_rgba(p: &[u8], photometric: u16, out: &mut [u8]) {
    if p.len() == 1 || (p.len() == 2 && matches!(photometric, 0 | 1)) {
        let g = if photometric == 0 { 255 - p[0] } else { p[0] };
        out.copy_from_slice(&[g, g, g, if p.len() == 2 { p[1] } else { 255 }]);
    } else {
        out[..3].copy_from_slice(&p[..3]);
        out[3] = if p.len() >= 4 { p[3] } else { 255 };
    }
}
fn bounded_extend(out: &mut Vec<u8>, data: &[u8], need: usize) {
    let take = (need - out.len()).min(data.len());
    out.extend_from_slice(&data[..take]);
}
fn decode_block(
    encoded: &[u8],
    info: &Info,
    samples: usize,
    resident: usize,
    limits: Limits,
) -> Result<Vec<u8>, String> {
    let count = product(product(info.tile_w, info.tile_h)?, samples)?;
    if info.compression == 7 {
        return jpeg_block(encoded, info, samples, count, resident, limits);
    }
    let step = if info.bits[0] == 16 { 2 } else { 1 };
    let need = product(count, step)?;
    let mut bytes = allocated(need, 0u8, resident, limits.memory_bytes)?;
    let mut filled = 0;
    match info.compression {
        1 => {
            filled = need.min(encoded.len());
            bytes[..filled].copy_from_slice(&encoded[..filled]);
        }
        8 | 32946 => {
            let mut decoder = flate2::read::ZlibDecoder::new(encoded);
            while filled < need {
                let n = decoder
                    .read(&mut bytes[filled..])
                    .map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            let mut scratch = [0u8; 8192];
            while decoder.read(&mut scratch).map_err(|e| e.to_string())? != 0 {}
        }
        32773 => {
            let mut at = 0;
            while at < encoded.len() {
                let h = encoded[at];
                at += 1;
                if h < 128 {
                    let n = usize::from(h) + 1;
                    let end = (at + n).min(encoded.len());
                    let n = (end - at).min(need - filled);
                    bytes[filled..filled + n].copy_from_slice(&encoded[at..at + n]);
                    filled += n;
                    at = end;
                } else if h > 128 {
                    if at < encoded.len() {
                        let n = (257 - usize::from(h)).min(need - filled);
                        bytes[filled..filled + n].fill(encoded[at]);
                        filled += n;
                    }
                    at += 1;
                }
            }
        }
        5 => filled = lzw_decode_into(encoded, &mut bytes)?,
        _ => return Err("Unsupported TIFF compression".into()),
    }
    if step == 2 && filled % 2 != 0 {
        return Err("Odd byte count in 16-bit TIFF samples".into());
    }
    if step == 1 {
        if info.predictor == 2 {
            for row in bytes.chunks_exact_mut(info.tile_w * samples) {
                for x in 1..info.tile_w {
                    for c in 0..samples {
                        let at = x * samples + c;
                        row[at] = row[at].wrapping_add(row[at - samples]);
                    }
                }
            }
        }
        return Ok(bytes);
    }
    // Predictor is applied in the native sample precision BEFORE taking the high byte.
    for y in 0..info.tile_h {
        for x in 0..info.tile_w {
            for c in 0..samples {
                let i = (y * info.tile_w + x) * samples + c;
                let at = i * 2;
                let mut sample = value(&bytes[at..at + 2], info.little_endian) as u16;
                if info.predictor == 2 && x > 0 {
                    let prev = at - samples * 2;
                    sample = sample
                        .wrapping_add(value(&bytes[prev..prev + 2], info.little_endian) as u16);
                }
                let stored = if info.little_endian {
                    sample.to_le_bytes()
                } else {
                    sample.to_be_bytes()
                };
                bytes[at..at + 2].copy_from_slice(&stored);
            }
        }
    }
    for i in 0..count {
        bytes[i] = (value(&bytes[i * 2..i * 2 + 2], info.little_endian) >> 8) as u8;
    }
    bytes.truncate(count);
    Ok(bytes)
}
fn lzw_decode_into(data: &[u8], out: &mut [u8]) -> Result<usize, String> {
    // Prefix table avoids allocating/copying a growing byte string for every code.
    let mut prefix = [0u16; 4096];
    let mut suffix = [0u8; 4096];
    let mut stack = [0u8; 4096];
    let (mut bits, mut table, mut prev) = (9usize, 258usize, None::<usize>);
    let (mut acc, mut nacc, mut pos, mut written) = (0u32, 0usize, 0usize, 0usize);
    loop {
        while nacc < bits && pos < data.len() {
            acc = (acc << 8) | u32::from(data[pos]);
            pos += 1;
            nacc += 8;
        }
        if nacc < bits {
            break;
        }
        let code = ((acc >> (nacc - bits)) & ((1 << bits) - 1)) as usize;
        nacc -= bits;
        acc &= if nacc == 0 { 0 } else { (1 << nacc) - 1 };
        if code == 256 {
            bits = 9;
            table = 258;
            prev = None;
            continue;
        }
        if code == 257 {
            break;
        }
        if code > table || code == table && prev.is_none() {
            break;
        }
        let special = code == table;
        let mut cursor = if special { prev.unwrap() } else { code };
        let mut n = 0usize;
        while cursor >= 258 {
            if cursor >= table || n >= 4095 {
                return Err("Invalid TIFF LZW prefix chain".into());
            }
            stack[n] = suffix[cursor];
            n += 1;
            cursor = prefix[cursor] as usize;
        }
        if cursor >= 256 {
            break;
        }
        let first = cursor as u8;
        stack[n] = first;
        n += 1;
        for b in stack[..n].iter().rev() {
            if written < out.len() {
                out[written] = *b;
                written += 1;
            }
        }
        if special && written < out.len() {
            out[written] = first;
            written += 1;
        }
        if let Some(previous) = prev {
            if table < 4096 {
                prefix[table] = previous as u16;
                suffix[table] = first;
                table += 1;
            }
        }
        prev = Some(code);
        if table + 1 >= 1 << bits && bits < 12 {
            bits += 1;
        }
    }
    Ok(written)
}
struct Jpeg(turbojpeg_sys::tjhandle);
impl Drop for Jpeg {
    fn drop(&mut self) {
        unsafe {
            turbojpeg_sys::tjDestroy(self.0);
        }
    }
}
fn jpeg_block(
    data: &[u8],
    info: &Info,
    samples: usize,
    count: usize,
    resident: usize,
    limits: Limits,
) -> Result<Vec<u8>, String> {
    let tables = &info.jpeg_tables;
    let splice =
        !tables.is_empty() && data.starts_with(&[255, 216]) && tables.ends_with(&[255, 217]);
    let mut merged = Vec::new();
    let encoded = if splice {
        let size = tables
            .len()
            .checked_add(data.len())
            .and_then(|n| n.checked_sub(4))
            .ok_or("JPEG table size overflow")?;
        merged = allocated(size, 0u8, resident, limits.memory_bytes)?;
        merged[..tables.len() - 2].copy_from_slice(&tables[..tables.len() - 2]);
        merged[tables.len() - 2..].copy_from_slice(&data[2..]);
        &merged[..]
    } else {
        data
    };
    let mut out = allocated(
        count,
        0u8,
        resident + merged.capacity(),
        limits.memory_bytes,
    )?;
    let handle = unsafe { turbojpeg_sys::tjInitDecompress() };
    if handle.is_null() {
        return Err("Could not allocate JPEG tile decoder".into());
    }
    let _jpeg = Jpeg(handle);
    let (mut w, mut h, mut sampling, mut color) = (0, 0, 0, 0);
    if unsafe {
        turbojpeg_sys::tjDecompressHeader3(
            handle,
            encoded.as_ptr(),
            encoded.len() as _,
            &mut w,
            &mut h,
            &mut sampling,
            &mut color,
        )
    } != 0
    {
        return Ok(out);
    }
    if w <= 0 || h <= 0 {
        return Ok(out);
    }
    let pixels = product(w as usize, h as usize)?;
    let raw_size = product(pixels, 4)?;
    let work = product((w as usize + 31) * (h as usize + 31), 64)?
        .checked_add(1024 * 1024)
        .ok_or("JPEG work overflow")?;
    let live = resident + merged.capacity() + out.capacity();
    if live
        .checked_add(work)
        .is_none_or(|n| n > limits.memory_bytes)
    {
        return Err("JPEG tile work exceeds CPU admission".into());
    }
    let mut raw = allocated(raw_size, 0u8, live, limits.memory_bytes)?;
    let cmyk = matches!(
        color,
        turbojpeg_sys::TJCS_TJCS_CMYK | turbojpeg_sys::TJCS_TJCS_YCCK
    );
    let format = if cmyk {
        turbojpeg_sys::TJPF_TJPF_CMYK
    } else {
        turbojpeg_sys::TJPF_TJPF_RGBA
    };
    if unsafe {
        turbojpeg_sys::tjDecompress2(
            handle,
            encoded.as_ptr(),
            encoded.len() as _,
            raw.as_mut_ptr(),
            w,
            0,
            h,
            format as _,
            turbojpeg_sys::TJFLAG_ACCURATEDCT as _,
        )
    } != 0
    {
        return Ok(out);
    }
    for y in 0..(h as usize).min(info.tile_h) {
        for x in 0..(w as usize).min(info.tile_w) {
            let p = &raw[(y * w as usize + x) * 4..][..4];
            let dest = &mut out[(y * info.tile_w + x) * samples..][..samples];
            if cmyk {
                for c in 0..samples.min(4) {
                    dest[c] = 255 - p[c];
                }
            } else if samples >= 3 {
                dest[..3].copy_from_slice(&p[..3]);
                if samples == 4 {
                    dest[3] = 255;
                }
            } else {
                dest[0] = p[0];
            }
        }
    }
    Ok(out)
}
