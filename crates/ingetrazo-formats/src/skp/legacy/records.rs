// SPDX-License-Identifier: GPL-3.0-or-later AND MIT
// Pinned OpenSKP legacy.py 291700bc: component, attribute and auxiliary layouts.
use super::*;
#[derive(Clone)]
pub(super) struct DefinitionRecord {
    pub name: String,
    pub roots: Vec<u32>,
    pub billboard: bool,
}
#[derive(Clone)]
pub(super) enum AnnotationRecord {
    Dimension {
        text: String,
        a: Option<[f64; 3]>,
        b: Option<[f64; 3]>,
        connect: [u32; 2],
        offset: f64,
    },
    Text {
        text: String,
        point: Option<[f64; 3]>,
        label: Option<[f64; 3]>,
        hidden: bool,
    },
}
impl Archive<'_> {
    pub(super) fn translate_reference(&self, id: u32) -> Option<u32> {
        let mut offset = 0;
        for &(start, width) in &self.burns {
            if id < start {
                break;
            }
            if id < start + width {
                return None;
            }
            offset += width;
        }
        id.checked_sub(offset)
    }
    pub(super) fn font(&mut self) -> Result<(), SkpError> {
        self.object(Some("CAttributeContainer"))?;
        if self.ver >= 17 {
            let mask = self.r.u8()?;
            self.r.raw(mask.count_ones() as usize)?;
        }
        self.string()?;
        self.r.raw(15)?;
        Ok(())
    }
    fn triplet(bytes: &[u8], offset: usize) -> Result<[f64; 3], SkpError> {
        let mut p = [0.; 3];
        for (i, value) in p.iter_mut().enumerate() {
            *value = f64::from_le_bytes(
                bytes[offset + i * 8..offset + i * 8 + 8]
                    .try_into()
                    .unwrap(),
            );
            if !value.is_finite() {
                return Err(SkpError::Invalid(
                    "Nonfinite legacy annotation point".into(),
                ));
            }
        }
        Ok(p)
    }
    pub(super) fn dimension(&mut self) -> Result<AnnotationRecord, SkpError> {
        self.preamble()?;
        self.drawbase()?;
        let text = self.string()?;
        self.object(Some("CSkFont"))?;
        let first = self.r.raw(37)?;
        let a = if first[5..9] == 1u32.to_le_bytes() {
            Some(Self::triplet(first, 13)?)
        } else {
            None
        };
        let c1 = self.entity_ref()?;
        let second = self.r.raw(42)?;
        let b = if second[10..14] == 1u32.to_le_bytes() {
            Some(Self::triplet(second, 18)?)
        } else {
            None
        };
        let c2 = self.entity_ref()?;
        let end = self.r.raw(82)?;
        let offset = f64::from_le_bytes(end[62..70].try_into().unwrap());
        if !offset.is_finite() {
            return Err(SkpError::Invalid(
                "Nonfinite legacy dimension offset".into(),
            ));
        }
        Ok(AnnotationRecord::Dimension {
            text,
            a,
            b,
            connect: [c1, c2],
            offset,
        })
    }
    pub(super) fn text(&mut self) -> Result<AnnotationRecord, SkpError> {
        self.preamble()?;
        let db = self.drawbase()?;
        self.object(Some("CSkFont"))?;
        let start = self.r.pos;
        let limit = self.r.bytes.len().min(start.saturating_add(512));
        let at = (start..limit.saturating_sub(2))
            .find(|&at| {
                at >= 11
                    && self.r.bytes[at..at + 3] == [255, 254, 255]
                    && self.r.bytes[at - 11..at - 7] == [1, 0, 0, 0]
                    && u32::from_le_bytes(self.r.bytes[at - 5..at - 1].try_into().unwrap()) <= 8
                    && self.r.bytes[at - 1] == 1
            })
            .ok_or_else(|| SkpError::Invalid("Legacy text delimiter".into()))?;
        let middle = &self.r.bytes[start..at];
        let mut point = None;
        let mut label = None;
        if let Some(offset) = middle
            .windows(8)
            .position(|b| b == [1, 0, 0, 0, 4, 0, 0, 0])
        {
            if offset + 32 <= middle.len() {
                point = Some(Self::triplet(middle, offset + 8)?);
                let lo = offset + 44;
                if lo + 24 <= middle.len() {
                    let p = Self::triplet(middle, lo)?;
                    if p.iter().any(|v| v.abs() > 1e-12) {
                        label = Some(p);
                    }
                }
            }
        }
        self.r.pos = at;
        let text = self.string()?;
        self.r.raw(5)?;
        while self.r.peek(6).len() == 6 && self.r.peek(2) == [255, 127] {
            let id = u32::from_le_bytes(self.r.peek(6)[2..6].try_into().unwrap());
            if id & 0x80000000 != 0 {
                break;
            }
            self.r.raw(6)?;
        }
        Ok(AnnotationRecord::Text {
            text,
            point,
            label,
            hidden: db.display & 1 != 0,
        })
    }
    fn string(&mut self) -> Result<String, SkpError> {
        let s = self.r.utf16()?;
        self.charge(s.capacity().saturating_add(64))?;
        Ok(s)
    }
    pub(super) fn entity_ref(&mut self) -> Result<u32, SkpError> {
        let tag = self.r.u16()?;
        let id = if tag == 32767 {
            self.r.u32()?
        } else {
            tag as u32
        };
        if id & 0x80000000 != 0 || tag == 65535 || (tag != 32767 && tag & 32768 != 0) {
            return Err(SkpError::Invalid("Legacy entity reference is class".into()));
        }
        Ok(id)
    }
    fn class_ref_at(&self, at: usize, slot: u32) -> bool {
        let bytes = &self.r.bytes[at.min(self.r.bytes.len())..];
        if slot < 32767 {
            bytes.starts_with(&((32768 | slot) as u16).to_le_bytes())
        } else {
            bytes.starts_with(&[255, 127])
                && bytes.get(2..6) == Some((0x80000000 | slot).to_le_bytes().as_slice())
        }
    }
    pub(super) fn next_class_is(&self, name: &str) -> bool {
        self.classes
            .get(name)
            .is_some_and(|&slot| self.class_ref_at(self.r.pos, slot))
            || (self.r.peek(name.len() + 6).get(..2) == Some([255, 255].as_slice())
                && self.r.peek(name.len() + 6).get(6..) == Some(name.as_bytes()))
    }
    fn plausible_tag(&self, at: usize) -> bool {
        let Some(bytes) = self.r.bytes.get(at..at.saturating_add(2)) else {
            return false;
        };
        let tag = u16::from_le_bytes(bytes.try_into().unwrap());
        tag == 0
            || tag == 65535
            || tag == 32767
            || if tag & 32768 != 0 {
                matches!(self.slots.get(&(tag as u32 & 32767)), Some(Slot::Class(_)))
            } else {
                self.slots.contains_key(&(tag as u32)) || (tag as u32) < self.base
            }
    }
    pub(super) fn retry_filler(&mut self, at: usize, limit: u32) -> Result<Option<u32>, SkpError> {
        let mut marker = None;
        for p in at..at
            .saturating_add(12)
            .min(self.r.bytes.len().saturating_sub(3))
        {
            if self.r.bytes.get(p..p + 3) == Some([255, 254, 255].as_slice()) {
                marker = Some(p);
                break;
            }
        }
        let Some(marker) = marker else {
            return Ok(None);
        };
        if self.r.bytes.get(marker + 3) != Some(&0) {
            return Ok(None);
        }
        let mut p = marker + 4;
        while self.r.bytes.get(p) == Some(&0) {
            p += 1;
        }
        for back in 0..4 {
            let Some(at) = p.checked_sub(back).filter(|&p| p >= marker + 4) else {
                continue;
            };
            let Some(bytes) = self.r.bytes.get(at..at.saturating_add(4)) else {
                continue;
            };
            let count = u32::from_le_bytes(bytes.try_into().unwrap());
            if count > 0 && count <= limit && self.plausible_tag(at + 4) {
                self.r.pos = at + 4;
                return Ok(Some(count));
            }
        }
        Ok(None)
    }
    pub(super) fn read_attributes(&mut self) -> Result<Vec<u32>, SkpError> {
        self.preamble()?;
        let mut children = Vec::new();
        loop {
            let Some((id, _)) = self.object(Some("CAttributeNamed"))? else {
                break;
            };
            self.charge(16)?;
            children.push(id);
        }
        Ok(children)
    }
    fn typed_value(&mut self, kind: u8, depth: usize) -> Result<serde_json::Value, SkpError> {
        if depth >= 32 {
            return Err(SkpError::Invalid("Legacy attribute value depth".into()));
        }
        self.charge(96)?;
        Ok(match kind {
            0 => serde_json::Value::Null,
            4 => serde_json::json!(self.r.u32()? as i32),
            6 | 12 => serde_json::json!(self.r.f64()?),
            7 => serde_json::json!(self.r.u8()?),
            9 => serde_json::json!(self.r.u32()?),
            10 => serde_json::json!(self.string()?),
            11 => {
                let count = self.r.u32()?;
                if count > 100_000 {
                    return Err(SkpError::Invalid("Legacy attribute array admission".into()));
                }
                let mut values = Vec::new();
                for _ in 0..count {
                    let kind = self.r.u8()?;
                    values.push(self.typed_value(kind, depth + 1)?);
                }
                serde_json::Value::Array(values)
            }
            17 | 18 => serde_json::json!([self.r.f64()?, self.r.f64()?, self.r.f64()?]),
            _ => {
                return Err(SkpError::Unsupported(format!(
                    "legacy attribute type {kind}"
                )))
            }
        })
    }
    pub(super) fn read_named(
        &mut self,
    ) -> Result<(String, serde_json::Map<String, serde_json::Value>), SkpError> {
        self.preamble()?;
        self.r.raw(4)?;
        let name = self.string()?;
        let mut entries = serde_json::Map::new();
        loop {
            let key = self.string()?;
            if key.is_empty() {
                break;
            }
            let kind = self.r.u8()?;
            let value = self.typed_value(kind, 0)?;
            self.charge(128)?;
            entries.insert(key, value);
        }
        self.r.u32()?;
        Ok((name, entries))
    }
    pub(super) fn read_definition(&mut self) -> Result<DefinitionRecord, SkpError> {
        self.preamble()?;
        self.r.raw(if self.ver >= 17 { 22 } else { 20 })?;
        let layers = self.r.u32()?;
        if layers > 10_000 {
            return Err(SkpError::Invalid("Legacy definition layers".into()));
        }
        let mut got = 0;
        while got < layers {
            if self.r.peek(2) == [0, 0] {
                self.r.u16()?;
                continue;
            }
            self.object(Some("CLayer"))?;
            got += 1;
        }
        if self.r.u16()? == 32767 {
            self.r.u32()?;
        }
        let mut count = if self.ver >= 20 {
            self.retry_filler(self.r.pos, 5_000_000)?
        } else {
            None
        };
        if count.is_none() {
            self.r.u32()?;
            count = Some(self.r.u32()?);
        }
        let mut count = count.unwrap();
        if count == 0 || count > 5_000_000 {
            count = self
                .retry_filler(self.r.pos - 4, 5_000_000)?
                .unwrap_or(count);
        }
        if count > 5_000_000 {
            return Err(SkpError::Invalid("Legacy definition entities".into()));
        }
        let mut roots = Vec::new();
        self.burn_credits.push(0);
        let mut read = 0;
        while read + self.burn_credits.last().copied().unwrap_or(0) < count {
            read += 1;
            if let Some((id, _)) = self.object(None)? {
                self.charge(16)?;
                roots.push(id);
            }
        }
        self.burn_credits.pop();
        let mut relationships = self.r.u32()?;
        if relationships > 100_000 {
            relationships = self
                .retry_filler(self.r.pos - 4, 100_000)?
                .unwrap_or(relationships);
        }
        if relationships > 100_000 {
            return Err(SkpError::Invalid("Legacy definition relationships".into()));
        }
        for _ in 0..relationships {
            self.object(Some("CRelationship"))?;
        }
        self.r.u16()?;
        if self.r.peek(19).get(16..19) != Some([255, 254, 255].as_slice()) {
            for skip in 1..5 {
                if self
                    .r
                    .bytes
                    .get(self.r.pos + skip + 16..self.r.pos + skip + 19)
                    == Some([255, 254, 255].as_slice())
                {
                    self.r.raw(skip)?;
                    break;
                }
            }
        }
        self.r.raw(16)?;
        let name = self.string()?;
        self.r.utf16()?;
        self.r.utf16()?;
        self.r.u32()?;
        let tail = (0..96)
            .find(|&offset| {
                let at = self.r.pos + offset;
                self.classes
                    .get("CThumbnail")
                    .is_some_and(|&slot| self.class_ref_at(at, slot))
                    || self
                        .r
                        .bytes
                        .get(at..at.saturating_add(16))
                        .is_some_and(|b| {
                            b[..2] == [255, 255] && b[4..6] == [10, 0] && &b[6..16] == b"CThumbnail"
                        })
            })
            .ok_or_else(|| SkpError::Invalid("Legacy definition thumbnail tail".into()))?;
        let gap = self.r.raw(tail)?;
        let billboard = gap.len() >= 9 && gap[gap.len() - 9] & 1 != 0;
        self.object(Some("CThumbnail"))?;
        Ok(DefinitionRecord {
            name,
            roots,
            billboard,
        })
    }
    pub(super) fn read_instance(&mut self) -> Result<Instance, SkpError> {
        self.preamble()?;
        let flags = self.drawbase()?;
        let (definition, _) = self
            .object(Some("CComponentDefinition"))?
            .ok_or_else(|| SkpError::Invalid("Null legacy instance definition".into()))?;
        if self.object_classes.get(&definition).map(String::as_str) != Some("CComponentDefinition")
        {
            return Err(SkpError::Invalid("Legacy instance definition class".into()));
        }
        let mut matrix = [0.; 12];
        for v in &mut matrix {
            *v = self.r.f64()?;
        }
        self.r.f64()?;
        let name = self.string()?;
        if self.ver >= 14 {
            self.r.raw(16)?;
        }
        Ok(Instance {
            definition: definition as u64,
            name,
            matrix,
            flags,
        })
    }
}

