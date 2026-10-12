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
    pub fn choose_obj_import(&mut self) { self.choose_unit_import("obj"); }
    pub fn choose_dxf_import(&mut self) { self.choose_unit_import("dxf"); }
    fn choose_unit_import(&mut self, kind: &str) {
        let title = ingetrazo_core::i18n::tr(if kind == "obj" { "Import OBJ" } else { "Import DXF" });
        let filter = if kind == "obj" { "*.obj|OBJ" } else { "*.dxf|DXF" };
        if let Some(path) = crate::dialogs::pick_open_file(&title, filter) {
            if let Err(error) = self.begin_unit_import(&path) { self.status_message = error; }
        }
    }
    pub fn begin_unit_import(&mut self, path: &Path) -> Result<(), String> {
        let kind = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["obj", "dxf"].contains(&kind.as_str()) {
            return Err("Unit confirmation supports OBJ and DXF".into());
        }
        // File-owned immutable confirmation bytes, bounded before allocation.
        // Parsing still consumes this same buffer exactly once after acceptance.
        const MAX_BYTES: u64 = 512 * 1024 * 1024;
        let mut file=std::fs::File::open(path).map_err(|e|e.to_string())?;
        let size=file.metadata().map_err(|e|e.to_string())?.len();
        if size>MAX_BYTES { return Err("Import file exceeds 512 MiB admission".into()); }
        let mut bytes=Vec::new();
        bytes.try_reserve_exact(size as usize).map_err(|_|"Import buffer allocation refused")?;
        use std::io::Read;
        file.by_ref().take(MAX_BYTES+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
        if bytes.len() as u64>MAX_BYTES { return Err("Import file exceeds 512 MiB admission".into()); }
        self.begin_unit_import_bytes(path, &kind, bytes)
    }
    pub(crate) fn begin_unit_import_bytes(&mut self, path: &Path, kind: &str, bytes: Vec<u8>) -> Result<(), String> {
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
            kind: kind.into(),
            code,
            bytes,
        });
        Ok(())
    }
    pub fn confirm_unit_import(&mut self) -> Result<Option<Confirmed>, String> {
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
        let mut reader = std::io::Cursor::new(confirmed.bytes);
        enum Plan {
            Obj(ingetrazo_formats::obj::ObjPlan),
            Dxf(ingetrazo_formats::dxf::DxfPlan),
        }
        // Parse exactly once before choosing/cloning a destination owner.
        let plan = if confirmed.kind == "obj" {
            Plan::Obj(
                ingetrazo_formats::obj::parse_obj_import(
                    &mut reader,
                    &self.scene,
                    confirmed.factor,
                    &confirmed.path,
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            Plan::Dxf(
                ingetrazo_formats::dxf::parse_dxf_with_units(&mut reader, confirmed.factor)
                    .map_err(|e| e.to_string())?,
            )
        };
        let root = match &plan {
            Plan::Obj(plan) => plan.targets_document_root(),
            Plan::Dxf(_) => true,
        };
        let parked = root && !self.component_edit_stack.is_empty();
        let owner = if parked {
            &self.component_edit_stack[0].original_scene
        } else {
            &self.scene
        };
        let mut candidate = owner.clone();
        candidate.layers = self.scene.layers.clone();
        if parked {
            candidate.embedded_textures = self.scene.embedded_textures.clone();
        }
        let stem = confirmed
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model");
        match plan {
            Plan::Obj(plan) => plan
                .emit_source(&mut candidate, stem)
                .map_err(|e| e.to_string())?,
            Plan::Dxf(plan) => plan
                .emit_source(&mut candidate, stem)
                .map_err(|e| e.to_string())?,
        }
        if parked {
            self.publish_root_import(candidate)?;
        } else {
            self.undo_stack
                .try_reserve(1)
                .map_err(|_| "Import history allocation refused")?;
            self.save_undo_state();
            self.scene = candidate;
            if !root {
                if let Some(session) = self.component_edit_stack.last_mut() {
                    session.note_source_mesh_import();
                }
            }
            self.bump_model_revision();
        }
        if confirmed.kind == "dxf" {
            let fitted=self.source_zoom_extents_applied();
            // Source camera fit is global presentation, outside SnapshotImport.
            // Both nested exits keep this accepted fit (same owner as surveys).
            if fitted {for session in &mut self.component_edit_stack {session.note_survey_camera();}}
        }
        self.status_message = format!("Imported {}", confirmed.path.display());
        Ok(())
    }
    pub(crate) fn publish_root_import(&mut self,candidate:ingetrazo_core::scene::Scene)->Result<(),String>{self.publish_root_import_with_mesh(candidate,None)}
    pub(crate) fn publish_root_import_with_mesh(
        &mut self,
        candidate: ingetrazo_core::scene::Scene,
        active_mesh:Option<ingetrazo_core::mesh::Mesh>,
    ) -> Result<(), String> {
        self.publish_root_import_policy(candidate,active_mesh,false)
    }
    pub(crate) fn publish_root_import_policy(&mut self,candidate:ingetrazo_core::scene::Scene,active_mesh:Option<ingetrazo_core::mesh::Mesh>,skp:bool)->Result<(),String>{
        // Imported groups are document-global in the original; active Scene.groups
        // is only the native local child view. Prepare every checkpoint first.
        let root = &self.component_edit_stack[0];
        let old_count = root.original_scene.groups.len();
        if candidate.groups.len() < old_count {
            return Err("Import did not append root groups".into());
        }
        let new_shares = candidate.mesh_shares.iter().filter(|(id,_)| !root.original_scene.mesh_shares.contains_key(*id)).map(|(id,mesh)| (*id,mesh.clone())).collect::<Vec<_>>();
        let new_definitions = candidate.component_definitions.iter().filter(|(id,_)| !root.original_scene.component_definitions.contains_key(*id)).map(|(id,definition)| (*id,definition.clone())).collect::<Vec<_>>();
        let mut before = root.original_scene.clone();
        if active_mesh.is_some() || skp{before.history_file_import=Some(ingetrazo_core::scene::FileImportHistory{context:self.scene.history_context.clone(),mesh:self.scene.mesh.clone()});}
        // Original SnapshotImport does not own adoption of geographic datum.
        before.geo_datum=candidate.geo_datum.clone();
        before.history_skp_import=skp;
        if skp{crate::skp_import::persistent_registry(&candidate,&mut before);before.dimensions=self.scene.dimensions.clone();before.text_labels=self.scene.text_labels.clone();before.saved_views=self.scene.saved_views.clone();before.history_views.0=Some(ingetrazo_core::saved_views::ViewMembershipDelta::between(&before.saved_views,&candidate.saved_views));}
        before.layers = self.scene.layers.clone();
        if !skp{before.embedded_textures = self.scene.embedded_textures.clone();}
        before.history_layers.0 = Some(ingetrazo_core::layers::LayerDelta::between(
            &before.layers,
            &candidate.layers,
        ));
        crate::document_presentation::preserve(&self.scene, &mut before);
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Import history allocation refused")?;
        let cap = self.document_session.preferences.undo_steps;
        let evicted = if cap > 0 {
            (self.undo_stack.len() + 1).saturating_sub(cap)
        } else {
            0
        };
        struct Checkpoint {
            cancel_groups: Option<Vec<ingetrazo_core::groups::Group>>,
            skp_metadata:Option<(std::collections::HashMap<String,ingetrazo_core::materials::Material>,Vec<ingetrazo_core::dimensions::Dimension>,Vec<ingetrazo_core::textlabel::TextLabel>,Vec<ingetrazo_core::saved_views::SavedView>,serde_json::Value)>,
            original_layers: Vec<ingetrazo_core::layers::Layer>,
            cancel_layers: Vec<ingetrazo_core::layers::Layer>,
            undo: Vec<ingetrazo_core::scene::Scene>,
            original_assets: std::collections::HashMap<String, Vec<u8>>,
            cancel_assets: std::collections::HashMap<String, Vec<u8>>,
            original_shares: Vec<(uuid::Uuid,ingetrazo_core::mesh::Mesh)>,
            cancel_shares: Vec<(uuid::Uuid,ingetrazo_core::mesh::Mesh)>,
            original_definitions: Vec<(uuid::Uuid,ingetrazo_core::components::ComponentDefinition)>,
            cancel_definitions: Vec<(uuid::Uuid,ingetrazo_core::components::ComponentDefinition)>,
        }
        let mut checkpoints = Vec::new();
        checkpoints
            .try_reserve(self.component_edit_stack.len())
            .map_err(|_| "Import checkpoint allocation refused")?;
        for (index, session) in self.component_edit_stack.iter().enumerate() {
            let mut path = session.original_scene.history_context.clone();
            path.push(session.group_id);
            let mut checkpoint_undo = Vec::new();
            checkpoint_undo
                .try_reserve(self.undo_stack.len() + 1 - evicted)
                .map_err(|_| "Import checkpoint history allocation refused")?;
            checkpoint_undo.extend(
                self.undo_stack
                    .iter()
                    .chain(std::iter::once(&before))
                    .skip(evicted)
                    .filter(|entry| !entry.history_context.starts_with(&path))
                    .cloned(),
            );
            let cancel_groups = if index == 0 {
                let mut groups = session.cancel_scene.groups.clone();
                groups
                    .try_reserve(candidate.groups.len() - old_count)
                    .map_err(|_| "Import group allocation refused")?;
                groups.extend(candidate.groups[old_count..].iter().cloned());
                Some(groups)
            } else {
                None
            };
            checkpoints.push(Checkpoint {
                cancel_groups,
                skp_metadata:skp.then(||(candidate.materials.clone(),candidate.dimensions.clone(),candidate.text_labels.clone(),candidate.saved_views.clone(),candidate.native_document.get("back_face_color").cloned().unwrap_or_default())),
                original_layers: candidate.layers.clone(),
                cancel_layers: candidate.layers.clone(),
                undo: checkpoint_undo,
                original_assets: candidate.embedded_textures.clone(),
                cancel_assets: candidate.embedded_textures.clone(),
                original_shares: if index==0 {Vec::new()}else{new_shares.clone()},
                cancel_shares: new_shares.clone(),
                original_definitions: if index==0 {Vec::new()}else{new_definitions.clone()},
                cancel_definitions: new_definitions.clone(),
            });
        }
        let live_skp=skp.then(||(candidate.materials.clone(),candidate.dimensions.clone(),candidate.text_labels.clone(),candidate.saved_views.clone(),candidate.native_document.get("back_face_color").cloned().unwrap_or_default()));
        let live_layers = candidate.layers.clone();
        let live_assets = candidate.embedded_textures.clone();
        // Reserve destination registry slots without replacing old prototype owners.
        for session in &mut self.component_edit_stack {
            for scene in [&mut session.original_scene,&mut session.cancel_scene] {
                scene.mesh_shares.try_reserve(new_shares.len()).map_err(|_| "Import shared mesh registry allocation refused")?;
                scene.component_definitions.try_reserve(new_definitions.len()).map_err(|_| "Import component registry allocation refused")?;
            }
        }
        self.scene.mesh_shares.try_reserve(new_shares.len()).map_err(|_| "Import live mesh registry allocation refused")?;
        self.scene.component_definitions.try_reserve(new_definitions.len()).map_err(|_| "Import live component registry allocation refused")?;
        // No fallible parsing/emission/preparation remains beyond this point.
        self.component_edit_stack[0].original_scene = candidate;
        for (session, checkpoint) in self.component_edit_stack.iter_mut().zip(checkpoints) {
            if let Some(groups) = checkpoint.cancel_groups {
                session.cancel_scene.groups = groups;
            }
            if let Some((materials,dimensions,texts,views,back))=checkpoint.skp_metadata {
                session.original_scene.skp_registry_persistent=true;session.cancel_scene.skp_registry_persistent=true;
                session.original_scene.materials=materials.clone();session.cancel_scene.materials=materials;
                session.original_scene.dimensions=dimensions.clone();session.cancel_scene.dimensions=dimensions;
                session.original_scene.text_labels=texts.clone();session.cancel_scene.text_labels=texts;
                session.original_scene.saved_views=views.clone();session.cancel_scene.saved_views=views;
                session.original_scene.native_document["back_face_color"]=back.clone();session.cancel_scene.native_document["back_face_color"]=back;
            }
            session.original_scene.layers = checkpoint.original_layers;
            session.cancel_scene.layers = checkpoint.cancel_layers;
            session.original_scene.embedded_textures = checkpoint.original_assets;
            session.cancel_scene.embedded_textures = checkpoint.cancel_assets;
            session.original_scene.mesh_shares.extend(checkpoint.original_shares);
            session.cancel_scene.mesh_shares.extend(checkpoint.cancel_shares);
            session.original_scene.component_definitions.extend(checkpoint.original_definitions);
            session.cancel_scene.component_definitions.extend(checkpoint.cancel_definitions);
            session.original_undo = checkpoint.undo;
            session.original_redo.clear();
        }
        self.scene.skp_registry_persistent=self.component_edit_stack[0].original_scene.skp_registry_persistent;
        self.scene.geo_datum=self.component_edit_stack[0].original_scene.geo_datum.clone();
        if let Some(mesh)=active_mesh{self.scene.mesh=mesh;if let Some(session)=self.component_edit_stack.last_mut(){session.note_source_mesh_import();}}
        if let Some((materials,dimensions,texts,views,back))=live_skp {
            self.scene.materials=materials;self.scene.dimensions=dimensions;self.scene.text_labels=texts;self.scene.saved_views=views;self.scene.native_document["back_face_color"]=back;
        }
        self.scene.layers = live_layers;
        self.scene.embedded_textures = live_assets;
        self.scene.mesh_shares.extend(new_shares);
        self.scene.component_definitions.extend(new_definitions);
        self.push_prepared_undo_snapshot(before);
        self.bump_model_revision();
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
        let choice=egui::ComboBox::from_id_salt("import-unit-choice")
            .selected_text(unit_label(&d.code))
            .show_ui(ui, |ui| {
                for code in if d.kind == "obj" {
                    OBJ_CODES
                } else {
                    DXF_CODES
                } {
                    let response=ui.selectable_value(&mut d.code, code.into(), unit_label(code));
                    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ctx.data_mut(|data|data.insert_temp(egui::Id::new(format!("import-unit-option-{code}")),response.rect));}
                }
            });
        if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ctx.data_mut(|data|data.insert_temp(egui::Id::new("import-unit-combo"),choice.response.rect));}
        ui.horizontal(|ui| {
            let import=ui.button("Import");let dismiss=ui.button("Cancel");
            if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ctx.data_mut(|data|{data.insert_temp(egui::Id::new("import-unit-accept"),import.rect);data.insert_temp(egui::Id::new("import-unit-cancel"),dismiss.rect);});}
            accept=import.clicked();cancel=dismiss.clicked();
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
