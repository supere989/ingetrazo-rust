//! Literal source encoded-byte physical-picture sampling.
pub use ingetrazo_core::paper_picture::{Picture, Pixels};
/// StraightRGBA storage is shared with PDF. Raster sampling premultiplies at
/// the literal Qt byte boundary, then blends vertical before horizontal.
pub fn sample(image: &Pixels, fx: i64, fy: i64) -> [u8; 4] {
    let x = fx >> 16;
    let y = fy >> 16;
    let dx = ((fx & 65535) >> 8) as u32;
    let dy = ((fy & 65535) >> 8) as u32;
    let pixel = |x: i64, y: i64| {
        let x = x.clamp(0, i64::from(image.width()) - 1) as usize;
        let y = y.clamp(0, i64::from(image.height()) - 1) as usize;
        let p = &image.rgba()[(y * image.width() as usize + x) * 4..][..4];
        let a = u32::from(p[3]);
        std::array::from_fn::<_, 4, _>(|c| {
            if c == 3 {
                p[3]
            } else {
                let t = u32::from(p[c]) * a;
                ((t + (t >> 8) + 128) >> 8) as u8
            }
        })
    };
    let a = pixel(x, y);
    let b = pixel(x + 1, y);
    let c = pixel(x, y + 1);
    let d = pixel(x + 1, y + 1);
    std::array::from_fn(|channel| {
        let left = (u32::from(a[channel]) * (256 - dy) + u32::from(c[channel]) * dy) >> 8;
        let right = (u32::from(b[channel]) * (256 - dy) + u32::from(d[channel]) * dy) >> 8;
        ((left * (256 - dx) + right * dx) >> 8) as u8
    })
}

use std::sync::{Arc, RwLock};
/// Cold global-page sampling recipe. Tile motion is an integer origin only;
/// rows and source pixels are immutable and retained through native completion.
#[derive(Debug)]
pub struct Sampling {
    pub image: Arc<Pixels>,
    pub rect_fixed: [i64; 4],
    pub x: [i32; 3],
    pub fx: [i32; 3],
    pub fdx: i32,
    pub top: i32,
    pub fy: Vec<i32>,
    pub page: [u32; 2],
}
impl Sampling {
    pub fn new(picture: &Picture, density: f64, page: [u32; 2]) -> Result<Arc<Self>, String> {
        picture.validate()?;
        if !density.is_finite()
            || density <= 0.
            || page.contains(&0)
            || page.iter().any(|&v| v > 1_000_000)
        {
            return Err("Paper picture sample page domain invalid".into());
        }
        let r = picture.rect_mm.map(|v| v * density);
        let right = r[0] + r[2];
        let bottom = r[1] + r[3];
        if [r[0], r[1], right, bottom]
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        {
            return Err("Paper picture device rectangle domain invalid".into());
        }
        let fixed = [r[0], r[1], right, bottom].map(|v| (v * 65536.) as i64);
        let left = (fixed[0] >> 16).max(0).min(i64::from(page[0]));
        let end = ((fixed[2] + 65535) >> 16).max(left).min(i64::from(page[0]));
        let top = (fixed[1] >> 16).max(0).min(i64::from(page[1]));
        let last = ((fixed[3] + 65535) >> 16).max(top).min(i64::from(page[1]));
        let mx = 1. / (r[2] / f64::from(picture.image.width()));
        let my = 1. / (r[3] / f64::from(picture.image.height()));
        let dx = -r[0] * mx;
        let dy = -r[1] * my;
        let checked = |value: f64| -> Result<i32, String> {
            if !value.is_finite() || value < i32::MIN as f64 + 32768. || value > i32::MAX as f64 {
                return Err("Paper picture requires unsupported slow sampling domain".into());
            }
            Ok(value as i32)
        };
        let fdx = checked(mx * 65536.)?;
        let x = [
            left,
            left.saturating_add(1).min(end),
            end.saturating_sub(1).max(left),
        ]
        .map(|v| v as i32);
        let mut fx = [0; 3];
        for i in 0..3 {
            fx[i] = checked((mx * (f64::from(x[i]) + 0.5) + dx) * 65536.)? - 32768;
        }
        let rows =
            usize::try_from(last - top).map_err(|_| "Paper picture row inventory overflow")?;
        let mut fy = Vec::new();
        fy.try_reserve_exact(rows)
            .map_err(|_| "Paper picture sample row allocation failed")?;
        for y in top..last {
            fy.push(checked((my * (y as f64 + 0.5) + dy) * 65536.)? - 32768);
        }
        Ok(Arc::new(Self {
            image: picture.image.clone(),
            rect_fixed: fixed,
            x,
            fx,
            fdx,
            top: top as i32,
            fy,
            page,
        }))
    }
    pub fn bytes(&self) -> usize {
        self.fy.capacity() * 4 + std::mem::size_of::<Self>()
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub sampling: Arc<Sampling>,
    pub origin: [i32; 2],
}
/// Dedicated physical-picture callback. It never becomes per-pixel geometry.
pub struct Callback {
    snapshot: RwLock<Snapshot>,
}
impl Callback {
    pub fn new(sampling: Arc<Sampling>) -> Arc<Self> {
        Arc::new(Self {
            snapshot: RwLock::new(Snapshot {
                sampling,
                origin: [0; 2],
            }),
        })
    }
    pub fn set_origin(&self, origin: [i32; 2]) -> Result<(), String> {
        self.snapshot
            .write()
            .map_err(|_| "Paper picture callback lock poisoned")?
            .origin = origin;
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(self
            .snapshot
            .read()
            .map_err(|_| "Paper picture callback lock poisoned")?
            .clone())
    }
}
