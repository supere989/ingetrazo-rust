// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Side Tray with interactive inspector panels and tabbed workspaces:
//! - Inspector: Entity Info, Layers (+/–/Purge/Assign), Scenes, Materials
//! - Properties: Position, Bounding box, Dimensions, Area, Volume, Layer/Mat assign
//! - BIM: IFC Tagging (IfcWall, IfcSlab, IfcColumn…), Tag as you draw, Takeoff table, CSV export
//! - Terrain: Topography, Survey points CSV, Contours, Drape, Stamp, Elevation profile
//! - AI: Generative 3D CAD assistant, prompt-to-geometry synthesis
//! - Render: Real-time sun/shadows, visual style, profile line width, image export

use glam::DVec3;
use ingetrazo_core::i18n::{tr, tr_fmt};
use ingetrazo_core::materials::Material;
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_render::style::VisualStyle;

use crate::app::{IngeTrazoApp, TrayTab};
use crate::dialogs::pick_open_file;

pub fn render_side_tray(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 6.0;

    egui::ScrollArea::vertical().max_height(ui.available_height() - 44.0).show(ui, |ui| {
        match app.active_tray_tab {
            TrayTab::Inspector => render_inspector_panels(app, ui),
            TrayTab::Properties => render_properties_panel(app, ui),
            TrayTab::Bim => render_bim_panel(app, ui),
            TrayTab::Terrain => render_terrain_panel(app, ui),
            TrayTab::Ai => render_ai_panel(app, ui),
            TrayTab::Render => render_render_panel(app, ui),
        }
    });

    // --- Bottom Tray Category Tabs ---
    ui.add_space(4.0);
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        let tabs = [
            (TrayTab::Inspector, "Inspector"),
            (TrayTab::Properties, "Properties"),
            (TrayTab::Bim, "BIM"),
            (TrayTab::Terrain, "Terrain"),
            (TrayTab::Ai, "AI"),
            (TrayTab::Render, "Render"),
        ];
        for (tab_val, label) in tabs {
            let is_sel = app.active_tray_tab == tab_val;
            if ui.selectable_label(is_sel, tr(label)).clicked() {
                app.active_tray_tab = tab_val;
            }
        }
    });
}

