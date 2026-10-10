// SPDX-License-Identifier: GPL-3.0-or-later AND MIT
// Pinned OpenSKP legacy.py archive bookkeeping at 291700bc213655a00461838de71e5f206311e619.
// Copyright (c) 2024-2026 Ahsan Mehmood; see OPENSKP-LICENSE.
// Geometry/annotation readers are required next; never publish a truncated entity walk.
use super::*;
use std::sync::Arc;
#[derive(Clone)]
enum Object {
    Vertex([f64; 3]),
    Edge(Edge),
    EdgeUse(u64, bool),
    Loop(Vec<(u64, bool)>),
    Face(Face),
    Material(String, [f64; 4]),
    Layer(String, bool),
    Attributes,
    Premodel,
}
#[derive(Clone)]
enum Slot {
    Class(String),
    Object(Arc<Object>),
}
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Cursor<'_> {
    fn raw(&mut self, n: usize) -> Result<&[u8], SkpError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&v| v <= self.bytes.len())
            .ok_or_else(|| SkpError::Invalid("Truncated legacy archive".into()))?;
        let b = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8, SkpError> {
        Ok(self.raw(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SkpError> {
        Ok(u16::from_le_bytes(self.raw(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, SkpError> {
        Ok(u32::from_le_bytes(self.raw(4)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, SkpError> {
        let v = f64::from_le_bytes(self.raw(8)?.try_into().unwrap());
        if !v.is_finite() {
            return Err(SkpError::Invalid("Nonfinite legacy scalar".into()));
        }
        Ok(v)
    }
    fn peek(&self, n: usize) -> &[u8] {
        &self.bytes[self.pos..self.bytes.len().min(self.pos.saturating_add(n))]
    }
    fn utf16(&mut self) -> Result<String, SkpError> {
        if self.raw(3)? != [255, 254, 255] {
            return Err(SkpError::Invalid("Legacy string marker".into()));
        }
        let mut n = self.u8()? as usize;
        if n == 255 {
            n = self.u16()? as usize;
            if n == 65535 {
                n = self.u32()? as usize
            }
        }
        if n > 1_000_000 {
            return Err(SkpError::Invalid("Legacy string admission".into()));
        }
        let b = self.raw(
            n.checked_mul(2)
                .ok_or_else(|| SkpError::Invalid("Legacy string overflow".into()))?,
        )?;
        Ok(String::from_utf16_lossy(
            &b.chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        ))
    }
}
struct Archive<'a> {
    r: Cursor<'a>,
    ver: u32,
    base: u32,
    next: u32,
    slots: BTreeMap<u32, Slot>,
    classes: HashMap<String, u32>,
    depth: usize,
    current_loop: Option<u32>,
    decoded_bytes: usize,
}
impl<'a> Archive<'a> {
    fn new(bytes: &'a [u8], ver: u32, start: usize, base: u32) -> Self {
        Self {
            r: Cursor { bytes, pos: start },
            ver,
            base,
            next: base,
            slots: BTreeMap::new(),
            classes: HashMap::new(),
            depth: 0,
            current_loop: None,
            decoded_bytes: 0,
        }
    }
    fn charge(&mut self, bytes: usize) -> Result<(), SkpError> {
        self.decoded_bytes = self
            .decoded_bytes
            .checked_add(bytes)
            .filter(|&n| n <= 256 * 1024 * 1024)
            .ok_or_else(|| SkpError::Invalid("Legacy aggregate decoded admission".into()))?;
        Ok(())
    }
    fn allocate(&mut self, s: Slot) -> Result<u32, SkpError> {
        if self.next - self.base >= 1_000_000 {
            return Err(SkpError::Invalid("Legacy store map admission".into()));
        }
        self.charge(512)?; // Conservative slot, tree node and immutable object bookkeeping.
        let n = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| SkpError::Invalid("Legacy store map overflow".into()))?;
        if self.slots.insert(n, s).is_some() {
            return Err(SkpError::Invalid("Legacy store slot collision".into()));
        }
        Ok(n)
    }
    fn object(&mut self, expect: Option<&str>) -> Result<Option<(u32, Arc<Object>)>, SkpError> {
        if self.depth >= 128 {
            return Err(SkpError::Invalid("Legacy object depth".into()));
        }
        self.depth += 1;
        let result = self.object_inner(expect);
        self.depth -= 1;
        result
    }
    fn object_inner(
        &mut self,
        expect: Option<&str>,
    ) -> Result<Option<(u32, Arc<Object>)>, SkpError> {
        let tag = self.r.u16()?;
        if tag == 0 {
            return Ok(None);
        }
        if tag == 65535 {
            self.r.u16()?;
            let len = self.r.u16()? as usize;
            if len > 40 {
                return Err(SkpError::Invalid("Legacy class name admission".into()));
            }
            let name = std::str::from_utf8(self.r.raw(len)?)
                .map_err(|_| SkpError::Invalid("Legacy class encoding".into()))?
                .to_owned();
            let slot = self.allocate(Slot::Class(name.clone()))?;
            self.classes.insert(name.clone(), slot);
            return self.new_object(&name);
        }
        let reference = if tag == 32767 {
            self.r.u32()?
        } else if tag & 32768 != 0 {
            (tag as u32 & 32767) | 0x8000_0000
        } else {
            tag as u32
        };
        if reference & 0x8000_0000 != 0 {
            let slot = reference & 0x7fff_ffff;
            let name = match self.slots.get(&slot) {
                Some(Slot::Class(n)) => n.clone(),
                Some(_) => {
                    return Err(SkpError::Invalid(
                        "Legacy class reference targets object".into(),
                    ))
                }
                None => {
                    let name = expect
                        .ok_or_else(|| SkpError::Invalid("Unknown legacy class reference".into()))?
                        .to_owned();
                    self.charge(512)?;
                    self.slots.insert(slot, Slot::Class(name.clone()));
                    self.classes.insert(name.clone(), slot);
                    name
                }
            };
            return self.new_object(&name);
        }
        match self.slots.get(&reference) {
            Some(Slot::Object(o)) => Ok(Some((reference, o.clone()))),
            Some(_) => Err(SkpError::Invalid(
                "Legacy object reference targets class".into(),
            )),
            None if reference < self.base => Ok(Some((reference, Arc::new(Object::Premodel)))),
            None => Err(SkpError::Invalid("Unknown legacy object reference".into())),
        }
    }
    fn preamble(&mut self) -> Result<(), SkpError> {
        self.object(Some("CAttributeContainer"))?;
        if self.ver >= 17 {
            let mask = self.r.u8()?;
            self.r.raw(mask.count_ones() as usize)?;
        }
        Ok(())
    }
    fn drawbase(&mut self) -> Result<EntityFlags, SkpError> {
        let b = self.r.raw(8)?.to_vec();
        let tag = u16::from_le_bytes(
            self.r
                .peek(2)
                .try_into()
                .map_err(|_| SkpError::Invalid("Legacy drawbase layer truncated".into()))?,
        );
        let layer = if self
            .classes
            .get("CLayer")
            .is_some_and(|c| tag as u32 == 32768 | c)
        {
            self.object(Some("CLayer"))?;
            0
        } else if tag == 32767 {
            self.r.u16()?;
            let reference = self.r.u32()?;
            if reference & 0x80000000 != 0 {
                return Err(SkpError::Invalid(
                    "Legacy drawbase layer class reference".into(),
                ));
            }
            0
        } else {
            self.r.u16()?
        };
        let material = u16::from_le_bytes([b[0], b[1]]);
        Ok(EntityFlags {
            hidden: b[2] != 0,
            display: 6 | if b[5] != 0 { 16 } else { 0 } | if b[6] != 0 { 32 } else { 0 },
            material: (material != 0).then_some(material as u64),
            layer: (layer != 0).then_some(layer as u64),
        })
    }
    fn vertex_ref(&mut self) -> Result<u64, SkpError> {
        let (id, value) = self
            .object(Some("CVertex"))?
            .ok_or_else(|| SkpError::Invalid("Null legacy vertex".into()))?;
        if !matches!(value.as_ref(), Object::Vertex(_)) {
            return Err(SkpError::Invalid("Legacy vertex class mismatch".into()));
        }
        Ok(id as u64)
    }
    fn new_object(&mut self, name: &str) -> Result<Option<(u32, Arc<Object>)>, SkpError> {
        let slot = self.allocate(Slot::Object(Arc::new(Object::Premodel)))?;
        let value = match name {
            "CVertex" => {
                self.preamble()?;
                Object::Vertex([self.r.f64()?, self.r.f64()?, self.r.f64()?])
            }
            "CEdge" => {
                self.preamble()?;
                let flags = self.drawbase()?;
                let vertices = [self.vertex_ref()?, self.vertex_ref()?];
                if self.object(None)?.is_some() {
                    return Err(SkpError::Unsupported("legacy curve/arc reader".into()));
                }
                Object::Edge(Edge {
                    vertices,
                    flags,
                    curve: None,
                })
            }
            "CEdgeUse" => {
                self.preamble()?;
                let (edge, value) = self
                    .object(Some("CEdge"))?
                    .ok_or_else(|| SkpError::Invalid("Null legacy edge use".into()))?;
                if !matches!(value.as_ref(), Object::Edge(_)) {
                    return Err(SkpError::Invalid("Legacy edge use class mismatch".into()));
                }
                let reverse = self.r.u8()? != 0;
                let tag = self.r.u16()?;
                let parent = if tag == 32767 {
                    let parent = self.r.u32()?;
                    if parent & 0x80000000 != 0 {
                        return Err(SkpError::Invalid("Legacy loop parent is new object".into()));
                    }
                    parent
                } else if tag == 65535 || tag & 32768 != 0 {
                    return Err(SkpError::Invalid("Legacy loop parent is new object".into()));
                } else {
                    tag as u32
                };
                if self.current_loop != Some(parent) {
                    return Err(SkpError::Invalid(
                        "Legacy loop parent mismatch (annotation burns require next stage)".into(),
                    ));
                }
                Object::EdgeUse(edge as u64, !reverse)
            }
            "CLoop" => {
                let old = self.current_loop;
                self.current_loop = Some(slot);
                self.preamble()?;
                self.r.raw(2)?;
                let mut uses = Vec::new();
                loop {
                    if self.r.peek(2) == [0, 0] {
                        self.r.u16()?;
                        break;
                    }
                    if uses.len() >= 1_000_000 {
                        return Err(SkpError::Invalid("Legacy loop use admission".into()));
                    }
                    let (_, value) = self
                        .object(Some("CEdgeUse"))?
                        .ok_or_else(|| SkpError::Invalid("Null legacy coedge".into()))?;
                    let Object::EdgeUse(edge, sense) = value.as_ref() else {
                        return Err(SkpError::Invalid("Legacy coedge class mismatch".into()));
                    };
                    self.charge(64)?; // Includes geometric Vec growth, not only initialized length.
                    uses.push((*edge, *sense));
                }
                self.current_loop = old;
                Object::Loop(uses)
            }
            "CFace" => {
                self.preamble()?;
                let flags = self.drawbase()?;
                let normal = [self.r.f64()?, self.r.f64()?, self.r.f64()?];
                self.r.f64()?;
                let count = self.r.u32()?;
                if count > 10_000 {
                    return Err(SkpError::Invalid("Legacy face loop count".into()));
                }
                let mut loops = Vec::new();
                for _ in 0..count {
                    let (_, value) = self
                        .object(Some("CLoop"))?
                        .ok_or_else(|| SkpError::Invalid("Null legacy loop".into()))?;
                    let Object::Loop(ring) = value.as_ref() else {
                        return Err(SkpError::Invalid("Legacy face loop class mismatch".into()));
                    };
                    self.charge(ring.len().saturating_mul(32).saturating_add(64))?;
                    loops.push(ring.clone())
                }
                let back = self.r.u16()?;
                Object::Face(Face {
                    normal: Some(normal),
                    loops,
                    flags,
                    back_material: (back != 0).then_some(back as u64),
                    uv_transform: None,
                    uv_transform_back: None,
                })
            }
            "CMaterial" => {
                self.preamble()?;
                let name = self.r.utf16()?;
                self.charge(name.capacity())?;
                if self.r.u16()? != 0 {
                    return Err(SkpError::Unsupported(
                        "legacy textured material/DIB reader".into(),
                    ));
                }
                let b = self.r.raw(4)?.to_vec();
                self.r.utf16()?;
                self.r.raw(8)?;
                let transparency = self.r.f64()?;
                let use_op = self.r.u8()?;
                Object::Material(
                    name,
                    [
                        b[0] as f64 / 255.,
                        b[1] as f64 / 255.,
                        b[2] as f64 / 255.,
                        if use_op != 0 {
                            (1. - transparency).clamp(0., 1.)
                        } else {
                            1.
                        },
                    ],
                )
            }
            "CLayer" => {
                self.preamble()?;
                let name = self.r.utf16()?;
                self.charge(name.capacity())?;
                let mut flags = Vec::new();
                while flags.len() < 8 && self.r.peek(3) != [255, 254, 255] {
                    flags.push(self.r.u8()?);
                }
                self.r.utf16()?;
                if self.r.u16()? & 255 != 0 {
                    return Err(SkpError::Unsupported(
                        "legacy textured layer/DIB reader".into(),
                    ));
                }
                self.r.raw(4)?;
                self.r.utf16()?;
                self.r.raw(21)?;
                Object::Layer(name, flags.first().is_some_and(|b| *b != 0))
            }
            "CAttributeContainer" => {
                self.preamble()?;
                if self.r.u16()? != 0 {
                    return Err(SkpError::Unsupported(
                        "legacy named attribute reader".into(),
                    ));
                }
                Object::Attributes
            }
            other => {
                return Err(SkpError::Unsupported(format!(
                    "legacy archive class {other}"
                )))
            }
        };
        let value = Arc::new(value);
        self.slots.insert(slot, Slot::Object(Arc::clone(&value)));
        Ok(Some((slot, value)))
    }
    fn prefix(
        &mut self,
        count: u32,
    ) -> Result<(Vec<(u32, Arc<Object>)>, Vec<(u32, Arc<Object>)>), SkpError> {
        let mut materials = Vec::new();
        for _ in 0..count {
            let item = self
                .object(Some("CMaterial"))?
                .ok_or_else(|| SkpError::Invalid("Null legacy material".into()))?;
            if !matches!(item.1.as_ref(), Object::Material(..)) {
                return Err(SkpError::Invalid("Legacy material class mismatch".into()));
            }
            self.charge(64)?;
            materials.push(item)
        }
        self.r.u32()?;
        if self.ver >= 17 {
            self.r.u8()?;
        }
        let n = self.r.u32()?;
        if n > 100_000 {
            return Err(SkpError::Invalid("Legacy layer count".into()));
        }
        let mut layers = Vec::new();
        while layers.len() < n as usize {
            let tag = u16::from_le_bytes(
                self.r
                    .peek(2)
                    .try_into()
                    .map_err(|_| SkpError::Invalid("Truncated legacy layer list".into()))?,
            );
            if tag == 0 {
                self.r.u16()?;
                continue;
            }
            if tag != 65535 && tag & 32768 == 0 {
                break;
            }
            let item = self
                .object(Some("CLayer"))?
                .ok_or_else(|| SkpError::Invalid("Null legacy layer".into()))?;
            if !matches!(item.1.as_ref(), Object::Layer(..)) {
                return Err(SkpError::Invalid("Legacy layer class mismatch".into()));
            }
            self.charge(64)?;
            layers.push(item)
        }
        loop {
            let tag = u16::from_le_bytes(
                self.r
                    .peek(2)
                    .try_into()
                    .map_err(|_| SkpError::Invalid("Truncated legacy layer tail".into()))?,
            );
            if tag == 0 {
                self.r.u16()?;
                continue;
            }
            if self
                .classes
                .get("CLayer")
                .is_some_and(|c| tag as u32 == 32768 | c)
            {
                if let Some(item) = self.object(Some("CLayer"))? {
                    self.charge(64)?;
                    layers.push(item)
                }
                continue;
            }
            break;
        }
        Ok((materials, layers))
    }
}
pub(super) fn parse(bytes: &[u8], name: &str) -> Result<Payload, SkpError> {
    let ascii = bytes
        .iter()
        .take(96)
        .filter(|b| **b != 0)
        .copied()
        .collect::<Vec<_>>();
    let text = String::from_utf8_lossy(&ascii);
    let a = text
        .find('{')
        .ok_or_else(|| SkpError::Invalid("Legacy version header".into()))?;
    let b = text[a..]
        .find('}')
        .ok_or_else(|| SkpError::Invalid("Legacy version header".into()))?;
    let version = &text[a..a + b + 1];
    let ver = version[1..]
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| SkpError::Invalid("Legacy version number".into()))?;
    let anchor = |name: &[u8]| {
        bytes.windows(name.len() + 6).position(|b| {
            b[..2] == [255, 255]
                && u16::from_le_bytes([b[4], b[5]]) as usize == name.len()
                && &b[6..] == name
        })
    };
    let (start, count) = if let Some(start) = anchor(b"CMaterial") {
        if start < 4 {
            return Err(SkpError::Invalid("Legacy material anchor".into()));
        }
        let count = u32::from_le_bytes(bytes[start - 4..start].try_into().unwrap());
        if count > 100_000 {
            return Err(SkpError::Invalid("Legacy material count".into()));
        }
        (start, count)
    } else {
        let layer = anchor(b"CLayer")
            .ok_or_else(|| SkpError::Unsupported("legacy material/layer anchor".into()))?;
        (
            layer
                .checked_sub(if ver >= 17 { 9 } else { 8 })
                .ok_or_else(|| SkpError::Invalid("Legacy layer anchor".into()))?,
            0,
        )
    };
    let mut probe = Archive::new(bytes, ver, start, 1 << 20);
    let bases = if count >= 2 {
        probe.object(Some("CMaterial"))?;
        let tag = probe.r.u16()?;
        if tag == 65535 || tag & 32768 == 0 {
            return Err(SkpError::Invalid("Legacy material base bootstrap".into()));
        }
        vec![(tag & 32767) as u32]
    } else {
        let (_, layers) = probe.prefix(count)?;
        let (anchor, value) = probe
            .object(None)?
            .ok_or_else(|| SkpError::Invalid("Null legacy active layer".into()))?;
        if !matches!(value.as_ref(), Object::Premodel) {
            return Err(SkpError::Invalid("Legacy layer base probe".into()));
        }
        layers
            .iter()
            .filter_map(|(slot, _)| anchor.checked_sub(slot - (1 << 20)))
            .filter(|b| *b > 0 && *b < (1 << 20))
            .collect::<Vec<_>>()
    };
    let mut error = SkpError::Invalid("No viable legacy store base".into());
    for base in bases {
        match model(bytes, ver, start, count, base, name, version) {
            Ok(p) => return Ok(p),
            Err(e) => error = e,
        }
    }
    Err(error)
}
fn model(
    bytes: &[u8],
    ver: u32,
    start: usize,
    count: u32,
    base: u32,
    name: &str,
    version: &str,
) -> Result<Payload, SkpError> {
    let mut ar = Archive::new(bytes, ver, start, base);
    let (materials, layers) = ar.prefix(count)?;
    let (_, anchor) = ar
        .object(None)?
        .ok_or_else(|| SkpError::Invalid("Null legacy active layer".into()))?;
    if !matches!(anchor.as_ref(), Object::Layer(..)) {
        return Err(SkpError::Invalid(
            "Legacy definition-list anchor class".into(),
        ));
    }
    let definitions = ar.r.u32()?;
    if definitions != 0 {
        return Err(SkpError::Unsupported(
            "legacy definition/geometry readers".into(),
        ));
    }
    let root = ar.r.u32()?;
    if root > 5_000_000 {
        return Err(SkpError::Invalid("Legacy entity count admission".into()));
    }
    let mut roots = Vec::new();
    let mut root_ids = HashSet::new();
    for _ in 0..root {
        if let Some(entity) = ar.object(None)? {
            if !root_ids.contains(&entity.0) {
                ar.charge(64)?; // Vec capacity plus hash-table entry.
                root_ids.insert(entity.0);
                roots.push(entity.0);
            }
        }
    }
    if bytes.windows(9).any(|b| b == b"CViewPage") {
        return Err(SkpError::Unsupported("legacy saved page records".into()));
    }
    ar.charge(ar.decoded_bytes)?; // Candidate output clones coexist with immutable archive.
    let mut p = Payload {
        name: name.into(),
        version: version.into(),
        ..Payload::default()
    };
    for (id, material) in materials {
        if let Object::Material(name, color) = material.as_ref() {
            if p.material_names.insert(id as u64, name.clone()).is_some()
                || p.materials
                    .insert(
                        name.clone(),
                        MaterialRecord {
                            color: *color,
                            texture: None,
                        },
                    )
                    .is_some()
            {
                return Err(SkpError::Invalid(
                    "Duplicate legacy material identity".into(),
                ));
            }
        }
    }
    for (id, layer) in layers {
        if let Object::Layer(name, hidden) = layer.as_ref() {
            if p.layers
                .insert(
                    id as u64,
                    LayerRecord {
                        name: name.clone(),
                        visible: !*hidden,
                    },
                )
                .is_some()
            {
                return Err(SkpError::Invalid("Duplicate legacy layer identity".into()));
            }
        }
    }
    for id in roots {
        let Some(Slot::Object(value)) = ar.slots.get(&id) else {
            unreachable!()
        };
        match value.as_ref() {
            Object::Face(face) => {
                for ring in &face.loops {
                    for &(edge, _) in ring {
                        add_edge(&mut p.root, edge, &ar.slots)?;
                    }
                }
                if !p.root.faces.contains_key(&(id as u64)) {
                    p.root.faces.insert(id as u64, face.clone());
                    p.root.face_order.push(id as u64);
                }
            }
            Object::Edge(_) => add_edge(&mut p.root, id as u64, &ar.slots)?,
            other => {
                let _ = other;
                return Err(SkpError::Unsupported("legacy root entity kind".into()));
            }
        }
    }
    Ok(p)
}

fn add_edge(def: &mut Definition, id: u64, slots: &BTreeMap<u32, Slot>) -> Result<(), SkpError> {
    let Some(Slot::Object(object)) = slots.get(&(id as u32)) else {
        return Err(SkpError::Invalid("Legacy edge slot unresolved".into()));
    };
    let Object::Edge(edge) = object.as_ref() else {
        return Err(SkpError::Invalid("Legacy edge slot unresolved".into()));
    };
    for &vertex in &edge.vertices {
        let Some(Slot::Object(object)) = slots.get(&(vertex as u32)) else {
            return Err(SkpError::Invalid("Legacy vertex slot unresolved".into()));
        };
        let Object::Vertex(position) = object.as_ref() else {
            return Err(SkpError::Invalid("Legacy vertex slot unresolved".into()));
        };
        def.vertices.entry(vertex).or_insert(*position);
    }
    def.edges.entry(id).or_insert_with(|| edge.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn repeated_archive_references_share_owner_and_budget_refuses() {
        let bytes = [1_u8, 0, 1, 0];
        let mut ar = Archive::new(&bytes, 17, 0, 1);
        let owner = Arc::new(Object::Loop(vec![(17, true); 100_000]));
        ar.allocate(Slot::Object(Arc::clone(&owner))).unwrap();
        let before = ar.decoded_bytes;
        let a = ar.object(None).unwrap().unwrap().1;
        let b = ar.object(None).unwrap().unwrap().1;
        assert!(Arc::ptr_eq(&owner, &a) && Arc::ptr_eq(&a, &b));
        assert_eq!(ar.decoded_bytes, before);
        ar.decoded_bytes = 256 * 1024 * 1024 - 1;
        assert!(matches!(ar.charge(2), Err(SkpError::Invalid(s)) if s.contains("aggregate")));
    }
    #[test]
    fn real_empty_v17_raw_metadata_and_application_refusal() {
        let p = parse(
            include_bytes!("../../../../docs/audits/skp-native-legacy/single_material_v17.skp"),
            "empty",
        )
        .unwrap();
        assert_eq!(p.material_names.get(&10).unwrap(), "RedMat");
        assert_eq!(p.materials["RedMat"].color, [1., 0., 0., 1.]);
        assert_eq!(p.layers[&12].name, "Layer0");
        assert!(
            matches!(scene_from_payload(&p),Err(SkpError::Unsupported(s))if s.contains("empty"))
        );
    }
    #[test]
    fn cursor_and_store_map_malformed_records_refuse() {
        let mut r = Cursor {
            bytes: &[255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            pos: 0,
        };
        assert!(r.utf16().is_err());
        let mut r = Cursor {
            bytes: &[0; 3],
            pos: 0,
        };
        assert!(r.u32().is_err());
        let mut ar = Archive::new(&[9, 0], 17, 0, 9);
        ar.slots.insert(9, Slot::Class("CVertex".into()));
        assert!(matches!(ar.object(None),Err(SkpError::Invalid(s))if s.contains("targets class")));
        let mut ar = Archive::new(&[10, 0], 17, 0, 9);
        assert!(matches!(ar.object(None),Err(SkpError::Invalid(s))if s.contains("Unknown")));
    }
}
