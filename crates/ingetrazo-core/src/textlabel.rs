// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Leader text annotation.

use glam::DVec3;
use serde::{Deserialize, Serialize};

fn fresh_history_identity() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLabel {
    #[serde(skip, default = "fresh_history_identity")]
    pub history_id: u64,
    pub anchor: DVec3,
    pub offset: DVec3,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
}

impl TextLabel {
    pub fn new(anchor: DVec3, offset: DVec3, text: impl Into<String>) -> Self {
        Self {
            history_id: fresh_history_identity(),
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

impl PartialEq for TextLabel {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            history_id: _,
            anchor,
            offset,
            text,
            layer,
        } = self;
        anchor == &other.anchor
            && offset == &other.offset
            && text == &other.text
            && layer == &other.layer
    }
}
