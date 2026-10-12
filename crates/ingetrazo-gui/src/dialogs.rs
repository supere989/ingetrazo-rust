// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Modal and inspector dialogs:
//! - Native file picker wrappers (kdialog / zenity)
//! - About IngeTrazo
//! - File Open / Import & Save / Export
//! - Preferences (Language, Units, Style, Shadows)
//! - Command Search (F3 quick action palette)
//! - Solid Inspector (Manifold watertight topology, border edges, volume, healing)
//! - Interactive Scripting Console
//! - Texture Edit (Scale U/V, rotation, opacity)

use std::path::PathBuf;
use ingetrazo_core::i18n::tr;
use ingetrazo_tools::base::ToolType;
use crate::app::IngeTrazoApp;

/// Launch native desktop file picker for opening a file.
pub fn pick_open_file(title: &str, filter: &str) -> Option<PathBuf> {
    if let Ok(output) = std::process::Command::new("kdialog")
        .arg("--title")
        .arg(title)
        .arg("--getopenfilename")
        .arg(".")
        .arg(filter)
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    if let Ok(output) = std::process::Command::new("zenity")
        .arg("--file-selection")
        .arg("--title")
        .arg(title)
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

/// Launch native desktop file picker for saving a file.
pub fn pick_save_file(title: &str, default_name: &str, filter: &str) -> Option<PathBuf> {
    if let Ok(output) = std::process::Command::new("kdialog")
        .arg("--title")
        .arg(title)
        .arg("--getsavefilename")
        .arg(default_name)
        .arg(filter)
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    if let Ok(output) = std::process::Command::new("zenity")
        .arg("--file-selection")
        .arg("--save")
        .arg("--confirm-overwrite")
        .arg("--filename")
        .arg(default_name)
        .arg("--title")
        .arg(title)
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

pub fn render_dialogs(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    crate::import_units::render(app, ctx);
    crate::survey_points::render(app,ctx);
    app.render_material_editor(ctx);
    // 1. About Dialog
    if app.show_about_dialog {
        egui::Window::new(tr("About IngeTrazo"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.heading("IngeTrazo 0.3.15 (Rust Native)");
                ui.label(tr("Free 3D modeler for architecture, engineering and 3D design."));
                ui.label(tr("Part of the IngePresupuestos ecosystem."));
                ui.separator();
                ui.label("Authors: Marco Sumari Tellez and IngeTrazo contributors");
                ui.label("License: GNU General Public License v3.0 or later (GPL-3.0-or-later)");
                ui.label("Repository: https://github.com/ingelibre/ingetrazo");
                ui.separator();
                if ui.button(tr("Close")).clicked() {
                    app.show_about_dialog = false;
                }
            });
    }

    if app.show_geographic_import_dialog {
        egui::Window::new(tr("Import georef")).collapsible(false).show(ctx, |ui| {
            ui.label(tr("KML / KMZ / GeoJSON alignment file"));
            ui.text_edit_singleline(&mut app.dialog_path_input);
            if ui.button(tr("Browse…")).clicked() {
                if let Some(path)=pick_open_file(&tr("Import georef"), "*.kml *.kmz *.geojson *.json|Georef") {app.dialog_path_input=path.to_string_lossy().into_owned();}
            }
            if ui.button(tr("Import")).clicked() {
                let path=PathBuf::from(app.dialog_path_input.trim());
                match app.import_geographic_paths(&path) {
                    Ok(_)=>app.show_geographic_import_dialog=false,
                    Err(err)=>app.status_message=format!("Import failed: {err}"),
                }
            }
            if ui.button(tr("Cancel")).clicked(){app.show_geographic_import_dialog=false;}
        });
    }
    // 2. Open / Import Dialog
    if app.show_open_dialog {
        egui::Window::new(tr("Open Document"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(tr("Enter path to file (.igz, .obj, .stl):"));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut app.dialog_path_input);
                    if ui.button(format!("📁 {}", tr("Browse…"))).clicked() {
                        if let Some(path) = pick_open_file(&tr("Open Document"), "*.igz *.obj *.stl *.skp *.dae *.glb|Supported 3D formats") {
                            app.dialog_path_input = path.to_string_lossy().to_string();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Open")).clicked() {
                        let path = PathBuf::from(app.dialog_path_input.trim());
                        if path.exists() {
                            app.open_chosen_document(&path);
                        } else {
                            app.status_message = format!("{}: {}", tr("File not found"), path.display());
                        }
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        app.cancel_open_document();
                    }
                });
            });
    }

    // 3. Save / Export Dialog
    if app.show_save_dialog {
        egui::Window::new(tr("Save Document"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(tr("Enter save path (.igz, .obj, .stl, .glb, .dae, .ifc):"));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut app.dialog_path_input);
                    if ui.button(format!("📁 {}", tr("Browse…"))).clicked() {
                        if let Some(path) = pick_save_file(&tr("Save Document"), &app.dialog_path_input, "*.igz *.obj *.stl *.glb *.dae *.ifc|3D formats") {
                            app.dialog_path_input = path.to_string_lossy().to_string();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Save")).clicked() {
                        let path = PathBuf::from(app.dialog_path_input.trim());
                        app.save_to_path(&path);
                        app.show_save_dialog = false;
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        app.show_save_dialog = false;
                        app.cancel_document_action();
                    }
                });
            });
    }

    // 4. Preferences: one cold draft, source OK/Cancel ownership.
    crate::preferences_editor::render(app,ctx);

    // 5. Command Search Dialog (F3)
    if app.show_command_search {
        egui::Window::new(tr("Search commands (F3)"))
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -100.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    let text_edit = egui::TextEdit::singleline(&mut app.command_search_query)
                        .hint_text(tr("Type command name…"));
                    let resp = ui.add(text_edit);
                    resp.request_focus();
                });
                ui.separator();

                let q = app.command_search_query.to_lowercase();
                let commands: [(&str, Box<dyn Fn(&mut IngeTrazoApp)>); 18] = [
                    ("Select Tool (Space)", Box::new(|a| a.set_tool(ToolType::Select))),
                    ("Line Tool (L)", Box::new(|a| a.set_tool(ToolType::Line))),
                    ("Rectangle Tool (R)", Box::new(|a| a.set_tool(ToolType::Rectangle))),
                    ("Rotated Rectangle Tool", Box::new(|a| a.set_tool(ToolType::RotatedRectangle))),
                    ("Circle Tool (C)", Box::new(|a| a.set_tool(ToolType::Circle))),
                    ("Polygon Tool", Box::new(|a| a.set_tool(ToolType::Polygon))),
                    ("Arc Tool (A)", Box::new(|a| a.set_tool(ToolType::Arc))),
                    ("Push / Pull Tool (P)", Box::new(|a| a.set_tool(ToolType::PushPull))),
                    ("Move Tool (M)", Box::new(|a| a.set_tool(ToolType::Move))),
                    ("Rotate Tool (Q)", Box::new(|a| a.set_tool(ToolType::Rotate))),
                    ("Scale Tool (S)", Box::new(|a| a.set_tool(ToolType::Scale))),
                    ("Tape Measure Tool (T)", Box::new(|a| a.set_tool(ToolType::Tape))),
                    ("Dimension Tool (D)", Box::new(|a| a.set_tool(ToolType::Dimension))),
                    ("Section Plane Tool", Box::new(|a| a.set_tool(ToolType::SectionPlane))),
                    ("Paint / Material Tool (B)", Box::new(|a| a.set_tool(ToolType::Paint))),
                    ("Zoom Extents (Shift+Z)", Box::new(|a| a.zoom_extents())),
                    ("Make Group (Ctrl+G)", Box::new(|a| a.make_group())),
                    ("Solid Inspector", Box::new(|a| a.show_solid_inspector = true)),
                ];

                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    for (cmd_name, action) in commands {
                        if q.is_empty() || cmd_name.to_lowercase().contains(&q) {
                            if ui.button(cmd_name).clicked() {
                                action(app);
                                app.show_command_search = false;
                            }
                        }
                    }
                });

                ui.separator();
                if ui.button(tr("Close")).clicked() {
                    app.show_command_search = false;
                }
            });
    }

    // 6. Solid Inspector Dialog
    if app.show_solid_inspector {
        egui::Window::new(tr("Solid Inspector"))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(380.0, 320.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.heading(tr("Manifold Solid Topology"));
                ui.separator();

                let is_solid = app.scene.mesh.is_manifold_solid();
                let vol = app.scene.mesh.volume();
                let mut border_edges = 0;
                let mut non_manifold_edges = 0;

                for e in &app.scene.mesh.edges {
                    if e.faces.len() == 1 {
                        border_edges += 1;
                    } else if e.faces.len() > 2 {
                        non_manifold_edges += 1;
                    }
                }

                if is_solid {
                    ui.label(egui::RichText::new("✓ Watertight closed 2-manifold solid").color(egui::Color32::from_rgb(40, 180, 80)).strong());
                    if let Some(v) = vol {
                        ui.label(format!("• Enclosed volume: {v:.4} m³"));
                    }
                } else {
                    ui.label(egui::RichText::new("⚠ Not a closed solid").color(egui::Color32::from_rgb(220, 140, 40)).strong());
                    ui.label(format!("• Open border edges (surface holes): {border_edges}"));
                    ui.label(format!("• Non-manifold edges (>2 faces): {non_manifold_edges}"));
                }

                ui.separator();
                ui.label(format!("Total vertices: {}", app.scene.mesh.vertices.len()));
                ui.label(format!("Total edges: {}", app.scene.mesh.edges.len()));
                ui.label(format!("Total faces: {}", app.scene.mesh.faces.len()));

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("Heal / Weld geometry")).clicked() {
                        app.heal_overlaps();
                    }
                    if ui.button(tr("Reverse Faces")).clicked() {
                        app.reverse_faces();
                    }
                    if ui.button(tr("Close")).clicked() {
                        app.show_solid_inspector = false;
                    }
                });
            });
    }

    // 7. Scripting Console Dialog
    if app.show_scripting_console {
        egui::Window::new(tr("IngeTrazo Scripting Console"))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(500.0, 360.0))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Native commands: add_cube, select_all, stats, clear").small().color(egui::Color32::from_rgb(160, 160, 170)));
                #[cfg(feature="engineering-python")]
                ui.checkbox(&mut app.engineering_python_enabled,"Engineering Python: current mesh only (full Scene scripting incomplete)");
                ui.separator();

                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for line in &app.console_logs {
                        ui.monospace(line);
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    let resp = ui.text_edit_singleline(&mut app.console_input);
                    if (ui.button(tr("Run")).clicked() || resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)))
                        && !app.console_input.is_empty()
                    {
                        let cmd = app.console_input.clone();
                        app.console_input.clear();
                        app.run_script(&cmd);
                    }
                    if ui.button(tr("Clear")).clicked() {
                        app.console_logs.clear();
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label(tr("Snippets:"));
                    if ui.small_button("add_cube").clicked() {
                        app.run_script("add_cube");
                    }
                    if ui.small_button("stats").clicked() {
                        app.run_script("stats");
                    }
                    if ui.small_button("select_all").clicked() {
                        app.run_script("select_all");
                    }
                });

                ui.separator();
                if ui.button(tr("Close")).clicked() {
                    app.show_scripting_console = false;
                }
            });
    }

    // 8. Texture Edit Dialog
    if app.show_texture_edit_dialog {
        egui::Window::new(tr("Edit Texture Mapping"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("{}: {}", tr("Active material"), app.active_material));
                ui.separator();

                egui::Grid::new("tex_edit_grid").spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label(tr("Scale U (m):"));
                    ui.add(egui::Slider::new(&mut app.texture_scale_u, 0.05..=10.0));
                    ui.end_row();

                    ui.label(tr("Scale V (m):"));
                    ui.add(egui::Slider::new(&mut app.texture_scale_v, 0.05..=10.0));
                    ui.end_row();

                    ui.label(tr("Rotation (°):"));
                    ui.add(egui::Slider::new(&mut app.texture_rot_deg, 0.0..=360.0));
                    ui.end_row();


                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("Apply to Selection")).clicked() {
                        match app.edit_selected_texture_mapping(app.texture_scale_u as f64,app.texture_scale_v as f64,app.texture_rot_deg as f64){
                            Ok(_)=>app.show_texture_edit_dialog=false,
                            Err(error)=>app.status_message=error,
                        }
                    }
                    if ui.button(tr("Close")).clicked() {
                        app.show_texture_edit_dialog = false;
                    }
                });
            });
    }
}