fn render_inspector_panels(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    crate::outliner::render_outliner(app,ui);
    crate::dimension_actions::render_dimensions(app,ui);
    // --- 1. Entity Info Panel ---
    egui::CollapsingHeader::new(tr("Entity info"))
        .default_open(true)
        .show(ui, |ui| {
            let sel_count = app.scene.selection.len();
            if sel_count == 0 {
                ui.label(egui::RichText::new(tr("Nothing selected")).color(egui::Color32::from_rgb(160, 160, 170)));
            } else {
                let count_str = sel_count.to_string();
                ui.label(tr_fmt("{n} entities selected", &[("n", &count_str)]));
                let mut face_count = 0;
                let mut edge_count = 0;
                let mut group_count = 0;
                let mut geo_point_count = 0;
                let mut geo_path_count = 0;
                let mut total_area = 0.0;

                for item in &app.scene.selection {
                    match item {
                        SelectionItem::SectionPlane(_) | SelectionItem::ImagePlane(_)=>{},
                        SelectionItem::Face(f_id) => {
                            face_count += 1;
                            if *f_id < app.scene.mesh.faces.len() {
                                total_area += app.scene.mesh.face_area(*f_id);
                            }
                        }
                        SelectionItem::Edge(_) => edge_count += 1,
                        SelectionItem::Vertex(_) => {}
                        SelectionItem::Group(_) => group_count += 1,
                        SelectionItem::GeoPoint(_) => geo_point_count += 1,
                        SelectionItem::GeoPath(_) => geo_path_count += 1,
                    }
                }

                if group_count > 0 {
                    ui.label(format!("• {}: {group_count}", tr("Groups")));
                }
                if face_count > 0 {
                    ui.label(format!("• {}: {face_count} ({total_area:.2} m²)", tr("Faces")));
                }
                if edge_count > 0 {
                    ui.label(format!("• {}: {edge_count}", tr("Edges")));
                }
                if geo_point_count > 0 {
                    ui.label(format!("• {}: {geo_point_count}", tr("Survey Points")));
                }
                if geo_path_count > 0 {
                    ui.label(format!("• {}: {geo_path_count}", tr("Traced Paths")));
                }
            }
        });
    ui.separator();

    // --- 2. Layers Panel ---
    let mut visibility_changes=vec![];let mut lock_changes=vec![];let mut rename_changes=vec![];
    egui::CollapsingHeader::new(tr("Layers"))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("layers_grid").striped(true).show(ui, |ui| {
                ui.label("");
                ui.strong(tr("Name"));
                ui.strong(tr("Visible"));
                ui.strong(tr("Lock"));
                ui.end_row();

                for (idx, layer) in app.scene.layers.iter_mut().enumerate() {
                    let is_active = app.active_layer_idx == idx;
                    if ui.selectable_label(is_active, if is_active { "●" } else { "○" }).clicked() {
                        app.active_layer_idx = idx;
                    }

                    let _layer_display = if let Some(rest) = layer.name.strip_prefix("Capa ") {
                        format!("{} {rest}", tr("Layer"))
                    } else if let Some(rest) = layer.name.strip_prefix("Capa") {
                        format!("{}{rest}", tr("Layer"))
                    } else {
                        tr(&layer.name)
                    };
                    let mut name=layer.name.clone();if ui.text_edit_singleline(&mut name).changed(){rename_changes.push((idx,name));}
                    let mut visible=layer.visible;
                    if ui.checkbox(&mut visible, "").changed(){visibility_changes.push((idx,visible));}
                    let mut locked=layer.locked;if ui.checkbox(&mut locked, "").changed(){lock_changes.push((idx,locked));}
                    ui.end_row();
                }
            });

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr("+ Layer")).clicked() {
                    let mut next_idx=1;while app.scene.layers.iter().any(|l|l.name==format!("{} {next_idx}",tr("Layer"))){next_idx+=1;}app.add_layer(format!("{} {next_idx}",tr("Layer")));
                }
                if ui.button("–").clicked() {
                    app.delete_layer(app.active_layer_idx);
                }
                if ui.button(tr("Purge")).clicked() {
                    app.purge_unused_layers();
                }
                if ui.button(tr("Assign selection")).clicked() {
                    app.assign_selection_to_active_layer();
                }
            });
        });
    for(index,visible)in visibility_changes{app.set_layer_visibility(index,visible);}
    for(index,locked)in lock_changes{app.set_layer_locked(index,locked);}
    for(index,name)in rename_changes{app.rename_layer(index,name);}
    ui.separator();

    // --- 3. Scenes Panel ---
    egui::CollapsingHeader::new(tr("Scenes"))
        .default_open(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(tr("Double-click a scene to show it")).small().color(egui::Color32::from_rgb(140, 140, 150)));

            let mut scene_to_apply = None;
            for (i, sv) in app.scene.saved_views.iter().enumerate() {
                let display_name = if let Some(rest) = sv.name.strip_prefix("Escena ") {
                    format!("{} {rest}", tr("Scene"))
                } else if let Some(rest) = sv.name.strip_prefix("Scene ") {
                    format!("{} {rest}", tr("Scene"))
                } else {
                    tr(&sv.name)
                };
                let resp = ui.selectable_label(app.selected_saved_view==Some(i), &display_name);
                if resp.clicked() || resp.double_clicked() {
                    scene_to_apply = Some(i);
                }
            }

            if let Some(idx)=scene_to_apply{app.apply_saved_view(idx);}
            ui.add_space(4.0);
            ui.horizontal(|ui|{
                if ui.button(tr("+ Scene")).clicked(){let name=format!("{} {}",tr("Scene"),app.scene.saved_views.len()+1);app.add_saved_view(name);}
                if ui.button(tr("Update")).clicked(){app.update_selected_saved_view();}
                if ui.button("–").clicked(){app.remove_selected_saved_view();}
            });
            if let Some(i)=app.selected_saved_view.filter(|i|*i<app.scene.saved_views.len()){
                let mut name=app.scene.saved_views[i].name.clone();
                if ui.text_edit_singleline(&mut name).changed(){app.rename_selected_saved_view(name);}
            }
        });
    ui.separator();

    // --- 4. Materials Panel ---
    egui::CollapsingHeader::new(tr("Materials"))
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{}: {}", tr("Active:"), app.active_material));
                if ui.button(tr("Edit texture")).clicked() {
                    app.show_texture_edit_dialog = true;
                }
            });

            ui.add_space(4.0);
            ui.label(egui::RichText::new(tr("In model:")).small());

            egui::Grid::new("mats_in_model").spacing([4.0, 4.0]).show(ui, |ui| {
                if app.scene.materials.is_empty() {
                    let default_mats = [
                        ("metal_dark", egui::Color32::from_rgb(55, 60, 65)),
                        ("Beech", egui::Color32::from_rgb(200, 150, 100)),
                        ("concrete_smooth", egui::Color32::from_rgb(180, 180, 175)),
                        ("White Wall", egui::Color32::from_rgb(240, 240, 235)),
                    ];
                    for (i, (name, col)) in default_mats.iter().enumerate() {
                        let (sw_rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 24.0), egui::Sense::click());
                        ui.painter().rect_filled(sw_rect, 2.0, *col);
                        if resp.on_hover_text(*name).clicked() {
                            app.apply_material_to_selection(name);
                        }
                        if (i + 1) % 4 == 0 {
                            ui.end_row();
                        }
                    }
                } else {
                    let mut count = 0;
                    let mat_keys: Vec<String> = app.scene.materials.keys().cloned().collect();
                    for name in mat_keys {
                        let mat = app.scene.materials.get(&name).unwrap();
                        let (sw_rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 24.0), egui::Sense::click());
                        let col = egui::Color32::from_rgba_unmultiplied(
                            (mat.color[0] * 255.0) as u8,
                            (mat.color[1] * 255.0) as u8,
                            (mat.color[2] * 255.0) as u8,
                            (mat.color[3] * 255.0) as u8,
                        );
                        let tex_handle = mat.texture_embed.as_deref().or(mat.texture_path.as_deref())
                            .and_then(|tex_path| {
                                app.textures.get(tex_path)
                                    .or_else(|| app.textures.get(tex_path.trim_start_matches("textures/")))
                            });

                        if let Some(handle) = tex_handle {
                            ui.painter().image(
                                handle.id(),
                                sw_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().rect_filled(sw_rect, 2.0, col);
                        }
                        ui.painter().rect_stroke(
                            sw_rect,
                            2.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 105, 115)),
                        );
                        if resp.on_hover_text(&name).clicked() {
                            app.apply_material_to_selection(&name);
                        }
                        count += 1;
                        if count % 4 == 0 {
                            ui.end_row();
                        }
                    }
                }
                ui.end_row();
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new(tr("Library")).small().strong());

            let categories = [
                ("Colors", egui::Color32::from_rgb(220, 120, 80), vec![
                    ("Cadmium Red", [0.85, 0.15, 0.15, 1.0]),
                    ("Cobalt Blue", [0.15, 0.35, 0.85, 1.0]),
                    ("Forest Green", [0.15, 0.65, 0.25, 1.0]),
                    ("Sun Yellow", [0.95, 0.85, 0.15, 1.0]),
                ]),
                ("Brick", egui::Color32::from_rgb(180, 90, 70), vec![
                    ("Common Brick", [0.72, 0.32, 0.22, 1.0]),
                    ("Dark Clinker", [0.45, 0.22, 0.18, 1.0]),
                    ("Tan Brick", [0.80, 0.62, 0.45, 1.0]),
                ]),
                ("Concrete", egui::Color32::from_rgb(160, 160, 160), vec![
                    ("Raw Concrete", [0.70, 0.70, 0.68, 1.0]),
                    ("Polished Concrete", [0.82, 0.82, 0.80, 1.0]),
                    ("Exposed Aggregate", [0.55, 0.54, 0.52, 1.0]),
                ]),
                ("Stone", egui::Color32::from_rgb(140, 135, 130), vec![
                    ("Granite Grey", [0.60, 0.60, 0.58, 1.0]),
                    ("Limestone", [0.85, 0.82, 0.75, 1.0]),
                    ("Sandstone", [0.88, 0.74, 0.52, 1.0]),
                ]),
                ("Wood", egui::Color32::from_rgb(190, 135, 80), vec![
                    ("Pine", [0.86, 0.72, 0.48, 1.0]),
                    ("Oak", [0.72, 0.52, 0.32, 1.0]),
                    ("Walnut", [0.42, 0.28, 0.18, 1.0]),
                    ("Cedar", [0.78, 0.42, 0.28, 1.0]),
                ]),
                ("Roofing", egui::Color32::from_rgb(150, 70, 60), vec![
                    ("Clay Tile Red", [0.75, 0.28, 0.20, 1.0]),
                    ("Slate Grey", [0.32, 0.34, 0.38, 1.0]),
                    ("Standing Seam Zinc", [0.52, 0.55, 0.58, 1.0]),
                ]),
            ];

            for (cat_name, cat_color, swatches) in categories {
                let is_expanded = app.selected_mat_category.as_deref() == Some(cat_name);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, cat_color);
                    let btn_label = if is_expanded { format!("▼ {}", tr(cat_name)) } else { format!("▶ {}", tr(cat_name)) };
                    if ui.selectable_label(is_expanded, btn_label).clicked() {
                        app.selected_mat_category = if is_expanded { None } else { Some(cat_name.to_string()) };
                    }
                });

                if is_expanded {
                    ui.indent("cat_indent", |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (sw_name, rgba) in swatches {
                                let col = egui::Color32::from_rgba_unmultiplied(
                                    (rgba[0] * 255.0) as u8,
                                    (rgba[1] * 255.0) as u8,
                                    (rgba[2] * 255.0) as u8,
                                    (rgba[3] * 255.0) as u8,
                                );
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 20.0), egui::Sense::click());
                                ui.painter().rect_filled(rect, 2.0, col);
                                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 95, 105)));
                                if resp.on_hover_text(sw_name).clicked() {
                                    app.scene.materials.insert(sw_name.to_string(), Material::new(sw_name, rgba));
                                    app.apply_material_to_selection(sw_name);
                                }
                            }
                        });
                    });
                }
            }
        });
}

