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

                if let Some(SheetItem::ViewFrame(frame))=sheet.selected_item.and_then(|i|sheet.items.get_mut(i)) {
                    ui.menu_button(tr("Ink"),|ui|{
                        bump_needed |= ui.checkbox(&mut frame.profiles,tr("Profiles")).changed();
                        bump_needed |= ui.checkbox(&mut frame.hidden_lines,tr("Hidden lines")).changed();
                        for(label,value)in [("Cut",&mut frame.pen_cut_mm),("Profile",&mut frame.pen_profile_mm),("Edge",&mut frame.pen_edge_mm)] {
                            ui.horizontal(|ui|{ui.label(tr(label));bump_needed |=ui.add(egui::DragValue::new(value).speed(0.01).range(0.05..=5.).suffix(" mm")).changed();});
                        }
                    });
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

fn frame_scene<'a>(scene:&'a Scene,frame:&ViewFrame)->std::borrow::Cow<'a,Scene>{if let Some(name)=frame.view_key.strip_prefix("scene:"){if let Some(view)=scene.saved_views.iter().find(|view|view.name==name){let mut snapshot=scene.clone();let mut camera=scene.camera.clone();view.apply_scene(&mut snapshot,&mut camera);return std::borrow::Cow::Owned(snapshot);}}std::borrow::Cow::Borrowed(scene)}
fn section_projected_edges(scene:&Scene)->Vec<(DVec3,DVec3)>{fn mesh(mesh:&ingetrazo_core::mesh::Mesh,transform:glam::DMat4,scene:&Scene,edges:&mut Vec<(DVec3,DVec3)>){for (id,edge) in mesh.edges.iter().enumerate(){if !scene.mesh_edge_visible(mesh,id){continue;}let a=transform.transform_point3(mesh.vertices[edge.v0].position);let b=transform.transform_point3(mesh.vertices[edge.v1].position);if let Some(segment)=scene.section_segment(a,b){edges.push(segment);}}
        if let Some(plane)=scene.active_section(){for id in 0..mesh.faces.len(){if !scene.mesh_face_visible(mesh,id){continue;}for(a,b,c)in mesh.triangulate_face(id){let points=[a,b,c].map(|p|transform.transform_point3(p));let d=points.map(|p|plane.signed_distance(p));if d.iter().all(|d|d.abs()<1e-7){continue;}let mut hits=vec![];for i in 0..3{let j=(i+1)%3;if d[i].abs()<1e-7{hits.push(points[i]);}if d[i]*d[j]< -1e-14{hits.push(points[i]+(points[j]-points[i])*(d[i]/(d[i]-d[j])));}}let mut unique=vec![];for p in hits{if !unique.iter().any(|other:&DVec3|(*other-p).length()<1e-7){unique.push(p);}}if unique.len()==2{edges.push((unique[0],unique[1]));}}}}
    }fn groups(groups:&[ingetrazo_core::groups::Group],parent:glam::DMat4,scene:&Scene,edges:&mut Vec<(DVec3,DVec3)>){for group in groups{if group.hidden||!scene.is_layer_visible(group.layer.as_deref()){continue;}let transform=parent*group.transform;mesh(&group.mesh,transform,scene,edges);children(&group.children,transform,scene,edges);}}fn children(children:&[ingetrazo_core::groups::Group],parent:glam::DMat4,scene:&Scene,edges:&mut Vec<(DVec3,DVec3)>){groups(children,parent,scene,edges)}let mut edges=vec![];mesh(&scene.mesh,glam::DMat4::IDENTITY,scene,&mut edges);groups(&scene.groups,glam::DMat4::IDENTITY,scene,&mut edges);edges}

