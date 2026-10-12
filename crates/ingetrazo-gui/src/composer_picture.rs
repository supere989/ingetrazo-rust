//! Cold cached source physical picture ownership and presentation.
use ingetrazo_core::paper_picture::{Picture, Pixels};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Weak},
};
const PEAK: usize = 256 * 1024 * 1024;
#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    content: [u8; 32],
    shape: String,
    fit: String,
    values: [u64; 5],
}
#[derive(Default)]
pub(crate) struct Cache {
    sources: HashMap<[u8; 32], Arc<Pixels>>,
    composites: HashMap<Key, Arc<Pixels>>,
    live: HashMap<u64, Weak<Pixels>>,
    pub decodes: u64,
    pub builds: u64,
}
fn number(value: &serde_json::Value, key: &str, default: f64) -> Result<f64, String> {
    let v = match value.get(key) {
        None => default,
        Some(v) => v.as_f64().ok_or_else(|| format!("Invalid picture {key}"))?,
    };
    if !v.is_finite() || v.abs() > 1_000_000. {
        return Err(format!("Invalid picture {key}"));
    }
    Ok(v)
}
fn option<'a>(v: &'a serde_json::Value, key: &str, default: &'a str) -> Result<&'a str, String> {
    match v.get(key) {
        None => Ok(default),
        Some(v) => {
            let s = v.as_str().ok_or_else(|| format!("Invalid picture {key}"))?;
            Ok(if s.is_empty() { default } else { s })
        }
    }
}
impl Cache {
    fn admit(&mut self, work: usize) -> Result<(), String> {
        self.live.retain(|_, v| v.strong_count() != 0);
        let mut bytes = 0usize;
        for v in self.live.values() {
            if let Some(v) = v.upgrade() {
                bytes = bytes
                    .checked_add(v.bytes())
                    .ok_or("Paper picture owner charge overflow")?;
            }
        }
        if bytes.checked_add(work).is_none_or(|n| n > PEAK) {
            return Err("Paper picture aggregate/cache/decode admission exceeded".into());
        }
        Ok(())
    }
    fn retain(&mut self, value: &Arc<Pixels>) -> Result<(), String> {
        if self.live.len() >= 4096 {
            return Err("Paper picture owner inventory exceeded".into());
        }
        self.live
            .try_reserve(1)
            .map_err(|_| "Paper picture owner allocation failed")?;
        self.live.insert(value.id(), Arc::downgrade(value));
        Ok(())
    }
    pub(crate) fn prepare(&mut self, value: &serde_json::Value) -> Result<Option<Picture>, String> {
        if !value.is_object() {
            return Err("Invalid picture record".into());
        }
        let rect = [
            number(value, "x_mm", 20.)?,
            number(value, "y_mm", 20.)?,
            number(value, "w_mm", 60.)?,
            number(value, "h_mm", 40.)?,
        ];
        if rect[2] <= 0. || rect[3] <= 0. {
            return Err("Picture box must have positive dimensions".into());
        }
        let path = value
            .get("path")
            .map_or(Ok(""), |v| v.as_str().ok_or("Invalid picture path"))?;
        let shape = option(value, "shape", "rect")?;
        let fit = option(value, "fit", "stretch")?;
        if !matches!(shape, "rect" | "rounded" | "ellipse")
            || !matches!(fit, "stretch" | "cover" | "contain")
        {
            return Err("Unsupported picture shape or fit".into());
        }
        let radius = number(value, "radius_mm", 4.)?;
        let feather = number(value, "feather_mm", 0.)?;
        let opacity = number(value, "opacity", 1.)?;
        let data = match std::fs::metadata(Path::new(path)) {
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(None)
            }
            Err(e) => return Err(e.to_string()),
            Ok(v) => {
                if !v.is_file() {
                    return Ok(None);
                }
                if v.len() > 16 * 1024 * 1024 {
                    return Err("Paper picture encoded byte budget exceeded".into());
                }
                // Metadata is only an early refusal: the file can grow before reading.
                // Reserve and admit the bounded owner before any source bytes arrive.
                const LIMIT: usize = 16 * 1024 * 1024;
                self.admit(LIMIT + 1)?;
                let mut data = Vec::new();
                data.try_reserve_exact(LIMIT + 1)
                    .map_err(|_| "Paper encoded picture allocation failed")?;
                self.admit(data.capacity())?;
                use std::io::Read;
                std::fs::File::open(path)
                    .map_err(|e| e.to_string())?
                    .take((LIMIT + 1) as u64)
                    .read_to_end(&mut data)
                    .map_err(|e| e.to_string())?;
                if data.len() > LIMIT {
                    return Err("Paper picture encoded byte budget exceeded".into());
                }
                data
            }
        };
        let content = *blake3::hash(&data).as_bytes();
        let source = if let Some(hit) = self.sources.get(&content) {
            hit.clone()
        } else {
            if self.sources.len() >= 24 {
                self.sources.clear();
                self.composites.clear();
            }
            self.admit(data.capacity() + 64 * 1024 * 1024)?;
            let decoded = ingetrazo_render::source_texture_decode::decode(
                &data,
                ingetrazo_render::source_texture_decode::Limits {
                    encoded_bytes: 16 * 1024 * 1024,
                    decoded_work_bytes: 64 * 1024 * 1024,
                    max_jpeg_scans: 64,
                },
            )?;
            let image = Pixels::new(decoded.width, decoded.height, decoded.rgba)?;
            self.retain(&image)?;
            self.sources
                .try_reserve(1)
                .map_err(|_| "Paper source cache allocation failed")?;
            self.sources.insert(content, image.clone());
            self.decodes += 1;
            image
        };
        let image = if shape == "rect" && feather <= 0. && opacity >= 1. && fit == "stretch" {
            source
        } else {
            let key = Key {
                content,
                shape: shape.into(),
                fit: fit.into(),
                values: [
                    radius.to_bits(),
                    feather.to_bits(),
                    opacity.to_bits(),
                    rect[2].to_bits(),
                    rect[3].to_bits(),
                ],
            };
            if let Some(hit) = self.composites.get(&key) {
                hit.clone()
            } else {
                if self.composites.len() >= 24 {
                    self.composites.clear();
                }
                let (sw, sh) = (f64::from(source.width()), f64::from(source.height()));
                let aspect = rect[2] / rect[3];
                if !aspect.is_finite() || aspect <= 0. {
                    return Err("Picture aspect domain invalid".into());
                }
                let mut src = [0., 0., sw, sh];
                let (w, h) = match fit {
                    "cover" => {
                        if sw / sh > aspect {
                            src[2] = sh * aspect;
                            src[0] = (sw - src[2]) / 2.;
                        } else {
                            src[3] = sw / aspect;
                            src[1] = (sh - src[3]) / 2.;
                        }
                        (src[2], src[3])
                    }
                    "contain" => {
                        if sw / sh > aspect {
                            (sw, sw / aspect)
                        } else {
                            (sh * aspect, sh)
                        }
                    }
                    _ => (sw, sw / aspect),
                };
                let m = (2048. / w.max(h).max(1.)).min(1.);
                let width = (w * m).round_ties_even().max(1.) as u32;
                let height = (h * m).round_ties_even().max(1.) as u32;
                if width > 2048 || height > 2048 {
                    return Err("Picture composite dimensions exceeded".into());
                }
                let count = width as usize * height as usize * 4;
                self.admit(data.capacity() + count * 2)?;
                let mut rgba = Vec::new();
                rgba.try_reserve_exact(count)
                    .map_err(|_| "Picture composite allocation failed")?;
                rgba.resize(count, 0);
                let mut dst = [0., 0., f64::from(width), f64::from(height)];
                if fit == "contain" {
                    let k = (dst[2] / sw).min(dst[3] / sh);
                    let dw = sw * k;
                    let dh = sh * k;
                    dst = [(dst[2] - dw) / 2., (dst[3] - dh) / 2., dw, dh];
                }
                let mx = 1. / (dst[2] / src[2]);
                let my = 1. / (dst[3] / src[3]);
                let tx = src[0] - dst[0] * mx;
                let ty = src[1] - dst[1] * my;
                let fdx = (mx * 65536.) as i64;
                let rounded = |v: f64| {
                    if v >= 0. {
                        (v + 0.5).floor() as i64
                    } else {
                        (v - 0.5).ceil() as i64
                    }
                };
                let left = rounded(dst[0]).clamp(0, i64::from(width));
                let right = rounded(dst[0] + dst[2]).clamp(0, i64::from(width));
                let top = rounded(dst[1]).clamp(0, i64::from(height));
                let bottom = rounded(dst[1] + dst[3]).clamp(0, i64::from(height));
                let input_policy = InputPolicy::native();
                for y in top..bottom {
                    let fy = (((y as f64 + 0.5) * my + ty) * 65536.) as i64 - 32768;
                    let start = (((left as f64 + 0.5) * mx + tx) * 65536.) as i64 - 32768;
                    let mut group_begin = None;
                    let mut input_flags = None;
                    for x in left..right {
                        let relative = x - left;
                        if let Some((begin, count)) = input_policy.group(relative, right - left) {
                            if group_begin != Some(begin) {
                                input_flags = Some(composite_group_opaque(
                                    &source,
                                    start + begin * fdx,
                                    fdx,
                                    count,
                                    fy,
                                ));
                                group_begin = Some(begin);
                            }
                        } else {
                            input_flags = None;
                        }
                        let p = composite_sample64(
                            &source,
                            start + (x - left) * fdx,
                            fy,
                            x - left < ((right - left) / 4) * 4,
                            input_flags,
                        );
                        let i = (y as usize * width as usize + x as usize) * 4;
                        rgba[i..i + 4].copy_from_slice(&p);
                    }
                }
                let a = f64::from(width) / 2.;
                let b = f64::from(height) / 2.;
                let px_per_mm = f64::from(width) / rect[2];
                let radius = (radius * px_per_mm).max(0.).min(a).min(b);
                let feather = feather * px_per_mm;
                for y in 0..height {
                    for x in 0..width {
                        let dx = f64::from(x) + 0.5 - a;
                        let dy = f64::from(y) + 0.5 - b;
                        let dist = if shape == "ellipse" {
                            let nd = ((dx / a).powi(2) + (dy / b).powi(2)).sqrt();
                            let angle = dy.atan2(dx);
                            let rd =
                                1. / ((angle.cos() / a).powi(2) + (angle.sin() / b).powi(2)).sqrt();
                            (1. - nd) * rd
                        } else {
                            let r = if shape == "rounded" { radius } else { 0. };
                            let qx = dx.abs() - (a - r);
                            let qy = dy.abs() - (b - r);
                            -((qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt() + qx.max(qy).min(0.)
                                - r)
                        };
                        let alpha = if feather <= 0.5 {
                            (dist + 0.5).clamp(0., 1.)
                        } else {
                            let t = (dist / feather).clamp(0., 1.);
                            t * t * (3. - 2. * t)
                        } * opacity.clamp(0., 1.);
                        let i = (y as usize * width as usize + x as usize) * 4 + 3;
                        rgba[i] = (f64::from(rgba[i]) * alpha).clamp(0., 255.) as u8;
                    }
                }
                let image = Pixels::new(width, height, rgba)?;
                self.retain(&image)?;
                self.composites
                    .try_reserve(1)
                    .map_err(|_| "Paper composite cache allocation failed")?;
                self.composites.insert(key, image.clone());
                self.builds += 1;
                image
            }
        };
        let picture = Picture {
            image,
            rect_mm: rect,
        };
        picture.validate()?;
        Ok(Some(picture))
    }
}