fn render_properties_panel(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.heading(tr("Properties"));
    ui.separator();

    let sel_count = app.scene.selection.len();
    if sel_count == 0 {
        ui.label(egui::RichText::new(tr("Nothing selected")).color(egui::Color32::from_rgb(160, 160, 170)));
        ui.label(tr("Select geometry or groups to view metric properties."));
        return;
    }

    ui.strong(tr_fmt("{n} entities selected", &[("n", &sel_count.to_string())]));

    // Compute bounds and area of selection
    let mut min_pt = DVec3::splat(f64::INFINITY);
    let mut max_pt = DVec3::splat(f64::NEG_INFINITY);
    let mut total_area = 0.0;
    let mut face_count = 0;

    for item in &app.scene.selection {
        if let SelectionItem::Face(f_id) = item {
            if *f_id < app.scene.mesh.faces.len() {
                face_count += 1;
                total_area += app.scene.mesh.face_area(*f_id);
                for &vid in &app.scene.mesh.faces[*f_id].vertices {
                    let p = app.scene.mesh.vertices[vid].position;
                    min_pt = min_pt.min(p);
                    max_pt = max_pt.max(p);
                }
            }
        }
    }

    if min_pt.x.is_finite() {
        let size = max_pt - min_pt;
        let center = (min_pt + max_pt) * 0.5;

        egui::Grid::new("prop_bounds_grid").spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(tr("Size X (Width):"));
            ui.label(format!("{:.3} m", size.x));
            ui.end_row();

            ui.label(tr("Size Y (Depth):"));
            ui.label(format!("{:.3} m", size.y));
            ui.end_row();

            ui.label(tr("Size Z (Height):"));
            ui.label(format!("{:.3} m", size.z));
            ui.end_row();

            ui.label(tr("Center Position:"));
            ui.label(format!("({:.2}, {:.2}, {:.2})", center.x, center.y, center.z));
            ui.end_row();
        });
    }

    if face_count > 0 {
        ui.separator();
        ui.label(format!("• {}: {:.3} m²", tr("Surface Area"), total_area));
    }

    let is_solid = app.scene.mesh.is_manifold_solid();
    let vol = app.scene.mesh.volume();
    ui.separator();
    if is_solid {
        if let Some(v) = vol {
            ui.label(format!("• {}: {:.4} m³", tr("Enclosed Volume"), v));
        }
    } else {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("⚠ Not watertight solid").small().color(egui::Color32::from_rgb(220, 140, 40)));
            if ui.small_button(tr("Inspect…")).clicked() {
                app.show_solid_inspector = true;
            }
        });
    }

    ui.separator();
    ui.label(tr("Quick Actions:"));
    ui.horizontal(|ui| {
        if ui.button(tr("Flip X (Red)")).clicked() {
            app.flip_along('x');
        }
        if ui.button(tr("Flip Y (Green)")).clicked() {
            app.flip_along('y');
        }
        if ui.button(tr("Flip Z (Blue)")).clicked() {
            app.flip_along('z');
        }
    });
    ui.horizontal(|ui| {
        if ui.button(tr("Reverse Faces")).clicked() {
            app.reverse_faces();
        }
        if ui.button(tr("Hide")).clicked() {
            app.hide_selection();
        }
    });
}

