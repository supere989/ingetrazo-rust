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
}

impl Guide {
    pub fn point(p: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: GuideType::Point(p),
        }
    }

    pub fn infinite_line(point: DVec3, dir: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: GuideType::InfiniteLine {
                point,
                dir: dir.normalize_or_zero(),
            },
        }
    }

    pub fn segment(a: DVec3, b: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: GuideType::Segment { a, b },
        }
    }
}
