// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Toolbars matching IngeTrazo CAD conventions with 1:1 visual icon fidelity:
//! 1. Top horizontal toolbar: Save, Undo/Redo, Eraser/Paint/Eyedropper, PushPull/Move/Rotate/Scale/Flip/FollowMe/Offset/Fillet,
//!    Orbit/Pan/Zoom/ZoomWindow/Extents/Selection, Sections toggles, Standard Views (Iso, Top, Front, Right, Left, Back, Bottom), Styles, Shadows.
//! 2. Left vertical CAD palette: Select, Line, Freehand, Rectangle, RotatedRect, Circle, Polygon, 2-Pt Arc, 3-Pt Arc, Center Arc, Pie,
//!    Tape, Protractor, Dimension, Text, 3D Text, Geopath, Section Plane, Position Camera, Walk, Look Around, Solid Tools.

use ingetrazo_core::camera::StandardView;
use ingetrazo_core::i18n::tr;
use ingetrazo_render::style::VisualStyle;
use ingetrazo_tools::base::ToolType;

use crate::app::IngeTrazoApp;
use crate::icons::render_icon_button;

pub fn render_top_toolbar(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);

        // --- File & History ---
        let ic_save = app.icons.get(ui.ctx(), "save").cloned();
        if render_icon_button(ui, ic_save.as_ref(), "S", false, &format!("{} (Ctrl+S)", tr("Save"))).clicked() {
            app.save_document();
        }

        ui.separator();

        let ic_undo = app.icons.get(ui.ctx(), "undo").cloned();
        if render_icon_button(ui, ic_undo.as_ref(), "U", false, &format!("{} (Ctrl+Z)", tr("Undo"))).clicked() {
            app.undo();
        }
        let ic_redo = app.icons.get(ui.ctx(), "redo").cloned();
        if render_icon_button(ui, ic_redo.as_ref(), "R", false, &format!("{} (Ctrl+Y)", tr("Redo"))).clicked() {
            app.redo();
        }

        ui.separator();

        // --- Main Tools (Eraser, Paint, Eyedropper) ---
        let ic_eraser = app.icons.get(ui.ctx(), "eraser").cloned();
        let sel_eraser = app.active_tool_type == ToolType::Eraser;
        if render_icon_button(ui, ic_eraser.as_ref(), "E", sel_eraser, &format!("{} (E)", tr("Eraser"))).clicked() {
            app.set_tool(ToolType::Eraser);
        }
        let ic_paint = app.icons.get(ui.ctx(), "paint").cloned();
        let sel_paint = app.active_tool_type == ToolType::Paint;
        if render_icon_button(ui, ic_paint.as_ref(), "B", sel_paint, &format!("{} (B)", tr("Paint"))).clicked() {
            app.set_tool(ToolType::Paint);
        }
        let ic_eyedropper = app.icons.get(ui.ctx(), "eyedropper").cloned();
        if render_icon_button(ui, ic_eyedropper.as_ref(), "Alt", false, &tr("Sample material — click a face to pick up its paint or texture (or hold Alt with the Paint tool)")).clicked() {
            app.status_message = tr("Sample material: click a face to sample paint or texture");
        }

        ui.separator();

        // --- Modify Tools ---
        let ic_pushpull = app.icons.get(ui.ctx(), "pushpull").cloned();
        let sel_pp = app.active_tool_type == ToolType::PushPull;
        if render_icon_button(ui, ic_pushpull.as_ref(), "P", sel_pp, &format!("{} (P)", tr("Push/Pull"))).clicked() {
            app.set_tool(ToolType::PushPull);
        }

        let ic_move = app.icons.get(ui.ctx(), "move").cloned();
        let sel_move = app.active_tool_type == ToolType::Move;
        if render_icon_button(ui, ic_move.as_ref(), "M", sel_move, &format!("{} (M)", tr("Move"))).clicked() {
            app.set_tool(ToolType::Move);
        }

        let ic_rotate = app.icons.get(ui.ctx(), "rotate").cloned();
        let sel_rotate = app.active_tool_type == ToolType::Rotate;
        if render_icon_button(ui, ic_rotate.as_ref(), "Q", sel_rotate, &format!("{} (Q)", tr("Rotate"))).clicked() {
            app.set_tool(ToolType::Rotate);
        }

        let ic_scale = app.icons.get(ui.ctx(), "scale").cloned();
        let sel_scale = app.active_tool_type == ToolType::Scale;
        if render_icon_button(ui, ic_scale.as_ref(), "S", sel_scale, &format!("{} (S)", tr("Scale"))).clicked() {
            app.set_tool(ToolType::Scale);
        }

        let ic_flip = app.icons.get(ui.ctx(), "flip").cloned();
        if render_icon_button(ui, ic_flip.as_ref(), "F", false, &tr("Flip / Reverse Faces")).clicked() {
            app.reverse_faces();
        }

        let ic_followme = app.icons.get(ui.ctx(), "followme").cloned();
        let sel_followme = app.active_tool_type == ToolType::FollowMe;
        if render_icon_button(ui, ic_followme.as_ref(), "FM", sel_followme, &tr("Follow Me — extrude a face along a path")).clicked() {
            app.set_tool(ToolType::FollowMe);
        }

        let ic_offset = app.icons.get(ui.ctx(), "offset").cloned();
        let sel_offset = app.active_tool_type == ToolType::Offset;
        if render_icon_button(ui, ic_offset.as_ref(), "O", sel_offset, &format!("{} (F)", tr("Offset"))).clicked() {
            app.set_tool(ToolType::Offset);
        }

        let ic_fillet = app.icons.get(ui.ctx(), "fillet").cloned();
        let sel_fillet = app.active_tool_type == ToolType::Fillet;
        if render_icon_button(ui, ic_fillet.as_ref(), "Fi", sel_fillet, &tr("Fillet — round edges and corners")).clicked() {
            app.set_tool(ToolType::Fillet);
        }

        ui.separator();

        // --- Camera Navigation ---
        let ic_orbit = app.icons.get(ui.ctx(), "orbit").cloned();
        if render_icon_button(ui, ic_orbit.as_ref(), "O", false, &format!("{} (O)", tr("Orbit"))).clicked() {
            app.status_message = tr("Orbit (O) — drag with middle mouse or left click");
        }

        let ic_pan = app.icons.get(ui.ctx(), "pan").cloned();
        if render_icon_button(ui, ic_pan.as_ref(), "H", false, &format!("{} (H)", tr("Pan"))).clicked() {
            app.status_message = tr("Pan (H) — drag to slide the view");
        }

        let ic_zoom = app.icons.get(ui.ctx(), "zoom").cloned();
        if render_icon_button(ui, ic_zoom.as_ref(), "Z", false, &format!("{} (Z)", tr("Zoom"))).clicked() {
            app.status_message = tr("Zoom (Z) — scroll or drag up/down");
        }

        let ic_zoom_win = app.icons.get(ui.ctx(), "zoom_window").cloned();
        if render_icon_button(ui, ic_zoom_win.as_ref(), "ZW", false, &tr("Zoom Window — drag a box to zoom to region")).clicked() {
            app.status_message = tr("Zoom Window — drag a rectangle over the target area");
        }

        let ic_extents = app.icons.get(ui.ctx(), "zoom_extents").cloned();
        if render_icon_button(ui, ic_extents.as_ref(), "ZE", false, &format!("{} (Shift+Z / F2)", tr("Zoom Extents"))).clicked() {
            app.zoom_extents();
        }

        let ic_zoom_sel = app.icons.get(ui.ctx(), "zoom_selection").cloned();
        if render_icon_button(ui, ic_zoom_sel.as_ref(), "ZS", false, &format!("{} (Ctrl+Alt+Z)", tr("Zoom Selection"))).clicked() {
            app.zoom_selection();
        }

        ui.separator();

        // --- Section Toggles ---
        let ic_splanes = app.icons.get(ui.ctx(), "section_planes").cloned();
        if render_icon_button(ui, ic_splanes.as_ref(), "SP", app.scene.show_section_planes, &tr("Section Planes — toggle visibility of section plane cutting frames")).clicked() {
            app.scene.show_section_planes = !app.scene.show_section_planes;
            app.status_message = if app.scene.show_section_planes { tr("Section planes visible") } else { tr("Section planes hidden") };
        }

        let ic_scuts = app.icons.get(ui.ctx(), "section_cuts").cloned();
        if render_icon_button(ui, ic_scuts.as_ref(), "SC", app.scene.show_section_cuts, &tr("Section Cuts — cut model open at active section planes")).clicked() {
            app.scene.show_section_cuts = !app.scene.show_section_cuts;
            app.status_message = if app.scene.show_section_cuts { tr("Section cuts active") } else { tr("Section cuts inactive") };
        }

        let ic_sfill = app.icons.get(ui.ctx(), "section_fill").cloned();
        if render_icon_button(ui, ic_sfill.as_ref(), "SF", app.scene.show_section_fill, &tr("Section Fill — fill cut faces of a section with solid color")).clicked() {
            app.scene.show_section_fill = !app.scene.show_section_fill;
            app.status_message = if app.scene.show_section_fill { tr("Section fill active") } else { tr("Section fill inactive") };
        }

        ui.separator();

        // --- Standard Views ---
        let ic_iso = app.icons.get(ui.ctx(), "view_iso").cloned();
        if render_icon_button(ui, ic_iso.as_ref(), "Iso", false, &tr("Isometric View")).clicked() {
            app.camera.set_standard_view(StandardView::Iso);
        }

        let ic_top = app.icons.get(ui.ctx(), "view_top").cloned();
        if render_icon_button(ui, ic_top.as_ref(), "Top", false, &tr("Top View")).clicked() {
            app.camera.set_standard_view(StandardView::Top);
        }

        let ic_front = app.icons.get(ui.ctx(), "view_front").cloned();
        if render_icon_button(ui, ic_front.as_ref(), "Front", false, &tr("Front View")).clicked() {
            app.camera.set_standard_view(StandardView::Front);
        }

        let ic_right = app.icons.get(ui.ctx(), "view_right").cloned();
        if render_icon_button(ui, ic_right.as_ref(), "Right", false, &tr("Right View")).clicked() {
            app.camera.set_standard_view(StandardView::Right);
        }

        let ic_left = app.icons.get(ui.ctx(), "view_left").cloned();
        if render_icon_button(ui, ic_left.as_ref(), "Left", false, &tr("Left View")).clicked() {
            app.camera.set_standard_view(StandardView::Left);
        }

        let ic_back = app.icons.get(ui.ctx(), "view_back").cloned();
        if render_icon_button(ui, ic_back.as_ref(), "Back", false, &tr("Back View")).clicked() {
            app.camera.set_standard_view(StandardView::Back);
        }

        let ic_bottom = app.icons.get(ui.ctx(), "view_bottom").cloned();
        if render_icon_button(ui, ic_bottom.as_ref(), "Bottom", false, &tr("Bottom View")).clicked() {
            app.camera.set_standard_view(StandardView::Bottom);
        }

        ui.separator();

        // --- Styles & Shadows ---
        let ic_styles = app.icons.get(ui.ctx(), "styles").cloned();
        if render_icon_button(ui, ic_styles.as_ref(), "Style", false, &tr("Display Style — cycle Shaded, Wireframe, Hidden line, Monochrome, X-Ray")).clicked() {
            app.style.visual_style = match app.style.visual_style {
                VisualStyle::ShadedWithTextures => VisualStyle::HiddenLine,
                VisualStyle::HiddenLine => VisualStyle::Wireframe,
                VisualStyle::Wireframe => VisualStyle::Monochrome,
                VisualStyle::Monochrome => VisualStyle::XRay,
                VisualStyle::XRay => VisualStyle::ShadedWithTextures,
            };
            app.status_message = format!("{}: {:?}", tr("Display style"), app.style.visual_style);
        }

        let ic_shadows = app.icons.get(ui.ctx(), "shadows").cloned();
        if render_icon_button(ui, ic_shadows.as_ref(), "Sun", app.shadows_enabled, &tr("Shadows — toggle sun shadows")).clicked() {
            app.shadows_enabled = !app.shadows_enabled;
        }
    });
}