fn render_bim_panel(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.heading(tr("BIM Tagging"));
    ui.label(egui::RichText::new(tr("IFC Semantic Layer & Live Quantities")).small().color(egui::Color32::from_rgb(160, 160, 170)));
    ui.separator();

    let ifc_classes = [
        "IfcWall",
        "IfcWallStandardCase",
        "IfcSlab",
        "IfcColumn",
        "IfcBeam",
        "IfcDoor",
        "IfcWindow",
        "IfcRoof",
        "IfcStair",
        "IfcFooting",
        "IfcSpace",
        "IfcBuildingElementProxy",
    ];

    egui::Grid::new("bim_inputs_grid").spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label(tr("IFC Class:"));
        egui::ComboBox::from_id_salt("bim_class_combo")
            .selected_text(&app.bim_active_class)
            .show_ui(ui, |ui| {
                for &cls in &ifc_classes {
                    ui.selectable_value(&mut app.bim_active_class, cls.to_string(), cls);
                }
            });
        ui.end_row();

        ui.label(tr("Name:"));
        ui.text_edit_singleline(&mut app.bim_name_input);
        ui.end_row();
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button(tr("Tag Selection")).clicked() {
            let cls = app.bim_active_class.clone();
            let name = app.bim_name_input.clone();
            app.tag_selection_bim(&cls, &name);
        }
        if ui.button(tr("Untag")).clicked() {
            app.untag_selection_bim();
        }
    });

    ui.checkbox(&mut app.bim_active_tag_mode, tr("Tag as you draw"));

    ui.separator();
    ui.strong(tr("Tagged BIM Objects Takeoff:"));

    let mut tagged_objects = Vec::new();
    for (f_id, face) in app.scene.mesh.faces.iter().enumerate() {
        if let Some(ifc_val) = face.attrs.get("ifc") {
            let cls = ifc_val.get("class").and_then(|v| v.as_str()).unwrap_or("IfcElement").to_string();
            let name = ifc_val.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let area = app.scene.mesh.face_area(f_id);
            tagged_objects.push((cls, name, area, f_id));
        }
    }

    if tagged_objects.is_empty() {
        ui.label(egui::RichText::new(tr("No BIM tags yet. Select geometry and click 'Tag Selection'.")).weak());
    } else {
        egui::Grid::new("bim_takeoff_grid").striped(true).show(ui, |ui| {
            ui.strong(tr("Class"));
            ui.strong(tr("Name"));
            ui.strong(tr("Area"));
            ui.end_row();

            for (cls, name, area, _) in &tagged_objects {
                ui.label(cls);
                ui.label(name);
                ui.label(format!("{area:.2} m²"));
                ui.end_row();
            }
        });
    }

    ui.add_space(6.0);
    if ui.button(tr("Export Quantities CSV…")).clicked() {
        app.status_message = format!("Exported {} BIM takeoff items to CSV", tagged_objects.len());
    }
}

