// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Section planes — cutting planes slicing through 3D models.
//!
//! A section plane consists of a point and unit normal vector.
//! The active cut clips the display to peer inside closed geometries.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Calculate the next conventional letter symbol (A, B, C, ..., AA, AB...).
pub fn next_symbol(existing: &[SectionPlane]) -> String {
    let taken: std::collections::HashSet<String> = existing
        .iter()
        .map(|p| p.symbol.trim().to_uppercase())
        .collect();

    let mut i = 0;
    loop {
        let mut n = i;
        let mut label = String::new();
        loop {
            let rem = n % 26;
            label.insert(0, (b'A' + rem as u8) as char);
            n = n / 26 - 1;
            if n < 0 {
                break;
            }
        }
        if !taken.contains(&label) {
            return label;
        }
        i += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionPlane {
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub point: DVec3,
    pub normal: DVec3,
    pub name: String,
    pub symbol: String,
    pub active: bool,
}

impl SectionPlane {
    pub fn new(point: DVec3, normal: DVec3, name: impl Into<String>, symbol: impl Into<String>) -> Self {
        let n = if normal.length_squared() < 1e-12 {
            DVec3::Z
        } else {
            normal.normalize()
        };
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            id: Uuid::new_v4(),
            point,
            normal: n,
            name: name.into(),
            symbol: symbol.into(),
            active: true,
        }
    }

    /// Reverse normal: cut shows the opposite side.
    pub fn flip(&mut self) {
        self.normal = -self.normal;
    }

    /// Signed distance to point `p`. Positive if on the clipped/hidden side.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.normal.dot(p - self.point)
    }

    /// Check if point is clipped by this plane (i.e. on the normal side).
    pub fn is_clipped(&self, p: DVec3) -> bool {
        self.signed_distance(p) > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_symbols() {
        let planes = vec![
            SectionPlane::new(DVec3::ZERO, DVec3::Z, "Section 1", "A"),
            SectionPlane::new(DVec3::ZERO, DVec3::Z, "Section 2", "B"),
        ];
        assert_eq!(next_symbol(&planes), "C");
    }

    #[test]
    fn test_section_clipping() {
        let mut sp = SectionPlane::new(DVec3::new(0.0, 0.0, 5.0), DVec3::Z, "Cut", "A");
        assert!(sp.is_clipped(DVec3::new(0.0, 0.0, 6.0)));
        assert!(!sp.is_clipped(DVec3::new(0.0, 0.0, 4.0)));
        sp.flip();
        assert!(!sp.is_clipped(DVec3::new(0.0, 0.0, 6.0)));
        assert!(sp.is_clipped(DVec3::new(0.0, 0.0, 4.0)));
    }
}