pub fn render_left_cad_palette(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 3.0);

        // --- 1. Pointer ---
        let ic_sel = app.icons.get(ui.ctx(), "select").cloned();
        let is_sel = app.active_tool_type == ToolType::Select;
        if render_icon_button(ui, ic_sel.as_ref(), "👆", is_sel, &format!("{} (Space)", tr("Select"))).clicked() {
            app.set_tool(ToolType::Select);
        }

        ui.separator();

        // --- 2. Drawing Tools ---
        let ic_line = app.icons.get(ui.ctx(), "line").cloned();
        let is_line = app.active_tool_type == ToolType::Line;
        if render_icon_button(ui, ic_line.as_ref(), "L", is_line, &format!("{} (L)", tr("Line"))).clicked() {
            app.set_tool(ToolType::Line);
        }

        let ic_freehand = app.icons.get(ui.ctx(), "freehand").cloned();
        let is_freehand = app.active_tool_type == ToolType::Freehand;
        if render_icon_button(ui, ic_freehand.as_ref(), "~", is_freehand, &tr("Freehand")).clicked() {
            app.set_tool(ToolType::Freehand);
        }

        let ic_rect = app.icons.get(ui.ctx(), "rectangle").cloned();
        let is_rect = app.active_tool_type == ToolType::Rectangle;
        if render_icon_button(ui, ic_rect.as_ref(), "R", is_rect, &format!("{} (R)", tr("Rectangle"))).clicked() {
            app.set_tool(ToolType::Rectangle);
        }

        let ic_rot_rect = app.icons.get(ui.ctx(), "rotated_rect").cloned();
        let is_rot_rect = app.active_tool_type == ToolType::RotatedRectangle;
        if render_icon_button(ui, ic_rot_rect.as_ref(), "RR", is_rot_rect, &tr("Rotated Rectangle")).clicked() {
            app.set_tool(ToolType::RotatedRectangle);
        }

        let ic_circle = app.icons.get(ui.ctx(), "circle").cloned();
        let is_circle = app.active_tool_type == ToolType::Circle;
        if render_icon_button(ui, ic_circle.as_ref(), "C", is_circle, &format!("{} (C)", tr("Circle"))).clicked() {
            app.set_tool(ToolType::Circle);
        }

        let ic_poly = app.icons.get(ui.ctx(), "polygon").cloned();
        let is_poly = app.active_tool_type == ToolType::Polygon;
        if render_icon_button(ui, ic_poly.as_ref(), "P", is_poly, &tr("Polygon")).clicked() {
            app.set_tool(ToolType::Polygon);
        }

        let ic_arc = app.icons.get(ui.ctx(), "arc").cloned();
        let is_arc = app.active_tool_type == ToolType::Arc;
        if render_icon_button(ui, ic_arc.as_ref(), "A", is_arc, &format!("{} (A)", tr("2-Point Arc"))).clicked() {
            app.set_tool(ToolType::Arc);
        }

        let ic_arc3 = app.icons.get(ui.ctx(), "arc3").cloned();
        if render_icon_button(ui, ic_arc3.as_ref(), "A3", false, &tr("3-Point Arc")).clicked() {
            app.set_tool(ToolType::Arc);
        }

        let ic_carc = app.icons.get(ui.ctx(), "center_arc").cloned();
        if render_icon_button(ui, ic_carc.as_ref(), "CA", false, &tr("Center and 2 Points Arc")).clicked() {
            app.set_tool(ToolType::Arc);
        }

        let ic_pie = app.icons.get(ui.ctx(), "pie").cloned();
        if render_icon_button(ui, ic_pie.as_ref(), "Pie", false, &tr("Pie")).clicked() {
            app.set_tool(ToolType::Arc);
        }

        ui.separator();

        // --- 3. Measurement & Annotation ---
        let ic_tape = app.icons.get(ui.ctx(), "tape").cloned();
        let is_tape = app.active_tool_type == ToolType::Tape;
        if render_icon_button(ui, ic_tape.as_ref(), "T", is_tape, &format!("{} (T)", tr("Tape Measure"))).clicked() {
            app.set_tool(ToolType::Tape);
        }

        let ic_prot = app.icons.get(ui.ctx(), "protractor").cloned();
        let is_prot = app.active_tool_type == ToolType::Protractor;
        if render_icon_button(ui, ic_prot.as_ref(), "Pr", is_prot, &tr("Protractor")).clicked() {
            app.set_tool(ToolType::Protractor);
        }

        let ic_dim = app.icons.get(ui.ctx(), "dimension").cloned();
        let is_dim = app.active_tool_type == ToolType::Dimension;
        if render_icon_button(ui, ic_dim.as_ref(), "D", is_dim, &format!("{} (D)", tr("Dimension"))).clicked() {
            app.set_tool(ToolType::Dimension);
        }

        let ic_text = app.icons.get(ui.ctx(), "text").cloned();
        let is_text = app.active_tool_type == ToolType::Text;
        if render_icon_button(ui, ic_text.as_ref(), "Txt", is_text, &tr("Text")).clicked() {
            app.set_tool(ToolType::Text);
        }

        let ic_3dtext = app.icons.get(ui.ctx(), "text3d").cloned();
        if render_icon_button(ui, ic_3dtext.as_ref(), "3D", false, &tr("3D Text…")).clicked() {
            app.status_message = tr("3D Text — place 3D extruded text geometry");
        }

        let ic_geopath = app.icons.get(ui.ctx(), "geopath").cloned();
        if render_icon_button(ui, ic_geopath.as_ref(), "GP", false, &tr("Geopath alignment")).clicked() {
            app.set_tool(ToolType::Line);
        }

        ui.separator();

        // --- 4. Section Plane ---
        let ic_sec = app.icons.get(ui.ctx(), "section").cloned();
        let is_sec = app.active_tool_type == ToolType::SectionPlane;
        if render_icon_button(ui, ic_sec.as_ref(), "Sec", is_sec, &tr("Section Plane")).clicked() {
            app.set_tool(ToolType::SectionPlane);
        }

        ui.separator();

        // --- 5. Walkthrough ---
        let ic_poscam = app.icons.get(ui.ctx(), "position_camera").cloned();
        if render_icon_button(ui, ic_poscam.as_ref(), "Eye", false, &tr("Position Camera")).clicked() {
            app.status_message = tr("Position Camera — click where the observer stands");
        }

        let ic_walk = app.icons.get(ui.ctx(), "walk").cloned();
        if render_icon_button(ui, ic_walk.as_ref(), "Wk", false, &tr("Walk")).clicked() {
            app.status_message = tr("Walk — arrow keys or mouse to walk through the model");
        }

        let ic_look = app.icons.get(ui.ctx(), "look_around").cloned();
        if render_icon_button(ui, ic_look.as_ref(), "Lk", false, &tr("Look Around")).clicked() {
            app.status_message = tr("Look Around — turn camera around observer position");
        }

        ui.separator();

        // --- 6. Solid Tools ---
        let ic_solid = app.icons.get(ui.ctx(), "outer_shell").cloned();
        if render_icon_button(ui, ic_solid.as_ref(), "ST", false, &tr("Solid Tools (Outer Shell / Union / Subtract / Trim / Intersect)")).clicked() {
            app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Union);
        }
    });
}
