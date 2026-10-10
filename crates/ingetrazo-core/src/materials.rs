// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Named materials, RGBA colors, and texture definitions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub color: [f32; 4],
    pub texture_path: Option<String>,
    pub texture_embed: Option<String>,
    pub roughness: f32,
    pub opacity: f32,
}

impl Material {
    pub fn new(name: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            name: name.into(),
            color,
            texture_path: None,
            texture_embed: None,
            roughness: 0.5,
            opacity: color[3],
        }
    }

    pub fn with_texture(name: impl Into<String>, texture_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: [1.0, 1.0, 1.0, 1.0],
            texture_path: Some(texture_path.into()),
            texture_embed: None,
            roughness: 0.5,
            opacity: 1.0,
        }
    }
}
