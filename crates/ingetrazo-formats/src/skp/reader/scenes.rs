//! OpenSKP _parse_pages 757–829 at pinned 291700bc, borrowed TLV records.
use super::*;
fn body<'a>(nodes: &[Node<'a>], tag: [u8; 2]) -> Option<&'a [u8]> {
    nodes.iter().find(|n| n.tag == tag).map(|n| n.body)
}
fn vector(bytes: Option<&[u8]>) -> Result<Option<[f64; 3]>, SkpError> {
    let Some(bytes) = bytes.filter(|b| b.len() == 24) else {
        return Ok(None);
    };
    let value =
        std::array::from_fn(|i| f64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()));
    if value.iter().any(|v| !v.is_finite()) {
        return Err(SkpError::Invalid("Nonfinite saved scene vector".into()));
    }
    Ok(Some(value))
}
fn scalar(bytes: Option<&[u8]>, default: f64) -> Result<f64, SkpError> {
    let value = bytes
        .filter(|b| b.len() == 8)
        .map_or(default, |b| f64::from_le_bytes(b.try_into().unwrap()));
    if !value.is_finite() {
        return Err(SkpError::Invalid("Nonfinite saved scene scalar".into()));
    }
    Ok(value)
}
pub(super) fn parse(bytes: &[u8]) -> Result<Vec<SavedScene>, SkpError> {
    let mut out = Vec::new();
    let mut hidden_total = 0usize;
    for a in raw_nodes(bytes)? {
        if a.tag != [0x60, 0x6d] {
            continue;
        }
        for b in raw_nodes(a.body)? {
            if b.tag != [0x61, 0x6d] {
                continue;
            }
            for page in raw_nodes(b.body)? {
                if page.tag != [0x48, 0x71] {
                    continue;
                }
                if out.len() >= 10000 {
                    return Err(SkpError::Invalid("Saved scene inventory exceeded".into()));
                }
                let fields = raw_nodes(page.body)?;
                let head = body(&fields, [0x54, 0x6f])
                    .map(raw_nodes)
                    .transpose()?
                    .unwrap_or_default();
                let name = body(&head, [0x55, 0x6f]).map(text).unwrap_or_default();
                let wrap = body(&fields, [0x4a, 0x71])
                    .map(raw_nodes)
                    .transpose()?
                    .unwrap_or_default();
                let cam = body(&wrap, [0xbc, 0x34])
                    .map(raw_nodes)
                    .transpose()?
                    .unwrap_or_default();
                let (Some(eye), Some(target)) = (
                    vector(body(&cam, [0xbd, 0x34]))?,
                    vector(body(&cam, [0xbe, 0x34]))?,
                ) else {
                    continue;
                };
                let mut hidden_layer_ids = Vec::new();
                let mut pos = 0;
                if let Some(hidden) = body(&fields, [0x50, 0x71]) {
                    while pos < hidden.len() {
                        let len = usize::from(hidden[pos]);
                        if len == 0 || pos + 1 + len > hidden.len() {
                            break;
                        }
                        hidden_total += 1;
                        if hidden_total > 1000000 || len > 8 {
                            return Err(SkpError::Invalid(
                                "Saved scene hidden layer inventory exceeded".into(),
                            ));
                        }
                        let mut id = 0;
                        for (i, b) in hidden[pos + 1..pos + 1 + len].iter().enumerate() {
                            id |= u64::from(*b) << (i * 8);
                        }
                        hidden_layer_ids.try_reserve(1).map_err(|_| {
                            SkpError::Invalid("Saved scene allocation refused".into())
                        })?;
                        hidden_layer_ids.push(id);
                        pos += 1 + len;
                    }
                }
                out.try_reserve(1)
                    .map_err(|_| SkpError::Invalid("Saved scene allocation refused".into()))?;
                out.push(SavedScene {
                    name,
                    eye,
                    target,
                    up: vector(body(&cam, [0xbf, 0x34]))?,
                    fov: scalar(body(&cam, [0xc4, 0x34]), 35.)?,
                    parallel: body(&cam, [0xc2, 0x34])
                        .and_then(|b| b.first())
                        .is_some_and(|b| *b == 0),
                    ortho_height: scalar(body(&cam, [0xc3, 0x34]), 0.)?,
                    hidden_layer_ids,
                });
            }
        }
    }
    Ok(out)
}
