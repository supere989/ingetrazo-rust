//! Original DEM RGB heightfield law. Tiles stay runtime/disk-cache resources,
//! not authored topology or an approximate TerrainObject sampling substitute.
use glam::DVec3;
use ingetrazo_core::georef::{deg2num, SceneDatum};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Terrarium,
    Mapbox,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub zoom: u8,
}
pub struct Query {
    pub tile: Tile,
    pub pixel: [f64; 2],
}
pub fn query(datum: &SceneDatum, point: DVec3, zoom: u8) -> Result<Query, String> {
    if !point.is_finite() || zoom > 30 {
        return Err("DEM query is not finite or zoom is unsupported".into());
    }
    // Original profile passes a QVector3D before local_to_geodetic.
    let point = point.as_vec3();
    if !point.is_finite() {
        return Err("DEM query exceeds source QVector3D representation".into());
    }
    let (lat, lon, _) = datum.local_to_geodetic(point.as_dvec3());
    if !lat.is_finite() || !lon.is_finite() {
        return Err("DEM geodetic projection is not finite".into());
    }
    let (x, y) = deg2num(lat, lon, zoom);
    let tx = x.floor();
    let ty = y.floor();
    if tx < f64::from(i32::MIN)
        || tx > f64::from(i32::MAX)
        || ty < f64::from(i32::MIN)
        || ty > f64::from(i32::MAX)
    {
        return Err("DEM tile index exceeds source cache range".into());
    }
    Ok(Query {
        tile: Tile {
            x: tx as i32,
            y: ty as i32,
            zoom,
        },
        pixel: [(x - tx) * 256., (y - ty) * 256.],
    })
}
pub struct Field {
    width: usize,
    height: usize,
    values: Vec<f32>,
}
impl Field {
    pub fn from_rgba(
        width: u32,
        height: u32,
        rgba: &[u8],
        encoding: Encoding,
        available_bytes: usize,
    ) -> Result<Self, String> {
        let width = width as usize;
        let height = height as usize;
        let count = width.checked_mul(height).ok_or("DEM dimensions overflow")?;
        let bytes = count
            .checked_mul(4)
            .ok_or("DEM field byte count overflow")?;
        if width == 0 || height == 0 || rgba.len() != bytes {
            return Err("DEM image is empty or malformed".into());
        }
        if bytes > available_bytes {
            return Err("DEM field exceeds configured CPU budget".into());
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| "DEM field allocation refused")?;
        if values
            .capacity()
            .checked_mul(4)
            .ok_or("DEM field capacity overflow")?
            > available_bytes
        {
            return Err("DEM field capacity exceeds configured CPU budget".into());
        }
        for p in rgba.chunks_exact(4) {
            let r = f32::from(p[0]);
            let g = f32::from(p[1]);
            let b = f32::from(p[2]);
            values.push(match encoding {
                Encoding::Terrarium => (r * 256. + g + b / 256.) - 32768.,
                Encoding::Mapbox => -10000. + (r * 65536. + g * 256. + b) * 0.1,
            });
        }
        Ok(Self {
            width,
            height,
            values,
        })
    }
    pub fn resident_bytes(&self) -> usize {
        self.values.capacity() * 4
    }
    pub fn dimensions(&self) -> [usize; 2] {
        [self.width, self.height]
    }
    pub fn bilinear(&self, px: f64, py: f64) -> Result<f64, String> {
        if !px.is_finite() || !py.is_finite() {
            return Err("DEM pixel query is not finite".into());
        }
        let x = px.floor();
        let y = py.floor();
        let fx = px - x;
        let fy = py - y;
        let x0 = x.max(0.).min((self.width - 1) as f64) as usize;
        let y0 = y.max(0.).min((self.height - 1) as f64) as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        // NumPy 2 weak Python scalars narrow separately into the float32 law.
        let top = self.values[y0 * self.width + x0] * (1. - fx) as f32
            + self.values[y0 * self.width + x1] * fx as f32;
        let bot = self.values[y1 * self.width + x0] * (1. - fx) as f32
            + self.values[y1 * self.width + x1] * fx as f32;
        Ok(f64::from(top * (1. - fy) as f32 + bot * fy as f32))
    }
}

/// Original ensure_area's x-major/y-minor tile roster, represented by one
/// checked cursor rather than an eagerly allocated potentially huge Vec.
#[derive(Clone)]
pub struct Area {
    x: i32,
    y: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    zoom: u8,
    remaining: u64,
}
impl Area {
    pub fn for_path(datum: &SceneDatum, points: &[DVec3], zoom: u8) -> Result<Self, String> {
        if points.is_empty() || zoom > 30 {
            return Err("DEM area path or zoom is invalid".into());
        }
        let (mut south, mut west, mut north, mut east) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for point in points {
            if !point.is_finite() || !point.as_vec3().is_finite() {
                return Err("DEM area exceeds source finite coordinate representation".into());
            }
            let (la, lo, _) = datum.local_to_geodetic(point.as_vec3().as_dvec3());
            if !la.is_finite() || !lo.is_finite() {
                return Err("DEM area projection is not finite".into());
            }
            south = south.min(la);
            north = north.max(la);
            west = west.min(lo);
            east = east.max(lo);
        }
        let (a, b) = deg2num(north, west, zoom);
        let (c, d) = deg2num(south, east, zoom);
        let n = (1u64 << zoom) as f64;
        let xa = a.min(c).floor().max(0.);
        let xb = a.max(c).floor().min(n - 1.);
        let ya = b.min(d).floor().max(0.);
        let yb = b.max(d).floor().min(n - 1.);
        if xa > xb || ya > yb {
            return Ok(Self {
                x: 0,
                y: 0,
                x0: 0,
                y0: 0,
                x1: -1,
                y1: -1,
                zoom,
                remaining: 0,
            });
        }
        let (x0, x1, y0, y1) = (xa as i32, xb as i32, ya as i32, yb as i32);
        let count = (i64::from(x1) - i64::from(x0) + 1) as u64;
        let count = count
            .checked_mul((i64::from(y1) - i64::from(y0) + 1) as u64)
            .ok_or("DEM tile area count overflow")?;
        Ok(Self {
            x: x0,
            y: y0,
            x0,
            y0,
            x1,
            y1,
            zoom,
            remaining: count,
        })
    }
    pub fn key(&self) -> [i32; 5] {
        [self.x0, self.y0, self.x1, self.y1, i32::from(self.zoom)]
    }
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
    pub fn next_tile(&mut self) -> Option<Tile> {
        if self.remaining == 0 {
            return None;
        }
        let tile = Tile {
            x: self.x,
            y: self.y,
            zoom: self.zoom,
        };
        self.remaining -= 1;
        if self.y < self.y1 {
            self.y += 1;
        } else {
            self.x += 1;
            self.y = self.y0;
        }
        Some(tile)
    }
}