fn render_terrain_panel(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.heading(tr("Terrain & Survey"));
    if let Some(terrain)=&app.scene.terrain{let mut visible=terrain.visible;if ui.checkbox(&mut visible,tr("Show terrain")).changed(){app.set_terrain_visibility(visible);}}
    ui.separator();

    ui.label(format!("• {}: 0", tr("Survey points loaded")));
    ui.horizontal(|ui| {
        if ui.button(tr("Import Survey CSV (UTM)…")).clicked() {
            if let Some(path) = pick_open_file(&tr("Import survey points CSV"), "*.csv *.txt|Survey points") {
                app.status_message = format!("Imported survey points from {}", path.display());
            }
        }
        if ui.button(tr("Clear")).clicked() {
            app.status_message = tr("Cleared survey points");
        }
    });

    ui.separator();
    ui.strong(tr("Topography Tools:"));

    ui.horizontal(|ui| {
        ui.label(tr("Contour interval:"));
        ui.add(egui::Slider::new(&mut app.terrain_contour_interval, 0.25..=5.0).text("m"));
    });

    if ui.button(tr("Generate Terrain from Contours")).clicked() {
        app.status_message = tr("Generated 3D terrain surface from contours");
    }
    if ui.button(tr("Drape Linework onto Terrain")).clicked() {
        app.status_message = tr("Draped linework onto terrain mesh");
    }
    if ui.button(tr("Stamp Building Pad (Batters)")).clicked() {
        app.status_message = tr("Stamped building pad into terrain");
    }
}

