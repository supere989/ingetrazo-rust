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
        // Python camera.set_aspect divides in double precision.
        app.camera.aspect = f64::from(rect.width()) / f64::from(rect.height());
    }

    let painter = ui.painter_at(rect);

    // 1. Mouse Navigation (Orbit, Pan, Zoom)
    let pointer = ui.input(|i| i.pointer.clone());
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    let shift_down = ui.input(|i| i.modifiers.shift);
    let alt_down = ui.input(|i| i.modifiers.alt);
    let ctrl_down = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
    // Modifier release must retire the temporary lock even outside the canvas.
    let scale_modifiers=if app.active_tool_type==ToolType::Scale{crate::pointer_modifiers::get(ui.ctx())}else{None};
    let mut scale_modifier_cursor=0;
    let scale_input_admitted=scale_modifiers.as_ref().is_none_or(|handle|handle.lock().unwrap().admitted());
    if app.text3d_dialog.is_none() {
    app.snap_engine.sync_drawing_frame(app.scene.drawing_frame);
    if !scale_input_admitted{app.status_message="Native modifier batch exceeded bounded admission".into();}
    if app.active_tool_type == ToolType::Scale {
        app.snap_engine.update_shift_state(false, None);
        if scale_modifiers.is_none(){app.active_tool.note_shift_state(shift_down);app.dispatch_tool_control_state(ui.input(|i|i.modifiers.ctrl));}
    } else { app.snap_engine.update_shift_state(shift_down,app.active_tool.snap_anchor());app.dispatch_tool_control_state(ui.input(|i|i.modifiers.ctrl)); }

    }

    // Zoom on wheel
    if app.text3d_dialog.is_none() && scroll_delta != 0.0 && response.hovered() {
        let zoom_factor = if scroll_delta > 0.0 { 1.15 } else { 0.85 };
        app.camera.zoom(zoom_factor);
    }

    // Drag navigation
    if app.text3d_dialog.is_none() && response.dragged() {
        let drag_delta = response.drag_delta();
        let is_middle = pointer.middle_down();
        let is_alt_left =
            alt_down && pointer.primary_down() && app.active_tool_type != ToolType::FollowMe && app.navigation.mode.is_none();

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
    let mut active_center_hint = None;
    let mut navigation_preview_stroke=egui::Stroke::new(2.,egui::Color32::from_rgba_unmultiplied(242,115,41,217));

    if app.text3d_dialog.is_none() {
    if app.navigation.mode.is_some() {
        if let Some(result)=crate::navigation::input(app,ui,rect,&response){
            active_snap_glyph=SnapGlyph::from_snap_result(&result,None);
            use ingetrazo_core::snap::SnapKind;
            let (color,alpha,width)=match result.kind{
                SnapKind::AxisLock=>(result.color,1.,2.5),SnapKind::AxisInference=>(result.color,0.5,2.5),
                SnapKind::Reference|SnapKind::FromPoint=>(result.color,1.,2.5),
                SnapKind::Close=>([0.20,0.40,0.78],0.95,2.),_ =>([0.95,0.45,0.16],0.85,2.)};
            navigation_preview_stroke=egui::Stroke::new(width,egui::Color32::from_rgba_unmultiplied((color[0]*255.).round()as u8,(color[1]*255.).round()as u8,(color[2]*255.).round()as u8,(alpha*255_f32).round()as u8));
            let viewport=ingetrazo_core::screen_pick::PixelViewport::new(&app.camera,rect.width()as f64,rect.height()as f64);
            if let Some(mouse)=pointer.hover_pos(){active_center_hint=app.snap_engine.center_hint_pixel(&viewport,glam::DVec2::new((mouse.x-rect.min.x)as f64,(mouse.y-rect.min.y)as f64));}
        }
    } else {
    if let Some(mouse_pos) = pointer.hover_pos() {
        if rect.contains(mouse_pos) {
            let rel_x = (mouse_pos.x - rect.min.x) as f64;
            let rel_y = (mouse_pos.y - rect.min.y) as f64;

            let cursor_ray =
                app.camera
                    .screen_to_ray(rel_x, rel_y, rect.width() as f64, rect.height() as f64);

            // Tool-local work planes keep rectangle/Freehand hover coordinates
            // coplanar with the face where the gesture began.
            let plane_point_normal=cursor_work_plane(app,&cursor_ray);
            let (plane_point,plane_normal)=plane_point_normal;
            let ground_plane=Plane::new(plane_point,plane_normal);
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
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && !app.active_tool.lock_drawing_plane(app.scene.drawing_axes().x) {
                app.snap_engine.locked_axis = if app.snap_engine.locked_axis == Some('x') {
                    None
                } else {
                    Some('x')
                };
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && !app.active_tool.lock_drawing_plane(app.scene.drawing_axes().y) {
                app.snap_engine.locked_axis = if app.snap_engine.locked_axis == Some('y') {
                    None
                } else {
                    Some('y')
                };
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !app.active_tool.lock_drawing_plane(app.scene.drawing_axes().z) {
                app.snap_engine.locked_axis = if app.snap_engine.locked_axis == Some('z') {
                    None
                } else {
                    Some('z')
                };
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !app.active_tool.toggle_inference_lock() {
                let viewport=ingetrazo_core::screen_pick::PixelViewport::new(&app.camera,rect.width() as f64,rect.height() as f64);
                let reference=ingetrazo_core::screen_pick::hovered_reference(&app.scene,&viewport,glam::DVec2::new(rel_x,rel_y),&cursor_ray);
                if !app.active_tool.lock_drawing_reference(reference){
                    app.snap_engine.locked_axis = None;
                    app.snap_engine.locked_line = None;
                }
            }
            // A key can change the plane in this frame. Reintersect before
            // producing the tool context, including click-on-key frames.
            let (plane_point,plane_normal)=cursor_work_plane(app,&cursor_ray);
            if let Some(t)=Plane::new(plane_point,plane_normal).intersect_ray(&cursor_ray){
                if t>0.{cursor_world_pos=cursor_ray.at(t);}
            }

            // Run Snap Engine
            let start_pt = app
                .paste_preview
                .as_ref()
                .map(|p| p.reference)
                .or_else(|| app.active_tool.snap_anchor());
            let screen_drawing=app.blender_is_picking()||matches!(app.active_tool_type,ToolType::Line|ToolType::Rectangle|ToolType::Move|ToolType::Scale);
            app.snap_engine.note_model_revision(app.model_revision);
            let snap_res=if !app.active_tool.uses_snap(){ingetrazo_core::snap::SnapResult::default()}else if screen_drawing{
                let context=ingetrazo_core::snap_screen::ScreenSnapContext{
                    viewport:ingetrazo_core::screen_pick::PixelViewport::new(&app.camera,rect.width() as f64,rect.height() as f64),
                    cursor:glam::DVec2::new(rel_x,rel_y),candidate:cursor_world_pos,plane_normal,
                };
                let result=app.snap_engine.snap_screen_with_policy(&cursor_ray,&app.scene,start_pt,app.active_tool.snap_close(),&context,&ingetrazo_core::snap_screen::ScreenSnapPolicy{exclusions:app.active_tool.snap_exclusions(),magnetic_axis_deg:app.active_tool.magnetic_axis_deg(),screen_axis_px:app.active_tool.screen_axis_px(),face_under_cursor:ingetrazo_core::screen_pick::hover_face(&app.scene,&cursor_ray).is_some()});
                app.snap_engine.note_screen_result(&result,start_pt.is_some(),ui.input(|i|i.time));
                if result.kind!=ingetrazo_core::snap::SnapKind::Center || app.native_gpu_viewport_enabled {
                    active_center_hint=app.snap_engine.center_hint_pixel(&context.viewport,context.cursor);
                }
                result
            }else{app.snap_engine.snap(&cursor_ray,&app.scene,start_pt,app.active_tool.snap_close())};
            if app.active_tool_type!=ToolType::Freehand && snap_res.kind != ingetrazo_core::snap::SnapKind::None {
                cursor_world_pos = snap_res.point;
                active_snap_glyph = SnapGlyph::from_snap_result(&snap_res, start_pt);

            }
            app.active_tool.note_snap_result(&snap_res);
            app.snap_engine.note_inference_result(&snap_res);

            app.active_tool.note_cursor_plane(plane_point,plane_normal);
            if matches!(app.active_tool_type,ToolType::Select|ToolType::Scale) {app.active_tool.note_pick_viewport(ingetrazo_core::screen_pick::PixelViewport::new(&app.camera,rect.width() as f64,rect.height() as f64),app.camera.eye(),app.model_revision);}
            let mut tool_ctx = ToolContext {
                screen: Some(glam::DVec2::new(rel_x,rel_y)),
                world: cursor_world_pos,
                ray: cursor_ray,
                shift_held: shift_down,
                ctrl_held: ctrl_down,
                alt_held: alt_down,
            };

            // Qt Select receives the mouse event's modifier snapshot, even when
            // a later modifier release shares this egui frame.
            if app.active_tool_type==ToolType::Select&&(response.clicked()||response.double_clicked()) {
                if let Some(modifiers)=ui.input(|input|input.events.iter().rev().find_map(|event|match event {
                    egui::Event::PointerButton{button:egui::PointerButton::Primary,pressed:false,modifiers,..}=>Some(*modifiers),_=>None,
                })) {
                    tool_ctx.shift_held=modifiers.shift;
                    tool_ctx.ctrl_held=modifiers.ctrl||modifiers.command;
                    tool_ctx.alt_held=modifiers.alt;
                }
            }

            if response.clicked()&&!alt_down&&app.handle_blender_click(&tool_ctx,mouse_pos,rect,snap_res.kind==ingetrazo_core::snap::SnapKind::OnFace) {
                // Light placement and picked bulbs own this click.
            } else if app.paste_preview.is_some() {
                app.update_paste_preview(cursor_world_pos);
                if response.clicked() && !alt_down {
                    app.commit_paste_preview();
                }
            } else {
                let follow_me = matches!(app.active_tool_type,ToolType::FollowMe|ToolType::Freehand|ToolType::Scale);
                if follow_me && scale_input_admitted && !alt_down && pointer.button_pressed(egui::PointerButton::Primary) {
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(response.id.with("followme_press"), mouse_pos)
                    });
                    if app.active_tool_type == ToolType::Scale {
                        // Acquisition belongs to the press event, not the final
                        // hover position after coalesced pointer motion.
                        let press = ui.input(|input| input.events.iter().enumerate().find_map(|(index,event)| match event {
                            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers } => Some((index,*pos, *modifiers)),
                            _ => None,
                        }));
                        if let Some((index,pos, modifiers)) = press {
                            scale_modifier_changes(app,scale_modifiers.as_ref(),index,&mut scale_modifier_cursor);
                            ui.ctx().data_mut(|data| data.insert_temp(response.id.with("followme_press"), pos));
                            let mut press_ctx = tool_ctx.clone();
                            press_ctx.screen = Some(glam::DVec2::new((pos.x-rect.min.x) as f64,(pos.y-rect.min.y) as f64));
                            press_ctx.shift_held = modifiers.shift;
                            press_ctx.ctrl_held = modifiers.ctrl || modifiers.command;
                            press_ctx.alt_held = modifiers.alt;
                            let screen = press_ctx.screen.unwrap();
                            press_ctx.ray = app.camera.screen_to_ray(screen.x,screen.y,rect.width() as f64,rect.height() as f64);
                            if pos != mouse_pos && app.active_tool.pending_model_preview() {
                                let (point,normal)=cursor_work_plane(app,&press_ctx.ray);
                                let candidate=Plane::new(point,normal).intersect_ray(&press_ctx.ray).filter(|t| *t>0.).map_or(point,|t|press_ctx.ray.at(t));
                                let context=ingetrazo_core::snap_screen::ScreenSnapContext {viewport:ingetrazo_core::screen_pick::PixelViewport::new(&app.camera,rect.width() as f64,rect.height() as f64),cursor:screen,candidate,plane_normal:normal};
                                let press_snap=app.snap_engine.snap_screen_with_policy(&press_ctx.ray,&app.scene,app.active_tool.snap_anchor(),app.active_tool.snap_close(),&context,&ingetrazo_core::snap_screen::ScreenSnapPolicy {exclusions:app.active_tool.snap_exclusions(),magnetic_axis_deg:app.active_tool.magnetic_axis_deg(),screen_axis_px:app.active_tool.screen_axis_px(),face_under_cursor:ingetrazo_core::screen_pick::hover_face(&app.scene,&press_ctx.ray).is_some()});
                                press_ctx.world=press_snap.point;
                                app.active_tool.note_snap_result(&press_snap);
                            }
                            app.dispatch_tool_click(&press_ctx);
                            app.active_tool.note_snap_result(&snap_res);
                        }
                    } else {
                        app.dispatch_tool_click(&tool_ctx);
                    }
                }
                let scale_motion = app.active_tool_type == ToolType::Scale && ui.input(|input| {
                    let press = input.events.iter().rposition(|event| matches!(event, egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed: true, .. }));
                    input.events.iter().enumerate().any(|(index,event)| matches!(event,egui::Event::PointerMoved(_)) && press.is_none_or(|press| index > press))
                });
                if app.active_tool_type != ToolType::Scale || (scale_input_admitted && scale_motion) {
                    if app.active_tool_type==ToolType::Scale{let index=ui.input(|i|i.events.iter().rposition(|e|matches!(e,egui::Event::PointerMoved(_))).unwrap_or(0));scale_modifier_changes(app,scale_modifiers.as_ref(),index,&mut scale_modifier_cursor);}
                    app.dispatch_tool_move(&tool_ctx);
                }
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
    if scale_input_admitted && app.paste_preview.is_none()
        && matches!(app.active_tool_type,ToolType::FollowMe|ToolType::Freehand|ToolType::Scale)
        && pointer.button_released(egui::PointerButton::Primary)
    {
        if app.active_tool_type==ToolType::Scale{let index=ui.input(|i|i.events.iter().position(|e|matches!(e,egui::Event::PointerButton{button:egui::PointerButton::Primary,pressed:false,..})).unwrap_or(usize::MAX));scale_modifier_changes(app,scale_modifiers.as_ref(),index,&mut scale_modifier_cursor);}
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

    scale_modifier_changes(app,scale_modifiers.as_ref(),usize::MAX,&mut scale_modifier_cursor);

    // Handle Esc to cancel active tool and clear locks
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.snap_engine.reset_operation_state();
        if app.cancel_blender_pick() {
        } else if app.paste_preview.is_some() {
            app.cancel_paste_preview();
        } else {
            app.dispatch_tool_cancel();
        }
        ui.ctx().data_mut(|data| {
            data.remove::<egui::Pos2>(response.id.with("followme_press"));
            data.remove::<ToolContext>(response.id.with("followme_context"));
        });
    }

    } // ordinary geometry tool input
    } // modal input ownership

    // 3. Render 3D Scene
    // Decode and upload embedded textures into egui texture manager
    for (name, bytes) in app.scene.embedded_textures.iter().chain(app.paste_preview.iter().flat_map(|preview|preview.scene.embedded_textures.iter())) {
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

    app.native_gpu_viewport_rect=Some(rect);
    #[cfg(target_os="macos")]
    let metal_route_reason={app.sync_document_presentation();crate::native_metal::world_viewport::route(app,rect,ui.ctx().pixels_per_point())};

    if app.native_gpu_viewport_enabled {
        if let Some(texture)=app.native_gpu_viewport_texture {
            painter.image(texture,rect,egui::Rect::from_min_max(egui::pos2(0.0,0.0),egui::pos2(1.0,1.0)),egui::Color32::WHITE);
        } else {painter.rect_filled(rect,0.0,egui::Color32::from_rgb(210,214,212));ui.ctx().request_repaint();}
    } else {
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

    }
    #[cfg(target_os="macos")]
    if let Some(reason)=metal_route_reason.as_ref(){painter.text(rect.left_top()+egui::vec2(8.,8.),egui::Align2::LEFT_TOP,reason,egui::FontId::proportional(12.),egui::Color32::from_rgb(160,40,20));}

    if app.native_gpu_viewport_enabled && app.navigation.mode==Some(crate::navigation::Mode::Position){
        if let Some(glyph)=active_snap_glyph.as_ref(){ingetrazo_render::pipeline::render_snap_glyphs(std::slice::from_ref(glyph),&app.camera,&painter,rect);}
    }

    #[cfg(target_os="macos")]
    if app.native_metal_world.is_some()&&app.native_gpu_viewport_enabled&&app.navigation.mode!=Some(crate::navigation::Mode::Position){
        if let Some(glyph)=active_snap_glyph.as_ref(){ingetrazo_render::pipeline::render_snap_glyphs(std::slice::from_ref(glyph),&app.camera,&painter,rect);}
    }

    app.draw_blender_lights(&painter,rect);

    // Original QPainter image borders are independent overlay strokes, drawn
    // above the native world texture. Cached native CAD callbacks preserve order.
    if app.native_gpu_viewport_enabled {
        if let Err(error)=app.image_outlines.paint(&app.scene,&app.camera,&painter,rect,app.native_composer.factory.as_ref()){
            app.status_message=format!("Image outline: {error}");
        }
    }

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

    // Source center approach marker is a UI overlay, shared by native Vulkan
    // and the software viewport. Native world rendering has no snap glyph pass.
    if let Some(pixel)=active_center_hint {
        let pos=rect.min+egui::vec2(pixel.x as f32,pixel.y as f32);
        let rgb=ingetrazo_core::snap::COLOR_ENDPOINT;
        let green=egui::Color32::from_rgb((rgb[0]*255.) as u8,(rgb[1]*255.) as u8,(rgb[2]*255.) as u8);
        painter.circle_filled(pos,4.,green);
        painter.circle_stroke(pos,4.,egui::Stroke::new(3.,egui::Color32::from_white_alpha(230)));
        painter.circle_stroke(pos,4.,egui::Stroke::new(1.5,green));
    }

    crate::navigation::paint_overlay(&app.navigation,&painter,rect);

    // 4. Render Active Tool Rubber Band (if any)
    for ((p0,p1),stroke) in app.active_tool.rubber_band_lines().into_iter().map(|line|(line,egui::Stroke::new(2.0,egui::Color32::from_rgb(255,60,60)))).chain(app.navigation.preview_line().map(|line|(line,navigation_preview_stroke))) {
        let view_proj = app.camera.projection_matrix() * app.camera.view_matrix();
        if let (Some((s0, _)), Some((s1, _))) = (
            project_point(p0, &view_proj, rect),
            project_point(p1, &view_proj, rect),
        ) {
            painter.line_segment(
                [s0, s1],
                stroke,
            );
        }
    }

    if app.native_gpu_viewport_enabled {
        if let Some(egui::Shape::Callback(callback))=&app.native_gpu_annotation_shape{
            let mut callback=callback.clone();callback.rect=rect;painter.add(egui::Shape::Callback(callback));
        }
    }

    if let Some(state)=app.active_tool.scale_box_state() {
        let viewport=ingetrazo_core::screen_pick::PixelViewport::new(&app.camera,rect.width() as f64,rect.height() as f64);
        let project=|p|viewport.project_for_snapping(p).map(|p|rect.min+egui::vec2(p.x as f32,p.y as f32));
        for (a,b) in state.segments {if let Some((a,b))=crate::scale_overlay::clip_segment_front(viewport.matrix,a,b){if let (Some(a),Some(b))=(project(a),project(b)){
            painter.line_segment([a,b],egui::Stroke::new(1.,egui::Color32::from_rgb(214,174,0)));
        }}}
        for (p,active) in state.grips {if let Some(p)=project(p){
            let color=if active{egui::Color32::from_rgb(220,40,40)}else{egui::Color32::from_rgb(60,160,60)};
            let square=egui::Rect::from_center_size(p,egui::Vec2::splat(if active{10.}else{8.}));
            painter.rect_filled(square,0.,color);
            painter.rect_stroke(square,0.,egui::Stroke::new(1.,egui::Color32::from_rgb(30,30,30)));
        }}
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

    if app.navigation.mode!=Some(crate::navigation::Mode::FirstPerson){response.context_menu(|ui|{if app.selected_text3d().is_some() && ui.button("Edit 3D Text…").clicked(){app.open_edit_text3d();ui.close_menu();}if app.scene.selection.iter().any(|item|matches!(item,ingetrazo_core::scene::SelectionItem::SectionPlane(_))){crate::section_actions::render_section_controls(app,ui);}});}

    // 5. Render Section Planes (cutting frames and symbol tags)
    if app.scene.show_section_planes {
        let view_proj = app.camera.projection_matrix() * app.camera.view_matrix();
        for (index,sp) in app.scene.section_planes.iter().enumerate() {
            let Some(corners)=app.scene.section_frame(index)else{continue;};
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
                if let Some((tag_sc, _)) = project_point((corners[2]+corners[3])*0.5+(corners[2]-corners[1]).normalize_or_zero()*0.3, &view_proj, rect)
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

pub fn cursor_work_plane(app:&IngeTrazoApp,ray:&ingetrazo_core::math::Ray)->(DVec3,DVec3){
 cursor_work_plane_with_center(app,ray,None)
}
pub(crate) fn cursor_work_plane_with_center(app:&IngeTrazoApp,ray:&ingetrazo_core::math::Ray,cached_center:Option<DVec3>)->(DVec3,DVec3){
    if let Some(paste)=&app.paste_preview{return (paste.reference,DVec3::Z);}
    let forward=app.camera.forward();let near_horizon=forward.z.abs()<15_f64.to_radians().sin();let vertical=if forward.x.abs()>=forward.y.abs(){DVec3::X}else{DVec3::Y};
    if let Some((point,normal))=app.active_tool.drawing_plane(){if app.active_tool_type==ToolType::Line&&normal.z.abs()>0.94&&near_horizon{return(point,vertical);}return(point,normal);}
    if let Some(point)=app.active_tool.snap_anchor(){
        if app.active_tool.prefers_vertical_drag(){
            // Original viewport._start_point_plane: Move contains world Z and
            // faces the camera heading, except within 15 degrees of top view.
            let heading=DVec3::new(forward.x,forward.y,0.);
            let normal=if heading.length()>=15_f64.to_radians().sin(){heading.normalize()}else{DVec3::Z};
            let base=(point,normal);
            // Original _current_work_plane/_hover_face_plane: an unlocked
            // second point may land on another, unselected face. Axis-aligned
            // movement keeps its grab plane; selected faces never offer it.
            if app.snap_engine.locked_axis.is_none(){
                if let Some(hit)=ingetrazo_core::screen_pick::hover_face_placement(&app.scene,ray){
                    let selected=if hit.group_path.is_empty(){
                        app.scene.selection.contains(&ingetrazo_core::scene::SelectionItem::Face(hit.face))
                    }else{
                        app.scene.selection.iter().any(|item|if let ingetrazo_core::scene::SelectionItem::Group(id)=item{
                            app.scene.groups.get(*id).is_some_and(|g|hit.group_path.first()==Some(&g.id))
                        }else{false})
                    };
                    let alignment=Plane::new(point,normal).intersect_ray(ray).filter(|t|*t>=0.).map(|t|{
                        let delta=ray.at(t)-point;let length=delta.length();
                        length>=1e-6 && delta.abs().max_element()/length>=app.active_tool.magnetic_axis_deg().unwrap_or(3.).to_radians().cos()
                    }).unwrap_or(false);
                    if !selected&&!alignment{return(hit.point,hit.normal);}
                }
            }
            return base;
        }
        return(point,if near_horizon{vertical}else{DVec3::Z});
    }
    if let Some((point,normal,_))=ingetrazo_core::screen_pick::hover_face(&app.scene,ray){return(point,normal);}
    if let Some((id,_))=app.scene.image_plane_hit(ray,ingetrazo_core::image_plane::ImagePickMode::Inference){let image=&app.scene.image_planes[id];return(image.center(),image.normal());}
    if forward.abs().max_element()>=0.97{
        let center=cached_center.unwrap_or_else(||crate::navigation::first_point_bounds_center(&app.scene));if let Some(plane)=ingetrazo_core::snap::first_point_work_plane(forward,center){return plane;}
    }
    (DVec3::ZERO,DVec3::Z)
}

#[cfg(test)]mod drafting_plane_tests{
 use super::*;use ingetrazo_core::{math::Ray,scene::Scene};
 #[test]
 fn locked_reference_still_offers_tilted_drawing_plane() {
  let mut app=IngeTrazoApp::new();app.system_clipboard=None;
  let mut p=ingetrazo_core::image_plane::ImagePlane::new("scan",DVec3::ZERO,DVec3::X*2.,DVec3::Z*2.);p.locked=true;app.scene.image_planes.push(p);
  let ray=Ray::new(DVec3::new(1.,-2.,1.),DVec3::Y);
  assert_eq!(cursor_work_plane(&app,&ray),(DVec3::new(1.,0.,1.),-DVec3::Y));
 }
 #[test]fn standard_view_uses_model_center_oblique_uses_ground_and_line_can_rise(){
  let mut app=IngeTrazoApp::new();app.scene=Scene::new();let a=app.scene.mesh.add_vertex(DVec3::new(10.,20.,30.));let b=app.scene.mesh.add_vertex(DVec3::new(14.,24.,34.));app.scene.mesh.add_edge(a,b);app.camera.pitch=0.;app.camera.yaw=0.;let ray=Ray::new(DVec3::new(-5.,-5.,-5.),DVec3::X);let (point,normal)=cursor_work_plane(&app,&ray);assert_eq!(point,DVec3::new(12.,22.,32.));assert_eq!(normal,-DVec3::X);
  app.camera.pitch=30_f64.to_radians();app.camera.yaw=45_f64.to_radians();assert_eq!(cursor_work_plane(&app,&ray),(DVec3::ZERO,DVec3::Z));app.set_tool(ToolType::Line);let ctx=ToolContext::new(DVec3::new(1.,2.,3.),ray);app.dispatch_tool_click(&ctx);app.camera.pitch=0.;app.camera.yaw=0.;assert_eq!(cursor_work_plane(&app,&ray),(ctx.world,DVec3::X));
 }
}

#[cfg(test)]mod move_plane_tests{
 use super::*;use ingetrazo_core::{math::Ray,scene::{Scene,SelectionItem}};
 #[test]fn move_second_point_yields_to_unselected_face_but_axis_and_selection_keep_grab_plane(){
  let mut app=IngeTrazoApp::new();app.scene=Scene::new();
  let point=DVec3::new(0.,0.,2.);let v=app.scene.mesh.add_vertex(point);
  let ring=[DVec3::new(-20.,-20.,0.),DVec3::new(20.,-20.,0.),DVec3::new(20.,20.,0.),DVec3::new(-20.,20.,0.)].map(|p|app.scene.mesh.add_vertex(p));
  let face=app.scene.mesh.add_face(ring.to_vec(),vec![]).unwrap();
  app.scene.select(SelectionItem::Vertex(v));app.set_tool(ToolType::Move);app.camera.pitch=30_f64.to_radians();app.camera.yaw=0.;
  app.dispatch_tool_click(&ToolContext::new(point,Ray::new(DVec3::new(0.,0.,10.),-DVec3::Z)));
  let ray=Ray::new(DVec3::new(10.,10.,10.),DVec3::new(-6.,-1.,-10.).normalize());
  let (origin,normal)=cursor_work_plane(&app,&ray);assert!(origin.z.abs()<1e-10);assert_eq!(normal,DVec3::Z);
  let magnetic_ray=Ray::new(DVec3::new(10.,10.,10.),DVec3::new(-6.,-5.,-10.).normalize());
  assert_eq!(cursor_work_plane(&app,&magnetic_ray),(point,-DVec3::X));
  app.snap_engine.locked_axis=Some('x');assert_eq!(cursor_work_plane(&app,&ray),(point,-DVec3::X));app.snap_engine.locked_axis=None;
  let axis_ray=Ray::new(DVec3::new(10.,0.,10.),DVec3::new(-1.,0.,-1.).normalize());assert_eq!(cursor_work_plane(&app,&axis_ray),(point,-DVec3::X));
  app.scene.select(SelectionItem::Face(face));assert_eq!(cursor_work_plane(&app,&ray),(point,-DVec3::X));
 }
 #[test]fn move_grab_uses_camera_heading_plane_and_top_view_fallback(){
  let mut app=IngeTrazoApp::new();app.scene=Scene::new();
  let point=DVec3::new(3.,4.,7.);let v=app.scene.mesh.add_vertex(point);
  app.scene.select(SelectionItem::Vertex(v));app.set_tool(ToolType::Move);
  let ray=Ray::new(DVec3::new(3.,4.,20.),-DVec3::Z);
  app.dispatch_tool_click(&ToolContext::new(point,ray));
  assert_eq!(app.active_tool.snap_anchor(),Some(point));
  for yaw in [0_f64,37_f64,90_f64]{
   app.camera.pitch=30_f64.to_radians();app.camera.yaw=yaw.to_radians();
   let forward=app.camera.forward();let expected=DVec3::new(forward.x,forward.y,0.).normalize();
   let (origin,normal)=cursor_work_plane(&app,&ray);assert_eq!(origin,point);assert!((normal-expected).length()<1e-12);assert_eq!(normal.z,0.);
  }
  app.camera.pitch=85_f64.to_radians();assert_eq!(cursor_work_plane(&app,&ray),(point,DVec3::Z));
  app.active_tool.on_cancel(&mut app.scene);assert_eq!(app.active_tool.snap_anchor(),None);
 }
}

#[cfg(test)]
mod native_overlay_dispatch_tests {
    use super::*;
    #[test]
    fn production_viewport_submits_one_image_outline_per_frame() {
        let mut app=IngeTrazoApp::new();
        app.native_gpu_viewport_enabled=true;
        app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
        app.camera.perspective=false;
        app.camera.target=DVec3::ZERO;
        app.camera.distance=5.;
        app.scene.image_planes.push(ingetrazo_core::image_plane::ImagePlane::new("missing-image",DVec3::new(-0.5,-0.5,0.),DVec3::X,DVec3::Y));
        let factory=ingetrazo_render::cad_callback::Factory::default();
        app.native_composer.factory=Some(crate::composer_backend::Factory::MetalProducer(factory.clone()));
        let ctx=egui::Context::default();
        for _ in 0..2 {
            let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.))),..Default::default()},|ctx|{
                egui::CentralPanel::default().show(ctx,|ui|render_viewport(&mut app,ui));
            });
            let callbacks=output.shapes.iter().filter(|s|matches!(&s.shape,egui::Shape::Callback(c) if c.callback.is::<ingetrazo_render::cad_callback::Callback>())).count();
            assert_eq!(callbacks,1,"actual viewport must enqueue each outline once");
        }
        assert_eq!(factory.metrics().handles,1);
        assert_eq!(factory.metrics().unchanged,1);
        assert_eq!(app.image_outlines.cold_builds,1);
    }
}

// Modifier keys have no egui Key variant; the native ingress retains their
// transitions. Consuming by raw-event index prevents a later tap from changing
// the acquisition/commit that occurred earlier in this same frame.
fn scale_modifier_changes(app:&mut IngeTrazoApp,handle:Option<&crate::pointer_modifiers::Handle>,index:usize,cursor:&mut usize){
 let Some(handle)=handle else{return;};
 match handle.lock(){
  Ok(timeline)=>if let Err(error)=timeline.visit_changes_before(index,cursor,|m|{app.dispatch_tool_control_state(m.ctrl);app.active_tool.note_shift_state(m.shift);}){app.status_message=error.into();},
  Err(_)=>app.status_message="Native modifier timeline unavailable".into(),
 }
}
