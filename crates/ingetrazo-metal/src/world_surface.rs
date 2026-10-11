//! Cold source material submission. Native owners consume these immutable runs;
//! camera/model changes neither repack colors nor decode texture bytes.
use ingetrazo_render::{
    buffer::{MeshRenderBuffer, Vertex},
    native_textures,
    pipeline::SceneRenderData,
};
use std::collections::BTreeMap;
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAX_RUNS: usize = 65536;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    DefaultBack,
    TransparentFront,
    OpaqueBack,
    TransparentBack,
}
#[derive(Debug, Clone, Copy)]
pub struct Run {
    pub first: usize,
    pub count: usize,
    pub opacity: f32,
    pub cull_front: bool,
    pub front_only: bool,
    pub phase: Phase,
}
#[derive(Debug)]
pub struct Texture {
    pub path: String,
    pub geometry: native_textures::Geometry,
}
#[derive(Debug)]
pub struct Packet {
    pub opaque: MeshRenderBuffer,
    pub colored: MeshRenderBuffer,
    pub runs: Vec<Run>,
    pub textures: Vec<Texture>,
    pub retained_payload_bytes: usize,
}
fn rgb(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| (v.clamp(0., 1.) * 64.).round_ties_even() / 64.)
}
fn bucket(alpha: f64) -> Result<i32, String> {
    if !alpha.is_finite() || !(0. ..=1.).contains(&alpha) {
        return Err("Metal source opacity invalid".into());
    }
    let rounded = format!("{alpha:.3}")
        .parse::<f64>()
        .map_err(|_| "Metal source opacity rounding refused")?;
    Ok((rounded * 1000.).round() as i32)
}
fn validate(b: &MeshRenderBuffer) -> Result<(), String> {
    let bytes = b
        .vertices
        .len()
        .checked_mul(std::mem::size_of::<Vertex>())
        .and_then(|n| {
            b.indices
                .len()
                .checked_mul(4)
                .and_then(|i| n.checked_add(i))
        })
        .filter(|n| *n <= MAX_BYTES)
        .ok_or("Metal material geometry exceeds256MiB")?;
    let _ = bytes;
    if b.indices.len() % 3 != 0
        || b.indices.iter().any(|&i| i as usize >= b.vertices.len())
        || b.vertices.iter().any(|v| {
            v.position
                .iter()
                .chain(v.normal.iter())
                .chain(v.color.iter())
                .chain(v.uv.iter())
                .any(|v| !v.is_finite())
                || v.color.iter().any(|v| !(0. ..=1.).contains(v))
        })
    {
        return Err("Metal material vertex/index invalid".into());
    }
    let mut end = 0;
    for &(first, count) in &b.source_front_cull {
        span(first, count, b.indices.len())?;
        if first < end {
            return Err("Metal front-cull spans unordered".into());
        }
        end = first + count;
    }
    let mut last = None;
    for &(first, opacity) in &b.source_opacity_runs {
        if first % 3 != 0 || first >= b.indices.len() || last.is_some_and(|old| first <= old) {
            return Err("Metal opacity spans unordered".into());
        }
        bucket(opacity)?;
        last = Some(first);
    }
    for p in &b.source_back_paints {
        span(p.first, p.count, b.indices.len())?;
        bucket(p.opacity)?;
        if p.color
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Metal source back color invalid".into());
        }
    }
    Ok(())
}
fn span(first: usize, count: usize, len: usize) -> Result<(), String> {
    if first % 3 != 0
        || count == 0
        || count % 3 != 0
        || first.checked_add(count).is_none_or(|n| n > len)
    {
        return Err("Metal material triangle span invalid".into());
    }
    Ok(())
}
impl Packet {
    pub fn prepare(data: SceneRenderData) -> Result<Self, String> {
        if !data.billboards.is_empty() {
            return Err("Metal material billboard stage unavailable".into());
        }
        validate(&data.opaque_mesh)?;
        validate(&data.transparent_mesh)?;
        let texture_runs = data.textured_meshes.values().try_fold(0usize, |n, b| {
            n.checked_add(b.source_texture_runs.len())
                .ok_or("Metal texture runs overflow")
        })?;
        let paint_runs = [&data.opaque_mesh, &data.transparent_mesh]
            .into_iter()
            .chain(data.textured_meshes.values())
            .try_fold(texture_runs, |n, b| {
                n.checked_add(b.source_back_paints.len())
                    .and_then(|n| n.checked_add(b.source_opacity_runs.len()))
                    .ok_or("Metal paint runs overflow")
            })?;
        if paint_runs > MAX_RUNS || data.textured_meshes.len() > 4096 {
            return Err("Metal material metadata admission exceeded".into());
        }
        // Admission precedes bucket copies: worst case one expanded vertex for every
        // transparent index and every independently painted back index.
        let back_indices = [&data.opaque_mesh, &data.transparent_mesh]
            .into_iter()
            .chain(data.textured_meshes.values())
            .try_fold(0usize, |n, b| {
                b.source_back_paints
                    .iter()
                    .try_fold(n, |n, p| -> Result<usize, String> {
                        span(p.first, p.count, b.indices.len())?;
                        n.checked_add(p.count)
                            .ok_or("Metal back geometry size overflow".into())
                    })
            })?;
        let default_indices = data.textured_meshes.values().try_fold(0usize, |n, b| {
            b.source_default_back.iter().try_fold(
                n,
                |n, &(first, count)| -> Result<usize, String> {
                    span(first, count, b.indices.len())?;
                    n.checked_add(count)
                        .ok_or("Metal texture default back size overflow".into())
                },
            )
        })?;
        let expanded = data
            .transparent_mesh
            .indices
            .len()
            .checked_add(default_indices)
            .and_then(|n| n.checked_add(back_indices))
            .ok_or("Metal material geometry overflow")?;
        let bound = expanded
            .checked_mul(std::mem::size_of::<Vertex>() * 2 + 8)
            .and_then(|n| {
                paint_runs
                    .checked_add(expanded / 3)
                    .and_then(|runs| runs.checked_mul(std::mem::size_of::<Run>() + 64))
                    .and_then(|m| n.checked_add(m))
            })
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("Metal expanded material cold work exceeds256MiB")?;
        let _ = bound;
        let texture_work = data.textured_meshes.values().try_fold(0usize, |n, b| {
            b.indices
                .len()
                .checked_mul(std::mem::size_of::<native_textures::TextureVertex>() + 1)
                .and_then(|v| n.checked_add(v))
                .and_then(|n| {
                    b.source_texture_runs
                        .len()
                        .checked_mul(std::mem::size_of::<native_textures::Draw>())
                        .and_then(|v| n.checked_add(v))
                })
                .ok_or("Metal texture cold geometry overflow")
        })?;
        let opaque_work = data
            .opaque_mesh
            .vertices
            .len()
            .checked_mul(std::mem::size_of::<Vertex>())
            .and_then(|n| {
                data.opaque_mesh
                    .indices
                    .len()
                    .checked_mul(4)
                    .and_then(|v| n.checked_add(v))
            })
            .ok_or("Metal opaque geometry work overflow")?;
        if bound
            .checked_add(texture_work)
            .and_then(|n| n.checked_add(opaque_work))
            .is_none_or(|n| n > MAX_BYTES)
        {
            return Err("Metal aggregate material cold work exceeds256MiB".into());
        }
        let mut fronts = BTreeMap::<(i32, bool), Vec<Vertex>>::new();
        let mut opacity_span = 0;
        let mut cull_span = 0;
        let mesh = &data.transparent_mesh;
        for (i, triangle) in mesh.indices.chunks_exact(3).enumerate() {
            let offset = i * 3;
            while opacity_span + 1 < mesh.source_opacity_runs.len()
                && mesh.source_opacity_runs[opacity_span + 1].0 <= offset
            {
                opacity_span += 1;
            }
            while cull_span < mesh.source_front_cull.len()
                && mesh.source_front_cull[cull_span].0 + mesh.source_front_cull[cull_span].1
                    <= offset
            {
                cull_span += 1;
            }
            let cull = mesh
                .source_front_cull
                .get(cull_span)
                .is_some_and(|r| r.0 <= offset && offset < r.0 + r.1);
            let opacity = mesh
                .source_opacity_runs
                .get(opacity_span)
                .filter(|r| r.0 <= offset)
                .map_or(mesh.vertices[triangle[0] as usize].color[3] as f64, |r| r.1);
            let key = bucket(opacity)?;
            let out = fronts.entry((key, cull)).or_default();
            out.try_reserve(3)
                .map_err(|_| "Metal alpha bucket allocation refused")?;
            for &index in triangle {
                let mut v = mesh.vertices[index as usize];
                let c = rgb([v.color[0], v.color[1], v.color[2]]);
                v.color = [c[0], c[1], c[2], key as f32 / 1000.];
                out.push(v);
            }
        }
        let mut colored = MeshRenderBuffer::default();
        let mut runs = Vec::new();
        runs.try_reserve_exact(
            paint_runs
                .checked_add(fronts.len())
                .and_then(|n| n.checked_add(1))
                .ok_or("Metal run count overflow")?,
        )
        .map_err(|_| "Metal material run allocation refused")?;
        fn append(
            colored: &mut MeshRenderBuffer,
            runs: &mut Vec<Run>,
            vertices: Vec<Vertex>,
            key: i32,
            cull: bool,
            phase: Phase,
        ) -> Result<(), String> {
            let first = colored.indices.len();
            let count = vertices.len();
            if count == 0 {
                return Ok(());
            }
            let base = u32::try_from(colored.vertices.len())
                .map_err(|_| "Metal material index overflow")?;
            colored
                .vertices
                .try_reserve_exact(count)
                .map_err(|_| "Metal material vertex allocation refused")?;
            colored
                .indices
                .try_reserve_exact(count)
                .map_err(|_| "Metal material index allocation refused")?;
            for i in 0..count {
                colored.indices.push(
                    base.checked_add(
                        u32::try_from(i).map_err(|_| "Metal material index overflow")?,
                    )
                    .ok_or("Metal material index overflow")?,
                );
            }
            colored.vertices.extend(vertices);
            runs.push(Run {
                first,
                count,
                opacity: key as f32 / 1000.,
                cull_front: phase != Phase::TransparentFront,
                front_only: cull,
                phase,
            });
            Ok(())
        }
        for ((key, cull), vertices) in fronts {
            append(
                &mut colored,
                &mut runs,
                vertices,
                key,
                cull,
                Phase::TransparentFront,
            )?;
        }
        let mut defaults = Vec::new();
        defaults
            .try_reserve_exact(default_indices)
            .map_err(|_| "Metal texture default back allocation refused")?;
        for buffer in data.textured_meshes.values() {
            for &(first, count) in &buffer.source_default_back {
                for &index in &buffer.indices[first..first + count] {
                    defaults.push(buffer.vertices[index as usize]);
                }
            }
        }
        append(
            &mut colored,
            &mut runs,
            defaults,
            1000,
            false,
            Phase::DefaultBack,
        )?;
        let mut paints = Vec::new();
        paints
            .try_reserve_exact(paint_runs)
            .map_err(|_| "Metal back ordering allocation refused")?;
        for source in [&data.opaque_mesh, &data.transparent_mesh]
            .into_iter()
            .chain(data.textured_meshes.values())
        {
            for paint in &source.source_back_paints {
                paints.push((paints.len(), source, paint));
            }
        }
        paints.sort_unstable_by_key(|(index, _, p)| (p.order, *index));
        let mut backs = BTreeMap::<i32, Vec<Vertex>>::new();
        let mut opaque_back = Vec::new();
        for (_, source, p) in paints {
            let key = bucket(p.opacity)?;
            let out = if p.opacity < 0.999 {
                backs.entry(key).or_default()
            } else {
                &mut opaque_back
            };
            out.try_reserve(p.count)
                .map_err(|_| "Metal back bucket allocation refused")?;
            let c = rgb(p.color);
            for &index in &source.indices[p.first..p.first + p.count] {
                let mut v = source.vertices[index as usize];
                v.color = [c[0], c[1], c[2], key as f32 / 1000.];
                out.push(v);
            }
        }
        append(
            &mut colored,
            &mut runs,
            opaque_back,
            1000,
            false,
            Phase::OpaqueBack,
        )?;
        for (key, vertices) in backs {
            append(
                &mut colored,
                &mut runs,
                vertices,
                key,
                false,
                Phase::TransparentBack,
            )?;
        }
        let mut textures = Vec::new();
        textures
            .try_reserve_exact(data.textured_meshes.len())
            .map_err(|_| "Metal texture list allocation refused")?;
        for (path, buffer) in data.textured_meshes {
            if path.len() > 4096 {
                return Err("Metal texture resource path exceeds4096".into());
            }
            validate(&buffer)?;
            textures.push(Texture {
                path,
                geometry: native_textures::Geometry::prepare(&buffer)?,
            });
        }
        textures.sort_unstable_by_key(|t| {
            t.geometry
                .draws
                .first()
                .map_or(usize::MAX, |d| d.source.order)
        });
        let retained_payload_bytes = colored
            .vertices
            .capacity()
            .checked_mul(std::mem::size_of::<Vertex>())
            .and_then(|n| {
                colored
                    .indices
                    .capacity()
                    .checked_mul(4)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                runs.capacity()
                    .checked_mul(std::mem::size_of::<Run>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                textures
                    .capacity()
                    .checked_mul(std::mem::size_of::<Texture>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or("Metal material retained size overflow")?;
        let retained_payload_bytes = textures.iter().try_fold(retained_payload_bytes, |n, t| {
            n.checked_add(t.path.capacity())
                .and_then(|n| {
                    t.geometry
                        .vertices
                        .capacity()
                        .checked_mul(std::mem::size_of::<native_textures::TextureVertex>())
                        .and_then(|v| n.checked_add(v))
                })
                .and_then(|n| {
                    t.geometry
                        .draws
                        .capacity()
                        .checked_mul(std::mem::size_of::<native_textures::Draw>())
                        .and_then(|v| n.checked_add(v))
                })
                .ok_or("Metal texture retained size overflow")
        })?;
        if retained_payload_bytes > MAX_BYTES {
            return Err("Metal retained material capacity exceeds256MiB".into());
        }
        Ok(Self {
            opaque: data.opaque_mesh,
            colored,
            runs,
            textures,
            retained_payload_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_render::buffer::SourceBackPaint;
    fn triangle(alpha: f32, x: f32) -> MeshRenderBuffer {
        let mut m = MeshRenderBuffer::new();
        let v = Vertex::new([x, 0., 0.], [0., 0., 1.], [1., 0., 0., alpha], [0., 0.]);
        m.push_triangle(v, v, v);
        m
    }
    #[test]
    fn material_bins_preserve_source_opacity_cull_and_back_pass_order() {
        let mut data = SceneRenderData::default();
        data.transparent_mesh = triangle(0.75, 7.);
        data.transparent_mesh.append(&triangle(0.5, 5.));
        data.transparent_mesh.source_opacity_runs = vec![(0, 0.75), (3, 0.5)];
        data.transparent_mesh.source_front_cull = vec![(3, 3)];
        data.opaque_mesh = triangle(1., 1.);
        data.opaque_mesh.source_back_paints = vec![
            SourceBackPaint {
                first: 0,
                count: 3,
                order: 1,
                color: [0., 0., 1.],
                opacity: 1.,
            },
            SourceBackPaint {
                first: 0,
                count: 3,
                order: 0,
                color: [0., 1., 0.],
                opacity: 0.25,
            },
        ];
        let packet = Packet::prepare(data).unwrap();
        assert_eq!(
            packet
                .runs
                .iter()
                .map(|r| (r.phase, r.opacity, r.front_only))
                .collect::<Vec<_>>(),
            vec![
                (Phase::TransparentFront, 0.5, true),
                (Phase::TransparentFront, 0.75, false),
                (Phase::OpaqueBack, 1., false),
                (Phase::TransparentBack, 0.25, false)
            ]
        );
        assert_eq!(packet.colored.vertices[0].position[0], 5.);
        assert_eq!(packet.colored.vertices[3].position[0], 7.);
        assert_eq!(packet.colored.vertices[6].color, [0., 0., 1., 1.]);
        assert_eq!(packet.colored.vertices[9].color, [0., 1., 0., 0.25]);
    }
    #[test]
    fn textured_default_back_is_retained_separately_from_sampled_front() {
        let mut data = SceneRenderData::default();
        let mut mesh = triangle(1., 0.);
        mesh.source_default_back = vec![(0, 3)];
        mesh.source_texture_runs
            .push(ingetrazo_render::buffer::SourceTextureRun {
                back_side: false,
                order: 0,
                first: 0,
                count: 3,
                shade: 1.,
                opacity: 1.,
                front_cull: false,
            });
        *data.textured_meshes.entry("image".into()).or_default() = mesh;
        let packet = Packet::prepare(data).unwrap();
        assert_eq!(packet.textures.len(), 1);
        assert_eq!(packet.runs.len(), 1);
        assert_eq!(packet.runs[0].phase, Phase::DefaultBack);
        assert_eq!(packet.runs[0].count, 3);
        assert!(packet.runs[0].cull_front);
    }
    #[test]
    fn malformed_paints_refuse_before_material_copy() {
        let mut data = SceneRenderData::default();
        data.transparent_mesh = triangle(0.5, 0.);
        data.transparent_mesh.source_opacity_runs = vec![(1, 0.5)];
        assert!(Packet::prepare(data).is_err());
        let mut data = SceneRenderData::default();
        data.opaque_mesh = triangle(1., 0.);
        data.opaque_mesh.source_back_paints.push(SourceBackPaint {
            first: 0,
            count: 6,
            order: 0,
            color: [1.; 3],
            opacity: 1.,
        });
        assert!(Packet::prepare(data).is_err());
    }
}
