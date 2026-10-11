// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Construction guides (lines and points) created by the Tape Measure tool.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuideType {
    Point(DVec3),
    InfiniteLine { point: DVec3, dir: DVec3 },
    Segment { a: DVec3, b: DVec3 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub id: Uuid,
    pub kind: GuideType,
    #[serde(default)]
    pub origin: Option<DVec3>,
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
}

impl Guide {
    pub fn point(p: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            origin: None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            kind: GuideType::Point(p),
        }
    }

    pub fn infinite_line(point: DVec3, dir: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            origin: None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            kind: GuideType::InfiniteLine {
                point,
                dir: if [point, dir].into_iter().all(|p| p.is_finite() && p.as_vec3().as_dvec3() == p) {
                    crate::math::source_normalized(dir.as_vec3()).as_dvec3()
                } else { dir.normalize_or_zero() },
            },
        }
    }

    pub fn segment(a: DVec3, b: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            origin: None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            kind: GuideType::Segment { a, b },
        }
    }
}
