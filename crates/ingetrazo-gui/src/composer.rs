// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! 2D Sheet Composer & Drawing Layout GUI (Track 4).
//!
//! Provides interactive paper canvas (mm units), exact architectural scales (1:50, 1:100, 1:200),
//! live 3D model viewports (MarcoVista), title blocks (Cajetín), text, north arrows, and graphic scales.

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::DVec3;
use ingetrazo_core::composition::{
    format_scale, model_height_for_frame, GraphicScaleItem, NorthArrowItem, Orientation, PaperSize,
    RectMm, SheetItem, TitleBlock, ViewFrame,
};
use ingetrazo_core::i18n::tr;
use ingetrazo_core::scene::Scene;
use ingetrazo_render::pipeline::project_point;

use crate::app::IngeTrazoApp;

/// Interactive state for the Sheet Composer view.
#[derive(Debug, Clone)]
pub struct ComposerState {
    pub pan_offset: Vec2,
    pub zoom: f32, // pixels per mm
    pub dragging_item_idx: Option<usize>,
    pub drag_start_pos: Option<Pos2>,
    pub drag_item_orig_pos: Option<(f64, f64)>,
}

impl Default for ComposerState {
    fn default() -> Self {
        Self {
            pan_offset: Vec2::ZERO,
            zoom: 1.5, // 1.5 px per mm by default (~400mm A3 = 600px wide)
            dragging_item_idx: None,
            drag_start_pos: None,
            drag_item_orig_pos: None,
        }
    }
}