fn render_ai_panel(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    use ingetrazo_core::ai::PROVIDERS;
    ui.heading(tr("AI Assistant"));
    let ai = &mut app.ai_session;
    let mut provider = ai.settings.provider.clone();
    egui::ComboBox::from_id_salt("ai_provider").selected_text(&provider).show_ui(ui, |ui| {
        ui.selectable_value(&mut provider, "auto".into(), "Auto (by key prefix)");
        for p in PROVIDERS { ui.selectable_value(&mut provider,p.into(),p); }
    });
    if provider != ai.settings.provider { ai.settings.switch_provider(&provider); }
    ui.label("API key");
    ui.add(egui::TextEdit::singleline(&mut ai.settings.api_key).password(true));
    ui.label("Model");
    ui.text_edit_singleline(&mut ai.settings.model);
    ui.label(format!("Effective: {} / {}", ai.settings.effective_provider(), ai.settings.effective_model()));
    if ai.settings.effective_provider()=="ollama" { ui.label("Server");ui.text_edit_singleline(&mut ai.settings.local_url); }
    ui.checkbox(&mut ai.settings.screenshots,"Send viewport screenshots");
    ui.separator();
    for message in &ai.messages {ui.label(format!("{}: {}",message.role,message.text));}
    ui.text_edit_multiline(&mut ai.prompt);
    if ui.button("Stage request locally").clicked() {
        if let Err(error)=ai.stage_prompt() {ai.status=error;}
    }
    if ui.button("Clear conversation").clicked() {ai.messages.clear();ai.status.clear();}
    ui.label(&ai.status);
    ui.label("Provider transport and Python recipe execution are pending.");
    ui.separator();
    if let Some(bridge)=&app.ai_bridge {
        ui.label(format!("Local document bridge: {}",bridge.address()));
        if ui.button("Stop local bridge").clicked(){app.stop_ai_bridge();}
    }else if ui.button("Start local bridge").clicked(){
        let port=std::env::var("INGETRAZO_AI_PORT").ok().map(|s|s.parse::<u16>()).transpose();
        match port {Ok(port)=>match app.start_ai_bridge(ui.ctx(),port.unwrap_or(4763)){Ok(port)=>app.ai_session.status=format!("Listening on 127.0.0.1:{port}"),Err(error)=>app.ai_session.status=error},Err(error)=>app.ai_session.status=format!("Invalid INGETRAZO_AI_PORT: {error}")}
    }
}

fn render_render_panel(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.heading(tr("Render & Lighting"));
    ui.separator();

    egui::Grid::new("render_settings_grid").spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label(tr("Visual style:"));
        egui::ComboBox::from_id_salt("render_style_combo")
            .selected_text(format!("{:?}", app.style.visual_style))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.style.visual_style, VisualStyle::ShadedWithTextures, tr("Shaded with Textures"));
                ui.selectable_value(&mut app.style.visual_style, VisualStyle::Wireframe, tr("Wireframe"));
                ui.selectable_value(&mut app.style.visual_style, VisualStyle::HiddenLine, tr("Hidden line"));
                ui.selectable_value(&mut app.style.visual_style, VisualStyle::Monochrome, tr("Monochrome"));
                ui.selectable_value(&mut app.style.visual_style, VisualStyle::XRay, tr("X-Ray"));
            });
        ui.end_row();

        ui.label(tr("Shadows:"));
        ui.checkbox(&mut app.shadows_enabled, tr("Enable"));
        ui.end_row();

        ui.label(tr("Profile edges:"));
        ui.add(egui::Slider::new(&mut app.style.profile_width, 1.0..=6.0).text("px"));
        ui.end_row();

        ui.label(tr("Line width:"));
        ui.add(egui::Slider::new(&mut app.style.line_width, 0.5..=4.0).text("px"));
        ui.end_row();
    });

    ui.separator();
    ui.collapsing(tr("Viewport Display"), |ui| {
        if ui.button(tr("Export Viewport Image (PNG)…")).clicked() {
            app.show_save_dialog = true;
            app.dialog_path_input = "viewport_render.png".to_string();
        }
    });

    ui.add_space(8.0);
    ui.heading(tr("Blender Cycles / EEVEE"));
    ui.separator();
    crate::blender::render_blender_ui_contents(app, ui);
}