pub(crate) struct PictureDraws {
    pub draws: Vec<ingetrazo_formats::sheet_pdf::Draw>,
    pub rectangles: Vec<usize>,
    pub source_lines: Vec<usize>,
}
fn draw(
    origin: [f64; 2],
    path: Vec<ingetrazo_formats::sheet_pdf::PathOp>,
    fill: Option<[f32; 4]>,
    stroke: Option<([f32; 4], f64)>,
) -> ingetrazo_formats::sheet_pdf::Draw {
    ingetrazo_formats::sheet_pdf::Draw {
        picture: None,
        rotation_deg: 0.,
        origin_mm: origin,
        text: None,
        even_odd: false,
        path,
        stroke,
        stroke_options: stroke.map(|_| ingetrazo_formats::sheet_pdf::StrokeOptions {
            cap: 2,
            join: 2,
            dash: vec![],
        }),
        fill,
        clip: None,
        clip_intersection: None,
        flatten_transform: false,
    }
}
impl Cache {
    pub(crate) fn draws(&mut self, value: &serde_json::Value) -> Result<PictureDraws, String> {
        use ingetrazo_core::sheet_shape::ShapePath;
        use ingetrazo_formats::sheet_pdf::{Draw, PathOp};
        let origin = [number(value, "x_mm", 20.)?, number(value, "y_mm", 20.)?];
        let size = glam::DVec2::new(number(value, "w_mm", 60.)?, number(value, "h_mm", 40.)?);
        let rectangle = || {
            ingetrazo_formats::sheet_pdf_shapes::source_outline(&ShapePath::Polygon(vec![
                glam::DVec2::ZERO,
                glam::DVec2::new(size.x, 0.),
                size,
                glam::DVec2::new(0., size.y),
            ]))
        };
        let mut draws = Vec::new();
        let mut rectangles = Vec::new();
        let mut source_lines = Vec::new();
        if let Some(picture) = self.prepare(value)? {
            draws.push(Draw {
                picture: Some(picture),
                ..draw([0.; 2], vec![], None, None)
            });
            let border = value.get("border").map_or(Ok(false), |v| {
                v.as_bool().ok_or("Invalid picture border flag")
            })?;
            if border {
                let color = option(value, "border_color", "#282e36")?;
                let color = ingetrazo_core::sheet_color::parse_sheet_color(color)?
                    .map(|v| f32::from(v) / 255.);
                let width = number(value, "border_mm", 0.3)?.max(0.1);
                let shape = match option(value, "shape", "rect")? {
                    "ellipse" => ShapePath::Ellipse { size },
                    "rounded" => ShapePath::RoundedRect {
                        size,
                        radius: number(value, "radius_mm", 0.)?
                            .max(0.)
                            .min(size.x / 2.)
                            .min(size.y / 2.),
                    },
                    _ => ShapePath::Polygon(vec![
                        glam::DVec2::ZERO,
                        glam::DVec2::new(size.x, 0.),
                        size,
                        glam::DVec2::new(0., size.y),
                    ]),
                };
                draws.push(draw(
                    origin,
                    ingetrazo_formats::sheet_pdf_shapes::source_outline(&shape),
                    None,
                    Some((color, width)),
                ));
            }
        } else {
            rectangles.push(0);
            draws.push(draw(
                origin,
                rectangle(),
                Some([240. / 255., 240. / 255., 242. / 255., 1.]),
                None,
            ));
            let color = [170. / 255., 176. / 255., 184. / 255., 1.];
            draws.push(draw(origin, rectangle(), None, Some((color, 0.25))));
            source_lines.push(draws.len());
            draws.push(draw(
                origin,
                vec![PathOp::Move([0., 0.]), PathOp::Line(size.to_array())],
                None,
                Some((color, 0.25)),
            ));
            source_lines.push(draws.len());
            draws.push(draw(
                origin,
                vec![PathOp::Move([size.x, 0.]), PathOp::Line([0., size.y])],
                None,
                Some((color, 0.25)),
            ));
        }
        Ok(PictureDraws {
            draws,
            rectangles,
            source_lines,
        })
    }
}

