//! Retained overlay plans/layouts; no geometry, font or resource scans on an idle hit.
use super::{plan, Plan, Projector};
use crate::{app::IngeTrazoApp, composer_annotations::Draws, composer_compute::ComposerCompute};
use ingetrazo_core::{
    camera::OrbitCamera,
    composition::{SheetItem, ViewFrame},
    scene::Scene,
};
use ingetrazo_formats::{
    paper_font::FontKey,
    photo_profile::{Limits, Survey},
    sheet_pdf::{Draw, PathOp, TextDecoration, TextGlyph},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use uuid::Uuid;
#[derive(Default)]
pub(crate) struct Ground {
    stamp: Option<(Uuid, u64, u64)>,
    photo: Option<([u8; 32], Arc<Survey>)>,
    active: bool,
}
impl Ground {
    fn prepare(&mut self, scene: &Scene, document: Uuid, revision: u64) -> Result<(), String> {
        let stamp = (document, revision, scene.version);
        if self.stamp == Some(stamp) {
            return Ok(());
        }
        let meta = scene
            .native_document
            .get("photo_mesh")
            .filter(|m| !m.is_null());
        let active = meta.is_some_and(|m| m.get("visible").is_none_or(truth));
        if active {
            let member = meta
                .unwrap()
                .get("geometry")
                .map(|v| v.as_str().ok_or("Survey geometry reference must be text"))
                .transpose()?
                .unwrap_or("survey/geometry.npz");
            let bytes = scene
                .embedded_textures
                .get(member)
                .ok_or("Frame overlay survey geometry resource is missing")?;
            let hash: [u8; 32] = Sha256::digest(bytes).into();
            if self.stamp.is_none_or(|s| s.0 != document)
                || self.photo.as_ref().is_none_or(|(h, _)| *h != hash)
            {
                let held = self.photo.as_ref().map_or(0, |(_, s)| s.resident_bytes());
                let available = Limits::default()
                    .bytes
                    .checked_sub(held)
                    .ok_or("Frame overlay survey budget exceeded")?;
                let survey = Survey::decode(bytes, Limits { bytes: available })?;
                survey.ensure_index_with_limit(available)?;
                self.photo = Some((hash, Arc::new(survey)));
            }
        } else {
            self.photo = None;
        }
        self.active = active;
        self.stamp = Some(stamp);
        Ok(())
    }
    fn point(&self, scene: &Scene, p: glam::DVec3) -> Result<glam::DVec3, String> {
        let mut z = None;
        if self.active {
            if let Some((_, s)) = &self.photo {
                z = s.height_at(p.x as f32, p.y as f32)?.map(f64::from);
            }
        }
        if z.is_none() {
            z = scene
                .terrain
                .as_ref()
                .filter(|t| t.visible)
                .and_then(|t| t.height_at(p.x, p.y));
        }
        Ok(z.map_or(p, |z| glam::DVec3::new(p.x, p.y, f64::from(z as f32))))
    }
}
fn truth(v: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match v {
        Null => false,
        Bool(v) => *v,
        Number(v) => v.as_f64() != Some(0.),
        String(v) => !v.is_empty(),
        Array(v) => !v.is_empty(),
        Object(v) => !v.is_empty(),
    }
}
pub(crate) fn build(
    scene: &Scene,
    live: &OrbitCamera,
    compute: &mut ComposerCompute,
    document: Uuid,
    revision: u64,
    frame: &ViewFrame,
) -> Result<Plan, String> {
    let p = Projector::new(scene, live, frame)?;
    let bounds = if frame.section_marks && !scene.section_planes.is_empty() {
        let saved = frame
            .view_key
            .strip_prefix("scene:")
            .and_then(|n| scene.saved_views.iter().find(|v| v.name == n));
        compute
            .original_qt_paper_drawing_for_view(
                &ingetrazo_render::paper_scene::PaperSceneView::new(scene, saved),
                p.camera(),
                document,
                revision,
                &frame.view_key,
                frame.profiles,
                frame.hidden_lines,
            )?
            .source_bounds
    } else {
        None
    };
    if !scene.geo_paths.is_empty() {
        compute.frame_ground.prepare(scene, document, revision)?;
    }
    plan(
        scene,
        frame,
        &p,
        |v| compute.frame_ground.point(scene, v),
        bounds,
    )
}
#[derive(Clone)]
struct Entry {
    document: Uuid,
    revision: u64,
    version: u64,
    frame: ViewFrame,
    camera: OrbitCamera,
    plan: Arc<Plan>,
    draws: Option<Arc<Draws>>,
    bytes: usize,
    fonts: Vec<(FontKey, usize)>,
}
#[derive(Clone, Default)]
pub(crate) struct State {
    entries: HashMap<Uuid, Entry>,
    order: VecDeque<Uuid>,
    bytes: usize,
    fonts: HashMap<FontKey, (usize, usize)>,
    font_bytes: usize,
    canvas: super::super::annotation_canvas::Cache,
    builds: u64,
    hits: u64,
    layouts: u64,
}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameOverlays")
            .field("builds", &self.builds)
            .field("hits", &self.hits)
            .field("bytes", &(self.bytes + self.font_bytes))
            .finish()
    }
}
fn same_frame(a: &ViewFrame, b: &ViewFrame) -> bool {
    a.rect.w == b.rect.w
        && a.rect.h == b.rect.h
        && a.scale_n == b.scale_n
        && a.view_key == b.view_key
        && a.rot_deg == b.rot_deg
        && a.perspective == b.perspective
        && a.cam_target == b.cam_target
        && a.cam_yaw == b.cam_yaw
        && a.cam_pitch == b.cam_pitch
        && a.cam_distance == b.cam_distance
        && a.cam_fov == b.cam_fov
        && a.annotations == b.annotations
        && a.section_marks == b.section_marks
        && a.annot_text_mm == b.annot_text_mm
        && a.km_marks == b.km_marks
        && a.km_step_m == b.km_step_m
        && a.profiles == b.profiles
        && a.hidden_lines == b.hidden_lines
}
impl State {
    fn remove(&mut self, id: Uuid) {
        if let Some(e) = self.entries.remove(&id) {
            self.bytes -= e.bytes;
            for (k, bytes) in e.fonts {
                let n = &mut self.fonts.get_mut(&k).expect("owned overlay font").0;
                *n -= 1;
                if *n == 0 {
                    self.fonts.remove(&k);
                    self.font_bytes -= bytes;
                }
            }
        }
    }
    fn projected(&self, id: Uuid, e: &Entry) -> Result<usize, String> {
        let old = self.entries.get(&id);
        let mut fonts = self.font_bytes;
        if let Some(old) = old {
            for (k, b) in &old.fonts {
                if self.fonts[k].0 == 1 && !e.fonts.iter().any(|v| v.0 == *k) {
                    fonts -= b;
                }
            }
        }
        for (k, b) in &e.fonts {
            if let Some((_, old)) = self.fonts.get(k) {
                if old != b {
                    return Err("Overlay font identity changed size".into());
                }
            } else {
                fonts = fonts
                    .checked_add(*b)
                    .ok_or("Overlay font budget overflow")?;
            }
        }
        self.bytes
            .checked_sub(old.map_or(0, |e| e.bytes))
            .and_then(|n| n.checked_add(e.bytes))
            .and_then(|n| n.checked_add(fonts))
            .ok_or_else(|| "Overlay cache budget overflow".into())
    }
    fn insert(&mut self, id: Uuid, e: Entry) -> Result<(), String> {
        if e.fonts
            .iter()
            .try_fold(e.bytes, |n, (_, b)| n.checked_add(*b))
            .is_none_or(|n| n > 32 * 1024 * 1024)
        {
            return Err("Frame overlay entry exceeds memory budget".into());
        }
        self.projected(id, &e)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| "Overlay cache allocation refused")?;
        self.fonts
            .try_reserve(e.fonts.len())
            .map_err(|_| "Overlay font ledger allocation refused")?;
        let replacing = self.entries.contains_key(&id);
        if !replacing {
            self.order
                .try_reserve(1)
                .map_err(|_| "Overlay ledger allocation refused")?;
        }
        while self.entries.len() + usize::from(!replacing) > 10000
            || self.projected(id, &e)? > 64 * 1024 * 1024
        {
            let old = self
                .order
                .pop_front()
                .ok_or("Overlay cache cannot admit entry")?;
            if old == id {
                self.order.push_back(old);
                continue;
            }
            self.remove(old);
        }
        self.remove(id);
        self.bytes += e.bytes;
        for (k, b) in &e.fonts {
            let n = self.fonts.entry(*k).or_insert_with(|| {
                self.font_bytes += b;
                (0, *b)
            });
            n.0 += 1;
        }
        if !replacing {
            self.order.push_back(id);
        }
        self.entries.insert(id, e);
        Ok(())
    }
}
impl IngeTrazoApp {
    pub(crate) fn composer_frame_annotation_plan(
        &mut self,
        sheet: Uuid,
        frame: Uuid,
    ) -> Result<Arc<Plan>, String> {
        let f = self
            .scene
            .sheets
            .iter()
            .find(|s| s.id == sheet)
            .and_then(|s| {
                s.items.iter().find_map(|i| match i {
                    SheetItem::ViewFrame(f) if f.id == frame => Some(f),
                    _ => None,
                })
            })
            .ok_or("Missing overlay frame")?;
        if let Some(e) = self.composer_state.frame_overlays.entries.get(&frame) {
            if e.document == self.clipboard_document
                && e.revision == self.model_revision
                && e.version == self.scene.version
                && e.camera == self.camera
                && same_frame(&e.frame, f)
            {
                self.composer_state.frame_overlays.hits += 1;
                return Ok(e.plan.clone());
            }
        }
        let plan = Arc::new(build(
            &self.scene,
            &self.camera,
            &mut self.composer_compute,
            self.clipboard_document,
            self.model_revision,
            f,
        )?);
        let bytes = std::mem::size_of::<Entry>() + plan.bytes + f.view_key.len() + 128;
        let e = Entry {
            document: self.clipboard_document,
            revision: self.model_revision,
            version: self.scene.version,
            frame: f.clone(),
            camera: self.camera.clone(),
            plan: plan.clone(),
            draws: None,
            bytes,
            fonts: vec![],
        };
        self.composer_state.frame_overlays.insert(frame, e)?;
        self.composer_state.frame_overlays.builds += 1;
        Ok(plan)
    }
    pub fn composer_frame_annotation_observation(
        &mut self,
        sheet: Uuid,
        frame: Uuid,
    ) -> Result<serde_json::Value, String> {
        self.composer_frame_annotation_plan(sheet, frame)
            .map(|p| p.observation())
    }
    pub fn composer_frame_overlay_stats(&self) -> (u64, u64, u64, usize) {
        let s = &self.composer_state.frame_overlays;
        (s.builds, s.hits, s.layouts, s.bytes + s.font_bytes)
    }
}
fn layout_bytes(draws: &Draws) -> Result<(usize, Vec<(FontKey, usize)>), String> {
    let mut bytes = draws.draws.capacity() * std::mem::size_of::<Draw>()
        + (draws.source_lines.capacity()
            + draws.source_points.capacity()
            + draws.rectangles.capacity())
            * std::mem::size_of::<usize>();
    let mut fonts = HashMap::new();
    for d in &draws.draws {
        let n = (d.path.capacity()
            + d.clip.as_ref().map_or(0, Vec::capacity)
            + d.clip_intersection.as_ref().map_or(0, Vec::capacity))
            * std::mem::size_of::<PathOp>()
            + d.stroke_options
                .as_ref()
                .map_or(0, |o| o.dash.capacity() * 8);
        bytes = bytes.checked_add(n).ok_or("Overlay layout size overflow")?;
        if let Some(t) = &d.text {
            bytes = bytes
                .checked_add(
                    t.glyphs.capacity() * std::mem::size_of::<TextGlyph>()
                        + t.decorations.capacity() * std::mem::size_of::<TextDecoration>(),
                )
                .ok_or("Overlay glyph size overflow")?;
            fonts.insert(t.font.key(), t.font.shared_bytes().len());
        }
    }
    let fonts = fonts.into_iter().collect::<Vec<_>>();
    bytes += fonts.capacity() * std::mem::size_of::<(FontKey, usize)>();
    Ok((bytes, fonts))
}
pub(crate) fn paint(
    app: &mut IngeTrazoApp,
    painter: &egui::Painter,
    s: usize,
    index: usize,
    frame: &ViewFrame,
    zoom: f32,
    paper: egui::Pos2,
) -> Result<(), String> {
    let plan = app.composer_frame_annotation_plan(app.scene.sheets[s].id, frame.id)?;
    if plan.primitives.is_empty() {
        return Ok(());
    }
    if app.composer_state.frame_overlays.entries[&frame.id]
        .draws
        .is_none()
    {
        let draws = Arc::new(crate::composer_annotations::frame_overlay_draws(
            &plan,
            frame.rect.w,
            frame.rect.h,
            &mut app.composer_state.paper_text.borrow_mut(),
        )?);
        let (bytes, fonts) = layout_bytes(&draws)?;
        let mut e = app.composer_state.frame_overlays.entries[&frame.id].clone();
        e.draws = Some(draws);
        e.bytes = e
            .bytes
            .checked_add(bytes)
            .ok_or("Overlay entry size overflow")?;
        e.fonts = fonts;
        app.composer_state.frame_overlays.insert(frame.id, e)?;
        app.composer_state.frame_overlays.layouts += 1;
    }
    let state = &mut app.composer_state;
    let draws = state.frame_overlays.entries[&frame.id]
        .draws
        .as_ref()
        .unwrap();
    let factory = app
        .native_composer
        .factory
        .as_ref()
        .ok_or("Native frame annotation canvas provider unavailable")?;
    state.frame_overlays.canvas.paint(
        painter,
        factory,
        &mut state.text_canvas,
        (s, index),
        draws,
        zoom,
        paper + egui::vec2(frame.rect.x as f32 * zoom, frame.rect.y as f32 * zoom),
    )?;
    Ok(())
}
