// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Layers / Tags system.

use serde::{Deserialize, Serialize};

pub const DEFAULT_LAYER: &str = "Layer0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub color: Option<[f32; 3]>,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            visible: true,
            locked: false,
            color: None,
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new(DEFAULT_LAYER)
    }
}
