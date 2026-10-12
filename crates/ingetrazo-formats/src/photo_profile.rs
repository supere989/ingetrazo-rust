//! Source PhotoMesh XY highest-hit sampler, independent of topology/terrain grids.
use std::io::{BufReader, Cursor, Read};
use std::sync::{Mutex, OnceLock};
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 256 * 1024 * 1024,
        }
    }
}
/// Literal original PhotoMesh._ensure_index incidence law, not a native mesh cap.
pub const SOURCE_MAX_INCIDENCES: usize = 16_000_000;
const DECODE_SCRATCH: usize = 64 * 1024;
fn checked(n: usize, width: usize) -> Result<usize, String> {
    n.checked_mul(width)
        .ok_or_else(|| "Survey allocation size overflow".into())
}
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or_else(|| "Survey allocation size overflow".into())
}
fn admit(bytes: usize, limit: usize) -> Result<(), String> {
    if bytes > limit {
        Err(format!(
            "Survey profile CPU allocation {bytes} exceeds configured budget {limit}"
        ))
    } else {
        Ok(())
    }
}
fn vector<T>(count: usize, other: usize, limit: usize) -> Result<Vec<T>, String> {
    admit(
        add(checked(count, std::mem::size_of::<T>())?, other)?,
        limit,
    )?;
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| "Survey profile allocation refused")?;
    admit(
        add(checked(v.capacity(), std::mem::size_of::<T>())?, other)?,
        limit,
    )?;
    Ok(v)
}
struct Header {
    count: usize,
    bytes: usize,
    offset: usize,
}
fn header(file: &mut impl Read, size: u64, kind: &str, columns: usize) -> Result<Header, String> {
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic[..6] != b"\x93NUMPY" || !(1..=3).contains(&magic[6]) || magic[7] != 0 {
        return Err("Unsupported survey array header".into());
    }
    let lengthbytes = if magic[6] == 1 { 2 } else { 4 };
    let mut len = [0u8; 4];
    file.read_exact(&mut len[..lengthbytes])
        .map_err(|e| e.to_string())?;
    let n = if lengthbytes == 2 {
        u16::from_le_bytes([len[0], len[1]]) as usize
    } else {
        u32::from_le_bytes(len) as usize
    };
    if n > 4096 {
        return Err("Survey array header exceeds supported size".into());
    }
    let mut text = [0u8; 4096];
    file.read_exact(&mut text[..n]).map_err(|e| e.to_string())?;
    let text = std::str::from_utf8(&text[..n]).map_err(|e| e.to_string())?;
    if !text.contains(&format!("'descr': '{kind}'")) || !text.contains("'fortran_order': False") {
        return Err("Unsupported survey array layout (expected source little-endian float32/uint32 C arrays)".into());
    }
    let shape = text
        .split("'shape':")
        .nth(1)
        .and_then(|s| s.split('(').nth(1))
        .and_then(|s| s.split(')').next())
        .ok_or("Missing survey array shape")?;
    let mut dims = shape.split(',').map(str::trim).filter(|s| !s.is_empty());
    let count = dims
        .next()
        .ok_or("Missing survey array count")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    if dims.next().and_then(|s| s.parse::<usize>().ok()) != Some(columns) || dims.next().is_some() {
        return Err("Unsupported survey array shape".into());
    }
    let bytes = checked(count, checked(columns, 4)?)?;
    let offset = add(8 + lengthbytes, n)?;
    if size != add(bytes, offset)? as u64 {
        return Err("Survey array payload size mismatch".into());
    }
    Ok(Header {
        count,
        bytes,
        offset,
    })
}
#[derive(Debug)]
struct Grid {
    ids: Vec<u32>,
    starts: Vec<usize>,
    minx: f64,
    miny: f64,
    cell: f64,
    nx: usize,
    ny: usize,
}
#[derive(Debug)]
pub struct Survey {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
    index: OnceLock<Option<Grid>>,
    index_init: Mutex<()>,
    limits: Limits,
}
impl Survey {
    pub fn decode(bytes: &[u8], limits: Limits) -> Result<Self, String> {
        admit(DECODE_SCRATCH, limits.bytes)?;
        let mut archive =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Survey NPZ: {e}"))?;
        let mut vf = BufReader::with_capacity(
            32 * 1024,
            archive.by_name("vertices.npy").map_err(|e| e.to_string())?,
        );
        let size = vf.get_ref().size();
        let h = header(&mut vf, size, "<f4", 3)?;
        let mut vertices = vector::<[f32; 3]>(h.count, DECODE_SCRATCH, limits.bytes)?;
        let mut raw = [0u8; 12];
        for _ in 0..h.count {
            vf.read_exact(&mut raw).map_err(|e| e.to_string())?;
            let p = std::array::from_fn(|i| {
                f32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
            });
            if p.iter().any(|x| !x.is_finite()) {
                return Err("Survey vertices are not finite".into());
            }
            vertices.push(p);
        }
        drop(vf);
        let vertexbytes = checked(vertices.capacity(), 12)?;
        let mut tf = BufReader::with_capacity(
            32 * 1024,
            archive
                .by_name("triangles.npy")
                .map_err(|e| e.to_string())?,
        );
        let size = tf.get_ref().size();
        let h = header(&mut tf, size, "<u4", 3)?;
        if h.count > u32::MAX as usize {
            return Err("Survey triangle owner exceeds source index representation".into());
        }
        let mut triangles =
            vector::<[u32; 3]>(h.count, add(vertexbytes, DECODE_SCRATCH)?, limits.bytes)?;
        for _ in 0..h.count {
            tf.read_exact(&mut raw).map_err(|e| e.to_string())?;
            let row = std::array::from_fn(|i| {
                u32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
            });
            if row.iter().any(|&i| i as usize >= vertices.len()) {
                return Err("Survey triangle vertex no longer resolves".into());
            }
            triangles.push(row);
        }
        drop(tf);
        // Source unpack_mesh needs the UV member even though elevation ignores UVs.
        let mut uv = archive.by_name("uvs.npy").map_err(|e| e.to_string())?;
        let size = uv.size();
        let h = header(&mut uv, size, "<f4", 2)?;
        if h.count != vertices.len() {
            return Err("Survey UV owner count differs from source vertex array".into());
        }
        // Consume the complete member, including ZIP CRC verification, without
        // allocating an unused UV array. Source unpack_mesh also reads this owner.
        let consumed = std::io::copy(&mut uv, &mut std::io::sink())
            .map_err(|e| format!("Survey UV payload: {e}"))?;
        if consumed != h.bytes as u64 {
            return Err("Survey UV payload size mismatch".into());
        }
        let _ = h.offset;
        Ok(Self {
            vertices,
            triangles,
            index: OnceLock::new(),
            index_init: Mutex::new(()),
            limits,
        })
    }
    pub fn resident_bytes(&self) -> usize {
        self.vertices.capacity() * 12
            + self.triangles.capacity() * 12
            + self
                .index
                .get()
                .and_then(|g| g.as_ref())
                .map(|g| g.ids.capacity() * 4 + g.starts.capacity() * std::mem::size_of::<usize>())
                .unwrap_or(0)
    }
    pub fn ensure_index(&self) -> Result<(), String> {
        self.ensure_index_with_limit(self.limits.bytes)
    }
    pub fn ensure_index_with_limit(&self, limit: usize) -> Result<(), String> {
        if self.index.get().is_some() {
            return Ok(());
        }
        let _guard = self.index_init.lock().unwrap_or_else(|e| e.into_inner());
        if self.index.get().is_some() {
            return Ok(());
        }
        let index = self.build_index(limit.min(self.limits.bytes))?;
        let _ = self.index.set(index);
        Ok(())
    }
    pub fn source_index_unavailable(&self) -> bool {
        matches!(self.index.get(), Some(None))
    }
    fn build_index(&self, limit: usize) -> Result<Option<Grid>, String> {
        if self.triangles.is_empty() || self.vertices.is_empty() {
            return Ok(None);
        }
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        for tri in &self.triangles {
            for &id in tri {
                let p = self.vertices[id as usize];
                for axis in 0..2 {
                    lo[axis] = lo[axis].min(p[axis]);
                    hi[axis] = hi[axis].max(p[axis]);
                }
            }
        }
        let width = (f64::from(hi[0]) - f64::from(lo[0])).max(1e-6);
        let height = (f64::from(hi[1]) - f64::from(lo[1])).max(1e-6);
        let target = (libm::sqrt(self.triangles.len() as f64 / 4.) as usize)
            .max(1)
            .min(2048);
        let cell = (width / target as f64).max(height / target as f64);
        let nx = ((width / cell) as usize + 1).max(1);
        let ny = ((height / cell) as usize + 1).max(1);
        let cells = checked(nx, ny)?;
        let bounds = |tri: &[u32; 3]| {
            let mut min = [f32::INFINITY; 2];
            let mut max = [f32::NEG_INFINITY; 2];
            for &id in tri {
                let p = self.vertices[id as usize];
                for a in 0..2 {
                    min[a] = min[a].min(p[a]);
                    max[a] = max[a].max(p[a]);
                }
            }
            let index = |p: f32, axis: usize, n: usize| -> usize {
                floor_div32(p - lo[axis], cell as f32)
                    .max(0.)
                    .min((n - 1) as f32) as usize
            };
            (
                index(min[0], 0, nx),
                index(max[0], 0, nx),
                index(min[1], 1, ny),
                index(max[1], 1, ny),
            )
        };
        let mut total = 0usize;
        for tri in &self.triangles {
            let (i0, i1, j0, j1) = bounds(tri);
            total = add(total, checked(i1 - i0 + 1, j1 - j0 + 1)?)?;
            if total > SOURCE_MAX_INCIDENCES {
                return Ok(None);
            }
        }
        let resident = self.resident_bytes();
        let countbytes = checked(cells, std::mem::size_of::<usize>())?;
        let startbytes = checked(add(cells, 1)?, std::mem::size_of::<usize>())?;
        let peak = add(
            add(add(resident, countbytes)?, startbytes)?,
            checked(total, 4)?,
        )?;
        admit(peak, limit)?;
        let mut counts = vector::<usize>(cells, resident, limit)?;
        counts.resize(cells, 0);
        let countbytes = checked(counts.capacity(), std::mem::size_of::<usize>())?;
        for tri in &self.triangles {
            let (i0, i1, j0, j1) = bounds(tri);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    counts[j * nx + i] += 1;
                }
            }
        }
        let mut starts = vector::<usize>(cells + 1, add(resident, countbytes)?, limit)?;
        starts.push(0);
        for &count in &counts {
            starts.push(starts.last().unwrap() + count);
        }
        let startbytes = checked(starts.capacity(), std::mem::size_of::<usize>())?;
        let mut ids = vector::<u32>(total, add(add(resident, countbytes)?, startbytes)?, limit)?;
        ids.resize(total, 0);
        counts.copy_from_slice(&starts[..cells]);
        for (id, tri) in self.triangles.iter().enumerate() {
            let (i0, i1, j0, j1) = bounds(tri);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let c = j * nx + i;
                    ids[counts[c]] = id as u32;
                    counts[c] += 1;
                }
            }
        }
        Ok(Some(Grid {
            ids,
            starts,
            minx: lo[0] as f64,
            miny: lo[1] as f64,
            cell,
            nx,
            ny,
        }))
    }
    pub fn height_at(&self, x: f32, y: f32) -> Result<Option<f32>, String> {
        self.ensure_index()?;
        let Some(grid) = self.index.get().unwrap().as_ref() else {
            return Ok(None);
        };
        let i = floor_div64(f64::from(x) - grid.minx, grid.cell);
        let j = floor_div64(f64::from(y) - grid.miny, grid.cell);
        if !i.is_finite()
            || !j.is_finite()
            || i < 0.
            || j < 0.
            || i >= grid.nx as f64
            || j >= grid.ny as f64
        {
            return Ok(None);
        }
        let c = j as usize * grid.nx + i as usize;
        let mut height: Option<f32> = None;
        for &id in &grid.ids[grid.starts[c]..grid.starts[c + 1]] {
            let tri = self.triangles[id as usize];
            let [a, b, c] = tri.map(|i| self.vertices[i as usize]);
            let v0x = b[0] - a[0];
            let v0y = b[1] - a[1];
            let v1x = c[0] - a[0];
            let v1y = c[1] - a[1];
            let det = v0x * v1y - v1x * v0y;
            if det.abs() <= 1e-12f32 {
                continue;
            }
            let px = x - a[0];
            let py = y - a[1];
            let u = (px * v1y - v1x * py) / det;
            let v = (v0x * py - px * v0y) / det;
            // NumPy2 weak scalars narrow the source 1+1e-9 upper threshold to f32=1.
            if u >= -1e-9f32 && v >= -1e-9f32 && u + v <= 1. {
                let z = (a[2] + u * (b[2] - a[2])) + v * (c[2] - a[2]);
                height = Some(height.map(|old| old.max(z)).unwrap_or(z));
            }
        }
        Ok(height)
    }
}
fn floor_div32(a: f32, b: f32) -> f32 {
    let mut modulo = libm::fmodf(a, b);
    let mut div = (a - modulo) / b;
    if modulo != 0. && (modulo < 0.) != (b < 0.) {
        modulo += b;
        div -= 1.;
    }
    let _ = modulo;
    if div == 0. {
        return 0.0f32.copysign(a / b);
    }
    let floor = libm::floorf(div);
    if div - floor > 0.5 {
        floor + 1.
    } else {
        floor
    }
}
fn floor_div64(a: f64, b: f64) -> f64 {
    let modulo = libm::fmod(a, b);
    let mut div = (a - modulo) / b;
    if modulo != 0. && (modulo < 0.) != (b < 0.) {
        div -= 1.;
    }
    if div == 0. {
        return 0.0f64.copysign(a / b);
    }
    let floor = libm::floor(div);
    if div - floor > 0.5 {
        floor + 1.
    } else {
        floor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn survey(triangles: Vec<[u32; 3]>) -> Survey {
        Survey {
            vertices: vec![
                [0., 0., 0.],
                [16., 0., 8.],
                [0., 16., 4.],
                [0., 0., 10.],
                [16., 0., 10.],
                [0., 16., 10.],
                [4., 4., -20.],
                [4., 4., 20.],
                [4., 10., 0.],
            ],
            triangles,
            index: OnceLock::new(),
            index_init: Mutex::new(()),
            limits: Limits::default(),
        }
    }
    #[test]
    fn source_highest_hit_vertical_exclusion_and_footprint_gap() {
        let s = survey(vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]]);
        assert_eq!(s.height_at(4., 4.).unwrap(), Some(10.));
        assert_eq!(s.height_at(-1., 0.).unwrap(), None);
        assert_eq!(s.height_at(16., 0.).unwrap(), Some(10.));
    }
    #[test]
    fn source_incidence_unavailable_and_budget_refusal_are_distinct() {
        let s = survey(vec![[0, 1, 2]; 16384]);
        assert_eq!(s.height_at(1., 1.).unwrap(), None);
        assert!(s.source_index_unavailable());
        let s = survey(vec![[0, 1, 2]]);
        assert!(s.ensure_index_with_limit(0).is_err());
        assert!(s.index.get().is_none());
        assert!(s.ensure_index().is_ok());
        assert_eq!(s.height_at(4., 4.).unwrap(), Some(3.));
    }
    #[test]
    fn source_floor_division_returns_signed_zero() {
        assert_eq!(floor_div32(0.25, 1.).to_bits(), 0.0f32.to_bits());
        assert_eq!(floor_div64(0.25, 1.).to_bits(), 0.0f64.to_bits());
        assert_eq!(floor_div32(-0., 1.).to_bits(), (-0.0f32).to_bits());
        assert_eq!(floor_div64(-0., 1.).to_bits(), (-0.0f64).to_bits());
    }
}