#[derive(Clone, Copy)]
enum InputPolicy {
    Avx2,
    Sse4,
    Scalar,
}
impl InputPolicy {
    fn native() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") {
                return Self::Avx2;
            }
            if std::is_x86_feature_detected!("sse4.1") {
                return Self::Sse4;
            }
        }
        Self::Scalar
    }
    fn group(self, offset: i64, length: i64) -> Option<(i64, i64)> {
        match self {
            Self::Avx2 => {
                let begin = offset / 4 * 4;
                Some((begin, (length - begin).min(4)))
            }
            Self::Sse4 => {
                let begin = offset / 2 * 2;
                (length - begin >= 2).then_some((begin, 2))
            }
            Self::Scalar => None,
        }
    }
}
fn composite_group_opaque(image: &Pixels, first: i64, step: i64, count: i64, fy: i64) -> [bool; 2] {
    std::array::from_fn(|row| {
        let y = ((fy >> 16) + row as i64).clamp(0, i64::from(image.height()) - 1) as usize;
        (0..count).all(|i| {
            let x = (first + i * step) >> 16;
            [x, x + 1].iter().all(|x| {
                let x = (*x).clamp(0, i64::from(image.width()) - 1) as usize;
                image.rgba()[(y * image.width() as usize + x) * 4 + 3] == 255
            })
        })
    })
}
/// Qt AVX2 uses high-word/fixup for eight inputs, including its masked tail.
/// SSE4 uses four inputs and a scalar epilogue; generic conversion is scalar.
fn premultiplied_input64(p: &[u8], vector: bool, opaque_group: bool) -> [u64; 4] {
    let a = u64::from(p[3]) * 257;
    std::array::from_fn(|c| {
        if c == 3 {
            return a;
        }
        let color = u64::from(p[c]) * 257;
        if vector {
            if opaque_group {
                color
            } else {
                let high = (color * a) >> 16;
                high + (high >> 15)
            }
        } else {
            let product = color * a;
            (product + (product >> 16) + 32768) >> 16
        }
    })
}
/// QImage ARGB32 selects Qt's generic64 texture route. This cold composite
/// boundary is distinct from the RGB32 sheet sampler's byte interpolation.
fn composite_sample64(
    image: &Pixels,
    fx: i64,
    fy: i64,
    vector_store: bool,
    input_flags: Option<[bool; 2]>,
) -> [u8; 4] {
    let source_pixel = |x: i64, y: i64| {
        let x = x.clamp(0, i64::from(image.width()) - 1) as usize;
        let y = y.clamp(0, i64::from(image.height()) - 1) as usize;
        &image.rgba()[(y * image.width() as usize + x) * 4..][..4]
    };
    let y = fy >> 16;
    let [top_opaque, bottom_opaque] = input_flags.unwrap_or([false; 2]);
    let pixel = |x: i64, row: i64| {
        premultiplied_input64(
            source_pixel(x, row),
            input_flags.is_some(),
            if row == y { top_opaque } else { bottom_opaque },
        )
    };
    let x = fx >> 16;
    let y = fy >> 16;
    let dx = (fx & 65535) as u64;
    let dy = (fy & 65535) as u64;
    let tl = pixel(x, y);
    let tr = pixel(x + 1, y);
    let bl = pixel(x, y + 1);
    let br = pixel(x + 1, y + 1);
    let blend = |a: u64, b: u64, t: u64| {
        if t == 0 {
            a
        } else {
            (a * (65536 - t) >> 16) + (b * t >> 16)
        }
    };
    let p = std::array::from_fn::<u64, 4, _>(|c| {
        blend(blend(tl[c], bl[c], dy), blend(tr[c], br[c], dy), dx)
    });
    let byte = |v: u64| {
        let v = v + 128;
        ((v - (v >> 8)) >> 8).min(255) as u8
    };
    if p[3] == 0 {
        return [0; 4];
    }
    if vector_store && p[3] != 65535 {
        // Literal Qt SSE4 store: reciprocal estimate + one Newton step,
        // then encoded-channel conversion rounds to nearest/even.
        #[cfg(target_arch = "x86_64")]
        let inv = unsafe {
            use std::arch::x86_64::*;
            let a = _mm_set_ss(p[3] as f32);
            let r = _mm_rcp_ss(a);
            _mm_cvtss_f32(_mm_mul_ss(
                _mm_sub_ss(_mm_add_ss(r, r), _mm_mul_ss(r, _mm_mul_ss(r, a))),
                _mm_set_ss(255.),
            ))
        };
        #[cfg(not(target_arch = "x86_64"))]
        let inv = 255. / p[3] as f32;
        return [
            (p[0] as f32 * inv).round_ties_even().clamp(0., 255.) as u8,
            (p[1] as f32 * inv).round_ties_even().clamp(0., 255.) as u8,
            (p[2] as f32 * inv).round_ties_even().clamp(0., 255.) as u8,
            byte(p[3]),
        ];
    }
    let fa = (0xffff00008000u64 + p[3] / 2) / p[3];
    [
        byte((p[0] * fa + 0x80000000) >> 32),
        byte((p[1] * fa + 0x80000000) >> 32),
        byte((p[2] * fa + 0x80000000) >> 32),
        byte(p[3]),
    ]
}

