// SPDX-License-Identifier: GPL-3.0-or-later AND MIT
// Pinned OpenSKP _core.py dimension scanner and connection resolver,291700bc.
use super::*;
use std::collections::HashMap;
type Transform = [f64; 13];
// Candidate records are shallow borrowed TLVs. Charge cumulative work across
// overlapping byte-scan candidates and nested connection wrappers, not per call.
#[derive(Default)]
struct Work {
    bytes: usize,
    nodes: usize,
}
impl Work {
    fn parse<'a>(&mut self, bytes: &'a [u8]) -> Result<Vec<Node<'a>>, SkpError> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|&n| n <= 256 * 1024 * 1024)
            .ok_or_else(|| SkpError::Invalid("Dimension candidate byte admission".into()))?;
        let mut out = Vec::new();
        let mut pos = 0;
        while pos < bytes.len() {
            if bytes.len() - pos < 6 {
                return Ok(Vec::new());
            }
            let size = u32::from_le_bytes(bytes[pos + 2..pos + 6].try_into().unwrap()) as usize;
            let Some(end) = pos
                .checked_add(6)
                .and_then(|n| n.checked_add(size))
                .filter(|&n| n <= bytes.len())
            else {
                return Ok(Vec::new());
            };
            self.nodes = self
                .nodes
                .checked_add(1)
                .filter(|&n| n <= MAX_NODES)
                .ok_or_else(|| SkpError::Invalid("Dimension candidate node admission".into()))?;
            out.push(Node {
                tag: [bytes[pos], bytes[pos + 1]],
                body: &bytes[pos + 6..end],
                children: Vec::new(),
            });
            pos = end;
        }
        Ok(out)
    }
}
fn id_bytes<'a>(info: &Node<'a>) -> Option<&'a [u8]> {
    if info.body.starts_with(&[0xde, 5]) {
        let n = u32::from_le_bytes(info.body.get(2..6)?.try_into().ok()?) as usize;
        info.body.get(6..6usize.checked_add(n)?)
    } else {
        Some(info.body)
    }
}
fn triplet(b: &[u8]) -> Result<Option<[f64; 3]>, SkpError> {
    if b.len() != 24 {
        return Ok(None);
    }
    let mut p = [0.; 3];
    for (v, b) in p.iter_mut().zip(b.chunks_exact(8)) {
        *v = f64::from_le_bytes(b.try_into().unwrap());
        if !v.is_finite() {
            return Err(SkpError::Invalid("Nonfinite dimension geometry".into()));
        }
    }
    Ok(Some(p))
}
fn compose(parent: Option<Transform>, child: Option<Transform>) -> Option<Transform> {
    let (Some(a), Some(b)) = (parent, child) else {
        return child.or(parent);
    };
    let mut out = [0.; 13];
    for r in 0..3 {
        for c in 0..3 {
            out[r * 3 + c] = (0..4)
                .map(|i| {
                    let row = if i == 3 { a[9 + r] } else { a[r * 3 + i] };
                    let col = if i == 3 { 0. } else { b[i * 3 + c] };
                    row * col
                })
                .sum();
        }
        out[9 + r] = (0..4)
            .map(|i| {
                let row = if i == 3 { a[9 + r] } else { a[r * 3 + i] };
                let col = if i == 3 { 1. } else { b[9 + i] };
                row * col
            })
            .sum();
    }
    out[12] = a[12] * b[12];
    Some(out)
}
fn scan<'a>(
    nodes: &[Node<'a>],
    parent: Option<Transform>,
    vertices: &mut HashMap<&'a [u8], [f64; 3]>,
    instances: &mut HashMap<&'a [u8], Transform>,
) -> Result<(), SkpError> {
    for n in nodes {
        if n.tag == [0xc4, 9] {
            if let (Some(info), Some(pos)) = (n.child([0xdc, 5]), n.child([0xc5, 9])) {
                if let (Some(id), Some(p)) = (id_bytes(info), triplet(pos.body)?) {
                    vertices.insert(id, p);
                }
            }
        }
        let mut here = parent;
        if n.tag == [0x64, 0x19] {
            let local = if let Some(body) = n
                .child([0x66, 0x19])
                .map(|n| n.body)
                .filter(|b| b.len() == 104)
            {
                let mut m = [0.; 13];
                for (v, b) in m.iter_mut().zip(body.chunks_exact(8)) {
                    *v = f64::from_le_bytes(b.try_into().unwrap());
                    if !v.is_finite() {
                        return Err(SkpError::Invalid(
                            "Nonfinite dimension instance transform".into(),
                        ));
                    }
                }
                Some(m)
            } else {
                None
            };
            here = compose(parent, local);
            if here.is_some_and(|m| !m.iter().all(|v| v.is_finite())) {
                return Err(SkpError::Invalid(
                    "Dimension instance composition overflow".into(),
                ));
            }
            if let (Some(info), Some(matrix)) =
                (n.child([0xd0, 7]).and_then(|n| n.child([0xdc, 5])), here)
            {
                if let Some(id) = id_bytes(info) {
                    instances.insert(id, matrix);
                }
            }
        }
        if vertices.len().saturating_add(instances.len()) > (128 * 1024 * 1024) / 256 {
            return Err(SkpError::Invalid(
                "Dimension index retained allocation admission".into(),
            ));
        }
        scan(&n.children, here, vertices, instances)?;
    }
    Ok(())
}
fn point(
    body: Option<&Node<'_>>,
    vertices: &HashMap<&[u8], [f64; 3]>,
    instances: &HashMap<&[u8], Transform>,
    work: &mut Work,
) -> Result<Option<[f64; 3]>, SkpError> {
    let Some(body) = body else {
        return Ok(None);
    };
    let nodes = work.parse(body.body)?;
    let Some(block) = nodes.iter().find(|n| n.tag == [8, 0x52]) else {
        return Ok(None);
    };
    let nodes = work.parse(block.body)?;
    if nodes.iter().find(|n| n.tag == [9, 0x52]).map(|n| n.body)
        == Some(1u32.to_le_bytes().as_slice())
    {
        return nodes
            .iter()
            .find(|n| n.tag == [10, 0x52])
            .map(|n| triplet(n.body))
            .unwrap_or(Ok(None));
    }
    let Some(reference) = nodes.iter().find(|n| n.tag == [11, 0x52]) else {
        return Ok(None);
    };
    let refs = work.parse(reference.body)?;
    let Some(holder) = refs.iter().find(|n| n.tag == [0xfc, 0x53]) else {
        return Ok(None);
    };
    let refs = work.parse(holder.body)?;
    let Some(id) = refs.iter().find(|n| n.tag == [0xfd, 0x53]).map(|n| n.body) else {
        return Ok(None);
    };
    let Some(&local) = vertices.get(id) else {
        return Ok(None);
    };
    if let Some(id) = refs
        .iter()
        .find(|n| n.tag == [0xfe, 0x53])
        .map(|n| n.body)
        .filter(|b| !b.is_empty() && b[0] > 0 && b.len() > b[0] as usize)
    {
        if let Some(m) = instances.get(&id[1..1 + id[0] as usize]) {
            let mut p = [0.; 3];
            for r in 0..3 {
                p[r] = ((m[r * 3] * local[0] + m[r * 3 + 1] * local[1]) + m[r * 3 + 2] * local[2])
                    + m[9 + r];
            }
            if !p.iter().all(|v| v.is_finite()) {
                return Err(SkpError::Invalid("Dimension placed point overflow".into()));
            }
            return Ok(Some(p));
        }
    }
    Ok(Some(local))
}
pub(super) fn dimensions(data: &[u8], tree: &[Node<'_>]) -> Result<Vec<LegacyDimension>, SkpError> {
    let mut vertices = HashMap::new();
    let mut instances = HashMap::new();
    scan(tree, None, &mut vertices, &mut instances)?;
    let mut result = Vec::new();
    let mut work = Work::default();
    for (offset, pair) in data.windows(2).enumerate() {
        if pair != [0xcc, 0x5b] || offset + 6 > data.len() {
            continue;
        }
        let size = u32::from_le_bytes(data[offset + 2..offset + 6].try_into().unwrap()) as usize;
        let Some(end) = offset
            .checked_add(6)
            .and_then(|n| n.checked_add(size))
            .filter(|&n| n <= data.len())
        else {
            continue;
        };
        if size < 40 {
            continue;
        }
        let body = work.parse(&data[offset + 6..end])?;
        if !body.iter().any(|n| n.tag == [0xcd, 0x5b])
            || !body.iter().any(|n| n.tag == [0xce, 0x5b])
        {
            continue;
        }
        let a = point(
            body.iter().find(|n| n.tag == [0xcd, 0x5b]),
            &vertices,
            &instances,
            &mut work,
        )?;
        let b = point(
            body.iter().find(|n| n.tag == [0xce, 0x5b]),
            &vertices,
            &instances,
            &mut work,
        )?;
        let (Some(a), Some(b)) = (a, b) else {
            continue;
        };
        let normal = body
            .iter()
            .find(|n| n.tag == [0xd0, 0x5b])
            .map(|n| triplet(n.body))
            .transpose()?
            .flatten();
        let offset = body
            .iter()
            .find(|n| n.tag == [0xd2, 0x5b])
            .filter(|n| n.body.len() == 8)
            .map(|n| f64::from_le_bytes(n.body.try_into().unwrap()))
            .unwrap_or(0.);
        if !offset.is_finite() {
            return Err(SkpError::Invalid("Nonfinite dimension offset".into()));
        }
        if (vertices.len() + instances.len())
            .checked_mul(256)
            .and_then(|n| {
                result
                    .len()
                    .checked_add(1)
                    .and_then(|d| d.checked_mul(192))
                    .and_then(|d| n.checked_add(d))
            })
            .is_none_or(|n| n > 128 * 1024 * 1024)
        {
            return Err(SkpError::Invalid(
                "Dimension retained allocation admission".into(),
            ));
        }
        result.push(LegacyDimension {
            a,
            b,
            normal,
            offset,
            text: String::new(),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tlv(tag: [u8; 2], body: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }
    fn xyz(p: [f64; 3]) -> Vec<u8> {
        p.into_iter().flat_map(f64::to_le_bytes).collect()
    }
    fn free(p: [f64; 3]) -> Vec<u8> {
        let mut body = tlv([9, 0x52], &1u32.to_le_bytes());
        body.extend(tlv([10, 0x52], &xyz(p)));
        tlv([8, 0x52], &body)
    }
    #[test]
    fn free_endpoints_normal_and_offset_preserved() {
        let mut body = tlv([0xcd, 0x5b], &free([1., 2., 3.]));
        body.extend(tlv([0xce, 0x5b], &free([4., 5., 6.])));
        body.extend(tlv([0xd0, 0x5b], &xyz([0., 1., 0.])));
        body.extend(tlv([0xd2, 0x5b], &(-2f64).to_le_bytes()));
        let data = tlv([0xcc, 0x5b], &body);
        let out = dimensions(&data, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].a, [1., 2., 3.]);
        assert_eq!(out[0].b, [4., 5., 6.]);
        assert_eq!(out[0].normal, Some([0., 1., 0.]));
        assert_eq!(out[0].offset, -2.);
    }
    #[test]
    fn malformed_candidate_skips_but_nonfinite_geometry_refuses() {
        assert!(dimensions(&tlv([0xcc, 0x5b], &[0; 40]), &[])
            .unwrap()
            .is_empty());
        let mut body = tlv([0xcd, 0x5b], &free([f64::INFINITY, 0., 0.]));
        body.extend(tlv([0xce, 0x5b], &free([1., 0., 0.])));
        assert!(dimensions(&tlv([0xcc, 0x5b], &body), &[]).is_err());
    }
    #[test]
    fn instance_transform_source_order() {
        let a = [1., 0., 0., 0., 1., 0., 0., 0., 1., 10., 20., 30., 1.];
        let b = [2., 0., 0., 0., 3., 0., 0., 0., 4., 1., 2., 3., 1.];
        let out = compose(Some(a), Some(b)).unwrap();
        assert_eq!(out, [2., 0., 0., 0., 3., 0., 0., 0., 4., 11., 22., 33., 1.]);
    }
    #[test]
    fn overlapping_candidates_share_work_admission() {
        let inner = tlv([0xcc, 0x5b], &[0; 40]);
        let outer = tlv([0xcc, 0x5b], &inner);
        let mut work = Work {
            bytes: 256 * 1024 * 1024 - outer.len(),
            nodes: 0,
        };
        work.parse(&outer).unwrap();
        assert!(work.parse(&inner).is_err());
        let mut work = Work {
            bytes: 0,
            nodes: MAX_NODES,
        };
        assert!(work.parse(&tlv([1, 2], &[])).is_err());
    }
}
