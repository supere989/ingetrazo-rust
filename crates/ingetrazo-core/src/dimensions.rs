// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! 3D Linear dimensions and leader text annotations.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::units::{format_length, LengthUnit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    #[serde(alias = "a")]
    pub start: DVec3,
    #[serde(alias = "b")]
    pub end: DVec3,
    #[serde(default)]
    pub offset: DVec3,
    #[serde(default)]
    pub text_override: Option<String>,
}

impl Dimension {
    pub fn new(start: DVec3, end: DVec3, offset: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            start,
            end,
            offset,
            text_override: None,
        }
    }

    pub fn length(&self) -> f64 {
        (self.end - self.start).length()
    }

    pub fn display_text(&self, unit: LengthUnit, decimals: usize) -> String {
        if let Some(ref text) = self.text_override {
            text.clone()
        } else {
            format_length(self.length(), unit, decimals)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_deserialize_ab_format() {
        let json_str = r#"{"a": [0.0, 0.0, 0.0], "b": [0.0, 0.0, 2.4], "offset": [-1.0, 0.0, 0.0]}"#;
        let dim: Dimension = serde_json::from_str(json_str).expect("deserialize dimension");
        assert_eq!(dim.start, DVec3::new(0.0, 0.0, 0.0));
        assert_eq!(dim.end, DVec3::new(0.0, 0.0, 2.4));
        assert_eq!(dim.offset, DVec3::new(-1.0, 0.0, 0.0));
        assert!((dim.length() - 2.4).abs() < 1e-6);
    }
}
