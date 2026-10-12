//! Cold survey CSV import and source datum questions; no mutation while pending.
use crate::IngeTrazoApp;
use glam::DVec3;
use ingetrazo_core::georef::{self, SceneDatum, SurveyPointRow};
use ingetrazo_core::i18n::tr;
use std::path::Path;
pub struct Dialog {
    pub rows: Vec<SurveyPointRow>,
    pub zone: i32,
    pub northern: bool,
    pub hemisphere: bool,
}
impl IngeTrazoApp {
    pub fn survey_point_count(&self) -> String {
        tr("{n} points loaded").replace("{n}", &self.scene.geo_points.len().to_string())
    }
    pub fn begin_survey_import(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path)
            .map_err(|_| "Could not read survey points (expected P,N,E,Z,desc in UTM)")?;
        let rows = georef::parse_points_csv(&String::from_utf8_lossy(&bytes))?;
        if rows
            .iter()
            .any(|r| !r.north.is_finite() || !r.east.is_finite() || !r.z.is_finite())
        {
            return Err("Survey contains nonfinite coordinates".into());
        }
        if self.scene.geo_datum.is_some() {
            self.commit_survey_points(rows, None)?;
        } else {
            self.document_session.survey_import = Some(Dialog {
                rows,
                zone: 18,
                northern: false,
                hemisphere: false,
            });
        }
        Ok(())
    }
    pub fn cancel_survey_import(&mut self) {
        self.document_session.survey_import = None;
    }
    pub fn accept_survey_zone(&mut self) -> Result<(), String> {
        let dialog = self
            .document_session
            .survey_import
            .as_mut()
            .ok_or("No pending survey import")?;
        if !(1..=60).contains(&dialog.zone) {
            return Err("UTM zone must be between 1 and 60".into());
        }
        dialog.hemisphere = true;
        Ok(())
    }
    pub fn accept_survey_import(&mut self) -> Result<(), String> {
        let dialog = self
            .document_session
            .survey_import
            .as_ref()
            .ok_or("No pending survey import")?;
        if !dialog.hemisphere || !(1..=60).contains(&dialog.zone) {
            return Err("Survey datum questions are incomplete".into());
        }
        let datum = georef::datum_for_rows(&dialog.rows, dialog.zone, dialog.northern);
        let dialog = self.document_session.survey_import.take().unwrap();
        self.commit_survey_points(dialog.rows, Some(datum))
    }
    fn commit_survey_points(
        &mut self,
        rows: Vec<SurveyPointRow>,
        datum: Option<SceneDatum>,
    ) -> Result<(), String> {
        let frame = datum
            .as_ref()
            .or(self.scene.geo_datum.as_ref())
            .ok_or("Survey datum is missing")?;
        let points: Vec<_> = rows
            .into_iter()
            .map(|r| {
                georef::GeoPoint::new(
                    frame
                        .utm_to_local(r.east, r.north, r.z)
                        .as_vec3()
                        .as_dvec3(),
                    r.name,
                    r.desc,
                )
            })
            .collect();
        if points.is_empty() || points.iter().any(|p| !p.position.is_finite()) {
            return Err(
                "Survey coordinates cannot be represented in the source local frame".into(),
            );
        }
        let mut low = DVec3::splat(f64::INFINITY);
        let mut high = -low;
        for p in &points {
            low = low.min(p.position);
            high = high.max(p.position);
        }
        low = (low - DVec3::splat(5.)).as_vec3().as_dvec3();
        high = (high + DVec3::splat(5.)).as_vec3().as_dvec3();
        let mut camera = self.camera.clone();
        camera.target = ((low + high) * 0.5).as_vec3().as_dvec3();
        let delta = (high.as_vec3() - low.as_vec3()).abs();
        let max = delta.max_element();
        let length = if max == 0. {
            0.
        } else {
            let q = delta / max;
            max * ((q.x * q.x + q.y * q.y) + q.z * q.z).sqrt()
        };
        camera.distance = (f64::from(length).max(0.02) * 1.3
            / (2. * (camera.fov_deg.to_radians() * 0.5).tan()))
        .max(0.02);
        if !camera.target.is_finite() || !camera.distance.is_finite() {
            return Err("Survey extent is not finite".into());
        }
        self.scene
            .geo_points
            .try_reserve(points.len())
            .map_err(|_| "Could not reserve survey points")?;
        self.save_undo_state();
        self.undo_stack.last_mut().unwrap().history_survey = true;
        let count = points.len();
        if let Some(datum) = datum {
            self.scene.geo_datum = Some(datum);
        }
        self.scene.geo_points.extend(points);
        for session in &mut self.component_edit_stack {
            session.note_survey_camera();
        }
        self.camera = camera.clone();
        self.scene.camera = camera;
        self.scene.bump_version();
        self.bump_model_revision();
        self.status_message = tr("{n} survey points imported").replace("{n}", &count.to_string());
        Ok(())
    }
    pub fn clear_survey_points(&mut self) -> Result<bool, String> {
        if self.scene.geo_points.is_empty() {
            return Ok(false);
        }
        self.save_undo_state();
        self.undo_stack.last_mut().unwrap().history_survey = true;
        self.scene.geo_points.clear();
        self.scene.bump_version();
        self.bump_model_revision();
        Ok(true)
    }
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(dialog) = app.document_session.survey_import.as_mut() else {
        return;
    };
    let hemisphere = dialog.hemisphere;
    let result = egui::Modal::new(egui::Id::new("survey-datum-import")).show(ctx, |ui| {
        ui.heading(tr(if hemisphere { "Hemisphere" } else { "UTM zone" }));
        if hemisphere {
            ui.radio_value(&mut dialog.northern, false, tr("South"));
            ui.radio_value(&mut dialog.northern, true, tr("North"));
        } else {
            ui.label(tr("UTM zone of the survey (Peru: 17-19):"));
            ui.add(egui::DragValue::new(&mut dialog.zone).range(1..=60));
        }
        ui.horizontal(|ui| {
            let accept = ui.button(tr("OK")).clicked();
            let cancel = ui.button(tr("Cancel")).clicked();
            (accept, cancel)
        })
        .inner
    });
    if result.inner.1 || result.should_close() {
        app.cancel_survey_import();
    } else if result.inner.0 {
        let value = if hemisphere {
            app.accept_survey_import()
        } else {
            app.accept_survey_zone()
        };
        if let Err(error) = value {
            app.status_message = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        a: &mut IngeTrazoApp,
        c: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        c.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |c| a.update_ui(c),
        )
    }
    fn text(o: &egui::FullOutput, word: &str) -> egui::Pos2 {
        o.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == word).then_some(t.pos + t.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .unwrap()
    }
    fn click(
        a: &mut IngeTrazoApp,
        c: &egui::Context,
        t: &mut f64,
        pos: egui::Pos2,
    ) -> egui::FullOutput {
        let mut output = None;
        for pressed in [true, false] {
            *t += 0.1;
            output = Some(frame(
                a,
                c,
                *t,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            ));
        }
        output.unwrap()
    }
    #[test]
    fn actual_survey_modal_cancel_and_accept_owns_input() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.set_language("en");
        a.show_tray = false;
        a.document_session.show_recovery = false;
        let point = a
            .scene
            .mesh
            .add_vertex(DVec3::new(1. + 2f64.powi(-40), 0., 0.));
        assert_eq!(point, 0);
        let scene = a.scene.clone();
        let camera = a.camera.clone();
        let pending = || Dialog {
            rows: vec![SurveyPointRow {
                name: "A".into(),
                north: 8660000.,
                east: 300000.,
                z: 1200.,
                desc: "control".into(),
            }],
            zone: 18,
            northern: false,
            hemisphere: false,
        };
        a.document_session.survey_import = Some(pending());
        assert!(a.navigation_suspended());
        let c = egui::Context::default();
        let mut t = 0.;
        frame(&mut a, &c, t, vec![]);
        t += 0.1;
        frame(
            &mut a,
            &c,
            t,
            vec![
                egui::Event::Key {
                    key: egui::Key::Delete,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
                egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                },
            ],
        );
        assert_eq!(a.scene.mesh, scene.mesh);
        assert!(a.undo_stack.is_empty());
        assert_eq!(a.camera.target, camera.target);
        t += 0.1;
        frame(
            &mut a,
            &c,
            t,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(a.document_session.survey_import.is_none());
        assert!(a.scene.geo_points.is_empty());
        assert!(a.scene.geo_datum.is_none());
        assert!(a.undo_stack.is_empty());
        a.document_session.survey_import = Some(pending());
        t += 0.1;
        let o = frame(&mut a, &c, t, vec![]);
        let pos = text(&o, "OK");
        click(&mut a, &c, &mut t, pos);
        assert!(
            a.document_session
                .survey_import
                .as_ref()
                .unwrap()
                .hemisphere
        );
        t += 0.1;
        let o = frame(&mut a, &c, t, vec![]);
        let pos = text(&o, "Cancel");
        click(&mut a, &c, &mut t, pos);
        assert!(a.document_session.survey_import.is_none());
        assert!(a.undo_stack.is_empty());
        a.document_session.survey_import = Some(pending());
        t += 0.1;
        let o = frame(&mut a, &c, t, vec![]);
        click(&mut a, &c, &mut t, text(&o, "OK"));
        t += 0.1;
        let o = frame(&mut a, &c, t, vec![]);
        click(&mut a, &c, &mut t, text(&o, "OK"));
        assert!(a.document_session.survey_import.is_none());
        assert_eq!(a.scene.geo_points.len(), 1);
        assert_eq!(a.undo_stack.len(), 1);
        assert_eq!(a.scene.mesh, scene.mesh);
        a.undo();
        assert!(a.scene.geo_points.is_empty());
        assert!(a.scene.geo_datum.is_none());
        a.redo();
        assert_eq!(a.scene.geo_points.len(), 1);
    }
}