/// Projects and renders the 3D model inside the ViewFrame at exact scale 1:N.
fn frame_bounds(scene:&Scene)->Option<(DVec3,DVec3)>{
 let mut points=vec![];for edge in &scene.mesh.edges{if scene.is_layer_visible(edge.layer.as_deref()){points.extend([scene.mesh.vertices[edge.v0].position,scene.mesh.vertices[edge.v1].position]);}}for(id,face)in scene.mesh.faces.iter().enumerate(){if scene.mesh_face_visible(&scene.mesh,id){points.extend(face.vertices.iter().map(|&i|scene.mesh.vertices[i].position));}}
 fn groups(items:&[ingetrazo_core::groups::Group],parent:glam::DMat4,scene:&Scene,points:&mut Vec<DVec3>){for group in items{if group.hidden||!scene.is_layer_visible(group.layer.as_deref()){continue;}let transform=parent*group.transform;points.extend(group.mesh.vertices.iter().map(|v|transform.transform_point3(v.position)));groups(&group.children,transform,scene,points);}}
 groups(&scene.groups,glam::DMat4::IDENTITY,scene,&mut points);if points.is_empty(){None}else{Some(points.into_iter().fold((DVec3::splat(f64::INFINITY),DVec3::splat(f64::NEG_INFINITY)),|(a,b),p|(a.min(p),b.max(p))))}
}
/// Resolve a frame without changing the live document or visible camera.
pub fn camera_for_frame(scene:&Scene,frame:&ViewFrame)->ingetrazo_core::camera::OrbitCamera{let snapshot=frame_scene(scene,frame);frame_camera(&snapshot,frame)}
fn frame_camera(scene:&Scene,frame:&ViewFrame)->ingetrazo_core::camera::OrbitCamera{
 use ingetrazo_core::camera::StandardView;
 let mut camera=scene.camera.clone();let bound=frame.view_key.starts_with("scene:")&&scene.saved_views.iter().any(|v|Some(v.name.as_str())==frame.view_key.strip_prefix("scene:"));
 if frame.view_key.starts_with("std:"){camera.set_standard_view(match frame.view_key.as_str(){"std:front"=>StandardView::Front,"std:right"=>StandardView::Right,"std:back"=>StandardView::Back,"std:left"=>StandardView::Left,"std:iso"=>StandardView::Iso,"std:bottom"=>StandardView::Bottom,_=>StandardView::Top});if matches!(frame.view_key.as_str(),"std:top"|"std:bottom"){camera.pitch=if frame.view_key=="std:top"{std::f64::consts::FRAC_PI_2}else{-std::f64::consts::FRAC_PI_2};camera.up=DVec3::Y;}if let Some((lo,hi))=frame_bounds(scene){camera.target=(lo+hi)/2.;}}
 if let Some(yaw)=frame.cam_yaw{camera.yaw=yaw;}if let Some(pitch)=frame.cam_pitch{camera.pitch=pitch;if pitch.abs()<89.5_f64.to_radians(){camera.up=DVec3::Z;}}if let Some(target)=frame.cam_target{camera.target=DVec3::from_array(target);}
 let right=camera.right();let up=right.cross(camera.forward());let angle=frame.rot_deg.to_radians();camera.up=up*angle.cos()-right*angle.sin();camera.aspect=(frame.rect.w/25.4*ingetrazo_core::composition::RENDER_DPI).round_ties_even().max(1.)/(frame.rect.h/25.4*ingetrazo_core::composition::RENDER_DPI).round_ties_even().max(1.);
 camera.perspective=frame.perspective;
 if frame.perspective{if let Some(fov)=frame.cam_fov.or_else(||bound.then_some(camera.fov_deg)){camera.fov_deg=fov.clamp(5.,120.);}camera.distance=frame.cam_distance.or_else(||bound.then_some(camera.distance)).unwrap_or_else(||frame_bounds(scene).map(|(lo,hi)|(hi-lo).length()/2./(camera.fov_deg.to_radians()/2.).tan().max(1e-6)*1.15).filter(|d|*d>0.).unwrap_or(30.)).max(0.001);}else{camera.distance=ingetrazo_core::composition::ortho_distance_for_height(model_height_for_frame(frame.rect.h,frame.scale_n),camera.fov_deg);}
 camera
}
fn render_model_projection(
    scene: &Scene,
    painter: &egui::Painter,
    frame_rect: Rect,
    frame: &ViewFrame,
) {
    let painter = painter.with_clip_rect(frame_rect);
    let snapshot=frame_scene(scene,frame);let scene=snapshot.as_ref();

    let camera=frame_camera(scene,frame);
    let view_mat=camera.view_matrix();

    // Sacred scale equation: model_height_m = frame.rect.h * scale_n / 1000
    let model_h_m = model_height_for_frame(frame.rect.h, frame.scale_n);
    let model_w_m = model_h_m * (frame.rect.w / frame.rect.h);

    let proj_mat = if camera.perspective{camera.projection_matrix()}else{glam::DMat4::orthographic_rh(
        -model_w_m * 0.5,
        model_w_m * 0.5,
        -model_h_m * 0.5,
        model_h_m * 0.5,
        -camera.zfar,
        camera.zfar,
    )};
    let view_proj = proj_mat * view_mat;

    use ingetrazo_render::vector_hlr::LineKind;
    let vector=matches!(frame.style.as_str(),"technical"|"vectorial")&&!frame.perspective;
    let mut segments=if vector {
        ingetrazo_render::vector_hlr::parallel_segments(scene,&camera,frame.profiles,frame.hidden_lines)
            .into_iter().map(|s|(s.a,s.b,s.kind)).collect::<Vec<_>>()
    } else {section_projected_edges(scene).into_iter().map(|(a,b)|(a,b,LineKind::Edge)).collect()};
    // Thin hidden ink first, heavy cut ink last, as original _paint_hlr_lines_mm.
    segments.sort_by_key(|s|match s.2{LineKind::Hidden=>0,LineKind::Edge=>1,LineKind::Profile=>2,LineKind::Cut=>3});
    let paper_pixel=frame_rect.width()/frame.rect.w as f32;
    for(p0,p1,kind)in segments {
        if let(Some((s0,_)),Some((s1,_)))=(project_point(p0,&view_proj,frame_rect),project_point(p1,&view_proj,frame_rect)) {
            let pen=match kind {LineKind::Cut=>frame.pen_cut_mm,LineKind::Profile=>frame.pen_profile_mm,_=>frame.pen_edge_mm};
            let default=match kind {LineKind::Cut=>0.5,LineKind::Profile=>0.35,_=>0.18};
            let pen=if pen==0. {default}else{pen};
            let stroke=Stroke::new((pen.max(0.05) as f32)*paper_pixel,Color32::from_rgb(30,36,44));
            if kind==LineKind::Hidden {
                // Original dashed ink is 1.5 mm dash / 0.8 mm gap in paper units.
                let length=s0.distance(s1); let dash=1.5*paper_pixel;let period=2.3*paper_pixel;
                if period>0. {let direction=(s1-s0)/length.max(1e-6);let mut t=0.;while t<length {
                    painter.line_segment([s0+direction*t,s0+direction*(t+dash).min(length)],stroke);t+=period;
                }}
            }else{painter.line_segment([s0,s1],stroke);
                // QPainter solid ink has RoundCap; hidden strokes stay flat.
                painter.circle_filled(s0,stroke.width/2.,stroke.color);painter.circle_filled(s1,stroke.width/2.,stroke.color);
            }
        }
    }

    // Draw traced GeoPaths if present
    for gp in &scene.geo_paths {
        for (p0, p1) in gp.segments() {
            let Some((p0,p1))=scene.section_segment(p0,p1)else{continue;};
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

#[cfg(test)]mod section_frame_tests{use super::*;use ingetrazo_core::{groups::Group,mesh::Mesh,saved_views::SavedView,section::SectionPlane};#[test]fn bound_saved_scene_cut_clips_nested_edges_without_changing_live_document(){let mut mesh=Mesh::new();let a=mesh.add_vertex(DVec3::new(-2.,0.,0.));let b=mesh.add_vertex(DVec3::new(2.,0.,0.));mesh.add_edge(a,b);let mut child=Group::with_mesh("child",mesh);child.transform=glam::DMat4::from_translation(DVec3::X);let mut parent=Group::new("parent");parent.children.push(child);let mut scene=Scene::new();scene.groups.push(parent);scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::X,"Cut","A"));scene.show_section_cuts=true;let view=SavedView::capture_scene("Section",&scene,&scene.camera);scene.saved_views.push(view);scene.section_planes[0].active=false;let original=scene.clone();let mut frame=ViewFrame::default();frame.view_key="scene:Section".into();let snapshot=frame_scene(&scene,&frame);let edges=section_projected_edges(&snapshot);assert_eq!(edges.len(),1);assert_eq!(edges[0],(DVec3::new(-1.,0.,0.),DVec3::ZERO));assert_eq!(scene,original);assert_eq!(section_projected_edges(&scene)[0].1,DVec3::new(3.,0.,0.));}}

#[cfg(test)]mod frame_camera_tests{
 use super::*;
 #[test]fn hidden_face_boundaries_and_contours_are_absent_from_sheet(){
  let mut scene=Scene::new();let ids=[DVec3::new(-1.,0.,0.),DVec3::new(1.,0.,0.),DVec3::new(-1.,1.,0.)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);scene.section_planes.push(ingetrazo_core::section::SectionPlane::new(DVec3::ZERO,DVec3::X,"Cut","A"));scene.show_section_cuts=true;assert!(!section_projected_edges(&scene).is_empty());scene.mesh.faces[0].attrs.insert("hidden".into(),serde_json::json!(true));assert!(section_projected_edges(&scene).is_empty());
 }
 #[test]fn frame_pixel_aspect_saved_lens_clamp_and_two_point_follow_original(){
  let mut scene=Scene::new();scene.camera.fov_deg=200.;scene.camera.two_point=true;scene.saved_views.push(ingetrazo_core::saved_views::SavedView::capture_scene("Lens",&scene,&scene.camera));let mut frame=ViewFrame::default();frame.view_key="scene:Lens".into();frame.rect.w=111.11;frame.rect.h=77.77;frame.perspective=true;let camera=camera_for_frame(&scene,&frame);assert_eq!(camera.fov_deg,120.);assert!(camera.two_point);assert!((camera.aspect-1312./919.).abs()<1e-12);
 }
 #[test]fn frame_bounds_matches_original_edge_and_group_vertex_semantics(){
  let mut scene=Scene::new();let a=scene.mesh.add_vertex(DVec3::new(10.,0.,0.));let b=scene.mesh.add_vertex(DVec3::new(12.,0.,0.));let e=scene.mesh.add_edge(a,b).unwrap();scene.mesh.edges[e].hidden=true;assert_eq!(frame_bounds(&scene),Some((DVec3::new(10.,0.,0.),DVec3::new(12.,0.,0.))));let mut group=ingetrazo_core::groups::Group::new("Vertices");group.mesh.add_vertex(DVec3::new(30.,0.,0.));scene.groups.push(group);assert_eq!(frame_bounds(&scene).unwrap().1.x,30.);scene.groups[0].hidden=true;assert_eq!(frame_bounds(&scene).unwrap().1.x,12.);
 }
 #[test]fn frame_overrides_roll_scale_and_perspective_have_original_camera_semantics(){
  let mut scene=Scene::new();let ids=[DVec3::new(10.,20.,0.),DVec3::new(12.,20.,0.),DVec3::new(10.,24.,2.)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);let mut frame=ViewFrame::default();frame.scale_n=20.;frame.rot_deg=90.;let camera=camera_for_frame(&scene,&frame);assert_eq!(camera.target,DVec3::new(11.,22.,1.));assert!((camera.pitch-std::f64::consts::FRAC_PI_2).abs()<1e-12);assert!((camera.up+DVec3::X).length()<1e-12);assert!((camera.distance-ingetrazo_core::composition::ortho_distance_for_height(3.,45.)).abs()<1e-12);
  frame.perspective=true;frame.cam_yaw=Some(-1.2);frame.cam_pitch=Some(0.3);frame.cam_target=Some([4.,5.,6.]);frame.cam_distance=Some(7.);frame.cam_fov=Some(200.);let camera=camera_for_frame(&scene,&frame);assert_eq!(camera.target,DVec3::new(4.,5.,6.));assert_eq!(camera.distance,7.);assert_eq!(camera.fov_deg,120.);assert!(camera.perspective);frame.cam_fov=Some(60.);let camera=camera_for_frame(&scene,&frame);let matrix=camera.projection_matrix()*camera.view_matrix();let near=camera.eye()+camera.forward()*2.+camera.right();let far=camera.eye()+camera.forward()*4.+camera.right();let a=matrix*near.extend(1.);let b=matrix*far.extend(1.);assert!((a.x/a.w-2.*b.x/b.w).abs()<1e-12,"perspective must foreshorten far geometry");
 }
 #[test]fn saved_section_view_uses_original_camera_direction_without_mutating_live_state(){
  let mut scene=Scene::new();scene.camera.yaw=-1.1;scene.camera.pitch=0.42;scene.camera.target=DVec3::new(4.,5.,6.);scene.saved_views.push(ingetrazo_core::saved_views::SavedView::capture_scene("Cut",&scene,&scene.camera));let captured=scene.camera.clone();scene.camera.yaw=0.3;let original=scene.clone();let mut frame=ViewFrame::default();frame.view_key="scene:Cut".into();let snapshot=frame_scene(&scene,&frame);let camera=frame_camera(&snapshot,&frame);assert_eq!(camera.target,captured.target);assert!((camera.forward()-captured.forward()).length()<1e-12);assert!(!camera.perspective);assert_eq!(scene,original);
  frame.view_key="std:front".into();assert!(frame_camera(&scene,&frame).eye().y<0.);frame.view_key="std:right".into();assert!(frame_camera(&scene,&frame).eye().x>0.);
 }
}
