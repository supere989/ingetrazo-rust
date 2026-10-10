// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Interactive 3D Viewport canvas.
//!
//! Handles orbit, pan, zoom, cursor raycasting, inference snapping,
//! tool interactions, and custom rendering.

use glam::DVec3;
use ingetrazo_core::math::Plane;
use ingetrazo_render::pipeline::{build_scene_render_data, project_point, render_scene_to_egui};
use ingetrazo_render::snap_glyph::SnapGlyph;
use ingetrazo_tools::base::{ToolContext, ToolType};

use crate::app::IngeTrazoApp;

pub fn render_viewport(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    let (rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

    // Update camera aspect ratio
    if rect.width() > 1.0 && rect.height() > 1.0 {
        app.camera.aspect = (rect.width() / rect.height()) as f64;
    }

    let painter = ui.painter_at(rect);

    // 1. Mouse Navigation (Orbit, Pan, Zoom)
    let pointer = ui.input(|i| i.pointer.clone());
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    let shift_down = ui.input(|i| i.modifiers.shift);
    let alt_down = ui.input(|i| i.modifiers.alt);
    let ctrl_down = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);

    // Zoom on wheel
    if scroll_delta != 0.0 && response.hovered() {
        let zoom_factor = if scroll_delta > 0.0 { 1.15 } else { 0.85 };
        app.camera.zoom(zoom_factor);
    }

    // Drag navigation
    if response.dragged() {
        let drag_delta = response.drag_delta();
        let is_middle = pointer.middle_down();
        let is_alt_left =
            alt_down && pointer.primary_down() && app.active_tool_type != ToolType::FollowMe;

        if is_middle || is_alt_left {
            if shift_down {
                // Pan
                app.camera.pan(drag_delta.x as f64, drag_delta.y as f64);
            } else {
                // Orbit
                let delta_yaw = -(drag_delta.x as f64) * 0.01;
                let delta_pitch = (drag_delta.y as f64) * 0.01;
                app.camera.orbit(delta_yaw, delta_pitch);
            }
        }
    }

    // 2. Cursor Raycasting & Snapping
    let mut active_snap_glyph = None;

    if let Some(mouse_pos) = pointer.hover_pos() {
        if rect.contains(mouse_pos) {
            let rel_x = (mouse_pos.x - rect.min.x) as f64;
            let rel_y = (mouse_pos.y - rect.min.y) as f64;

            let cursor_ray =
                app.camera
                    .screen_to_ray(rel_x, rel_y, rect.width() as f64, rect.height() as f64);

            // Default ground plane intersection
            let ground_plane = Plane::new(
                app.paste_preview
                    .as_ref()
                    .map(|p| p.reference)
                    .unwrap_or(DVec3::ZERO),
                DVec3::Z,
            );
            let mut cursor_world_pos = if let Some(t) = ground_plane.intersect_ray(&cursor_ray) {
                if t > 0.0 {
                    cursor_ray.at(t)
                } else {
                    cursor_ray.at(10.0)
                }
            } else {
                cursor_ray.at(10.0)
            };

            // Arrow keys for explicit axis locks
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                app.snap_engine.locked_axis = if app.snap_engine.locked_axis == Some('x') {
                    None
                } else {
                    Some('x')
                };
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                app.snap_engine.locked_axis = if app.snap_engine.locked_axis == Some('y') {
                    None
                } else {
                    Some('y')
                };
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                app.snap_engine.locked_axis = if app.snap_engine.locked_axis == Some('z') {
                    None
                } else {
                    Some('z')
                };
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                app.snap_engine.locked_axis = None;
                app.snap_engine.locked_line = None;
            }

            // Run Snap Engine
            let start_pt = app
                .paste_preview
                .as_ref()
                .map(|p| p.reference)
                .or_else(|| app.active_tool.rubber_band().map(|(s, _)| s));
            let snap_res = app
                .snap_engine
                .snap(&cursor_ray, &app.scene, start_pt, None);
            if snap_res.kind != ingetrazo_core::snap::SnapKind::None {
                cursor_world_pos = snap_res.point;
                active_snap_glyph = SnapGlyph::from_snap_result(&snap_res, start_pt);

                // If Shift is held down and we have an active axis or guide, lock it
                if shift_down
                    && app.snap_engine.locked_axis.is_none()
                    && app.snap_engine.locked_line.is_none()
                {
                    if let Some(ax) = snap_res.axis {
                        app.snap_engine.locked_axis = Some(ax);
                    } else if let Some((g0, g1, _)) = snap_res.guides.first() {
                        app.snap_engine.locked_line = Some((*g0, *g1));
                    }
                }
            } else if !shift_down && app.snap_engine.locked_line.is_some() {
                // Release temporary shift line lock
                app.snap_engine.locked_line = None;
            }

            let tool_ctx = ToolContext {
                world: cursor_world_pos,
                ray: cursor_ray,
                shift_held: shift_down,
                ctrl_held: ctrl_down,
                alt_held: alt_down,
            };

            if app.paste_preview.is_some() {
                app.update_paste_preview(cursor_world_pos);
                if response.clicked() && !alt_down {
                    app.commit_paste_preview();
                }
            } else {
                let follow_me = app.active_tool_type == ToolType::FollowMe;
                if follow_me && pointer.button_pressed(egui::PointerButton::Primary) {
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(response.id.with("followme_press"), mouse_pos)
                    });
                    app.dispatch_tool_click(&tool_ctx);
                }
                app.active_tool.on_move(&mut app.scene, &tool_ctx);
                if follow_me {
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(response.id.with("followme_context"), tool_ctx.clone())
                    });
                }
                if !follow_me && response.double_clicked() && !alt_down {
                    app.dispatch_tool_double_click(&tool_ctx);
                } else if !follow_me && response.clicked() && !alt_down {
                    app.dispatch_tool_click(&tool_ctx);
                }
            }
        }
    }
    // Observe release beyond the viewport, without committing a tool during paste.
    if app.paste_preview.is_none()
        && app.active_tool_type == ToolType::FollowMe
        && pointer.button_released(egui::PointerButton::Primary)
    {
        let origin = ui
            .ctx()
            .data_mut(|data| data.remove_temp::<egui::Pos2>(response.id.with("followme_press")));
        let context = ui
            .ctx()
            .data(|data| data.get_temp::<ToolContext>(response.id.with("followme_context")));
        if let (Some(origin), Some(context), Some(end)) = (origin, context, pointer.hover_pos()) {
            app.dispatch_tool_release(&context, origin.distance(end) as f64);
        }
    }

    // Handle Esc to cancel active tool and clear locks
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.snap_engine.locked_axis = None;
        app.snap_engine.locked_line = None;
        app.snap_engine.clear_encouraged();
        if app.paste_preview.is_some() {
            app.cancel_paste_preview();
        } else {
            app.active_tool.on_cancel(&mut app.scene);
        }
        ui.ctx().data_mut(|data| {
            data.remove::<egui::Pos2>(response.id.with("followme_press"));
            data.remove::<ToolContext>(response.id.with("followme_context"));
        });
    }

    // 3. Render 3D Scene
    // Decode and upload embedded textures into egui texture manager
    for (name, bytes) in &app.scene.embedded_textures {
        if !app.textures.contains_key(name) {
            if let Ok(img) = image::load_from_memory(bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                let handle =
                    ui.ctx()
                        .load_texture(name, color_image, egui::TextureOptions::LINEAR_REPEAT);
                app.textures.insert(name.clone(), handle);
            }
        }
    }

    let mut tex_map = std::collections::HashMap::new();
    for (name, handle) in &app.textures {
        tex_map.insert(name.clone(), handle.id());
        if let Some(stripped) = name.strip_prefix("textures/") {
            tex_map.insert(stripped.to_string(), handle.id());
        }
        if let Some(pos) = name.rfind('/') {
            tex_map.insert(name[pos + 1..].to_string(), handle.id());
        }
    }

    let render_data = build_scene_render_data(
        &app.scene,
        &app.camera,
        &app.style,
        active_snap_glyph.as_ref(),
    );

    render_scene_to_egui(
        &render_data,
        &app.camera,
        &painter,
        rect,
        &app.style,
        &tex_map,
    );

    if let Some(preview) = &app.paste_preview {
        let mut camera = app.camera.clone();
        camera.target -= preview.offset;
        let ghost = build_scene_render_data(&preview.scene, &camera, &app.style, None);
        render_scene_to_egui(&ghost, &camera, &painter, rect, &app.style, &tex_map);
    } else if let Some(mesh) = app.active_tool.preview_mesh() {
        // Immutable tool preview shares no document or history mutation.
        let view_proj = app.camera.projection_matrix() * app.camera.view_matrix();
        let mut overlay = egui::Mesh::default();
        for id in 0..mesh.faces.len() {
            for (a, b, c) in mesh.triangulate_face(id) {
                if let (Some((a, _)), Some((b, _)), Some((c, _))) = (
                    project_point(a, &view_proj, rect),
                    project_point(b, &view_proj, rect),
                    project_point(c, &view_proj, rect),
                ) {
                    let start = overlay.vertices.len() as u32;
                    for pos in [a, b, c] {
                        overlay.colored_vertex(
                            pos,
                            egui::Color32::from_rgba_unmultiplied(255, 180, 60, 45),
                        );
                    }
                    overlay.add_triangle(start, start + 1, start + 2);
                }
            }
        }
        painter.add(egui::Shape::mesh(overlay));
        for edge in &mesh.edges {
            if let (Some((a, _)), Some((b, _))) = (
                project_point(mesh.vertices[edge.v0].position, &view_proj, rect),
                project_point(mesh.vertices[edge.v1].position, &view_proj, rect),
            ) {
                painter.line_segment(
                    [a, b],
                    egui::Stroke::new(1., egui::Color32::from_rgba_unmultiplied(255, 180, 60, 150)),
                );
            }
        }
    }

    // 4. Render Active Tool Rubber Band (if any)
    for (p0, p1) in app.active_tool.rubber_band_lines() {
        let view_proj = app.camera.projection_matrix() * app.camera.view_matrix();
        if let (Some((s0, _)), Some((s1, _))) = (
            project_point(p0, &view_proj, rect),
            project_point(p1, &view_proj, rect),
        ) {
            painter.line_segment(
                [s0, s1],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 60, 60)),
            );
        }
    }

    if let Some((label, point)) = app.active_tool.value_label() {
        let view_proj = app.camera.projection_matrix() * app.camera.view_matrix();
        if let Some((position, _)) = project_point(point, &view_proj, rect) {
            painter.text(
                position + egui::vec2(8., -8.),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(14.),
                egui::Color32::from_rgb(255, 200, 90),
            );
        }
    }

    // 5. Render Section Planes (cutting frames and symbol tags)
    if app.scene.show_section_planes {
        let view_proj = app.camera.projection_matrix() * app.camera.view_matrix();
        for sp in &app.scene.section_planes {
            let n = sp.normal;
            let (u, v) = ingetrazo_render::pipeline::projection_basis(n);
            let r = 2.0;
            let corners = [
                sp.point - u * r - v * r,
                sp.point + u * r - v * r,
                sp.point + u * r + v * r,
                sp.point - u * r + v * r,
            ];
            let mut screen_corners = Vec::new();
            for c in &corners {
                if let Some((sc, _)) = project_point(*c, &view_proj, rect) {
                    screen_corners.push(sc);
                }
            }
            if screen_corners.len() == 4 {
                let stroke = egui::Stroke::new(1.8, egui::Color32::from_rgb(243, 115, 41));
                for i in 0..4 {
                    painter.line_segment([screen_corners[i], screen_corners[(i + 1) % 4]], stroke);
                }
                if let Some((tag_sc, _)) = project_point(sp.point + v * (r + 0.3), &view_proj, rect)
                {
                    painter.text(
                        tag_sc,
                        egui::Align2::CENTER_CENTER,
                        &format!("Section {}", sp.symbol),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_rgb(243, 115, 41),
                    );
                }
            }
        }
    }
}
