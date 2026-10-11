//! Cold preferences draft. Cancel never publishes runtime or durable settings.
use crate::{session_state::Preferences, IngeTrazoApp};
use ingetrazo_core::i18n::tr;
use ingetrazo_render::style::VisualStyle;

pub struct Draft {
    pub preferences: Preferences,
    pub unit_code: String,
    pub precision: i64,
    pub units_for_new: bool,
    pub obj_unit: String,
    pub dxf_unit: String,
    pub visual_style: VisualStyle,
    pub profile_width: f32,
    pub line_width: f32,
    pub shadows: bool,
}
impl IngeTrazoApp {
    /// Explicit source Apply replaces the validated unit pair, never geometry.
    pub fn apply_model_units(&mut self, chosen: serde_json::Value) {
        let Some(code) = chosen["length"]
            .as_str()
            .filter(|c| ingetrazo_core::units::MODEL_UNIT_CODES.contains(c))
        else {
            return;
        };
        let precision = chosen["precision"].as_i64().unwrap_or(2);
        if code == ingetrazo_core::units::model_unit(&self.scene)
            && precision == ingetrazo_core::units::model_precision(&self.scene)
        {
            return;
        }
        self.scene.unit = match code {
            "cm" => ingetrazo_core::units::LengthUnit::Centimeters,
            "mm" => ingetrazo_core::units::LengthUnit::Millimeters,
            "in" => ingetrazo_core::units::LengthUnit::Inches,
            "ft" => ingetrazo_core::units::LengthUnit::Feet,
            _ => ingetrazo_core::units::LengthUnit::Meters,
        };
        self.scene.model_units_override =
            Some(serde_json::json!({"length":code,"precision":precision}));
        if self.scene.dimension_style().is_null()
            && self.scene.native_document.get("dimension_style").is_none()
        {
            self.scene.dimension_style_override = Some(
                serde_json::json!({"decimals":2,"units":"m","font_size":9,"color":[45,55,75],"norma":"iso","base_step_mm":8.0,"ends":"arrow"}),
            );
        }
        if self.scene.dimension_style().is_object() {
            let mut style = self.scene.dimension_style().clone();
            style["units"] = serde_json::json!(code);
            style["decimals"] = serde_json::json!(precision);
            self.scene.dimension_style_override = Some(style);
        }
        self.scene.bump_version();
    }
    pub(crate) fn initialize_document_units(&mut self, chosen: serde_json::Value) {
        let version = self.scene.version;
        self.scene.dimension_style_override = Some(
            serde_json::json!({"decimals":2,"units":"m","font_size":9,"color":[45,55,75],"norma":"iso","base_step_mm":8.0,"ends":"arrow"}),
        );
        self.apply_model_units(chosen);
        self.scene.version = version;
    }
    pub fn begin_preferences(&mut self) {
        if self.document_session.preferences_draft.is_none() {
            let mut preferences = self.document_session.preferences.clone();
            preferences.language = Some(self.language.clone());
            self.document_session.preferences_draft = Some(Draft {
                obj_unit: crate::import_units::preference(&preferences, "import/obj_unit", "m")
                    .to_owned(),
                dxf_unit: crate::import_units::preference(&preferences, "import/dxf_unit", "mm")
                    .to_owned(),
                preferences,
                unit_code: ingetrazo_core::units::model_unit(&self.scene).into(),
                precision: ingetrazo_core::units::model_precision(&self.scene).clamp(0, 6),
                units_for_new: self.document_session.preferences.new_document_units()
                    == serde_json::json!({"length":ingetrazo_core::units::model_unit(&self.scene),"precision":ingetrazo_core::units::model_precision(&self.scene)}),
                visual_style: self.style.visual_style,
                profile_width: self.style.profile_width,
                line_width: self.style.line_width,
                shadows: self.shadows_enabled,
            });
        }
        self.show_preferences_dialog = true;
    }
    pub fn cancel_preferences(&mut self) {
        self.show_preferences_dialog = false;
        self.document_session.preferences_draft = None;
    }
    pub fn accept_preferences(&mut self) {
        let Some(mut draft) = self.document_session.preferences_draft.take() else {
            return;
        };
        draft.preferences.clamp();
        draft
            .preferences
            .extra
            .insert("import/obj_unit".into(), serde_json::json!(draft.obj_unit));
        draft
            .preferences
            .extra
            .insert("import/dxf_unit".into(), serde_json::json!(draft.dxf_unit));
        crate::appearance_preferences::prepare_accept(
            &self.document_session.preferences,
            &mut draft.preferences,
        );
        self.document_session.preferences = draft.preferences;
        if let Some(language) = self.document_session.preferences.language.clone() {
            self.set_language(&language);
        }
        let chosen =
            serde_json::json!({"length":draft.unit_code,"precision":draft.precision.clamp(0,6)});
        self.apply_model_units(chosen.clone());
        if draft.units_for_new {
            self.document_session.preferences.extra.insert(
                "units/new_document".into(),
                serde_json::json!(chosen.to_string()),
            );
        }
        self.style.visual_style = draft.visual_style;
        self.style.profile_width = draft.profile_width;
        self.style.line_width = draft.line_width;
        self.shadows_enabled = draft.shadows;
        self.show_preferences_dialog = false;
        // Original _setup_autosave re-arms only when OK accepts the settings.
        self.document_session.last_autosave = std::time::Instant::now();
        self.persist_preferences();
    }
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if !app.show_preferences_dialog {
        app.document_session.preferences_draft = None;
        return;
    }
    app.begin_preferences();
    let draft = app.document_session.preferences_draft.as_mut().unwrap();
    let mut accepted = false;
    let mut canceled = false;
    let modal = egui::Modal::new(egui::Id::new("preferences-editor")).show(ctx, |ui| {
        ui.set_min_width(430.);
        ui.heading(tr("Preferences"));
        egui::Grid::new("pref_grid")
            .spacing([12., 8.])
            .show(ui, |ui| {
                ui.label(tr("Language"));
                let language = draft
                    .preferences
                    .language
                    .get_or_insert_with(|| "en".into());
                egui::ComboBox::from_id_salt("pref_lang_combo")
                    .selected_text(language.clone())
                    .show_ui(ui, |ui| {
                        for &(code, name) in ingetrazo_core::i18n::LANGUAGE_NAMES {
                            ui.selectable_value(language, code.to_owned(), name);
                        }
                    });
                ui.end_row();
                ui.label("Autosave");
                ui.checkbox(&mut draft.preferences.autosave, "Enable recovery copies");
                ui.end_row();
                ui.label("Autosave interval");
                ui.add_enabled(
                    draft.preferences.autosave,
                    egui::Slider::new(&mut draft.preferences.autosave_minutes, 1..=60)
                        .text("minutes"),
                );
                ui.end_row();
                ui.label("First-person look sensitivity");
                ui.add(egui::Slider::new(
                    &mut draft.preferences.look_sensitivity,
                    1..=100,
                ));
                ui.end_row();
                ui.label("Undo steps (0 = unlimited)");
                ui.add(egui::Slider::new(
                    &mut draft.preferences.undo_steps,
                    0..=5000,
                ));
                ui.end_row();
            });
        for (key, label, default) in [
            (
                "section_ask_name",
                "Ask for section name after placement",
                true,
            ),
            ("nav/invert_wheel", "Invert mouse wheel zoom", false),
            ("nav/invert_orbit_y", "Invert vertical orbit", false),
        ] {
            let mut value = draft
                .preferences
                .extra
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(default);
            if ui.checkbox(&mut value, tr(label)).changed() {
                draft
                    .preferences
                    .extra
                    .insert(key.into(), serde_json::json!(value));
            }
        }
        crate::appearance_preferences::render(&mut draft.preferences, ui);
        ui.separator();
        ui.label("Import defaults");
        for (code, label, codes) in [
            (
                &mut draft.obj_unit,
                "Suggested unit for OBJ",
                crate::import_units::OBJ_CODES,
            ),
            (
                &mut draft.dxf_unit,
                "Suggested unit for DXF/DWG",
                crate::import_units::DXF_CODES,
            ),
        ] {
            egui::ComboBox::from_id_salt(label)
                .selected_text(format!("{label}: {code}"))
                .show_ui(ui, |ui| {
                    for option in codes {
                        ui.selectable_value(code, option.to_owned(), option);
                    }
                });
        }
        ui.separator();
        ui.label("Units of this document");
        egui::ComboBox::from_id_salt("pref_units_combo")
            .selected_text(unit_label(&draft.unit_code))
            .show_ui(ui, |ui| {
                for code in ingetrazo_core::units::MODEL_UNIT_CODES {
                    ui.selectable_value(&mut draft.unit_code, code.to_owned(), unit_label(code));
                }
            });
        ui.add(egui::Slider::new(&mut draft.precision, 0..=6).text("Decimals"));
        ui.checkbox(&mut draft.units_for_new, "Also use for new documents");
        ui.collapsing("Model display", |ui| {
            egui::ComboBox::from_id_salt("pref_style_combo")
                .selected_text(format!("{:?}", draft.visual_style))
                .show_ui(ui, |ui| {
                    for (style, name) in [
                        (VisualStyle::ShadedWithTextures, "Shaded with Textures"),
                        (VisualStyle::Wireframe, "Wireframe"),
                        (VisualStyle::HiddenLine, "Hidden line"),
                        (VisualStyle::Monochrome, "Monochrome"),
                        (VisualStyle::XRay, "X-Ray"),
                    ] {
                        ui.selectable_value(&mut draft.visual_style, style, tr(name));
                    }
                });
            ui.add(
                egui::Slider::new(&mut draft.profile_width, 1.0..=6.0).text("Profile edges (px)"),
            );
            ui.add(egui::Slider::new(&mut draft.line_width, 0.5..=4.0).text("Line width (px)"));
            ui.checkbox(&mut draft.shadows, tr("Enable real-time solar shading"));
        });
        ui.separator();
        ui.horizontal(|ui| {
            accepted = ui.button(tr("OK")).clicked();
            canceled = ui.button(tr("Cancel")).clicked();
        });
    });
    if accepted {
        app.accept_preferences();
    } else if canceled || modal.should_close() {
        app.cancel_preferences();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        a: &mut IngeTrazoApp,
        ctx: &egui::Context,
        t: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100., 800.),
                )),
                time: Some(t),
                events,
                ..Default::default()
            },
            |ctx| a.update_ui(ctx),
        )
    }
    fn text(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
        out.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == label).then_some(t.pos + t.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("missing actual preferences text {label}"))
    }
    fn click(
        a: &mut IngeTrazoApp,
        ctx: &egui::Context,
        t: &mut f64,
        pos: egui::Pos2,
    ) -> egui::FullOutput {
        *t += 0.1;
        frame(a, ctx, *t, vec![egui::Event::PointerMoved(pos)]);
        let mut out = None;
        for pressed in [true, false] {
            *t += 0.1;
            out = Some(frame(
                a,
                ctx,
                *t,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            ));
        }
        out.unwrap()
    }
    #[test]
    fn units_modal_preserves_composite_then_explicit_metres_and_default() {
        let root = std::env::temp_dir().join(format!("units-ui-{}", uuid::Uuid::new_v4()));
        let mut a = IngeTrazoApp::new();
        a.set_language("en");
        a.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        a.apply_model_units(serde_json::json!({"length":"ft-in-frac","precision":4}));
        a.mark_document_clean();
        let before = a.scene.clone();
        let ctx = egui::Context::default();
        let mut time = 0.;
        a.begin_preferences();
        frame(&mut a, &ctx, time, vec![]);
        time += 0.1;
        let out = frame(&mut a, &ctx, time, vec![]);
        assert_eq!(
            a.document_session
                .preferences_draft
                .as_ref()
                .unwrap()
                .unit_code,
            "ft-in-frac"
        );
        click(&mut a, &ctx, &mut time, text(&out, "Cancel"));
        assert_eq!(a.scene, before);
        a.begin_preferences();
        time += 0.1;
        frame(&mut a, &ctx, time, vec![]);
        time += 0.1;
        let out = frame(&mut a, &ctx, time, vec![]);
        click(
            &mut a,
            &ctx,
            &mut time,
            text(&out, "Fractional feet and inches"),
        );
        time += 0.1;
        let out = frame(&mut a, &ctx, time, vec![]);
        click(&mut a, &ctx, &mut time, text(&out, "Metres (m)"));
        time += 0.1;
        let out = frame(&mut a, &ctx, time, vec![]);
        click(
            &mut a,
            &ctx,
            &mut time,
            text(&out, "Also use for new documents"),
        );
        time += 0.1;
        let out = frame(&mut a, &ctx, time, vec![]);
        click(&mut a, &ctx, &mut time, text(&out, "OK"));
        assert_eq!(ingetrazo_core::units::model_unit(&a.scene), "m");
        assert_eq!(ingetrazo_core::units::model_precision(&a.scene), 4);
        assert_eq!(a.scene.dimension_style()["units"], "m");
        assert_eq!(a.scene.dimension_style()["decimals"], 4);
        assert!(a.document_dirty());
        assert!(a.undo_stack.is_empty());
        // Source SnapshotCompound excludes live unit/style properties.
        a.save_undo_state();
        a.scene.mesh.add_vertex(glam::DVec3::new(1., 2., 3.));
        a.begin_preferences();
        {
            let d = a.document_session.preferences_draft.as_mut().unwrap();
            d.unit_code = "cm".into();
            d.precision = 6;
            d.units_for_new = false;
        }
        a.accept_preferences();
        a.undo();
        assert!(a.scene.mesh.vertices.is_empty());
        assert_eq!(ingetrazo_core::units::model_unit(&a.scene), "cm");
        assert_eq!(ingetrazo_core::units::model_precision(&a.scene), 6);
        a.redo();
        assert_eq!(a.scene.mesh.vertices.len(), 1);
        assert_eq!(a.scene.dimension_style()["decimals"], 6);
        let mut restart = IngeTrazoApp::new();
        restart.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        assert_eq!(ingetrazo_core::units::model_precision(&restart.scene), 4);
        assert!(!restart.document_dirty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn actual_modal_cancel_ok_escape_and_warm_draft_ownership() {
        let root = std::env::temp_dir().join(format!("preferences-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut a = IngeTrazoApp::new();
        a.set_language("en");
        a.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        a.document_session
            .preferences
            .extra
            .insert("proof/opaque".into(), serde_json::json!("owned-unknown"));
        a.persist_preferences();
        a.scene.mesh.add_vertex(glam::DVec3::X);
        a.save_undo_state();
        a.scene.mesh.add_vertex(glam::DVec3::Y);
        let before = a.scene.mesh.clone();
        let history = (a.undo_stack.len(), a.redo_stack.len());
        let timer = a.document_session.last_autosave;
        let file = root.join("config/settings.json");
        let bytes = std::fs::read(&file).unwrap();
        let ctx = egui::Context::default();
        let mut t = 0.;
        a.begin_preferences();
        let out = frame(&mut a, &ctx, t, vec![]);
        let owner = a
            .document_session
            .preferences_draft
            .as_ref()
            .unwrap()
            .preferences
            .extra["proof/opaque"]
            .as_str()
            .unwrap()
            .as_ptr();
        let check = text(&out, "Enable recovery copies");
        click(&mut a, &ctx, &mut t, check);
        assert!(
            !a.document_session
                .preferences_draft
                .as_ref()
                .unwrap()
                .preferences
                .autosave
        );
        assert!(a.document_session.preferences.autosave);
        t += 0.1;
        let out = frame(&mut a, &ctx, t, vec![]);
        assert_eq!(
            a.document_session
                .preferences_draft
                .as_ref()
                .unwrap()
                .preferences
                .extra["proof/opaque"]
                .as_str()
                .unwrap()
                .as_ptr(),
            owner
        );
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        let cancel = text(&out, "Cancel");
        click(&mut a, &ctx, &mut t, cancel);
        assert!(!a.show_preferences_dialog);
        assert!(a.document_session.preferences_draft.is_none());
        assert_eq!(a.scene.mesh, before);
        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
        assert_eq!(a.document_session.last_autosave, timer);
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        a.begin_preferences();
        t += 0.1;
        let out = frame(&mut a, &ctx, t, vec![]);
        click(&mut a, &ctx, &mut t, text(&out, "Enable recovery copies"));
        a.document_session
            .preferences_draft
            .as_mut()
            .unwrap()
            .preferences
            .undo_steps = 0;
        t += 0.1;
        let out = frame(&mut a, &ctx, t, vec![]);
        click(&mut a, &ctx, &mut t, text(&out, "OK"));
        assert!(!a.show_preferences_dialog);
        assert!(!a.document_session.preferences.autosave);
        assert_eq!(a.document_session.preferences.undo_steps, 0);
        assert_ne!(a.document_session.last_autosave, timer);
        let accepted = std::fs::read(&file).unwrap();
        assert_ne!(accepted, bytes);
        a.begin_preferences();
        a.document_session
            .preferences_draft
            .as_mut()
            .unwrap()
            .preferences
            .look_sensitivity = 99;
        t += 0.1;
        frame(
            &mut a,
            &ctx,
            t,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }],
        );
        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
        t += 0.1;
        frame(
            &mut a,
            &ctx,
            t,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!a.show_preferences_dialog);
        assert_eq!(a.scene.mesh, before);
        assert_eq!(std::fs::read(&file).unwrap(), accepted);
        let mut restarted = IngeTrazoApp::new();
        restarted.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        assert!(!restarted.document_session.preferences.autosave);
        assert_eq!(restarted.document_session.preferences.undo_steps, 0);
        assert_eq!(
            restarted.document_session.preferences.extra["proof/opaque"],
            serde_json::json!("owned-unknown")
        );
        let mut p = Preferences::default();
        p.undo_steps = 6000;
        p.clamp();
        assert_eq!(p.undo_steps, 5000);
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn unit_label(code: &str) -> &str {
    match code {
        "m" => "Metres (m)",
        "cm" => "Centimetres (cm)",
        "mm" => "Millimetres (mm)",
        "in" => "Decimal inches (in)",
        "ft" => "Decimal feet (ft)",
        "ft-in" => "Feet and inches",
        "in-frac" => "Fractional inches",
        "ft-in-frac" => "Fractional feet and inches",
        _ => code,
    }
}
