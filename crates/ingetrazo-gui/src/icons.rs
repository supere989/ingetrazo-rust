// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Embedded vector icons for IngeTrazo GUI.
//!
//! Icons are pre-rendered from the native IngeTrazo mathematical drawing definitions
//! at 48x48 resolution with theme-matching ink and signature IngeTrazo orange accents (#f37329).
//! All icons are compiled directly into the binary with zero external asset dependencies.

use std::collections::HashMap;
use egui::{ColorImage, Context, TextureHandle, TextureOptions};

pub struct IconManager {
    cache: HashMap<&'static str, TextureHandle>,
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new()
    }
}

impl IconManager {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&mut self, ctx: &Context, name: &'static str) -> Option<&TextureHandle> {
        if !self.cache.contains_key(name) {
            if let Some(bytes) = get_icon_bytes(name) {
                if let Ok(img) = image::load_from_memory(bytes) {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels = rgba.into_raw();
                    let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
                    let handle = ctx.load_texture(
                        format!("icon_{name}"),
                        color_image,
                        TextureOptions::LINEAR,
                    );
                    self.cache.insert(name, handle);
                }
            }
        }
        self.cache.get(name)
    }
}

pub fn get_icon_bytes(name: &str) -> Option<&'static [u8]> {
    match name {
        "save" => Some(include_bytes!("../resources/icons/save.png")),
        "undo" => Some(include_bytes!("../resources/icons/undo.png")),
        "redo" => Some(include_bytes!("../resources/icons/redo.png")),
        "eraser" => Some(include_bytes!("../resources/icons/eraser.png")),
        "paint" => Some(include_bytes!("../resources/icons/paint.png")),
        "eyedropper" => Some(include_bytes!("../resources/icons/eyedropper.png")),
        "pushpull" => Some(include_bytes!("../resources/icons/pushpull.png")),
        "move" => Some(include_bytes!("../resources/icons/move.png")),
        "rotate" => Some(include_bytes!("../resources/icons/rotate.png")),
        "scale" => Some(include_bytes!("../resources/icons/scale.png")),
        "flip" => Some(include_bytes!("../resources/icons/flip.png")),
        "followme" => Some(include_bytes!("../resources/icons/followme.png")),
        "offset" => Some(include_bytes!("../resources/icons/offset.png")),
        "fillet" => Some(include_bytes!("../resources/icons/fillet.png")),
        "orbit" => Some(include_bytes!("../resources/icons/orbit.png")),
        "pan" => Some(include_bytes!("../resources/icons/pan.png")),
        "zoom" => Some(include_bytes!("../resources/icons/zoom.png")),
        "zoom_window" => Some(include_bytes!("../resources/icons/zoom_window.png")),
        "zoom_extents" => Some(include_bytes!("../resources/icons/zoom_extents.png")),
        "zoom_selection" => Some(include_bytes!("../resources/icons/zoom_selection.png")),
        "section_planes" => Some(include_bytes!("../resources/icons/section_planes.png")),
        "section_cuts" => Some(include_bytes!("../resources/icons/section_cuts.png")),
        "section_fill" => Some(include_bytes!("../resources/icons/section_fill.png")),
        "view_iso" => Some(include_bytes!("../resources/icons/view_iso.png")),
        "view_top" => Some(include_bytes!("../resources/icons/view_top.png")),
        "view_front" => Some(include_bytes!("../resources/icons/view_front.png")),
        "view_right" => Some(include_bytes!("../resources/icons/view_right.png")),
        "view_left" => Some(include_bytes!("../resources/icons/view_left.png")),
        "view_back" => Some(include_bytes!("../resources/icons/view_back.png")),
        "view_bottom" => Some(include_bytes!("../resources/icons/view_bottom.png")),
        "styles" => Some(include_bytes!("../resources/icons/styles.png")),
        "shadows" => Some(include_bytes!("../resources/icons/shadows.png")),
        "select" => Some(include_bytes!("../resources/icons/select.png")),
        "line" => Some(include_bytes!("../resources/icons/line.png")),
        "freehand" => Some(include_bytes!("../resources/icons/freehand.png")),
        "rectangle" => Some(include_bytes!("../resources/icons/rectangle.png")),
        "rotated_rect" => Some(include_bytes!("../resources/icons/rotated_rect.png")),
        "circle" => Some(include_bytes!("../resources/icons/circle.png")),
        "polygon" => Some(include_bytes!("../resources/icons/polygon.png")),
        "arc" => Some(include_bytes!("../resources/icons/arc.png")),
        "arc3" => Some(include_bytes!("../resources/icons/arc3.png")),
        "center_arc" => Some(include_bytes!("../resources/icons/center_arc.png")),
        "pie" => Some(include_bytes!("../resources/icons/pie.png")),
        "tape" => Some(include_bytes!("../resources/icons/tape.png")),
        "protractor" => Some(include_bytes!("../resources/icons/protractor.png")),
        "dimension" => Some(include_bytes!("../resources/icons/dimension.png")),
        "text" => Some(include_bytes!("../resources/icons/text.png")),
        "text3d" => Some(include_bytes!("../resources/icons/text3d.png")),
        "geopath" => Some(include_bytes!("../resources/icons/geopath.png")),
        "section" => Some(include_bytes!("../resources/icons/section.png")),
        "position_camera" => Some(include_bytes!("../resources/icons/position_camera.png")),
        "walk" => Some(include_bytes!("../resources/icons/walk.png")),
        "look_around" => Some(include_bytes!("../resources/icons/look_around.png")),
        "first_person" => Some(include_bytes!("../resources/icons/first_person.png")),
        "outer_shell" => Some(include_bytes!("../resources/icons/outer_shell.png")),
        _ => None,
    }
}

pub fn render_icon_button(
    ui: &mut egui::Ui,
    icon: Option<&TextureHandle>,
    fallback_text: &str,
    selected: bool,
    tooltip: &str,
) -> egui::Response {
    let size = egui::vec2(22.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    if resp.hovered() || selected {
        let bg = if selected {
            egui::Color32::from_rgb(0, 120, 215)
        } else {
            egui::Color32::from_rgb(60, 65, 75)
        };
        ui.painter().rect_filled(rect, 3.0, bg);
    }
    if let Some(h) = icon {
        let icon_rect = egui::Rect::from_center_size(rect.center(), size);
        ui.painter().image(
            h.id(),
            icon_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            fallback_text,
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
    }
    resp.on_hover_text(tooltip)
}