pub fn render_sheet_composer(app: &mut IngeTrazoApp, ui: &mut egui::Ui, sheet_idx: usize) {
    if sheet_idx >= app.scene.sheets.len() {
        app.scene.active_sheet_index = None;
        return;
    }

    let (response_rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    let painter = ui.painter_at(response_rect);

    // Dark drafting table workspace background
    painter.rect_filled(response_rect, 0.0, Color32::from_rgb(32, 34, 40));

    // Handle Pan and Zoom
    let pointer = ui.input(|i| i.pointer.clone());
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    let space_down = ui.input(|i| i.key_down(egui::Key::Space));

    if scroll_delta != 0.0 && response.hovered() {
        let zoom_mult = if scroll_delta > 0.0 { 1.12 } else { 0.88 };
        // Clamp zoom between 0.2 (broad view) and 8.0 (close up detail)
        let new_zoom = (app.composer_state.zoom * zoom_mult).clamp(0.2, 8.0);
        app.composer_state.zoom = new_zoom;
    }

    // Pan with middle mouse or Space+drag
    if response.dragged() && (pointer.middle_down() || space_down) {
        app.composer_state.pan_offset += response.drag_delta();
    }

    // Extract sheet metrics and items
    let (paper_w_mm, paper_h_mm) = app.scene.sheets[sheet_idx].dimensions_mm();
    let margin_mm = app.scene.sheets[sheet_idx].margin_mm;
    let selected_item = app.scene.sheets[sheet_idx].selected_item;
    let items = app.scene.sheets[sheet_idx].items.clone();
    let zoom = app.composer_state.zoom;

    // Center paper on screen by default
    let center_x = response_rect.center().x + app.composer_state.pan_offset.x;
    let center_y = response_rect.center().y + app.composer_state.pan_offset.y;

    let paper_w_px = paper_w_mm as f32 * zoom;
    let paper_h_px = paper_h_mm as f32 * zoom;

    let paper_rect = Rect::from_center_size(
        Pos2::new(center_x, center_y),
        Vec2::new(paper_w_px, paper_h_px),
    );

    // Draw Paper Shadow & Sheet
    let shadow_rect = paper_rect.translate(Vec2::new(6.0, 8.0));
    painter.rect_filled(shadow_rect, 2.0, Color32::from_black_alpha(100));

    // Paper White Canvas
    painter.rect_filled(paper_rect, 0.0, Color32::WHITE);
    painter.rect_stroke(paper_rect, 0.0, Stroke::new(1.0, Color32::from_rgb(180, 180, 185)));

    // Dashed Printable Margin
    let margin_px = margin_mm as f32 * zoom;
    let margin_rect = paper_rect.shrink(margin_px);
    painter.rect_stroke(
        margin_rect,
        0.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(160, 175, 200, 150)),
    );

    // Helpers to convert mm to screen Pos2 / Rect
    let mm_to_screen = |x_mm: f64, y_mm: f64| -> Pos2 {
        Pos2::new(
            paper_rect.min.x + (x_mm as f32 * zoom),
            paper_rect.min.y + (y_mm as f32 * zoom),
        )
    };

    let rect_mm_to_screen = |r: RectMm| -> Rect {
        let p0 = mm_to_screen(r.x, r.y);
        Rect::from_min_size(p0, Vec2::new(r.w as f32 * zoom, r.h as f32 * zoom))
    };

    // Render Sheet Items
    for (idx, item) in items.iter().enumerate() {
        let is_selected = selected_item == Some(idx);

        match item {
            SheetItem::ViewFrame(frame) => {
                let fr = rect_mm_to_screen(frame.rect);

                // View frame background (paper white or technical background)
                painter.rect_filled(fr, 0.0, Color32::WHITE);

                // Render 3D model projection inside the view frame
                render_model_projection(&app.scene, &painter, fr, frame);

                // View Frame border
                if frame.border {
                    painter.rect_stroke(fr, 0.0, Stroke::new(1.2, Color32::from_rgb(40, 40, 45)));
                }

                // Title label
                if frame.show_title {
                    let title_text = if !frame.title_text.is_empty() {
                        &frame.title_text
                    } else {
                        "View"
                    };
                    let label_str = format!("{title_text}  {}", format_scale(frame.scale_n));
                    let title_pos = Pos2::new(fr.min.x, fr.max.y + 4.0 * zoom);
                    painter.text(
                        title_pos,
                        egui::Align2::LEFT_TOP,
                        label_str,
                        FontId::proportional((11.0 * (zoom / 1.5)).clamp(9.0, 16.0)),
                        Color32::from_rgb(20, 20, 25),
                    );
                }

                if is_selected {
                    render_selection_box(&painter, fr);
                }
            }

            SheetItem::TitleBlock(tb) => {
                let tr = rect_mm_to_screen(tb.rect);
                render_title_block(&painter, tr, tb, zoom);
                if is_selected {
                    render_selection_box(&painter, tr);
                }
            }

            SheetItem::Text(txt) => {
                let p = mm_to_screen(txt.pos.0, txt.pos.1);
                painter.text(
                    p,
                    egui::Align2::LEFT_TOP,
                    &txt.text,
                    FontId::proportional((txt.font_size_pt as f32 * zoom * 0.4).max(10.0)),
                    Color32::from_rgb(15, 15, 20),
                );
                if is_selected {
                    let text_rect = Rect::from_min_size(p, Vec2::new(60.0 * zoom, 12.0 * zoom));
                    render_selection_box(&painter, text_rect);
                }
            }

            SheetItem::NorthArrow(na) => {
                let p = mm_to_screen(na.pos.0, na.pos.1);
                let size_px = na.size_mm as f32 * zoom;
                let na_rect = Rect::from_min_size(p, Vec2::new(size_px, size_px));
                render_north_arrow(&painter, na_rect, na.rot_deg);
                if is_selected {
                    render_selection_box(&painter, na_rect);
                }
            }

            SheetItem::GraphicScale(gs) => {
                let p = mm_to_screen(gs.pos.0, gs.pos.1);
                let len_px = gs.length_mm as f32 * zoom;
                let gs_rect = Rect::from_min_size(p, Vec2::new(len_px, 8.0 * zoom));
                render_graphic_scale(&painter, gs_rect, gs, zoom);
                if is_selected {
                    render_selection_box(&painter, gs_rect);
                }
            }
        }
    }

    // Top Floating Toolbar for Composer Controls
    let toolbar_rect = Rect::from_min_size(
        Pos2::new(response_rect.min.x + 20.0, response_rect.min.y + 16.0),
        Vec2::new(580.0, 42.0),
    );

    let geo_north = app.scene.geo_datum.as_ref().map(|d| d.north).unwrap_or(0.0);
    let mut bump_needed = false;
    let mut fit_page_needed = false;

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(toolbar_rect), |ui| {
        egui::Frame::window(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr("Composer:")).strong());

                let sheet = &mut app.scene.sheets[sheet_idx];

                // Paper Size
                egui::ComboBox::from_id_salt("composer_paper_size")
                    .selected_text(format!("{:?}", sheet.paper_size))
                    .show_ui(ui, |ui| {
                        for p in &[PaperSize::A4, PaperSize::A3, PaperSize::A2, PaperSize::A1, PaperSize::Letter] {
                            if ui.selectable_value(&mut sheet.paper_size, *p, format!("{p:?}")).clicked() {
                                bump_needed = true;
                            }
                        }
                    });

                // Orientation
                let orient_label = match sheet.orientation {
                    Orientation::Landscape => tr("Landscape"),
                    Orientation::Portrait => tr("Portrait"),
                };
                if ui.button(orient_label).clicked() {
                    sheet.orientation = match sheet.orientation {
                        Orientation::Landscape => Orientation::Portrait,
                        Orientation::Portrait => Orientation::Landscape,
                    };
                    bump_needed = true;
                }

                ui.separator();

                // Add Items Menu
                if ui.button(tr("+ View")).on_hover_text(tr("Add 3D model viewport")).clicked() {
                    let mut vf = ViewFrame::default();
                    vf.rect = RectMm::new(sheet.margin_mm + 10.0, sheet.margin_mm + 10.0, 160.0, 110.0);
                    sheet.add_item(SheetItem::ViewFrame(vf));
                    bump_needed = true;
                }

                if ui.button(tr("+ Cajetín")).on_hover_text(tr("Add title block")).clicked() {
                    let mut tb = TitleBlock::default();
                    let (pw, ph) = sheet.dimensions_mm();
                    tb.rect = RectMm::new(pw - sheet.margin_mm - 140.0, ph - sheet.margin_mm - 50.0, 140.0, 50.0);
                    sheet.add_item(SheetItem::TitleBlock(tb));
                    bump_needed = true;
                }

                if ui.button(tr("+ North")).on_hover_text(tr("Add north arrow")).clicked() {
                    sheet.add_item(SheetItem::NorthArrow(NorthArrowItem {
                        id: uuid::Uuid::new_v4(),
                        pos: (sheet.margin_mm + 15.0, sheet.margin_mm + 15.0),
                        size_mm: 22.0,
                        rot_deg: geo_north,
                    }));
                    bump_needed = true;
                }

                if ui.button(tr("+ Scale")).on_hover_text(tr("Add graphic scale bar")).clicked() {
                    sheet.add_item(SheetItem::GraphicScale(GraphicScaleItem {
                        id: uuid::Uuid::new_v4(),
                        pos: (sheet.margin_mm + 15.0, sheet.dimensions_mm().1 - sheet.margin_mm - 20.0),
                        scale_n: 100.0,
                        length_mm: 50.0,
                        divisions: 4,
                    }));
                    bump_needed = true;
                }

                ui.separator();

                // Zoom to Fit Page
                if ui.button(tr("Fit Page")).clicked() {
                    fit_page_needed = true;
                }

                // Delete selected item
                if let Some(sel_idx) = sheet.selected_item {
                    if ui.button(tr("Delete Item")).clicked() {
                        sheet.remove_item(sel_idx);
                        bump_needed = true;
                    }
                }
            });
        });
    });

    if fit_page_needed {
        let fit_x = (response_rect.width() * 0.85) / paper_w_mm as f32;
        let fit_y = (response_rect.height() * 0.85) / paper_h_mm as f32;
        app.composer_state.zoom = fit_x.min(fit_y);
        app.composer_state.pan_offset = Vec2::ZERO;
    }

    if bump_needed {
        app.scene.bump_version();
    }

    // Handle Item Selection & Dragging on Canvas
    if let Some(hover_pos) = pointer.hover_pos() {
        let sheet = &mut app.scene.sheets[sheet_idx];
        if response.clicked() && !space_down {
            // Check reverse order (topmost first)
            let mut clicked_idx = None;
            for (idx, item) in sheet.items.iter().enumerate().rev() {
                let ir = rect_mm_to_screen(item.rect());
                if ir.contains(hover_pos) {
                    clicked_idx = Some(idx);
                    break;
                }
            }
            sheet.selected_item = clicked_idx;
        }

        // Drag selected item
        if response.drag_started() && !space_down && !pointer.middle_down() {
            if let Some(sel_idx) = sheet.selected_item {
                let ir = rect_mm_to_screen(sheet.items[sel_idx].rect());
                if ir.contains(hover_pos) {
                    app.composer_state.dragging_item_idx = Some(sel_idx);
                    app.composer_state.drag_start_pos = Some(hover_pos);
                    let orig_rect = sheet.items[sel_idx].rect();
                    app.composer_state.drag_item_orig_pos = Some((orig_rect.x, orig_rect.y));
                }
            }
        }

        if response.dragged() {
            if let (Some(drag_idx), Some(start_pos), Some((orig_x, orig_y))) = (
                app.composer_state.dragging_item_idx,
                app.composer_state.drag_start_pos,
                app.composer_state.drag_item_orig_pos,
            ) {
                if drag_idx < sheet.items.len() {
                    let delta_px = hover_pos - start_pos;
                    let delta_mm_x = delta_px.x as f64 / zoom as f64;
                    let delta_mm_y = delta_px.y as f64 / zoom as f64;
                    sheet.items[drag_idx].set_pos(orig_x + delta_mm_x, orig_y + delta_mm_y);
                }
            }
        }

        if response.drag_stopped() {
            if app.composer_state.dragging_item_idx.is_some() {
                app.composer_state.dragging_item_idx = None;
                app.composer_state.drag_start_pos = None;
                app.composer_state.drag_item_orig_pos = None;
                app.scene.bump_version();
            }
        }
    }
}

