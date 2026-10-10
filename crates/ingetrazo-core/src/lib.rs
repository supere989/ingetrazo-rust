// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

pub mod bim;
pub mod camera;
pub mod composition;
pub mod dimensions;
pub mod geometry;
pub mod georef;
pub mod groups;
pub mod guides;
pub mod history;
pub mod i18n;
pub mod layers;
pub mod materials;
pub mod math;
pub mod mesh;
pub mod render_blender;
pub mod saved_views;
pub mod scene;
pub mod section;
pub mod snap;
pub mod solids;
pub mod textlabel;
pub mod topology;
pub mod triangulate;
pub mod units;

pub use composition::{Orientation, PaperSize, Sheet, SheetItem, TitleBlock, ViewFrame};
pub use georef::{GeoPath, GeoPoint, Profile, SceneDatum, TerrainObject};
pub use i18n::{tr, tr_fmt};
pub use saved_views::SavedView;
pub use section::SectionPlane;
pub use textlabel::TextLabel;

pub mod components;

pub mod containers;

pub mod planar;
pub mod intersect;
pub mod heal;
pub mod dimension_anchors;
pub mod pieces;

pub mod parts;
pub mod offset;

pub mod simplify;

pub mod drafting;

mod drafting_earcut;

pub mod screen_pick;
pub mod snap_screen;
pub mod circle_inference;

pub mod drawing_axes;
