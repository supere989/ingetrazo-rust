// SPDX-License-Identifier: GPL-3.0-or-later AND MIT
// Pinned OpenSKP legacy.py archive bookkeeping at 291700bc213655a00461838de71e5f206311e619.
// Copyright (c) 2024-2026 Ahsan Mehmood; see OPENSKP-LICENSE.
// Geometry/annotation readers are required next; never publish a truncated entity walk.
use super::*;
use std::sync::Arc;
mod records;
#[derive(Clone)]
enum Object {
    Vertex([f64; 3]),
    Edge(Edge),
    EdgeUse(u64, bool),
    Loop(Vec<(u64, bool)>),
    Face(Face),
    Material(String, [f64; 4], Option<TextureRecord>, bool),
    Dib(Arc<[u8]>),
    Layer(String, bool),
    Attributes(Vec<u32>),
    Named(String, serde_json::Map<String, serde_json::Value>),
    Definition(records::DefinitionRecord),
    Instance(Instance),
    Auxiliary,
    Annotation(records::AnnotationRecord),
    Curve(Option<[f64; 14]>),
    TextureCoordinates([f64; 9], [f64; 9], bool, bool),
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
    object_classes: HashMap<u32, String>,
    class_schemas: HashMap<String, u16>,
    depth: usize,
    current_loop: Option<u32>,
    decoded_bytes: usize,
    burns: Vec<(u32, u32)>,
    burn_delta: u32,
    annotation_watermark: Option<u32>,
    burn_credits: Vec<u32>,
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
            object_classes: HashMap::new(),
            class_schemas: HashMap::new(),
            depth: 0,
            current_loop: None,
            decoded_bytes: 0,
            burns: Vec::new(),
            burn_delta: 0,
            annotation_watermark: None,
            burn_credits: Vec::new(),
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
            let schema = self.r.u16()?;
            let len = self.r.u16()? as usize;
            if len > 40 {
                return Err(SkpError::Invalid("Legacy class name admission".into()));
            }
            let name = std::str::from_utf8(self.r.raw(len)?)
                .map_err(|_| SkpError::Invalid("Legacy class encoding".into()))?
                .to_owned();
            let slot = self.allocate(Slot::Class(name.clone()))?;
            self.classes.insert(name.clone(), slot);
            self.class_schemas.insert(name.clone(), schema);
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
        let Some(reference) = self.translate_reference(reference) else {
            return Ok(Some((reference, Arc::new(Object::Auxiliary))));
        };
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
        self.preamble_data().map(|_| ())
    }
    fn preamble_data(&mut self) -> Result<Option<Arc<Object>>, SkpError> {
        let attributes = self
            .object(Some("CAttributeContainer"))?
            .map(|(_, value)| value);
        if self.ver >= 17 {
            let mask = self.r.u8()?;
            self.r.raw(mask.count_ones() as usize)?;
        }
        Ok(attributes)
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
            display: 6
                | if b[2] != 0 { 1 } else { 0 }
                | if b[5] != 0 { 8 } else { 0 }
                | if b[6] != 0 { 16 } else { 0 },
            material: (material != 0).then_some(material as u64),
            layer: (layer != 0).then_some(layer as u64),
        })
    }
    fn texture_block(&mut self) -> Result<([f64; 4], TextureRecord, bool), SkpError> {
        self.r.raw(if self.ver >= 17 { 2 } else { 1 })?;
        let (dib_id, object) = self
            .object(Some("CDib"))?
            .ok_or_else(|| SkpError::Invalid("Null legacy texture DIB".into()))?;
        let Object::Dib(data) = object.as_ref() else {
            return Err(SkpError::Invalid(
                "Legacy texture DIB class mismatch".into(),
            ));
        };
        let marker = self
            .r
            .peek(28)
            .windows(3)
            .position(|b| b == [255, 254, 255]);
        match marker {
            Some(20) => {
                self.r.u32()?;
            }
            Some(16) => {}
            _ => return Err(SkpError::Invalid("Legacy texture size alignment".into())),
        }
        let width = self.r.f64()?;
        let height = self.r.f64()?;
        let filename = self.r.utf16()?;
        self.charge(filename.capacity())?;
        let avg: [u8; 9] = self.r.raw(9)?.try_into().unwrap();
        self.r.utf16()?;
        let flags: [u8; 8] = self.r.raw(8)?.try_into().unwrap();
        let transparency = self.r.f64()?;
        let opacity = if self.r.u8()? != 0 {
            (1. - transparency).clamp(0., 1.)
        } else {
            1.
        };
        self.charge(data.len())?; // Neutral material ownership is distinct from archive DIB ownership.
        let texture = TextureRecord {
            archive_name: format!(
                "legacy-dib-{dib_id}.{}",
                if data.starts_with(b"\x89PNG") {
                    "png"
                } else {
                    "jpg"
                }
            ),
            bytes: data.to_vec(),
            width,
            height,
        };
        Ok((
            [
                avg[0] as f64 / 255.,
                avg[1] as f64 / 255.,
                avg[2] as f64 / 255.,
                opacity,
            ],
            texture,
            flags[4] != 0 || avg[3] == 255,
        ))
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
        self.object_classes.insert(slot, name.to_owned());
        let value = match name {
            "CDib" => {
                self.r.u32()?; // Source subtype retained only for record framing; codec is from payload bytes.
                let n = self.r.u32()? as usize;
                if n > 16 * 1024 * 1024 {
                    return Err(SkpError::Invalid("Legacy DIB byte admission".into()));
                }
                self.charge(n)?;
                Object::Dib(Arc::from(self.r.raw(n)?))
            }
            "CVertex" => {
                self.preamble()?;
                Object::Vertex([self.r.f64()?, self.r.f64()?, self.r.f64()?])
            }
            "CEdge" => {
                self.preamble()?;
                let flags = self.drawbase()?;
                let vertices = [self.vertex_ref()?, self.vertex_ref()?];
                let curve = self.object(None)?;
                if curve
                    .as_ref()
                    .is_some_and(|(_, value)| !matches!(value.as_ref(), Object::Curve(_)))
                {
                    return Err(SkpError::Invalid("Legacy edge curve class mismatch".into()));
                }
                Object::Edge(Edge {
                    vertices,
                    flags,
                    curve: curve.map(|(id, _)| id as u64),
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
                let expected = self
                    .current_loop
                    .and_then(|id| id.checked_add(self.burn_delta));
                if expected != Some(parent) {
                    let delta = expected
                        .and_then(|id| parent.checked_sub(id))
                        .filter(|&n| n > 0 && n <= 4096);
                    if let (Some(delta), Some(watermark)) = (delta, self.annotation_watermark) {
                        self.charge(32)?;
                        self.burns.push((
                            watermark
                                .checked_add(self.burn_delta)
                                .ok_or_else(|| SkpError::Invalid("Legacy burn overflow".into()))?,
                            delta,
                        ));
                        self.burn_delta = self
                            .burn_delta
                            .checked_add(delta)
                            .ok_or_else(|| SkpError::Invalid("Legacy burn overflow".into()))?;
                        self.annotation_watermark = None;
                        if let Some(credit) = self.burn_credits.last_mut() {
                            *credit += 1;
                        }
                    } else {
                        return Err(SkpError::Invalid("Legacy loop parent mismatch".into()));
                    }
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
                let attributes = self.preamble_data()?;
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
                let mut uv_transform = None;
                let mut uv_transform_back = None;
                let mut uv_projected = false;
                let mut uv_projected_back = false;
                if let Some(attributes) = attributes {
                    if let Object::Attributes(children) = attributes.as_ref() {
                        for id in children {
                            if let Some(Slot::Object(value)) = self.slots.get(id) {
                                if let Object::TextureCoordinates(front, back, fp, bp) =
                                    value.as_ref()
                                {
                                    uv_transform = Some(*front);
                                    uv_transform_back = Some(*back);
                                    uv_projected = *fp;
                                    uv_projected_back = *bp;
                                }
                            }
                        }
                    }
                }
                Object::Face(Face {
                    uv_projected,
                    uv_projected_back,
                    normal: Some(normal),
                    loops,
                    flags,
                    back_material: (back != 0).then_some(back as u64),
                    uv_transform,
                    uv_transform_back,
                })
            }
            "CMaterial" => {
                self.preamble()?;
                let name = self.r.utf16()?;
                self.charge(name.capacity())?;
                if self.r.u16()? != 0 {
                    let (rgba, texture, colorized) = self.texture_block()?;
                    let value = Arc::new(Object::Material(
                        name,
                        rgba,
                        (!texture.bytes.is_empty()).then_some(texture),
                        colorized,
                    ));
                    self.slots.insert(slot, Slot::Object(Arc::clone(&value)));
                    return Ok(Some((slot, value)));
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
                    None,
                    false,
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
                    self.texture_block()?;
                    self.r.raw(4)?;
                } else {
                    self.r.raw(4)?;
                    self.r.utf16()?;
                    self.r.raw(21)?;
                }
                Object::Layer(name, flags.first().is_some_and(|b| *b != 0))
            }
            "CFaceTextureCoords" => {
                self.preamble()?;
                self.r.u32()?;
                let mut values = [0.; 24];
                for value in &mut values {
                    *value = self.r.f64()?;
                }
                for _ in 0..2 {
                    let count = self.r.u32()?;
                    if count > 100_000 {
                        return Err(SkpError::Invalid("Legacy texture pin admission".into()));
                    }
                    self.r.raw(count as usize * 32)?;
                }
                let front = self.r.u32()?;
                let back = self.r.u32()?;
                Object::TextureCoordinates(
                    values[..9].try_into().unwrap(),
                    values[12..21].try_into().unwrap(),
                    front & 2 != 0,
                    back & 2 != 0,
                )
            }
            "CAttributeContainer" => Object::Attributes(self.read_attributes()?),
            "CAttributeNamed" => {
                let (name, entries) = self.read_named()?;
                Object::Named(name, entries)
            }
            "CComponentDefinition" => Object::Definition(self.read_definition()?),
            "CComponentInstance" | "CGroup" => Object::Instance(self.read_instance()?),
            "CCamera" => {
                self.r.raw(137)?;
                self.r.u16()?;
                self.r.utf16()?;
                self.r.raw(33)?;
                Object::Auxiliary
            }
            "CThumbnail" => {
                self.preamble()?;
                self.object(Some("CCamera"))?;
                self.object(Some("CDib"))?;
                Object::Auxiliary
            }
            "CSkFont" => {
                self.font()?;
                Object::Auxiliary
            }
            "CDimensionLinear" => Object::Annotation(self.dimension()?),
            "CText" => Object::Annotation(self.text()?),
            "CConstructionPoint" => {
                self.preamble()?;
                self.drawbase()?;
                for _ in 0..6 {
                    self.r.f64()?;
                }
                self.r.u8()?;
                Object::Auxiliary
            }
            "CRelationship" => {
                self.preamble()?;
                self.entity_ref()?;
                self.entity_ref()?;
                Object::Auxiliary
            }
            "CCurve" => {
                self.preamble()?;
                self.r.u8()?;
                self.r.u32()?;
                Object::Curve(None)
            }
            "CArcCurve" => {
                if self.class_schemas.get(name) != Some(&3) {
                    return Err(SkpError::Unsupported(
                        "Legacy arc schema other than pinned schema 3".into(),
                    ));
                }
                self.preamble()?;
                self.r.raw(5)?;
                let mut frame = [0.; 14];
                for v in &mut frame {
                    *v = self.r.f64()?;
                }
                Object::Curve(Some(frame))
            }
            other => {
                return Err(SkpError::Unsupported(format!(
                    "legacy archive class {other}"
                )))
            }
        };
        let value = Arc::new(value);
        self.slots.insert(slot, Slot::Object(Arc::clone(&value)));
        if matches!(name, "CDimensionLinear" | "CText") {
            self.annotation_watermark = Some(self.next);
        }
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
    drop(probe); // Base discovery must not coexist with the bounded model archive.
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
    let mut definitions = ar.r.u32()?;
    if definitions > 1_000_000 {
        definitions = ar
            .retry_filler(ar.r.pos - 4, 1_000_000)?
            .unwrap_or(definitions);
    }
    if definitions > 1_000_000 {
        return Err(SkpError::Invalid(
            "Legacy definition count admission".into(),
        ));
    }
    for _ in 0..definitions {
        ar.object(Some("CComponentDefinition"))?;
    }
    while ar.next_class_is("CComponentDefinition") {
        ar.object(Some("CComponentDefinition"))?;
    }
    let mut root = ar.r.u32()?;
    if root > 5_000_000 {
        root = ar.retry_filler(ar.r.pos - 4, 5_000_000)?.unwrap_or(root);
    }
    if root > 5_000_000 {
        return Err(SkpError::Invalid("Legacy entity count admission".into()));
    }
    let mut roots = Vec::new();
    let mut root_ids = HashSet::new();
    ar.burn_credits.push(0);
    let mut read = 0;
    while read + ar.burn_credits.last().copied().unwrap_or(0) < root {
        read += 1;
        if let Some(entity) = ar.object(None)? {
            if !root_ids.contains(&entity.0) {
                ar.charge(64)?; // Vec capacity plus hash-table entry.
                root_ids.insert(entity.0);
                roots.push(entity.0);
            }
        }
    }
    ar.burn_credits.pop();
    let legacy_pages = ar.scan_pages()?;
    ar.charge(ar.decoded_bytes)?; // Candidate output clones coexist with immutable archive.
    let mut p = Payload {
        name: name.into(),
        version: version.into(),
        legacy_pages,
        ..Payload::default()
    };
    let mut tint_cache = HashMap::<(String, [u8; 3]), Option<Arc<Vec<u8>>>>::new();
    for (id, material) in materials {
        if let Object::Material(name, color, texture, colorized) = material.as_ref() {
            let texture = if *colorized {
                if let Some(texture) = texture {
                    let target =
                        [color[0], color[1], color[2]].map(|c| (c * 255.).round_ties_even() as u8);
                    let key = (texture.archive_name.clone(), target);
                    if !tint_cache.contains_key(&key) {
                        let bytes = textures::tint_legacy(texture, target)?;
                        ar.charge(
                            key.0
                                .capacity()
                                .saturating_add(128)
                                .saturating_add(bytes.as_ref().map_or(0, Vec::capacity)),
                        )?;
                        tint_cache.insert(key.clone(), bytes.map(Arc::new));
                    }
                    if let Some(bytes) = &tint_cache[&key] {
                        ar.charge(bytes.len())?;
                        Some(TextureRecord {
                            archive_name: format!("legacy-material-{id}-tinted.png"),
                            bytes: bytes.as_ref().clone(),
                            width: texture.width,
                            height: texture.height,
                        })
                    } else {
                        Some(texture.clone())
                    }
                } else {
                    None
                }
            } else {
                texture.clone()
            };
            if p.material_names.insert(id as u64, name.clone()).is_some()
                || p.materials
                    .insert(
                        name.clone(),
                        MaterialRecord {
                            color: *color,
                            texture,
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
    for (&id, slot) in &ar.slots {
        if let Slot::Object(object) = slot {
            if let Object::Definition(definition) = object.as_ref() {
                let mut def = Definition {
                    name: definition.name.clone(),
                    billboard: definition.billboard,
                    ..Definition::default()
                };
                charge_output(
                    &mut ar.decoded_bytes,
                    definition.name.capacity().saturating_add(512),
                )?;
                fill_definition(
                    &mut def,
                    &definition.roots,
                    &ar.slots,
                    &mut ar.decoded_bytes,
                )?;
                p.definitions.insert(id as u64, def);
            }
        }
    }
    fill_definition(&mut p.root, &roots, &ar.slots, &mut ar.decoded_bytes)?;
    for id in &roots {
        let Some(Slot::Object(object)) = ar.slots.get(id) else {
            continue;
        };
        if let Object::Annotation(annotation) = object.as_ref() {
            match annotation {
                records::AnnotationRecord::Dimension {
                    text,
                    a,
                    b,
                    connect,
                    offset,
                } => {
                    let resolve = |point: Option<[f64; 3]>, reference: u32| -> Option<[f64; 3]> {
                        point.or_else(|| {
                            let id = ar.translate_reference(reference)?;
                            let Slot::Object(value) = ar.slots.get(&id)? else {
                                return None;
                            };
                            if let Object::Vertex(p) = value.as_ref() {
                                Some(*p)
                            } else {
                                None
                            }
                        })
                    };
                    if let (Some(a), Some(b)) = (resolve(*a, connect[0]), resolve(*b, connect[1])) {
                        charge_output(&mut ar.decoded_bytes, text.capacity() + 128)?;
                        p.legacy_dimensions.push(LegacyDimension {
                            a,
                            b,
                            offset: *offset,
                            text: text.clone(),
                        });
                    }
                }
                records::AnnotationRecord::Text {
                    text,
                    point: Some(point),
                    label,
                    hidden,
                } => {
                    charge_output(&mut ar.decoded_bytes, text.capacity() + 128)?;
                    p.legacy_texts.push(LegacyText {
                        point: *point,
                        label: *label,
                        text: text.clone(),
                        hidden: *hidden,
                    });
                }
                _ => {}
            }
        }
    }
    Ok(p)
}

fn charge_output(decoded: &mut usize, bytes: usize) -> Result<(), SkpError> {
    *decoded = decoded
        .checked_add(bytes)
        .filter(|&n| n <= 256 * 1024 * 1024)
        .ok_or_else(|| SkpError::Invalid("Legacy aggregate decoded output admission".into()))?;
    Ok(())
}
fn fill_definition(
    def: &mut Definition,
    roots: &[u32],
    slots: &BTreeMap<u32, Slot>,
    decoded: &mut usize,
) -> Result<(), SkpError> {
    let mut ids = HashSet::new();
    for &id in roots {
        if ids.contains(&id) {
            continue;
        }
        charge_output(decoded, 64)?;
        if !ids.insert(id) {
            continue;
        }
        let Some(Slot::Object(value)) = slots.get(&id) else {
            return Err(SkpError::Invalid(
                "Legacy root references unresolved premodel object".into(),
            ));
        };
        match value.as_ref() {
            Object::Face(face) => {
                for ring in &face.loops {
                    for &(edge, _) in ring {
                        add_edge(def, edge, slots, decoded)?;
                    }
                }
                let bytes = face
                    .loops
                    .iter()
                    .try_fold(512_usize, |total, ring| {
                        total.checked_add(ring.len().saturating_mul(32).saturating_add(64))
                    })
                    .ok_or_else(|| SkpError::Invalid("Legacy output size overflow".into()))?;
                charge_output(decoded, bytes)?;
                if def.faces.insert(id as u64, face.clone()).is_none() {
                    def.face_order.push(id as u64);
                }
            }
            Object::Edge(_) => add_edge(def, id as u64, slots, decoded)?,
            Object::Instance(instance) => {
                charge_output(decoded, instance.name.capacity().saturating_add(512))?;
                def.instances.push(instance.clone());
            }
            Object::Annotation(_) | Object::Auxiliary => {}
            Object::Curve(_) => {} // Analytic frame remains in archive; frontend imports polygon faces/soft edges.
            _ => return Err(SkpError::Unsupported("legacy root entity kind".into())),
        }
    }
    Ok(())
}

fn add_edge(
    def: &mut Definition,
    id: u64,
    slots: &BTreeMap<u32, Slot>,
    decoded: &mut usize,
) -> Result<(), SkpError> {
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
        if !def.vertices.contains_key(&vertex) {
            charge_output(decoded, 128)?;
            def.vertices.insert(vertex, *position);
        }
    }
    if !def.edges.contains_key(&id) {
        charge_output(decoded, 256)?;
        def.edges.insert(id, edge.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn annotation_bands_preserve_registered_ids_and_reserve_phantoms() {
        let bytes = [8u8, 0, 10, 0];
        let mut ar = Archive::new(&bytes, 17, 0, 1);
        ar.burns = vec![(8, 2), (15, 3)];
        ar.burn_delta = 5;
        ar.slots
            .insert(8, Slot::Object(Arc::new(Object::Vertex([1., 2., 3.]))));
        assert_eq!(ar.translate_reference(7), Some(7));
        assert_eq!(ar.translate_reference(8), None);
        assert_eq!(ar.translate_reference(9), None);
        assert_eq!(ar.translate_reference(10), Some(8));
        assert_eq!(ar.translate_reference(17), None);
        assert_eq!(ar.translate_reference(18), Some(13));
        let (_, phantom) = ar.object(None).unwrap().unwrap();
        assert!(matches!(phantom.as_ref(), Object::Auxiliary));
        let (id, value) = ar.object(None).unwrap().unwrap();
        assert_eq!(id, 8);
        assert!(matches!(value.as_ref(), Object::Vertex(_)));
    }
    #[test]
    fn annotation_entity_references_reject_class_tags() {
        let mut ar = Archive::new(&[255, 127, 0, 0, 0, 128], 17, 0, 1);
        assert!(matches!(ar.entity_ref(), Err(SkpError::Invalid(_))));
    }
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
    fn dib_records_keep_shared_payload_and_admit_before_copy() {
        let mut bytes = vec![255, 255, 0, 0, 4, 0];
        bytes.extend_from_slice(b"CDib");
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        let mut ar = Archive::new(&bytes, 17, 0, 1);
        let a = ar.object(None).unwrap().unwrap().1;
        let b = ar.object(None).unwrap().unwrap().1;
        assert!(Arc::ptr_eq(&a, &b));
        let Object::Dib(payload) = a.as_ref() else {
            panic!("DIB expected")
        };
        assert_eq!(payload.as_ref(), b"data");
        let mut excessive = bytes.clone();
        excessive[14..18].copy_from_slice(&(16_u32 * 1024 * 1024 + 1).to_le_bytes());
        let mut ar = Archive::new(&excessive, 17, 0, 1);
        assert!(
            matches!(ar.object(None), Err(SkpError::Invalid(s)) if s.contains("DIB byte admission"))
        );
    }
    #[test]
    fn unresolved_premodel_root_refuses_without_panicking() {
        let mut bytes =
            include_bytes!("../../../../docs/audits/skp-native-legacy/single_material_v17.skp")
                .to_vec();
        bytes[3573..3577].copy_from_slice(&1_u32.to_le_bytes());
        bytes[3577..3579].copy_from_slice(&1_u16.to_le_bytes());
        assert!(
            matches!(parse(&bytes, "invalid-root"), Err(SkpError::Invalid(s)) if s.contains("unresolved premodel"))
        );
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
