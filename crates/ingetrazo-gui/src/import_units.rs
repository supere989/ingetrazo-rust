//! Cold source import-unit confirmation. No document changes before acceptance.
use crate::IngeTrazoApp;
use std::path::{Path, PathBuf};
pub const OBJ_CODES: [&str; 5] = ["m", "cm", "mm", "in", "ft"];
pub const DXF_CODES: [&str; 5] = ["mm", "cm", "m", "in", "ft"];
pub fn unit_label(code: &str) -> &str {
    match code {
        "m" => "Metres",
        "cm" => "Centimetres",
        "mm" => "Millimetres",
        "in" => "Inches",
        "ft" => "Feet",
        _ => code,
    }
}
pub fn factor(code: &str) -> Option<f64> {
    match code {
        "m" => Some(1.),
        "cm" => Some(0.01),
        "mm" => Some(0.001),
        "in" => Some(0.0254),
        "ft" => Some(0.3048),
        _ => None,
    }
}
pub struct Confirmed {
    pub factor: f64,
    pub path: PathBuf,
    kind: String,
    bytes: Vec<u8>,
}
pub struct Dialog {
    pub path: PathBuf,
    pub code: String,
    pub kind: String,
    bytes: Vec<u8>,
}
pub fn preference<'a>(
    p: &'a crate::session_state::Preferences,
    key: &str,
    default: &'a str,
) -> &'a str {
    p.extra
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| factor(s).is_some())
        .unwrap_or(default)
}
fn obj_suggestion(bytes: &[u8]) -> &'static str {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for line in bytes.split(|b| *b == b'\n') {
        if !line.starts_with(b"v ") {
            continue;
        }
        let Ok(line) = std::str::from_utf8(line) else {
            return "m";
        };
        let mut fields = line.split_whitespace().skip(1);
        let (Some(x), Some(y), Some(z)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        let values = [x, y, z].map(str::parse::<f64>);
        let [Ok(x), Ok(y), Ok(z)] = values else {
            return "m";
        };
        let values = [x, y, z];
        for k in 0..3 {
            lo[k] = lo[k].min(values[k]);
            hi[k] = hi[k].max(values[k]);
        }
    }
    let span = if hi[0] > lo[0] {
        (0..3).map(|k| hi[k] - lo[k]).fold(0., f64::max)
    } else {
        0.
    };
    if span > 20000. {
        "mm"
    } else if span > 200. {
        "cm"
    } else {
        "m"
    }
}
fn pairs(bytes: &[u8]) -> Result<Vec<(i32, String)>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "DXF is not UTF-8")?;
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() % 2 != 0 {
        return Err("Incomplete DXF pair".into());
    }
    lines
        .chunks_exact(2)
        .map(|p| {
            Ok((
                p[0].trim().parse::<i32>().map_err(|_| "Invalid DXF code")?,
                p[1].trim().into(),
            ))
        })
        .collect()
}
fn dxf_suggestion(bytes: &[u8]) -> Result<Option<&'static str>, String> {
    let pairs = pairs(bytes)?;
    let mut header = None;
    for w in pairs.windows(2) {
        if w[0].1 == "$INSUNITS" {
            header = match w[1].1.parse::<i32>().unwrap_or(0) {
                1 => Some(0.0254),
                2 => Some(0.3048),
                4 => Some(0.001),
                5 => Some(0.01),
                6 => Some(1.),
                7 => Some(1000.),
                10 => Some(0.9144),
                _ => None,
            };
        }
    }
    let mut sizes = Vec::new();
    let mut i = 0;
    let mut entities = false;
    while i < pairs.len() {
        if pairs[i].1 == "ENTITIES" && pairs[i].0 == 2 {
            entities = true;
            i += 1;
            continue;
        }
        if pairs[i].1 == "ENDSEC" {
            entities = false;
        }
        if !entities || pairs[i].0 != 0 {
            i += 1;
            continue;
        }
        if sizes.len() >= 4000 {
            break;
        }
        let kind = &pairs[i].1;
        i += 1;
        let start = i;
        while i < pairs.len() && pairs[i].0 != 0 {
            i += 1;
        }
        let fields = &pairs[start..i];
        let n = |code: i32| {
            fields
                .iter()
                .find(|(c, _)| *c == code)
                .and_then(|(_, s)| s.parse::<f64>().ok())
                .unwrap_or(0.)
        };
        match kind.as_str() {
            "LINE" => {
                sizes.push(glam::DVec3::new(n(11) - n(10), n(21) - n(20), n(31) - n(30)).length())
            }
            "CIRCLE" | "ARC" => sizes.push(2. * n(40)),
            "LWPOLYLINE" => {
                let mut pts = Vec::new();
                for (code, value) in fields {
                    if *code == 10 {
                        pts.push([value.parse::<f64>().unwrap_or(0.), 0.]);
                    } else if *code == 20 {
                        if let Some(last) = pts.last_mut() {
                            last[1] = value.parse::<f64>().unwrap_or(0.);
                        }
                    }
                }
                for p in pts.windows(2) {
                    sizes.push(((p[1][0] - p[0][0]).powi(2) + (p[1][1] - p[0][1]).powi(2)).sqrt());
                }
            }
            _ => {}
        }
    }
    sizes.retain(|s| *s > 1e-9);
    sizes.sort_by(f64::total_cmp);
    let chosen = if sizes.is_empty() {
        header
    } else {
        let median = sizes[sizes.len() / 2];
        let sane = |f: f64| (0.05..=80.).contains(&(median * f));
        header
            .filter(|f| sane(*f))
            .or_else(|| {
                [1., 0.01, 0.001, 0.0254, 0.3048]
                    .into_iter()
                    .find(|f| sane(*f))
            })
            .or(header)
    };
    Ok(chosen.and_then(|f| OBJ_CODES.into_iter().find(|code| factor(code) == Some(f))))
}
impl IngeTrazoApp {
    pub fn begin_unit_import(&mut self, path: &Path) -> Result<(), String> {
        if !self.component_edit_stack.is_empty() {
            return Err("Finish or cancel the component edit before importing".into());
        }
        let kind = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["obj", "dxf"].contains(&kind.as_str()) {
            return Err("Unit confirmation supports OBJ and DXF".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let code = if kind == "obj" {
            let guessed = obj_suggestion(&bytes);
            if guessed == "m" {
                preference(&self.document_session.preferences, "import/obj_unit", "m")
            } else {
                guessed
            }
        } else {
            dxf_suggestion(&bytes)?.unwrap_or_else(|| {
                preference(&self.document_session.preferences, "import/dxf_unit", "mm")
            })
        }
        .into();
        self.document_session.unit_import = Some(Dialog {
            path: path.into(),
            kind,
            code,
            bytes,
        });
        Ok(())
    }
    pub fn confirm_unit_import(&mut self) -> Result<Option<Confirmed>, String> {
        if !self.component_edit_stack.is_empty() {
            return Err("Finish or cancel the component edit before importing".into());
        }
        let Some(dialog) = self.document_session.unit_import.as_ref() else {
            return Ok(None);
        };
        let f = factor(&dialog.code).ok_or("Unknown import unit")?;
        let dialog = self.document_session.unit_import.take().unwrap();
        // Original accepted QInputDialog persists the answer before file parsing.
        self.document_session.preferences.extra.insert(
            format!("import/{}_unit", dialog.kind),
            serde_json::json!(dialog.code),
        );
        self.persist_preferences();
        Ok(Some(Confirmed {
            factor: f,
            path: dialog.path,
            kind: dialog.kind,
            bytes: dialog.bytes,
        }))
    }
    pub fn commit_unit_import(&mut self, confirmed: Confirmed) -> Result<(), String> {
        if !self.component_edit_stack.is_empty() {
            return Err("Finish or cancel the component edit before importing".into());
        }
        let mut candidate = self.scene.clone();
        let mut reader = std::io::Cursor::new(confirmed.bytes);
        if confirmed.kind == "obj" {
            ingetrazo_formats::obj::import_obj_with_units(
                &mut reader,
                &mut candidate,
                confirmed.factor,
            )
            .map_err(|e| e.to_string())?;
        } else {
            ingetrazo_formats::dxf::import_dxf_with_units(
                &mut reader,
                &mut candidate,
                confirmed.factor,
            )
            .map_err(|e| e.to_string())?;
        }
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Import history allocation refused")?;
        self.save_undo_state();
        self.scene = candidate;
        self.bump_model_revision();
        self.status_message = format!("Imported {}", confirmed.path.display());
        Ok(())
    }
    pub fn accept_unit_import(&mut self) -> Result<(), String> {
        if let Some(confirmed) = self.confirm_unit_import()? {
            self.commit_unit_import(confirmed)?;
        }
        Ok(())
    }
    pub fn cancel_unit_import(&mut self) {
        self.document_session.unit_import = None;
    }
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(d) = app.document_session.unit_import.as_mut() else {
        return;
    };
    let mut accept = false;
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new("import-unit-confirmation")).show(ctx, |ui| {
        ui.heading(format!("Import {}", d.kind.to_uppercase()));
        ui.label(d.path.display().to_string());
        ui.label("What unit is this model in?");
        egui::ComboBox::from_id_salt("import-unit-choice")
            .selected_text(unit_label(&d.code))
            .show_ui(ui, |ui| {
                for code in if d.kind == "obj" {
                    OBJ_CODES
                } else {
                    DXF_CODES
                } {
                    ui.selectable_value(&mut d.code, code.into(), unit_label(code));
                }
            });
        ui.horizontal(|ui| {
            accept = ui.button("Import").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if cancel || modal.should_close() {
        app.cancel_unit_import();
    } else if accept {
        if let Err(e) = app.accept_unit_import() {
            app.status_message = e;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        a: &mut IngeTrazoApp,
        c: &egui::Context,
        t: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let modifiers = events
            .iter()
            .rev()
            .find_map(|e| match e {
                egui::Event::Key { modifiers, .. }
                | egui::Event::PointerButton { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        c.run(
            egui::RawInput {
                modifiers,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100., 800.),
                )),
                time: Some(t),
                events,
                ..Default::default()
            },
            |c| a.update_ui(c),
        )
    }
    fn text(o: &egui::FullOutput, label: &str) -> egui::Pos2 {
        o.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == label).then_some(t.pos + t.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("missing import text {label}"))
    }
    fn click(a: &mut IngeTrazoApp, c: &egui::Context, t: &mut f64, pos: egui::Pos2) {
        *t += 0.1;
        frame(a, c, *t, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            *t += 0.1;
            frame(
                a,
                c,
                *t,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
    }
    #[test]
    fn actual_import_modal_cancel_accept_and_background_history_ownership() {
        let root = std::env::temp_dir().join(format!("import-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("triangle.obj");
        std::fs::write(&path, "v 0 0 0\nv 100 0 0\nv 0 100 0\nf 1 2 3\n").unwrap();
        let mut a = IngeTrazoApp::new();
        a.set_language("en");
        a.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        a.persist_preferences();
        a.save_undo_state();
        a.scene.mesh.add_vertex(glam::DVec3::new(-1., 0., 0.));
        let before = a.scene.clone();
        let depth = a.undo_stack.len();
        let settings = std::fs::read(root.join("config/settings.json")).unwrap();
        let c = egui::Context::default();
        let mut t = 0.;
        a.begin_unit_import(&path).unwrap();
        let bytes = a
            .document_session
            .unit_import
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr();
        frame(&mut a, &c, t, vec![]);
        t += 0.1;
        let out = frame(
            &mut a,
            &c,
            t,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            }],
        );
        assert_eq!(a.scene, before);
        assert_eq!(a.undo_stack.len(), depth);
        assert_eq!(
            a.document_session
                .unit_import
                .as_ref()
                .unwrap()
                .bytes
                .as_ptr(),
            bytes
        );
        click(&mut a, &c, &mut t, text(&out, "Cancel"));
        assert_eq!(a.scene, before);
        assert_eq!(
            std::fs::read(root.join("config/settings.json")).unwrap(),
            settings
        );
        a.begin_unit_import(&path).unwrap();
        t += 0.1;
        frame(&mut a, &c, t, vec![]);
        t += 0.1;
        let out = frame(&mut a, &c, t, vec![]);
        click(&mut a, &c, &mut t, text(&out, "Metres"));
        t += 0.1;
        let out = frame(&mut a, &c, t, vec![]);
        click(&mut a, &c, &mut t, text(&out, "Centimetres"));
        t += 0.1;
        let out = frame(&mut a, &c, t, vec![]);
        click(&mut a, &c, &mut t, text(&out, "Import"));
        assert!(a.document_session.unit_import.is_none());
        assert_eq!(a.undo_stack.len(), depth + 1);
        assert_eq!(a.scene.mesh.faces.len(), 1);
        assert!(a
            .scene
            .mesh
            .vertices
            .iter()
            .any(|v| v.position == glam::DVec3::X));
        a.undo();
        assert_eq!(a.scene.mesh, before.mesh);
        a.redo();
        assert_eq!(a.scene.mesh.faces.len(), 1);
        let mut restart = IngeTrazoApp::new();
        restart.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        restart.begin_unit_import(&path).unwrap();
        assert_eq!(
            restart.document_session.unit_import.as_ref().unwrap().code,
            "cm"
        );
        restart.cancel_unit_import();
        std::fs::remove_dir_all(root).unwrap();
    }
}
