// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Visual styles and rendering configuration.
//!
//! Visual styles supported:
//! - `ShadedWithTextures`: Full material colors and textures.
//! - `Wireframe`: Edges and lines only; faces are omitted.
//! - `HiddenLine`: Flat white/monochrome opaque faces occluding back elements, with black edges.
//! - `Monochrome`: Default front/back face tones (no materials).
//! - `XRay`: Semi-transparent faces with visible edges throughout the model.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualStyle {
    ShadedWithTextures,
    Wireframe,
    HiddenLine,
    Monochrome,
    XRay,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self::ShadedWithTextures
    }
}

impl VisualStyle {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ShadedWithTextures => "Shaded with Textures",
            Self::Wireframe => "Wireframe",
            Self::HiddenLine => "Hidden Line",
            Self::Monochrome => "Monochrome",
            Self::XRay => "X-Ray",
        }
    }
}

/// Comprehensive style configuration for viewport rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleConfig {
    pub visual_style: VisualStyle,
    pub show_edges: bool,
    pub show_profiles: bool,
    pub show_soft_edges: bool,
    pub back_edges_hidden: bool,
    pub edge_color: [f32; 4],
    pub front_color: [f32; 4],
    pub back_color: [f32; 4],
    pub selection_color: [f32; 4],
    pub background_color: [f32; 4],
    pub xray_opacity: f32,
    pub line_width: f32,
    pub profile_width: f32,
    pub show_sky: bool,
    pub sky_color: [f32; 4],
    pub ground_color: [f32; 4],
    pub show_grid: bool,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self::default_shaded()
    }
}

impl StyleConfig {
    pub fn default_shaded() -> Self {
        Self {
            visual_style: VisualStyle::ShadedWithTextures,
            show_edges: true,
            show_profiles: true,
            show_soft_edges: false,
            back_edges_hidden: false,
            edge_color: [0.13, 0.17, 0.23, 1.0],
            front_color: [0.96, 0.95, 0.925, 1.0],
            back_color: [0.62, 0.70, 0.78, 1.0],
            selection_color: [0.12, 0.53, 0.90, 0.45],
            background_color: [0.90, 0.91, 0.92, 1.0],
            xray_opacity: 0.45,
            line_width: 1.5,
            profile_width: 2.5,
            show_sky: true,
            sky_color: [0.925, 0.935, 0.945, 1.0],
            ground_color: [0.815, 0.820, 0.815, 1.0],
            show_grid: false,
        }
    }

    pub fn wireframe() -> Self {
        let mut s = Self::default_shaded();
        s.visual_style = VisualStyle::Wireframe;
        s.show_sky = false;
        s.background_color = [1.0, 1.0, 1.0, 1.0];
        s
    }

    pub fn hidden_line() -> Self {
        let mut s = Self::default_shaded();
        s.visual_style = VisualStyle::HiddenLine;
        s.show_sky = false;
        s.background_color = [1.0, 1.0, 1.0, 1.0];
        s.front_color = [1.0, 1.0, 1.0, 1.0];
        s
    }

    pub fn monochrome() -> Self {
        let mut s = Self::default_shaded();
        s.visual_style = VisualStyle::Monochrome;
        s.show_sky = false;
        s.background_color = [1.0, 1.0, 1.0, 1.0];
        s
    }

    pub fn xray() -> Self {
        let mut s = Self::default_shaded();
        s.visual_style = VisualStyle::XRay;
        s.xray_opacity = 0.4;
        s
    }
}