/// Projects and renders the 3D model inside the ViewFrame at exact scale 1:N.
fn render_model_projection(
    scene: &Scene,
    painter: &egui::Painter,
    frame_rect: Rect,
    frame: &ViewFrame,
) {
    let painter = painter.with_clip_rect(frame_rect);

    let (yaw, pitch) = match frame.view_key.as_str() {
        "std:top" | "__current__" => (0.0, 89.9_f64.to_radians()),
        "std:front" => (0.0, 0.0),
        "std:right" => (90.0_f64.to_radians(), 0.0),
        "std:back" => (180.0_f64.to_radians(), 0.0),
        "std:left" => (-90.0_f64.to_radians(), 0.0),
        "std:iso" => (45.0_f64.to_radians(), 35.264_f64.to_radians()),
        _ => (0.0, 89.9_f64.to_radians()),
    };

    // Calculate camera target and ortho projection matrix
    let scene_center = DVec3::ZERO;
    let view_dir = DVec3::new(
        pitch.cos() * yaw.sin(),
        pitch.cos() * yaw.cos(),
        pitch.sin(),
    );
    let camera_pos = scene_center + view_dir * 100.0;
    let up = if pitch.cos().abs() < 1e-4 { DVec3::Y } else { DVec3::Z };
    let view_mat = glam::DMat4::look_at_rh(camera_pos, scene_center, up);

    // Sacred scale equation: model_height_m = frame.rect.h * scale_n / 1000
    let model_h_m = model_height_for_frame(frame.rect.h, frame.scale_n);
    let model_w_m = model_h_m * (frame.rect.w / frame.rect.h);

    let proj_mat = glam::DMat4::orthographic_rh(
        -model_w_m * 0.5,
        model_w_m * 0.5,
        -model_h_m * 0.5,
        model_h_m * 0.5,
        0.1,
        1000.0,
    );
    let view_proj = proj_mat * view_mat;

    // Line stroke for technical plan rendering
    let edge_stroke = Stroke::new(1.2, Color32::from_rgb(25, 25, 30));

    // Draw loose mesh edges
    for e in &scene.mesh.edges {
        if e.hidden {
            continue;
        }
        let p0 = scene.mesh.vertices[e.v0].position;
        let p1 = scene.mesh.vertices[e.v1].position;

        if let (Some((s0, _)), Some((s1, _))) = (
            project_point(p0, &view_proj, frame_rect),
            project_point(p1, &view_proj, frame_rect),
        ) {
            painter.line_segment([s0, s1], edge_stroke);
        }
    }

    // Draw group edges
    for g in &scene.groups {
        if g.hidden {
            continue;
        }
        for e in &g.mesh.edges {
            if e.hidden {
                continue;
            }
            let p0 = g.transform.transform_point3(g.mesh.vertices[e.v0].position);
            let p1 = g.transform.transform_point3(g.mesh.vertices[e.v1].position);

            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(p0, &view_proj, frame_rect),
                project_point(p1, &view_proj, frame_rect),
            ) {
                painter.line_segment([s0, s1], edge_stroke);
            }
        }
    }

    // Draw traced GeoPaths if present
    for gp in &scene.geo_paths {
        for (p0, p1) in gp.segments() {
            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(p0, &view_proj, frame_rect),
                project_point(p1, &view_proj, frame_rect),
            ) {
                painter.line_segment([s0, s1], Stroke::new(1.8, Color32::from_rgb(20, 100, 200)));
            }
        }
    }
}

