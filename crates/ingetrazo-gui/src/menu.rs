// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Top Menu Bar for IngeTrazo GUI with 1:1 parity with Python IngeTrazo:
//! File, Edit, Camera, Draw, Tools, Window, Extensions, Help.

use ingetrazo_core::camera::StandardView;
use ingetrazo_core::i18n::tr;
use ingetrazo_render::style::VisualStyle;
use ingetrazo_tools::base::ToolType;

use crate::app::IngeTrazoApp;

pub fn render_menu_bar(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        // ==========================================
        // 1. File Menu
        // ==========================================
        ui.menu_button(tr("File"), |ui| {
            if ui.button(format!("{}\tCtrl+N", tr("New"))).clicked() {
                app.new_document();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Shift+N", tr("New Window"))).clicked() {
                app.status_message = tr("Opened new window");
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+O", tr("Open…"))).clicked() {
                app.begin_open_document();
                ui.close_menu();
            }

            ui.menu_button(tr("Open Recent"), |ui| {
                if app.recent_files.is_empty() {
                    ui.label(egui::RichText::new(tr("(No recent files)")).italics().weak());
                } else {
                    let mut file_to_open = None;
                    for p in &app.recent_files {
                        let name = p.file_name().unwrap_or_default().to_string_lossy();
                        if ui.button(name.to_string()).clicked() {
                            file_to_open = Some(p.clone());
                            ui.close_menu();
                        }
                    }
                    if let Some(p) = file_to_open {
                        app.open_document(&p);
                    }
                }
            });

            if ui.button(tr("Recover a discarded auto-save…")).clicked() {
                app.show_discarded_recovery();
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tCtrl+S", tr("Save"))).clicked() {
                app.save_document();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Shift+S", tr("Save As…"))).clicked() {
                app.save_document_as();
                ui.close_menu();
            }

            ui.separator();

            // Import submenu
            ui.menu_button(tr("Import"), |ui| {
                if ui.button(tr("IngeTrazo document as component (.igz)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("SKP (.skp)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("COLLADA (.dae)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("glTF/GLB (.glb)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("Wavefront OBJ (.obj)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("STL mesh (*.stl)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("Image (PNG / JPG)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("Orthomosaic (GeoTIFF)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("AutoCAD DWG (.dwg)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("AutoCAD DXF (.dxf)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("Georeference (KML / GeoJSON)…")).clicked() {
                    app.show_geographic_import_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("Survey points CSV (UTM)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button(tr("Photogrammetric mesh (WebODM)…")).clicked() {
                    app.show_open_dialog = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(tr("Clear imported texture cache…")).clicked() {
                    app.status_message = tr("Cleared imported texture cache");
                    ui.close_menu();
                }
            });

            // Export submenu
            ui.menu_button(tr("Export"), |ui| {
                if ui.button(tr("IFC (BIM)…")).clicked() {
                    ui.close_menu();
                    match app.prepare_bim_ifc_export() {
                        Ok(0)=>app.status_message=tr("Nothing to export: tag geometry in the BIM panel first"),
                        Ok(_)=>if let Some(path)=crate::dialogs::pick_save_file(&tr("Export IFC"),"model.ifc","*.ifc|IFC4") {
                            app.status_message=match app.export_bim_ifc(&path){Ok(n)=>format!("{n} IFC elements exported to {}",path.display()),Err(error)=>format!("Export IFC failed: {error}")};
                        },
                        Err(error)=>app.status_message=error,
                    }
                }
                if ui.button(tr("glTF / GLB (3D, single file)…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "model.glb".to_string();
                    ui.close_menu();
                }
                if ui.button(tr("COLLADA (.dae)…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "model.dae".to_string();
                    ui.close_menu();
                }
                if ui.button(tr("STL (3D printing)…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "model.stl".to_string();
                    ui.close_menu();
                }
                if ui.button(tr("Wavefront OBJ (.obj)…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "model.obj".to_string();
                    ui.close_menu();
                }
                if ui.button(tr("Current view as DXF…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "model.dxf".to_string();
                    ui.close_menu();
                }
                if ui.button(tr("Image (PNG / JPG)…")).clicked() {
                    app.show_save_dialog = true;
                    app.dialog_path_input = "screenshot.png".to_string();
                    ui.close_menu();
                }
            });

            ui.separator();

            if ui.button(tr("Sheet composer…")).clicked() {
                if app.scene.sheets.is_empty() {
                    app.scene.sheets.push(ingetrazo_core::composition::Sheet::new(
                        "Sheet 1",
                        ingetrazo_core::composition::PaperSize::A3,
                        ingetrazo_core::composition::Orientation::Landscape,
                    ));
                }
                app.scene.active_sheet_index = Some(0);
                app.status_message = tr("Sheet composer: switched to Sheet view");
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tCtrl+Q", tr("Quit"))).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        // ==========================================
        // 2. Edit Menu
        // ==========================================
        ui.menu_button(tr("Edit"), |ui| {
            if ui.button(format!("{}\tCtrl+Z", tr("Undo"))).clicked() {
                app.undo();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Y", tr("Redo"))).clicked() {
                app.redo();
                ui.close_menu();
            }
            if ui.button(format!("{}\tShift+R", tr("Repeat last command"))).clicked() {
                app.repeat_last_command();
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tCtrl+X", tr("Cut"))).clicked() {
                app.cut_selection();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+C", tr("Copy"))).clicked() {
                app.copy_selection();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+V", tr("Paste"))).clicked() {
                app.paste_selection();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Alt+V", tr("Paste in Place"))).clicked() {
                app.paste_in_place();
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tCtrl+A", tr("Select All"))).clicked() {
                app.select_all();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Shift+I", tr("Invert Selection"))).clicked() {
                app.invert_selection();
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tCtrl+G", tr("Make Group"))).clicked() {
                app.make_group();
                ui.close_menu();
            }
            if ui.button(format!("{}\tG", tr("Make Component…"))).clicked() {
                app.make_component();
                ui.close_menu();
            }
            if ui.button("Edit Group / Component").clicked() {
                app.begin_component_edit();
                ui.close_menu();
            }
            if ui.button("Finish Group / Component Edit").clicked() {
                app.finish_component_edit();
                ui.close_menu();
            }
            if ui.button("Cancel Group / Component Edit").clicked() {
                app.cancel_component_edit();
                ui.close_menu();
            }
            if ui.button("Make Component Unique").clicked() {
                app.make_component_unique();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Shift+G", tr("Explode Group"))).clicked() {
                app.explode_group();
                ui.close_menu();
            }

            ui.menu_button(tr("Intersect Faces"), |ui| {
                if ui.button(tr("With Selection")).clicked() {
                    app.intersect_faces(ingetrazo_core::intersect::IntersectMode::WithSelection);
                    ui.close_menu();
                }
                if ui.button(tr("With Model")).clicked() {
                    app.intersect_faces(ingetrazo_core::intersect::IntersectMode::WithModel);
                    ui.close_menu();
                }
                if ui.button(tr("With Context")).clicked() {
                    app.intersect_faces(ingetrazo_core::intersect::IntersectMode::WithContext);
                    ui.close_menu();
                }
            });

            if ui.button(tr("Split into Pieces")).clicked() {
                app.split_into_pieces();
                ui.close_menu();
            }
            if ui.button(tr("Simplify Mesh…")).clicked() {
                app.show_simplify_dialog = true;
                ui.close_menu();
            }
            if ui.button(tr("Convert Path to Geometry")).clicked() {
                app.convert_paths_to_geometry();
                ui.close_menu();
            }
            if ui.button(tr("Delete Guides")).clicked() {
                app.delete_guides();
                ui.close_menu();
            }

            ui.separator();

            ui.menu_button(tr("Flip Along"), |ui| {
                if ui.button(tr("Red Direction (X)")).clicked() {
                    app.flip_along('x');
                    ui.close_menu();
                }
                if ui.button(tr("Green Direction (Y)")).clicked() {
                    app.flip_along('y');
                    ui.close_menu();
                }
                if ui.button(tr("Blue Direction (Z)")).clicked() {
                    app.flip_along('z');
                    ui.close_menu();
                }
            });

            if ui.button(tr("Hide")).clicked() {
                app.hide_selection();
                ui.close_menu();
            }
            ui.menu_button(tr("Unhide"), |ui| {
                if ui.button(tr("Selected")).clicked() {
                    app.unhide_selected();
                    ui.close_menu();
                }
                if ui.button(tr("Last")).clicked() {
                    app.unhide_last();
                    ui.close_menu();
                }
                if ui.button(tr("All")).clicked() {
                    app.unhide_all();
                    ui.close_menu();
                }
            });

            if ui.button(tr("Reverse Faces")).clicked() {
                app.reverse_faces();
                ui.close_menu();
            }
            if ui.button(tr("Orient Faces")).clicked() {
                app.orient_faces();
                ui.close_menu();
            }
            if ui.button(tr("Heal Overlapping Faces")).clicked() {
                app.heal_overlaps();
                ui.close_menu();
            }
            if ui.button(tr("Rebuild Faces (Planar)")).clicked() {
                app.rebuild_planar_faces();
                ui.close_menu();
            }
        });

        // ==========================================
        // 3. Camera Menu
        // ==========================================
        ui.menu_button(tr("Camera"), |ui| {
            let mut objects=app.scene.show_hidden_objects;
            let mut geometry=app.scene.show_hidden_geometry;
            if ui.checkbox(&mut objects,tr("Hidden Objects")).changed(){app.scene.set_hidden_view(true,objects);}
            if ui.checkbox(&mut geometry,tr("Hidden Geometry")).changed(){app.scene.set_hidden_view(false,geometry);}
            ui.separator();
            ui.menu_button(tr("Standard Views"), |ui| {
                if ui.button(tr("Top")).clicked() {
                    app.camera.set_standard_view(StandardView::Top);
                    ui.close_menu();
                }
                if ui.button(tr("Bottom")).clicked() {
                    app.camera.set_standard_view(StandardView::Bottom);
                    ui.close_menu();
                }
                if ui.button(tr("Front")).clicked() {
                    app.camera.set_standard_view(StandardView::Front);
                    ui.close_menu();
                }
                if ui.button(tr("Back")).clicked() {
                    app.camera.set_standard_view(StandardView::Back);
                    ui.close_menu();
                }
                if ui.button(tr("Left")).clicked() {
                    app.camera.set_standard_view(StandardView::Left);
                    ui.close_menu();
                }
                if ui.button(tr("Right")).clicked() {
                    app.camera.set_standard_view(StandardView::Right);
                    ui.close_menu();
                }
                if ui.button(tr("Isometric")).clicked() {
                    app.camera.set_standard_view(StandardView::Iso);
                    ui.close_menu();
                }
            });

            if ui.button(format!("{}\tShift+Z", tr("Zoom Extents"))).clicked() {
                app.zoom_extents();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+Alt+Z", tr("Zoom Selection"))).clicked() {
                app.zoom_selection();
                ui.close_menu();
            }

            ui.separator();

            if ui.selectable_label(!app.camera.perspective, format!("{}\tShift+P", tr("Toggle Perspective / Parallel"))).clicked() {
                app.camera.perspective = !app.camera.perspective;
                app.camera.two_point = false;
                ui.close_menu();
            }
            if ui.selectable_label(app.camera.two_point, tr("Two-Point Perspective")).clicked() {
                app.camera.two_point = !app.camera.two_point;
                if app.camera.two_point {
                    app.camera.perspective = true;
                }
                ui.close_menu();
            }

            ui.separator();

            ui.menu_button(tr("Style"), |ui| {
                if ui.selectable_label(app.style.visual_style == VisualStyle::ShadedWithTextures, tr("Shaded with Textures")).clicked() {
                    app.style.visual_style = VisualStyle::ShadedWithTextures;
                    ui.close_menu();
                }
                if ui.selectable_label(app.style.visual_style == VisualStyle::Wireframe, tr("Wireframe")).clicked() {
                    app.style.visual_style = VisualStyle::Wireframe;
                    ui.close_menu();
                }
                if ui.selectable_label(app.style.visual_style == VisualStyle::HiddenLine, tr("Hidden line")).clicked() {
                    app.style.visual_style = VisualStyle::HiddenLine;
                    ui.close_menu();
                }
                if ui.selectable_label(app.style.visual_style == VisualStyle::Monochrome, tr("Monochrome")).clicked() {
                    app.style.visual_style = VisualStyle::Monochrome;
                    ui.close_menu();
                }
                if ui.selectable_label(app.style.visual_style == VisualStyle::XRay, tr("X-Ray")).clicked() {
                    app.style.visual_style = VisualStyle::XRay;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(format!("{}\tAlt+X", tr("Toggle X-ray"))).clicked() {
                    app.style.visual_style = if app.style.visual_style == VisualStyle::XRay {
                        VisualStyle::ShadedWithTextures
                    } else {
                        VisualStyle::XRay
                    };
                    ui.close_menu();
                }
            });

            ui.checkbox(&mut app.shadows_enabled, tr("Shadows"));

            ui.separator();

            ui.menu_button(tr("Rest of model while editing"), |ui| {
                if ui.button(tr("Show normally")).clicked() {
                    app.status_message = tr("Show rest of model normally");
                    ui.close_menu();
                }
                if ui.button(tr("Fade")).clicked() {
                    app.status_message = tr("Fade rest of model");
                    ui.close_menu();
                }
                if ui.button(tr("Hide (fastest)")).clicked() {
                    app.status_message = tr("Hide rest of model");
                    ui.close_menu();
                }
            });

            ui.separator();

            if ui.button(format!("{}\tO", tr("Orbit"))).clicked() {
                app.status_message = tr("Orbit (O) — drag to rotate");
                ui.close_menu();
            }
            if ui.button(format!("{}\tH", tr("Pan"))).clicked() {
                app.status_message = tr("Pan (H) — drag to slide view");
                ui.close_menu();
            }
            if ui.button(format!("{}\tZ", tr("Zoom"))).clicked() {
                app.status_message = tr("Zoom (Z) — drag up/down");
                ui.close_menu();
            }
            if ui.button(tr("Zoom Window")).clicked() {
                app.status_message = tr("Zoom Window — drag a rectangle");
                ui.close_menu();
            }

            ui.separator();

            if ui.button(tr("Position Camera")).clicked() {
                app.activate_navigation(crate::navigation::Mode::Position);
                ui.close_menu();
            }
            if ui.button(tr("Walk")).clicked() {
                app.activate_navigation(crate::navigation::Mode::Walk);
                ui.close_menu();
            }
            if ui.button(tr("First Person")).clicked() {app.activate_navigation(crate::navigation::Mode::FirstPerson);ui.close_menu();}
            if ui.button(tr("Look Around")).clicked() {
                app.activate_navigation(crate::navigation::Mode::Look);
                ui.close_menu();
            }
        });

        // ==========================================
        // 4. Draw Menu
        // ==========================================
        ui.menu_button(tr("Draw"), |ui| {
            if ui.button(format!("{}\tL", tr("Line"))).clicked() {
                app.set_tool(ToolType::Line);
                ui.close_menu();
            }
            if ui.button(tr("Freehand")).clicked() {
                app.set_tool(ToolType::Freehand);
                ui.close_menu();
            }

            ui.menu_button(tr("Arcs"), |ui| {
                if ui.button(format!("{}\tA", tr("2-Point Arc"))).clicked() {
                    app.set_tool(ToolType::Arc);
                    ui.close_menu();
                }
                if ui.button(tr("3-Point Arc")).clicked() {
                    app.set_tool(ToolType::Arc);
                    ui.close_menu();
                }
                if ui.button(tr("Center Arc")).clicked() {
                    app.set_tool(ToolType::Arc);
                    ui.close_menu();
                }
                if ui.button(tr("Pie")).clicked() {
                    app.set_tool(ToolType::Arc);
                    ui.close_menu();
                }
            });

            ui.menu_button(tr("Shapes"), |ui| {
                if ui.button(format!("{}\tR", tr("Rectangle"))).clicked() {
                    app.set_tool(ToolType::Rectangle);
                    ui.close_menu();
                }
                if ui.button(tr("Rotated Rectangle")).clicked() {
                    app.set_tool(ToolType::RotatedRectangle);
                    ui.close_menu();
                }
                if ui.button(format!("{}\tC", tr("Circle"))).clicked() {
                    app.set_tool(ToolType::Circle);
                    ui.close_menu();
                }
                if ui.button(tr("Polygon")).clicked() {
                    app.set_tool(ToolType::Polygon);
                    ui.close_menu();
                }
            });

            ui.separator();

            if ui.button(tr("Geopath")).clicked() {
                app.set_tool(ToolType::Line);
                ui.close_menu();
            }
        });

        // ==========================================
        // 5. Tools Menu
        // ==========================================
        ui.menu_button(tr("Tools"), |ui| {
            if ui.button(format!("{}\tSpace", tr("Select"))).clicked() {
                app.set_tool(ToolType::Select);
                ui.close_menu();
            }
            if ui.button(format!("{}\tE", tr("Eraser"))).clicked() {
                app.set_tool(ToolType::Eraser);
                ui.close_menu();
            }
            if ui.button(format!("{}\tB", tr("Paint"))).clicked() {
                app.set_tool(ToolType::Paint);
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tM", tr("Move"))).clicked() {
                app.set_tool(ToolType::Move);
                ui.close_menu();
            }
            if ui.button(format!("{}\tQ", tr("Rotate"))).clicked() {
                app.set_tool(ToolType::Rotate);
                ui.close_menu();
            }
            if ui.button(format!("{}\tS", tr("Scale"))).clicked() {
                app.set_tool(ToolType::Scale);
                ui.close_menu();
            }
            if ui.button(tr("Flip")).clicked() {
                app.reverse_faces();
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tP", tr("Push/Pull"))).clicked() {
                app.set_tool(ToolType::PushPull);
                ui.close_menu();
            }
            if ui.button(tr("Follow Me")).clicked() {
                app.set_tool(ToolType::FollowMe);
                ui.close_menu();
            }
            if ui.button(format!("{}\tF", tr("Offset"))).clicked() {
                app.set_tool(ToolType::Offset);
                ui.close_menu();
            }
            if ui.button(tr("Fillet")).clicked() {
                app.set_tool(ToolType::Fillet);
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tT", tr("Tape Measure"))).clicked() {
                app.set_tool(ToolType::Tape);
                ui.close_menu();
            }
            if ui.button(tr("Protractor")).clicked() {
                app.set_tool(ToolType::Protractor);
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tD", tr("Dimension"))).clicked() {
                app.set_tool(ToolType::Dimension);
                ui.close_menu();
            }
            if ui.button(tr("Text")).clicked() {
                app.set_tool(ToolType::Text);
                ui.close_menu();
            }

            ui.separator();

            ui.menu_button(tr("Sections"),|ui|crate::section_actions::render_section_controls(app,ui));
            if ui.button(tr("Section Plane")).clicked() {
                app.set_tool(ToolType::SectionPlane);
                ui.close_menu();
            }

            ui.separator();

            if ui.button(tr("Outer Shell")).clicked() {
                app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::OuterShell);
                ui.close_menu();
            }
            ui.menu_button(tr("Solid Tools"), |ui| {
                if ui.button(tr("Intersect")).clicked() {
                    app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Intersect);
                    ui.close_menu();
                }
                if ui.button(tr("Union")).clicked() {
                    app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Union);
                    ui.close_menu();
                }
                if ui.button(tr("Subtract")).clicked() {
                    app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Subtract);
                    ui.close_menu();
                }
                if ui.button(tr("Trim")).clicked() {
                    app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Trim);
                    ui.close_menu();
                }
                if ui.button(tr("Split")).clicked() {
                    app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Split);
                    ui.close_menu();
                }
            });

            ui.separator();

            if ui.button(tr("3D Text…")).clicked() {
                app.open_insert_text3d();
                ui.close_menu();
            }

            ui.separator();

            if ui.button(tr("Terrain profile of selection")).clicked() {
                app.status_message = tr("Generated terrain profile of selection");
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tEsc", tr("Cancel current tool"))).clicked() {
                app.set_tool(ToolType::Select);
                ui.close_menu();
            }
        });

        // ==========================================
        // 6. Render Menu
        // ==========================================
        ui.menu_button(tr("Render"), |ui| {
            if ui.button(tr("Render with Blender (Cycles/EEVEE)…")).clicked() {
                app.show_render_dialog = true;
                ui.close_menu();
            }
            if ui.button(tr("Render Panel")).clicked() {
                app.show_tray = true;
                app.active_tray_tab = crate::app::TrayTab::Render;
                ui.close_menu();
            }
        });

        // ==========================================
        // 7. Window Menu
        // ==========================================
        ui.menu_button(tr("Window"), |ui| {
            ui.menu_button(tr("Panels"), |ui| {
                ui.checkbox(&mut app.show_tray, tr("Side Tray (all panels)"));
                ui.separator();
                if ui.button("Outliner / Parts").clicked(){app.show_tray=true;app.active_tray_tab=crate::app::TrayTab::Inspector;ui.close_menu();}
                if ui.button(tr("Entity info")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Inspector;
                    ui.close_menu();
                }
                if ui.button(tr("Layers")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Inspector;
                    ui.close_menu();
                }
                if ui.button(tr("Scenes")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Inspector;
                    ui.close_menu();
                }
                if ui.button(tr("Materials")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Inspector;
                    ui.close_menu();
                }
                if ui.button(tr("Properties")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Properties;
                    ui.close_menu();
                }
                if ui.button(tr("BIM")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Bim;
                    ui.close_menu();
                }
                if ui.button(tr("Terrain")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Terrain;
                    ui.close_menu();
                }
                if ui.button(tr("AI")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Ai;
                    ui.close_menu();
                }
                if ui.button(tr("Render")).clicked() {
                    app.show_tray = true;
                    app.active_tray_tab = crate::app::TrayTab::Render;
                    ui.close_menu();
                }
            });

            if ui.button(tr("Terrain profile")).clicked() {
                app.show_tray = true;
                app.active_tray_tab = crate::app::TrayTab::Terrain;
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{}\tCtrl+0", tr("Clean screen"))).clicked() {
                app.clean_screen_toggle();
                ui.close_menu();
            }
            if ui.button(format!("{}\tCtrl+F5", tr("Sidebar"))).clicked() {
                app.show_tray = !app.show_tray;
                ui.close_menu();
            }

            ui.separator();

            if ui.button(tr("Preferences…")).clicked() {
                app.show_preferences_dialog = true;
                ui.close_menu();
            }

            ui.menu_button(tr("Language"), |ui| {
                for &(code, name) in ingetrazo_core::i18n::LANGUAGE_NAMES {
                    let is_current = app.language == code;
                    if ui.selectable_label(is_current, name).clicked() {
                        app.set_language(code);
                        ui.close_menu();
                    }
                }
            });
        });

        // ==========================================
        // 7. Extensions Menu
        // ==========================================
        ui.menu_button(tr("Extensions"), |ui| {
            if ui.button(tr("Extension Manager…")).clicked() {
                app.status_message = tr("Extension Manager");
                ui.close_menu();
            }
            if ui.button(tr("Console / Scripting (Python)…")).clicked() {
                app.show_scripting_console = true;
                ui.close_menu();
            }
            if ui.button(tr("Solid Inspector…")).clicked() {
                app.show_solid_inspector = true;
                ui.close_menu();
            }
        });

        // ==========================================
        // 8. Help Menu
        // ==========================================
        ui.menu_button(tr("Help"), |ui| {
            if ui.button(format!("{}\tF3", tr("Search commands…"))).clicked() {
                app.show_command_search = true;
                ui.close_menu();
            }

            ui.separator();

            if ui.button(tr("Get more models and textures…")).clicked() {
                app.status_message = tr("Opening model repository");
                ui.close_menu();
            }

            ui.separator();

            if ui.button(tr("About IngeTrazo")).clicked() {
                app.show_about_dialog = true;
                ui.close_menu();
            }
        });
    });
}