impl Archive<'_> {
    fn skip_page_reference(&mut self, name: &str) -> Result<(), SkpError> {
        let tag = self.r.u16()?;
        if tag == 0 {
            return Ok(());
        }
        let body = if tag == 65535 {
            self.r.u16()?;
            let len = self.r.u16()? as usize;
            if len > 40 || self.r.raw(len)? != name.as_bytes() {
                return Err(SkpError::Invalid("Legacy page typed class".into()));
            }
            if !self.classes.contains_key(name) {
                let slot = self.allocate(Slot::Class(name.into()))?;
                self.classes.insert(name.into(), slot);
            }
            true
        } else if tag & 32768 != 0 {
            let slot = (tag & 32767) as u32;
            match self.slots.get(&slot) {
                Some(Slot::Class(class)) if class == name => {}
                None => {
                    self.charge(512)?;
                    self.slots.insert(slot, Slot::Class(name.into()));
                }
                _ => return Err(SkpError::Invalid("Legacy page class reference".into())),
            }
            true
        } else {
            false
        }; // Source narrow scan treats non-class references as two-byte skips.
        if body {
            if name == "CCamera" {
                self.r.raw(137)?;
                self.r.u16()?;
                self.r.utf16()?;
                self.r.raw(33)?;
            } else {
                self.r.u32()?;
                let n = self.r.u32()? as usize;
                self.r.raw(n)?;
            }
        }
        Ok(())
    }
    pub(super) fn scan_pages(&mut self) -> Result<Vec<LegacyPage>, SkpError> {
        let end = self.r.pos;
        let mut pages = Vec::new();
        let bytes = self.r.bytes;
        for at in end..bytes.len().saturating_sub(3) {
            if bytes.get(at..at + 3) != Some([255, 254, 255].as_slice()) {
                continue;
            }
            let n = bytes[at + 3] as usize;
            if n == 0 || n > 40 {
                continue;
            }
            let name_end = at + 4 + n * 2;
            if bytes.get(name_end..name_end + 4) != Some([255, 254, 255, 0].as_slice()) {
                continue;
            }
            let Some(flags) = bytes.get(name_end + 4..name_end + 8) else {
                continue;
            };
            let flags = u32::from_le_bytes(flags.try_into().unwrap());
            if flags != 0xc00 && flags != 0xc20 {
                continue;
            }
            self.r.pos = name_end + 8;
            let candidate = (|| -> Result<Vec<u32>, SkpError> {
                self.skip_page_reference("CCamera")?;
                self.skip_page_reference("CDib")?;
                let mut hidden = Vec::new();
                if flags & 0x20 != 0 {
                    loop {
                        let saved = self.r.pos;
                        let id = self.r.u16()? as u32;
                        if self.object_classes.get(&id).map(String::as_str) == Some("CLayer") {
                            self.charge(16)?;
                            hidden.push(id);
                        } else {
                            self.r.pos = saved;
                            break;
                        }
                    }
                    if self.r.u32()? != 1 {
                        return Err(SkpError::Invalid("Legacy page hidden-layer marker".into()));
                    }
                }
                Ok(hidden)
            })();
            if let Err(SkpError::Invalid(message)) = &candidate {
                if message.contains("admission")
                    || message.contains("overflow")
                    || message.contains("budget")
                {
                    return Err(SkpError::Invalid(message.clone()));
                }
            }
            if let Ok(hidden_layer_ids) = candidate {
                let name = String::from_utf16_lossy(
                    &bytes[at + 4..name_end]
                        .chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .collect::<Vec<_>>(),
                );
                self.charge(name.capacity().saturating_add(128))?;
                pages.push(LegacyPage {
                    name,
                    hidden_layer_ids,
                });
            }
        }
        self.r.pos = end;
        Ok(pages)
    }
}