/// Renders classical CAD Title Block (Cajetín).
fn render_title_block(painter: &egui::Painter, rect: Rect, tb: &TitleBlock, zoom: f32) {
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.4, Color32::BLACK));

    // Internal grid dividers
    let h_half = rect.height() * 0.5;
    let w_part = rect.width() * 0.65;

    // Horizontal divider
    painter.line_segment(
        [Pos2::new(rect.min.x, rect.min.y + h_half), Pos2::new(rect.max.x, rect.min.y + h_half)],
        Stroke::new(1.0, Color32::BLACK),
    );

    // Vertical divider
    painter.line_segment(
        [Pos2::new(rect.min.x + w_part, rect.min.y), Pos2::new(rect.min.x + w_part, rect.max.y)],
        Stroke::new(1.0, Color32::BLACK),
    );

    let font_title = FontId::proportional((11.0 * (zoom / 1.5)).clamp(9.0, 15.0));
    let font_body = FontId::proportional((9.0 * (zoom / 1.5)).clamp(7.5, 12.0));

    // Project Name & Drawing Title
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0),
        egui::Align2::LEFT_TOP,
        &tb.project_name,
        font_title.clone(),
        Color32::BLACK,
    );
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + h_half + 6.0),
        egui::Align2::LEFT_TOP,
        &tb.drawing_title,
        font_title,
        Color32::from_rgb(30, 30, 40),
    );

    // Right column metadata
    painter.text(
        Pos2::new(rect.min.x + w_part + 6.0, rect.min.y + 5.0),
        egui::Align2::LEFT_TOP,
        &format!("Date: {}", tb.date),
        font_body.clone(),
        Color32::from_rgb(50, 50, 50),
    );
    painter.text(
        Pos2::new(rect.min.x + w_part + 6.0, rect.min.y + 18.0),
        egui::Align2::LEFT_TOP,
        &format!("Scale: {}", tb.scale_text),
        font_body.clone(),
        Color32::from_rgb(50, 50, 50),
    );
    painter.text(
        Pos2::new(rect.min.x + w_part + 6.0, rect.min.y + h_half + 6.0),
        egui::Align2::LEFT_TOP,
        &format!("Sheet: {}", tb.sheet_number),
        font_body,
        Color32::from_rgb(20, 20, 20),
    );
}

