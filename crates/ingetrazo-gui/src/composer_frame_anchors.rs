// SPDX-License-Identifier: GPL-3.0-or-later
//! Derived frame anchors: source world/page pairs, no sheet history command.
use super::IngeTrazoApp;
use crate::composer_frame_projection::Projector;
use glam::DVec3;
use ingetrazo_core::{
    camera::OrbitCamera,
    composition::{SheetItem, ViewFrame},
};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
#[derive(Clone, Debug)]
pub struct SnapPair {
    pub page: [f64; 2],
    pub world: DVec3,
}
#[derive(Clone, Debug, Default)]
pub struct SyncStats {
    pub builds: u64,
    pub hits: u64,
    pub changed: u64,
}
#[derive(Clone, Debug)]
struct Entry {
    document: Uuid,
    revision: u64,
    version: u64,
    frame: ViewFrame,
    camera: OrbitCamera,
    pairs: Arc<[SnapPair]>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Cache {
    stamp: Option<(Uuid, u64, u64, OrbitCamera)>,
    retained_bytes: usize,
    document: Option<Uuid>,
    initialized: Option<Uuid>,
    pub(super) defer_rebuild: bool,
    coalesce: HashMap<(Uuid, usize), (Uuid, &'static str)>,
    pending_edit: Option<(Uuid, Uuid, &'static str, usize, bool)>,
    entries: HashMap<Uuid, Entry>,
    pub stats: SyncStats,
}
#[derive(Clone, Debug)]
pub(super) struct SnapHit {
    pub frame: Uuid,
    pub world: DVec3,
    pub page: [f64; 2],
}
impl IngeTrazoApp {
    pub fn delete_composer_frame(&mut self, sheet: Uuid, frame: Uuid) -> Result<(), String> {
        let s = self
            .scene
            .sheets
            .iter()
            .position(|s| s.id == sheet)
            .ok_or("Missing frame sheet")?;
        let index = self.scene.sheets[s]
            .items
            .iter()
            .position(|i| matches!(i,SheetItem::ViewFrame(f) if f.id==frame))
            .ok_or("Missing frame")?;
        self.enter_composer_sheet(s);
        self.scene.active_sheet_index = Some(s);
        self.finish_composer_edit();
        self.begin_composer_edit();
        self.scene.sheets[s].remove_item(index);
        self.finish_composer_edit();
        // Source unresolved anchors retain their UID/world/paper records unchanged.
        if let Err(error) = self.refresh_composer_frame_anchors() {
            self.status_message = format!("Frame deleted; anchored refresh refused: {error}");
        }
        Ok(())
    }
    pub fn select_composer_frame(&mut self, sheet: Uuid, frame: Uuid) -> Result<(), String> {
        let s = self
            .scene
            .sheets
            .iter()
            .position(|s| s.id == sheet)
            .ok_or("Missing frame sheet")?;
        let index = self.scene.sheets[s]
            .items
            .iter()
            .position(|i| matches!(i,SheetItem::ViewFrame(f) if f.id==frame))
            .ok_or("Missing frame")?;
        self.enter_composer_sheet(s);
        self.scene.active_sheet_index = Some(s);
        self.composer_state.annotation_authoring.selected = None;
        self.composer_state.picture_authoring.selected = None;
        self.composer_state.legend_authoring.selected = None;
        self.scene.sheets[s].selected_item = Some(index);
        Ok(())
    }
    /// Shared View panel controller. Unknown frame metadata is left untouched.
    pub fn edit_composer_frame(
        &mut self,
        sheet: Uuid,
        frame: Uuid,
        fields: Value,
    ) -> Result<bool, String> {
        self.apply_composer_frame_fields(sheet, frame, fields, true)
    }
    pub(super) fn apply_composer_frame_fields(
        &mut self,
        sheet: Uuid,
        frame: Uuid,
        fields: Value,
        finish: bool,
    ) -> Result<bool, String> {
        let s = self
            .scene
            .sheets
            .iter()
            .position(|s| s.id == sheet)
            .ok_or("Missing frame sheet")?;
        let index = self.scene.sheets[s]
            .items
            .iter()
            .position(|i| matches!(i,SheetItem::ViewFrame(f) if f.id==frame))
            .ok_or("Missing frame")?;
        let SheetItem::ViewFrame(before) = &self.scene.sheets[s].items[index] else {
            unreachable!()
        };
        let mut value = serde_json::to_value(before).map_err(|e| e.to_string())?;
        for (key, v) in fields.as_object().ok_or("Frame fields must be an object")? {
            match key.as_str() {
                "x_mm" => value["rect"]["x"] = v.clone(),
                "y_mm" => value["rect"]["y"] = v.clone(),
                "w_mm" => value["rect"]["w"] = v.clone(),
                "h_mm" => value["rect"]["h"] = v.clone(),
                "scale_n" | "view_key" | "style" | "show_title" | "title_text" | "border"
                | "perspective" | "profiles" | "hidden_lines" | "paper_bg" | "rot_deg"
                | "annotations" | "annot_text_mm" | "section_marks" | "km_marks" | "km_step_m"
                | "cam_fov" | "cam_target" | "cam_yaw" | "cam_pitch" | "cam_distance"
                | "cut_fill" | "cut_fill_color" | "cut_hatch_mm" | "pen_cut_mm"
                | "pen_profile_mm" | "pen_edge_mm" => {
                    value[key] = v.clone();
                }
                _ => return Err(format!("Unsupported frame property {key}")),
            }
        }
        let mut candidate: ViewFrame = serde_json::from_value(value).map_err(|e| e.to_string())?;
        // Actual source spin-box admission, limited to authored panel fields.
        let keys = fields.as_object().unwrap();
        if keys.contains_key("km_step_m") {
            candidate.km_step_m = (candidate.km_step_m * 10.).round() / 10.;
            candidate.km_step_m = candidate.km_step_m.clamp(0., 100000.);
        }
        if keys.contains_key("rot_deg") {
            candidate.rot_deg = (candidate.rot_deg * 10.).round() / 10.;
        }
        if keys.contains_key("cam_fov") {
            candidate.cam_fov = candidate.cam_fov.map(|n| n.round().clamp(10., 120.));
        }
        let view_changed = candidate.view_key != before.view_key;
        if view_changed {
            candidate.cam_target = None;
            candidate.cam_yaw = None;
            candidate.cam_pitch = None;
            candidate.cam_distance = None;
        }
        let perspective_changed =
            candidate.perspective != before.perspective || candidate.cam_fov != before.cam_fov;
        let mut seeded = false;
        if perspective_changed {
            candidate.cam_fov = Some(candidate.cam_fov.unwrap_or(45.));
            if candidate.perspective && candidate.cam_distance.is_none() {
                let saved = candidate
                    .view_key
                    .strip_prefix("scene:")
                    .and_then(|name| self.scene.saved_views.iter().find(|v| v.name == name));
                let distance = if let Some(saved) = saved.filter(|v| v.distance != 0.) {
                    saved.distance
                } else {
                    let diag =
                        super::frame_bounds(&self.scene).map_or(0., |(lo, hi)| (hi - lo).length());
                    if diag <= 0. {
                        30.
                    } else {
                        (diag / 2.)
                            / (candidate.cam_fov.unwrap_or(45.).to_radians() / 2.)
                                .tan()
                                .max(1e-6)
                            * 1.15
                    }
                };
                candidate.cam_distance = Some((distance * 10000.).round() / 10000.);
                seeded = true;
            }
        }
        let rotation_changed = candidate.rot_deg != before.rot_deg;
        let signature = if perspective_changed {
            if seeded {
                "perspective_seed"
            } else {
                "perspective"
            }
        } else if rotation_changed {
            "rotation"
        } else if view_changed {
            "properties_camera_reset"
        } else {
            "properties"
        };
        let rebuild = perspective_changed || rotation_changed;

        Projector::new(&self.scene, &self.camera, &candidate)?;
        if !candidate.annot_text_mm.is_finite()
            || candidate.annot_text_mm <= 0.
            || candidate.annot_text_mm > 1000.
            || !candidate.km_step_m.is_finite()
            || candidate.km_step_m < 0.
            || candidate.km_step_m > 1e6
        {
            return Err("Invalid frame annotation settings".into());
        }
        if &candidate == before {
            return Ok(false);
        }
        self.enter_composer_sheet(s);
        self.scene.active_sheet_index = Some(s);
        if finish {
            self.finish_composer_edit();
        }
        let history_len = self
            .composer_state
            .sheet_history
            .get(&sheet)
            .map_or(0, |h| h.0.len());
        let merge = self
            .composer_state
            .frame_anchors
            .coalesce
            .get(&(sheet, history_len))
            == Some(&(frame, signature))
            && history_len > 0;
        self.begin_composer_edit();
        self.scene.sheets[s].items[index] = SheetItem::ViewFrame(candidate);
        self.composer_state.frame_anchors.defer_rebuild = !rebuild;
        self.composer_state.frame_anchors.pending_edit =
            Some((sheet, frame, signature, history_len, merge));
        if rebuild {
            if let Err(error) = self.sync_composer_frame_anchors() {
                self.status_message = format!("Frame edited; anchored refresh refused: {error}");
            }
        }
        if finish {
            self.finish_composer_edit();
        }
        Ok(true)
    }
    pub(super) fn nearest_composer_frame_snap(
        &mut self,
        s: usize,
        point: [f64; 2],
        threshold: f64,
    ) -> Result<Option<SnapHit>, String> {
        let sheet = &self.scene.sheets[s];
        let sid = sheet.id;
        let frames: Vec<_> = sheet
            .items
            .iter()
            .filter_map(|i| match i {
                SheetItem::ViewFrame(f)
                    if sheet
                        .native_items
                        .get(&f.id)
                        .and_then(|v| v.get("hidden"))
                        .and_then(Value::as_bool)
                        != Some(true) =>
                {
                    Some(f.id)
                }
                _ => None,
            })
            .collect();
        let mut best = threshold * threshold;
        let mut hit = None;
        for frame in frames {
            let pairs = self.composer_frame_snap_pairs(sid, frame)?;
            for q in pairs.iter() {
                let dx = q.page[0] - point[0];
                let dy = q.page[1] - point[1];
                let d = dx * dx + dy * dy;
                if d < best {
                    best = d;
                    hit = Some(SnapHit {
                        frame,
                        world: q.world,
                        page: q.page,
                    });
                }
            }
        }
        Ok(hit)
    }

    pub fn composer_frame_snap_pairs(
        &mut self,
        sheet: Uuid,
        frame: Uuid,
    ) -> Result<Arc<[SnapPair]>, String> {
        let s = self
            .scene
            .sheets
            .iter()
            .find(|s| s.id == sheet)
            .ok_or("Missing anchor sheet")?;
        let f = s
            .items
            .iter()
            .find_map(|i| match i {
                SheetItem::ViewFrame(f) if f.id == frame => Some(f.clone()),
                _ => None,
            })
            .ok_or("Missing anchor frame")?;
        let projector = Projector::new(&self.scene, &self.camera, &f)?;
        if let Some(e) = self.composer_state.frame_anchors.entries.get(&frame) {
            if e.document == self.clipboard_document
                && e.revision == self.model_revision
                && e.version == self.scene.version
                && e.frame == f
                && e.camera == *projector.camera()
            {
                self.composer_state.frame_anchors.stats.hits += 1;
                return Ok(e.pairs.clone());
            }
        }
        let saved = f
            .view_key
            .strip_prefix("scene:")
            .and_then(|n| self.scene.saved_views.iter().find(|v| v.name == n));
        let view = ingetrazo_render::paper_scene::PaperSceneView::new(&self.scene, saved);
        let mut edges = Vec::new();
        super::picture_gestures::collect_edges(
            &view,
            &self.scene.mesh,
            glam::DMat4::IDENTITY,
            &mut edges,
        )?;
        super::picture_gestures::collect_groups(
            &view,
            &self.scene.groups,
            glam::DMat4::IDENTITY,
            &mut edges,
        )?;
        let exact = !f.perspective && edges.len() <= 20_000;
        let drawing = if exact {
            Some(self.composer_compute.original_qt_paper_drawing_for_view(
                &view,
                projector.camera(),
                self.clipboard_document,
                self.model_revision,
                &f.view_key,
                false,
                false,
            )?)
        } else {
            None
        };
        let pair_capacity = drawing
            .as_ref()
            .map_or(edges.len(), |d| {
                d.segments
                    .iter()
                    .filter(|e| e.kind != ingetrazo_render::vector_hlr::LineKind::Hidden)
                    .count()
            })
            .checked_mul(3)
            .ok_or("Anchor pair overflow")?;
        let candidate_bytes = pair_capacity
            .checked_mul(std::mem::size_of::<SnapPair>())
            .ok_or("Anchor pair byte overflow")?;
        if candidate_bytes > 32 * 1024 * 1024
            || self
                .composer_state
                .frame_anchors
                .retained_bytes
                .checked_add(candidate_bytes.saturating_mul(2))
                .is_none_or(|n| n > 128 * 1024 * 1024)
        {
            return Err("Anchor pair memory budget exceeded".into());
        }
        let mut pairs = Vec::new();
        pairs
            .try_reserve_exact(pair_capacity)
            .map_err(|_| "Anchor pair allocation failed")?;
        let mut push = |cam: [f64; 3], world: DVec3| -> Result<(), String> {
            let q = projector.camera_to_local(cam)?;
            let page = [f.rect.x + q[0], f.rect.y + q[1]];
            if projector.contains_page(page) {
                if pairs.len() >= 1_000_000 {
                    return Err("Anchor pair budget exceeded".into());
                }
                pairs.push(SnapPair { page, world });
            }
            Ok(())
        };
        if let Some(d) = drawing {
            for e in d
                .segments
                .iter()
                .filter(|e| e.kind != ingetrazo_render::vector_hlr::LineKind::Hidden)
            {
                let a = projector.camera_point(e.a);
                let b = projector.camera_point(e.b);
                push(a, e.a)?;
                push(b, e.b)?;
                push(
                    std::array::from_fn(|i| (a[i] + b[i]) / 2.),
                    (e.a + e.b) / 2.,
                )?;
            }
        } else {
            for part in 0..3 {
                for &(a, b) in &edges {
                    let ac = projector.camera_point(a);
                    let bc = projector.camera_point(b);
                    match part {
                        0 => push(ac, a)?,
                        1 => push(bc, b)?,
                        _ => push(std::array::from_fn(|i| (ac[i] + bc[i]) / 2.), (a + b) / 2.)?,
                    }
                }
            }
        }
        let pairs: Arc<[SnapPair]> = pairs.into();
        let cache = &mut self.composer_state.frame_anchors;
        // Bound cold retained inventory; a frame keeps one immutable pair owner.
        if cache.entries.len() >= 10000 || cache.retained_bytes + candidate_bytes > 64 * 1024 * 1024
        {
            cache.entries.clear();
            cache.retained_bytes = 0;
        }
        if let Some(old) = cache.entries.remove(&frame) {
            cache.retained_bytes -= old.pairs.len() * std::mem::size_of::<SnapPair>();
        }
        cache.retained_bytes += pairs.len() * std::mem::size_of::<SnapPair>();
        cache.entries.insert(
            frame,
            Entry {
                document: self.clipboard_document,
                revision: self.model_revision,
                version: self.scene.version,
                frame: f,
                camera: projector.camera().clone(),
                pairs: pairs.clone(),
            },
        );
        cache.stats.builds += 1;
        Ok(pairs)
    }
    pub fn composer_frame_anchor_stats(&self) -> SyncStats {
        self.composer_state.frame_anchors.stats.clone()
    }
    pub(crate) fn invalidate_composer_frame_entry(&mut self) {
        self.composer_state.frame_anchors.initialized = None;
    }
    pub(crate) fn ensure_composer_frame_anchors(&mut self) -> Result<SyncStats, String> {
        if self.composer_state.frame_anchors.document != Some(self.clipboard_document) {
            let c = &mut self.composer_state.frame_anchors;
            c.document = Some(self.clipboard_document);
            c.initialized = None;
            c.entries.clear();
            c.retained_bytes = 0;
            c.coalesce.clear();
        }
        let Some(s) = self
            .scene
            .active_sheet_index
            .filter(|&s| s < self.scene.sheets.len())
        else {
            return Ok(self.composer_frame_anchor_stats());
        };
        let id = self.scene.sheets[s].id;
        if self.composer_state.frame_anchors.initialized == Some(id) {
            return Ok(self.composer_frame_anchor_stats());
        }
        self.sync_composer_frame_anchors()
    }
    pub(crate) fn composer_frame_rebuild_boundary(&mut self, committed: bool) {
        // Signatures belong to history entries, so Undo/Redo expose the same top
        // command as the source. A new branch retires only its replaced tail.
        if committed {
            let pending = self.composer_state.frame_anchors.pending_edit.take();
            if let Some((sheet, _, _, n, true)) = pending {
                if let Some(h) = self.composer_state.sheet_history.get_mut(&sheet) {
                    if h.0.len() == n + 1 {
                        h.0.pop();
                    }
                }
            }
            if let Some(s) = self
                .scene
                .active_sheet_index
                .filter(|&s| s < self.scene.sheets.len())
            {
                let sheet = self.scene.sheets[s].id;
                let n = self
                    .composer_state
                    .sheet_history
                    .get(&sheet)
                    .map_or(0, |h| h.0.len());
                let signatures = &mut self.composer_state.frame_anchors.coalesce;
                signatures.retain(|&(owner, depth), _| owner != sheet || depth < n);
                if let Some((owner, frame, signature, _, _)) = pending {
                    if owner == sheet && n > 0 {
                        signatures.insert((sheet, n), (frame, signature));
                    }
                }
            }
        }
        if std::mem::take(&mut self.composer_state.frame_anchors.defer_rebuild) {
            return;
        }
        if let Err(error) = self.sync_composer_frame_anchors() {
            self.status_message = format!("Anchored refresh refused: {error}");
        }
    }
    pub fn refresh_composer_frame_anchors(&mut self) -> Result<SyncStats, String> {
        let stamp = (
            self.clipboard_document,
            self.model_revision,
            self.scene.version,
            self.camera.clone(),
        );
        if self.composer_state.frame_anchors.stamp.as_ref() == Some(&stamp) {
            return Ok(self.composer_frame_anchor_stats());
        }
        let result = self.sync_composer_frame_anchors()?;
        self.composer_state.frame_anchors.stamp = Some(stamp);
        Ok(result)
    }
    pub fn sync_composer_frame_anchors(&mut self) -> Result<SyncStats, String> {
        // Only anchored records are copied, never the document/model or whole sheet.
        let mut jobs = Vec::new();
        for (s, sheet) in self
            .scene
            .sheets
            .iter()
            .enumerate()
            .filter(|(s, _)| Some(*s) == self.scene.active_sheet_index)
        {
            for key in ["cotas", "etiquetas", "niveles"] {
                if let Some(rows) = sheet.native_document.get(key).and_then(Value::as_array) {
                    for (i, v) in rows.iter().enumerate() {
                        if v.get("anchor_uid")
                            .and_then(Value::as_str)
                            .is_some_and(|s| !s.is_empty())
                            || key == "etiquetas"
                                && v.get("leaders").and_then(Value::as_array).is_some_and(|a| {
                                    a.iter().any(|v| v.get("anchor_uid").is_some())
                                })
                        {
                            if jobs.len() >= 10000 {
                                return Err("Anchored annotation inventory budget exceeded".into());
                            }
                            jobs.try_reserve(1)
                                .map_err(|_| "Anchor jobs allocation failed")?;
                            jobs.push((s, key, i, v.clone()));
                        }
                    }
                }
            }
        }
        let mut contexts = HashMap::new();
        for (s, key, i, original) in jobs {
            let derived = (|| -> Result<Value, String> {
                let mut candidate = original.clone();
                let primary = (|| -> Result<(), String> {
                    if let Some(uid) = original.get("anchor_uid").and_then(Value::as_str) {
                        if let Some((frame, projector, pairs)) =
                            self.anchor_frame(s, uid, true, &mut contexts)?
                        {
                            if let Some(a) = world(&original, "a_world") {
                                let a = reattach(a, &projector, &pairs, frame.scale_n)?;
                                let ap = projector.page_point(a)?;
                                match key {
                                    "cotas" => {
                                        if let Some(b) = world(&original, "b_world") {
                                            let b = reattach(b, &projector, &pairs, frame.scale_n)?;
                                            let bp = projector.page_point(b)?;
                                            candidate["a_world"] = json!(a.to_array());
                                            candidate["b_world"] = json!(b.to_array());
                                            candidate["x_mm"] = json!(ap[0]);
                                            candidate["y_mm"] = json!(ap[1]);
                                            candidate["dx_mm"] = json!(bp[0] - ap[0]);
                                            candidate["dy_mm"] = json!(bp[1] - ap[1]);
                                            candidate["scale_n"] = json!(frame.scale_n);
                                        }
                                    }
                                    "etiquetas" => {
                                        candidate["a_world"] = json!(a.to_array());
                                        candidate["ax_mm"] =
                                            json!(ap[0] - number(&original, "x_mm"));
                                        candidate["ay_mm"] =
                                            json!(ap[1] - number(&original, "y_mm"));
                                    }
                                    "niveles" => {
                                        candidate["a_world"] = json!(a.to_array());
                                        candidate["x_mm"] =
                                            json!(ap[0] - number(&original, "ax_mm"));
                                        candidate["y_mm"] =
                                            json!(ap[1] - number(&original, "ay_mm"));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Ok(())
                })();
                if primary.is_err() {
                    candidate = original.clone();
                }
                if key == "etiquetas" {
                    if let Some(ls) = candidate.get("leaders").and_then(Value::as_array) {
                        let mut ls = ls.clone();
                        for l in &mut ls {
                            if let (Some(uid), Some(w)) = (
                                l.get("anchor_uid").and_then(Value::as_str),
                                world(l, "a_world"),
                            ) {
                                if let Ok(Some((_, p, _))) =
                                    self.anchor_frame(s, uid, false, &mut contexts)
                                {
                                    let Ok(q) = p.page_point(w) else { continue };
                                    l["ax_mm"] = json!(q[0] - number(&original, "x_mm"));
                                    l["ay_mm"] = json!(q[1] - number(&original, "y_mm"));
                                }
                            }
                        }
                        candidate["leaders"] = json!(ls);
                    }
                }
                Ok(candidate)
            })();
            // The original rebuild preserves this record if projection fails.
            let Ok(candidate) = derived else { continue };
            if candidate != original {
                self.scene.sheets[s].native_document[key][i] = candidate;
                self.composer_state.frame_anchors.stats.changed += 1;
                self.composer_state.annotation_authoring.geometry_epoch = self
                    .composer_state
                    .annotation_authoring
                    .geometry_epoch
                    .wrapping_add(1);
            }
        }
        if let Some(s) = self
            .scene
            .active_sheet_index
            .filter(|&s| s < self.scene.sheets.len())
        {
            self.composer_state.frame_anchors.initialized = Some(self.scene.sheets[s].id);
        }
        Ok(self.composer_frame_anchor_stats())
    }
    fn anchor_frame(
        &mut self,
        s: usize,
        uid: &str,
        snap: bool,
        contexts: &mut HashMap<
            (usize, String, bool),
            Option<(ViewFrame, Projector, Arc<[SnapPair]>)>,
        >,
    ) -> Result<Option<(ViewFrame, Projector, Arc<[SnapPair]>)>, String> {
        let key = (s, uid.to_owned(), snap);
        if let Some(value) = contexts.get(&key) {
            return Ok(value.clone());
        }

        let sheet = &self.scene.sheets[s];
        let f = sheet.items.iter().find_map(|i| match i {
            SheetItem::ViewFrame(f)
                if sheet
                    .native_items
                    .get(&f.id)
                    .and_then(|v| v.get("uid"))
                    .and_then(Value::as_str)
                    == Some(uid) =>
            {
                Some(f.clone())
            }
            _ => None,
        });
        let Some(f) = f else { return Ok(None) };
        let sheet = sheet.id;
        let p = Projector::new(&self.scene, &self.camera, &f)?;
        let pairs = if snap {
            self.composer_frame_snap_pairs(sheet, f.id)?
        } else {
            Arc::from([])
        };
        let value = Some((f, p, pairs));
        contexts.insert(key, value.clone());
        Ok(value)
    }
}
fn number(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.)
}
fn world(v: &Value, k: &str) -> Option<DVec3> {
    let a = v.get(k)?.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let p = DVec3::new(a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?);
    p.is_finite().then_some(p)
}
fn reattach(w: DVec3, p: &Projector, pairs: &[SnapPair], scale: f64) -> Result<DVec3, String> {
    if !p.contains_page(p.page_point(w)?) {
        return Ok(w);
    }
    let tol = (2.5 * scale / 1000.).min(0.5);
    let mut best = tol * tol;
    let mut found = None;
    for q in pairs {
        let d = (q.world - w).length_squared();
        if d <= best && (found.is_none() || d < best) {
            best = d;
            found = Some(q.world)
        }
    }
    Ok(found.unwrap_or(w))
}

pub(super) fn fields(frame: &ViewFrame) -> Value {
    let mut value = serde_json::to_value(frame).expect("finite admitted frame");
    let map = value.as_object_mut().unwrap();
    map.remove("id");
    map.remove("rect");
    map.insert("x_mm".into(), json!(frame.rect.x));
    map.insert("y_mm".into(), json!(frame.rect.y));
    map.insert("w_mm".into(), json!(frame.rect.w));
    map.insert("h_mm".into(), json!(frame.rect.h));
    value
}