#[cfg(test)]
mod alpha_input_tests {
    use super::{composite_group_opaque, premultiplied_input64, InputPolicy};
    use ingetrazo_core::paper_picture::Pixels;
    #[test]
    fn qt_argb32_input_vector_and_scalar_epilogue() {
        // Actual Qt6.12 ARGB32->RGBA64_PM QPainter observation on four
        // retained person.png pixels, independent of the source formulas.
        let pixels = [
            [58, 44, 36, 1],
            [58, 45, 37, 7],
            [57, 44, 36, 14],
            [59, 45, 37, 17],
        ];
        let expected = [
            [58, 44, 36, 257],
            [409, 317, 261, 1799],
            [804, 620, 507, 3598],
            [1010, 770, 633, 4369],
        ];
        for (p, out) in pixels.iter().zip(expected) {
            assert_eq!(premultiplied_input64(p, true, false), out);
        }
        // Actual AVX2 eight-input mixed-group and masked-six-input observations.
        assert_eq!(InputPolicy::Avx2.group(2, 3), Some((0, 3)));
        assert_eq!(InputPolicy::Avx2.group(4, 6), Some((4, 2)));
        assert_eq!(InputPolicy::Sse4.group(2, 3), None);
        assert_eq!(InputPolicy::Scalar.group(0, 6), None);
        let image = Pixels::new(
            5,
            1,
            [[100, 100, 100, 255]; 4]
                .into_iter()
                .chain([[57, 44, 36, 14]])
                .flatten()
                .collect(),
        )
        .unwrap();
        // Eight fetched neighbors span four output pixels. The partial neighbor
        // in the last pair makes the first opaque pair use the mixed-group law.
        assert_eq!(
            composite_group_opaque(&image, 0, 65536, 4, 0),
            [false, false]
        );
        assert_eq!(composite_group_opaque(&image, 0, 65536, 2, 0), [true, true]);

        assert_eq!(
            premultiplied_input64(&pixels[2], false, false),
            [804, 621, 508, 3598]
        );
        assert_eq!(
            premultiplied_input64(&[100, 100, 100, 255], true, true),
            [25700, 25700, 25700, 65535]
        );
        assert_eq!(
            premultiplied_input64(&[100, 100, 100, 255], true, false),
            [25699, 25699, 25699, 65535]
        );
    }
}
