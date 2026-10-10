// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! BIM / IFC tagging layer and quantity takeoff (area, volume).

use serde::{Deserialize, Serialize};

pub const IFC_CLASSES: &[&str] = &[
    "IfcWall", "IfcSlab", "IfcColumn", "IfcBeam", "IfcFooting",
    "IfcRoof", "IfcStair", "IfcRamp", "IfcDoor", "IfcWindow",
    "IfcRailing", "IfcCovering", "IfcPile", "IfcMember", "IfcSpace",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BimTag {
    pub id: usize,
    pub class_name: String,
    pub name: String,
}

impl BimTag {
    pub fn new(id: usize, class_name: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id,
            class_name: class_name.into(),
            name: name.into(),
        }
    }
}
