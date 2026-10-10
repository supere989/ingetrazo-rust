// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Leader text annotation.

use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLabel {
    pub anchor: DVec3,
    pub offset: DVec3,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
}

impl TextLabel {
    pub fn new(anchor: DVec3, offset: DVec3, text: impl Into<String>) -> Self {
        Self {
            anchor,
            offset,
            text: text.into(),
            layer: None,
        }
    }

    pub fn position(&self) -> DVec3 {
        self.anchor + self.offset
    }
}