/// Renders architectural North Arrow.
fn render_north_arrow(painter: &egui::Painter, rect: Rect, rot_deg: f64) {
    let center = rect.center();
    let r = rect.width() * 0.45;
    let rad = rot_deg.to_radians();

    // Circle ring
    painter.circle_stroke(center, r, Stroke::new(1.2, Color32::BLACK));

    // North arrow tip pointing up
    let tip = center + Vec2::new(-(rad.sin() as f32) * r * 0.9, -(rad.cos() as f32) * r * 0.9);
    let left = center + Vec2::new(-(rad.cos() as f32) * r * 0.4, (rad.sin() as f32) * r * 0.4);
    let right = center + Vec2::new((rad.cos() as f32) * r * 0.4, -(rad.sin() as f32) * r * 0.4);

    // Black half
    painter.add(egui::Shape::convex_polygon(
        vec![center, tip, left],
        Color32::BLACK,
        Stroke::new(1.0, Color32::BLACK),
    ));
    // White half
    painter.add(egui::Shape::convex_polygon(
        vec![center, tip, right],
        Color32::WHITE,
        Stroke::new(1.0, Color32::BLACK),
    ));

    // Letter 'N'
    let n_pos = tip + Vec2::new(0.0, -8.0);
    painter.text(n_pos, egui::Align2::CENTER_CENTER, "N", FontId::proportional(11.0), Color32::BLACK);
}

/// Renders Graphic Scale Bar.
fn render_graphic_scale(painter: &egui::Painter, rect: Rect, gs: &GraphicScaleItem, zoom: f32) {
    let bar_h = 4.0 * zoom;
    let div_w = rect.width() / gs.divisions as f32;

    for i in 0..gs.divisions {
        let fill = if i % 2 == 0 { Color32::BLACK } else { Color32::WHITE };
        let div_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + i as f32 * div_w, rect.min.y),
            Vec2::new(div_w, bar_h),
        );
        painter.rect_filled(div_rect, 0.0, fill);
        painter.rect_stroke(div_rect, 0.0, Stroke::new(1.0, Color32::BLACK));
    }

    // Metric length label (e.g. 5 m)
    let total_meters = (gs.length_mm * gs.scale_n) / 1000.0;
    painter.text(
        Pos2::new(rect.max.x, rect.min.y + bar_h + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("{total_meters:.1} m"),
        FontId::proportional((9.0 * (zoom / 1.5)).clamp(8.0, 12.0)),
        Color32::BLACK,
    );
}

/// Highlights selected item with dashed border and corner grips.
fn render_selection_box(painter: &egui::Painter, rect: Rect) {
    let highlight_color = Color32::from_rgb(30, 130, 240);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.5, highlight_color));

    // Corner handle boxes
    let handle_size = 6.0;
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for c in corners {
        let h_rect = Rect::from_center_size(c, Vec2::splat(handle_size));
        painter.rect_filled(h_rect, 0.0, Color32::WHITE);
        painter.rect_stroke(h_rect, 0.0, Stroke::new(1.2, highlight_color));
    }
}
