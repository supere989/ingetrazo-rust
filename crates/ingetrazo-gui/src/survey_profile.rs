//! Original PhotoMesh profile action and immutable cached deliverable owner.
use crate::IngeTrazoApp;
use glam::DVec3;
use ingetrazo_core::{
    georef::{GeoPath, Profile},
    i18n::tr,
    scene::SelectionItem,
};
use ingetrazo_formats::photo_profile::{Limits, Survey};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};
#[derive(Clone)]
struct HeldPath {
    id: u64,
    points: Vec<DVec3>,
    closed: bool,
}
struct Sampler {
    survey: Arc<Survey>,
    alt: f64,
}
#[derive(PartialEq, Eq)]
struct AssetKey {
    document: uuid::Uuid,
    member: String,
    hash: [u8; 32],
}
pub struct State {
    pub visible: bool,
    pub message: Option<String>,
    pub profile: Option<Arc<Profile>>,
    held: Option<HeldPath>,
    sampler: Option<Sampler>,
    asset: Option<(AssetKey, Arc<Survey>)>,
    last_version: Option<(uuid::Uuid, u64, u64)>,
    pub limits: Limits,
    pub decode_count: u64,
    config_error: Option<String>,
    chart: Option<Chart>,
    summary: Option<Summary>,
}
impl Default for State {
    fn default() -> Self {
        let mut limits = Limits::default();
        let mut error = None;
        if let Ok(v) = std::env::var("INGETRAZO_PROFILE_CPU_BUDGET_BYTES") {
            match v.parse::<usize>() {
                Ok(n) => limits.bytes = n,
                Err(_) => error = Some("Invalid survey profile CPU budget".into()),
            }
        }
        Self {
            visible: false,
            message: Some(tr("Select a polyline and click “Profile”.")),
            profile: None,
            held: None,
            sampler: None,
            asset: None,
            last_version: None,
            limits,
            decode_count: 0,
            config_error: error,
            chart: None,
            summary: None,
        }
    }
}
fn checked(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or_else(|| "Profile CPU allocation overflow".into())
}
fn bytes(n: usize, width: usize) -> Result<usize, String> {
    n.checked_mul(width)
        .ok_or_else(|| "Profile CPU allocation overflow".into())
}
fn budget(used: usize, limit: usize) -> Result<(), String> {
    if used > limit {
        Err(format!(
            "Survey profile CPU allocation {used} exceeds configured budget {limit}"
        ))
    } else {
        Ok(())
    }
}
impl State {
    pub fn resident_bytes(&self) -> usize {
        self.sampler
            .as_ref()
            .map(|s| s.survey.resident_bytes())
            .or_else(|| self.asset.as_ref().map(|(_, s)| s.resident_bytes()))
            .unwrap_or(0)
            + self
                .held
                .as_ref()
                .map(|p| p.points.capacity() * std::mem::size_of::<DVec3>())
                .unwrap_or(0)
            + self
                .profile
                .as_ref()
                .map(|p| {
                    p.samples.capacity()
                        * std::mem::size_of::<ingetrazo_core::georef::ProfileSample>()
                })
                .unwrap_or(0)
            + self.chart.as_ref().map(|c| c.bytes()).unwrap_or(0)
    }
    pub fn profile_path_index(&self, scene: &ingetrazo_core::scene::Scene) -> Option<usize> {
        let id = self.held.as_ref()?.id;
        scene.geo_paths.iter().position(|p| p.history_id == id)
    }
    pub fn sampler_name(&self) -> Option<&'static str> {
        self.sampler.as_ref().map(|_| "PhotoMeshSampler")
    }
    pub fn held_path_identity(&self) -> Option<u64> {
        self.held.as_ref().map(|p| p.id)
    }
}
fn truth(value: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match value {
        Null => false,
        Bool(b) => *b,
        Number(n) => n.as_f64() != Some(0.),
        String(s) => !s.is_empty(),
        Array(a) => !a.is_empty(),
        Object(o) => !o.is_empty(),
    }
}
fn photo_meta(scene: &ingetrazo_core::scene::Scene) -> Option<&serde_json::Value> {
    scene
        .native_document
        .get("photo_mesh")
        .filter(|p| !p.is_null())
}
fn visible_photo(scene: &ingetrazo_core::scene::Scene) -> bool {
    photo_meta(scene).is_some_and(|m| m.get("visible").is_none_or(truth))
}
pub fn vertical_reference(scene: &ingetrazo_core::scene::Scene) -> &str {
    if visible_photo(scene) {
        return photo_meta(scene)
            .and_then(|m| m.get("vertical_reference"))
            .and_then(|v| v.as_str())
            .unwrap_or("odm");
    }
    if scene.terrain.as_ref().is_some_and(|t| t.visible) {
        "dem"
    } else {
        "local"
    }
}
fn held(path: &GeoPath, other: usize, limit: usize) -> Result<HeldPath, String> {
    let count = checked(
        path.points.len(),
        usize::from(path.closed && path.points.len() > 2),
    )?;
    budget(
        checked(other, bytes(count, std::mem::size_of::<DVec3>())?)?,
        limit,
    )?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(count)
        .map_err(|_| "Could not reserve profile path")?;
    budget(
        checked(
            other,
            bytes(points.capacity(), std::mem::size_of::<DVec3>())?,
        )?,
        limit,
    )?;
    points.extend_from_slice(&path.points);
    if path.closed && points.len() > 2 {
        points.push(points[0]);
    }
    Ok(HeldPath {
        id: path.history_id,
        points,
        closed: path.closed,
    })
}
// Queries are monotonic stations. Keep the same literal prefix additions and
// interpolation law as core::point_at_station, without replaying old segments.
struct StationCursor<'a> {
    points: &'a [DVec3],
    segment: usize,
    prefix: f64,
    length: Option<f64>,
    advances: usize,
}
impl<'a> StationCursor<'a> {
    fn new(points: &'a [DVec3]) -> Self {
        Self {
            points,
            segment: 0,
            prefix: 0.,
            length: None,
            advances: 0,
        }
    }
    fn at(&mut self, station: f64) -> (f64, f64) {
        let Some(first) = self.points.first() else {
            return (0., 0.);
        };
        if station <= 0. {
            return (first.x, first.y);
        }
        while self.segment + 1 < self.points.len() {
            let a = self.points[self.segment];
            let b = self.points[self.segment + 1];
            let length = *self
                .length
                .get_or_insert_with(|| (b.x - a.x).hypot(b.y - a.y));
            if self.prefix + length >= station {
                let t = if length > 1e-12 {
                    (station - self.prefix) / length
                } else {
                    0.
                };
                return (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            }
            self.prefix += length;
            self.segment += 1;
            self.length = None;
            self.advances += 1;
        }
        let last = self.points.last().unwrap();
        (last.x, last.y)
    }
}
fn prepare_profile(
    path: &HeldPath,
    sampler: &Sampler,
    other: usize,
    accounted_survey: usize,
    limit: usize,
) -> Result<Profile, String> {
    if path.points.iter().any(|p| !p.is_finite()) {
        return Err("Profile path is not finite".into());
    }
    let mut length = 0.;
    for pair in path.points.windows(2) {
        length += (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y);
    }
    if !length.is_finite() {
        return Err("Profile chainage is not finite".into());
    }
    let spacing = (length / 256.).max(1.);
    let n = if path.points.len() < 2 || length <= 1e-9 {
        0
    } else {
        (length / spacing).ceil() as usize + 1
    };
    let samplebytes = bytes(
        n,
        std::mem::size_of::<ingetrazo_core::georef::ProfileSample>(),
    )?;
    let data_before = sampler.survey.resident_bytes();
    budget(
        checked(
            checked(other, data_before.saturating_sub(accounted_survey))?,
            samplebytes,
        )?,
        limit,
    )?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(n)
        .map_err(|_| "Could not reserve profile samples")?;
    let samplebytes = bytes(
        samples.capacity(),
        std::mem::size_of::<ingetrazo_core::georef::ProfileSample>(),
    )?;
    let plot_scratch = bytes(
        n.saturating_sub(1),
        2 * (4 * std::mem::size_of::<egui::epaint::Vertex>() + 6 * 4),
    )?;
    let retained_other = other
        .checked_sub(accounted_survey)
        .ok_or("Profile survey accounting mismatch")?;
    let index_limit = limit
        .checked_sub(checked(
            checked(retained_other, samplebytes)?,
            plot_scratch,
        )?)
        .ok_or("Profile result/scratch exceeds CPU budget")?;
    if n > 0 {
        sampler.survey.ensure_index_with_limit(index_limit)?;
    }
    let other = checked(
        other,
        sampler
            .survey
            .resident_bytes()
            .saturating_sub(accounted_survey),
    )?;
    budget(checked(checked(other, samplebytes)?, plot_scratch)?, limit)?;
    let mut complete = true;
    let mut cursor = StationCursor::new(&path.points);
    for i in 0..n {
        let station = length.min(i as f64 * spacing);
        let (x, y) = cursor.at(station);
        // Original profile samples through QVector3D(x,y,0) before NumPy-f32 math.
        if !(x as f32).is_finite() || !(y as f32).is_finite() {
            return Err("Profile query exceeds the source survey coordinate representation".into());
        }
        let elevation = sampler
            .survey
            .height_at(x as f32, y as f32)?
            .map(|z| f64::from(z) + sampler.alt);
        if elevation.is_none() {
            complete = false;
        }
        if !station.is_finite()
            || !x.is_finite()
            || !y.is_finite()
            || elevation.is_some_and(|e| !e.is_finite())
        {
            return Err("Profile sample is not finite".into());
        }
        samples.push(ingetrazo_core::georef::ProfileSample {
            station,
            x,
            y,
            elevation,
        });
    }
    Ok(Profile {
        samples,
        length: if n == 0 { 0. } else { length },
        complete,
    })
}
#[derive(Clone, Copy)]
struct Summary {
    min: Option<f64>,
    max: Option<f64>,
    gain: f64,
}
impl Summary {
    fn from(profile: &Profile) -> Self {
        let mut min: Option<f64> = None;
        let mut max: Option<f64> = None;
        for z in profile.samples.iter().filter_map(|s| s.elevation) {
            min = Some(min.map(|a| a.min(z)).unwrap_or(z));
            max = Some(max.map(|a| a.max(z)).unwrap_or(z));
        }
        Self {
            min,
            max,
            gain: profile.total_gain(),
        }
    }
}
fn prepared_chart(
    state: &State,
    profile: Arc<Profile>,
    new_other: usize,
    limit: usize,
) -> Result<Option<Chart>, String> {
    if Summary::from(&profile).min.is_none() {
        return Ok(None);
    }
    let rect = state
        .chart
        .as_ref()
        .map(|c| c.rect)
        .unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(580., 150.)));
    // Admit the candidate plus one same-sized resize scratch chart before publication.
    let scratch = bytes(
        profile.samples.len().saturating_sub(1),
        4 * std::mem::size_of::<egui::epaint::Vertex>() + 6 * 4,
    )?;
    let chart = chart(profile, rect, checked(new_other, scratch)?, limit)?;
    Ok(Some(chart))
}

impl IngeTrazoApp {
    pub fn open_survey_profile(&mut self) {
        self.survey_profile.visible = true;
        if let Err(error) = self.compute_survey_profile() {
            self.status_message = error;
        }
    }
    pub fn compute_survey_profile(&mut self) -> Result<bool, String> {
        if let Some(e) = &self.survey_profile.config_error {
            return Err(e.clone());
        }
        self.survey_profile.visible = true;
        let Some(datum) = self.scene.geo_datum.as_ref() else {
            self.survey_profile.profile = None;
            self.survey_profile.chart = None;
            self.survey_profile.summary = None;
            self.survey_profile.message =
                Some(tr("Set a base map location first (Tray ▸ Base map)."));
            self.survey_profile.last_version = Some((
                self.clipboard_document,
                self.scene.version,
                self.model_revision,
            ));
            return Ok(false);
        };
        let mut selected = self.scene.selection.iter().filter_map(|item| {
            if let SelectionItem::GeoPath(i) = item {
                Some(*i)
            } else {
                None
            }
        });
        let first = selected.next();
        let multiple = selected.next().is_some();
        let index = if !multiple {
            first.or_else(|| (self.scene.geo_paths.len() == 1).then_some(0))
        } else {
            None
        };
        let path = index.and_then(|i| self.scene.geo_paths.get(i));
        let Some(path) = path.filter(|p| p.points.len() >= 2) else {
            self.survey_profile.held = None;
            self.survey_profile.profile = None;
            self.survey_profile.chart = None;
            self.survey_profile.summary = None;
            self.survey_profile.message = Some(tr(
                "Trace a path with the Path tool (T), select it, then run Profile.",
            ));
            self.survey_profile.last_version = Some((
                self.clipboard_document,
                self.scene.version,
                self.model_revision,
            ));
            return Ok(false);
        };
        if !visible_photo(&self.scene) {
            return Err("Global DEM profile sampling is not yet available; load a visible original photogrammetric survey".into());
        }
        let meta = photo_meta(&self.scene).unwrap();
        let member = meta
            .get("geometry")
            .map(|v| {
                v.as_str()
                    .ok_or("Survey geometry reference is not a string")
            })
            .transpose()?
            .unwrap_or("survey/geometry.npz");
        let payload = self
            .scene
            .embedded_textures
            .get(member)
            .ok_or("Survey geometry resource is missing")?;
        // Hash only on the explicit action. Warm frames and path live-updates never
        // inspect compressed assets; content identity survives existing Scene clones.
        let hash = Sha256::digest(payload).into();
        let key = AssetKey {
            document: self.clipboard_document,
            member: member.to_owned(),
            hash,
        };
        let old = self.survey_profile.resident_bytes();
        let limit = self.survey_profile.limits.bytes;
        let reused = self
            .survey_profile
            .asset
            .as_ref()
            .is_some_and(|(k, _)| *k == key);
        let survey = if reused {
            self.survey_profile.asset.as_ref().unwrap().1.clone()
        } else {
            let available = limit
                .checked_sub(old)
                .ok_or("Retained profile already exceeds CPU budget")?;
            Arc::new(Survey::decode(payload, Limits { bytes: available })?)
        };
        let candidate_path = held(
            path,
            checked(old, if reused { 0 } else { survey.resident_bytes() })?,
            limit,
        )?;
        let pathbytes = bytes(
            candidate_path.points.capacity(),
            std::mem::size_of::<DVec3>(),
        )?;
        let sampler = Sampler {
            survey: survey.clone(),
            alt: datum.alt,
        };
        let accounted = if reused { survey.resident_bytes() } else { 0 };
        let profile = Arc::new(prepare_profile(
            &candidate_path,
            &sampler,
            checked(old, pathbytes)?,
            accounted,
            limit,
        )?);
        let new_owner_bytes = checked(
            checked(
                checked(old, pathbytes)?,
                bytes(
                    profile.samples.capacity(),
                    std::mem::size_of::<ingetrazo_core::georef::ProfileSample>(),
                )?,
            )?,
            if reused { 0 } else { survey.resident_bytes() },
        )?;
        let candidate_chart = prepared_chart(
            &self.survey_profile,
            profile.clone(),
            new_owner_bytes,
            limit,
        )?;
        let summary = Summary::from(&profile);
        // All allocation/math admissions precede publication of sampler/path/result.
        self.survey_profile.asset = Some((key, survey));
        self.survey_profile.sampler = Some(sampler);
        self.survey_profile.held = Some(candidate_path);
        self.survey_profile.profile = Some(profile);
        self.survey_profile.chart = candidate_chart;
        self.survey_profile.summary = Some(summary);
        self.survey_profile.message = None;
        self.survey_profile.last_version = Some((
            self.clipboard_document,
            self.scene.version,
            self.model_revision,
        ));
        if !reused {
            self.survey_profile.decode_count += 1;
        }
        Ok(true)
    }
    pub fn refresh_survey_profile(&mut self) {
        let state = &self.survey_profile;
        if !state.visible
            || state.last_version
                == Some((
                    self.clipboard_document,
                    self.scene.version,
                    self.model_revision,
                ))
        {
            return;
        }
        let Some(id) = state.held.as_ref().map(|p| p.id) else {
            self.survey_profile.last_version = Some((
                self.clipboard_document,
                self.scene.version,
                self.model_revision,
            ));
            return;
        };
        // Source leaves a detached held path/result intact, and re-profiles only
        // while the genuine acquired path is still in scene.geo_paths.
        let Some(path) = self.scene.geo_paths.iter().find(|p| p.history_id == id) else {
            self.survey_profile.last_version = Some((
                self.clipboard_document,
                self.scene.version,
                self.model_revision,
            ));
            return;
        };
        let Some(sampler) = state.sampler.as_ref() else {
            return;
        };
        let limit = state.limits.bytes;
        let other = state.resident_bytes();
        let candidate = held(path, other, limit).and_then(|p| {
            let pathbytes = bytes(p.points.capacity(), std::mem::size_of::<DVec3>())?;
            let profile = Arc::new(prepare_profile(
                &p,
                sampler,
                checked(other, pathbytes)?,
                sampler.survey.resident_bytes(),
                limit,
            )?);
            let peak = checked(
                checked(other, pathbytes)?,
                bytes(
                    profile.samples.capacity(),
                    std::mem::size_of::<ingetrazo_core::georef::ProfileSample>(),
                )?,
            )?;
            let chart = prepared_chart(state, profile.clone(), peak, limit)?;
            Ok((p, profile, chart))
        });
        match candidate {
            Ok((path, profile, chart)) => {
                self.survey_profile.summary = Some(Summary::from(&profile));
                self.survey_profile.held = Some(path);
                self.survey_profile.profile = Some(profile);
                self.survey_profile.chart = chart;
                self.survey_profile.message = None;
                self.survey_profile.last_version = Some((
                    self.clipboard_document,
                    self.scene.version,
                    self.model_revision,
                ));
            }
            Err(e) => self.status_message = e,
        }
    }
    pub fn export_survey_profile(&mut self, path: &Path) -> Result<bool, String> {
        let (Some(held), Some(sampler)) = (&self.survey_profile.held, &self.survey_profile.sampler)
        else {
            return Ok(false);
        };
        // Original CSV handler samples the held object afresh, even when detached.
        let live = self
            .scene
            .geo_paths
            .iter()
            .find(|p| p.history_id == held.id);
        let updated = if let Some(path) = live {
            Some(crate::survey_profile::held(
                path,
                self.survey_profile.resident_bytes(),
                self.survey_profile.limits.bytes,
            )?)
        } else {
            None
        };
        let target = updated.as_ref().unwrap_or(held);
        let other = checked(
            self.survey_profile.resident_bytes(),
            updated
                .as_ref()
                .map(|p| p.points.capacity() * std::mem::size_of::<DVec3>())
                .unwrap_or(0),
        )?;
        let profile = prepare_profile(
            target,
            sampler,
            other,
            sampler.survey.resident_bytes(),
            self.survey_profile.limits.bytes,
        )?;
        let text = profile_csv(
            &profile,
            vertical_reference(&self.scene),
            Some(self.scene.geo_datum.as_ref().map(|d| d.alt).unwrap_or(0.)),
        );
        std::fs::write(path, text).map_err(|e| format!("Profile CSV: {e}"))?;
        Ok(true)
    }
}
pub fn profile_csv(profile: &Profile, vertical: &str, alt: Option<f64>) -> String {
    let note=match vertical{"odm"=>"photogrammetric survey (ODM); WGS84 ellipsoidal heights unless the processing used ground control points","dem"=>"global DEM (SRTM-derived, orthometric)",_=>"local reference: heights are relative, not absolute"};
    let mut out = format!("# elevation reference: {note}\n");
    if let Some(alt) = alt {
        out.push_str(&format!("# scene Z=0 is at {alt:.3} m in that reference\n"));
    }
    out.push_str("station_m,x_m,y_m,elevation_m\n");
    for s in &profile.samples {
        let z = s.elevation.map(|e| format!("{e:.3}")).unwrap_or_default();
        out.push_str(&format!("{:.3},{:.3},{:.3},{}\n", s.station, s.x, s.y, z));
    }
    out
}
struct Chart {
    rect: egui::Rect,
    profile: Arc<Profile>,
    mesh: Arc<egui::Mesh>,
}
impl Chart {
    fn bytes(&self) -> usize {
        self.mesh.vertices.capacity() * std::mem::size_of::<egui::epaint::Vertex>()
            + self.mesh.indices.capacity() * 4
    }
}
fn chart(
    profile: Arc<Profile>,
    rect: egui::Rect,
    other: usize,
    limit: usize,
) -> Result<Chart, String> {
    let count = profile.samples.len().saturating_sub(1);
    budget(
        checked(
            other,
            bytes(
                count,
                4 * std::mem::size_of::<egui::epaint::Vertex>() + 6 * 4,
            )?,
        )?,
        limit,
    )?;
    let mut mesh = egui::Mesh::default();
    mesh.vertices
        .try_reserve_exact(count * 4)
        .map_err(|_| "Could not reserve profile plot vertices")?;
    mesh.indices
        .try_reserve_exact(count * 6)
        .map_err(|_| "Could not reserve profile plot indices")?;
    let low = profile.min_elevation().unwrap_or(0.);
    let high = profile.max_elevation().unwrap_or(1.);
    let (low, high) = if high - low < 1. {
        (low - 1., high + 1.)
    } else {
        (low, high)
    };
    let pad = (high - low) * 0.1;
    let low = low - pad;
    let high = high + pad;
    let length = profile.length.max(1e-9);
    let point = |s: &ingetrazo_core::georef::ProfileSample, z: f64| {
        egui::pos2(
            rect.left() + (s.station / length) as f32 * rect.width(),
            rect.bottom() - ((z - low) / (high - low)) as f32 * rect.height(),
        )
    };
    for pair in profile.samples.windows(2) {
        let (Some(a), Some(b)) = (pair[0].elevation, pair[1].elevation) else {
            continue;
        };
        let a = point(&pair[0], a);
        let b = point(&pair[1], b);
        let delta = b - a;
        let normal = egui::vec2(-delta.y, delta.x).normalized() * 0.75;
        let start = mesh.vertices.len() as u32;
        for p in [a - normal, a + normal, b - normal, b + normal] {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: p,
                uv: egui::epaint::WHITE_UV,
                color: egui::Color32::from_rgb(32, 111, 143),
            });
        }
        mesh.indices.extend_from_slice(&[
            start,
            start + 1,
            start + 2,
            start + 1,
            start + 3,
            start + 2,
        ]);
    }
    budget(
        checked(
            other,
            mesh.vertices.capacity() * std::mem::size_of::<egui::epaint::Vertex>()
                + mesh.indices.capacity() * 4,
        )?,
        limit,
    )?;
    Ok(Chart {
        rect,
        profile,
        mesh: Arc::new(mesh),
    })
}
pub fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if !app.survey_profile.visible {
        return;
    }
    app.refresh_survey_profile();
    let mut visible = true;
    let mut compute = false;
    let mut export = false;
    egui::Window::new(tr("Terrain profile"))
        .open(&mut visible)
        .default_width(620.)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                compute = ui.button(tr("Profile selection")).clicked();
                export = ui.button(tr("Export CSV…")).clicked();
            });
            if let Some(message) = &app.survey_profile.message {
                ui.label(message);
                return;
            }
            let Some(profile) = app.survey_profile.profile.clone() else {
                return;
            };
            let summary = app.survey_profile.summary.unwrap();
            let min = summary.min;
            let max = summary.max;
            if min.is_none() {
                ui.label(tr("Loading terrain…"));
                return;
            }
            ui.label(format!(
                "{} {:.2} m · {} {:.2}–{:.2} m · {} {:.2} m",
                tr("Length"),
                profile.length,
                tr("Elevation"),
                min.unwrap(),
                max.unwrap(),
                tr("Gain"),
                summary.gain
            ));
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 180.), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 0., egui::Color32::from_rgb(247, 248, 250));
            let plot = rect.shrink2(egui::vec2(10., 15.));
            let warm = app
                .survey_profile
                .chart
                .as_ref()
                .is_some_and(|c| c.rect == plot && Arc::ptr_eq(&c.profile, &profile));
            if !warm {
                match chart(
                    profile.clone(),
                    plot,
                    app.survey_profile.resident_bytes(),
                    app.survey_profile.limits.bytes,
                ) {
                    Ok(c) => app.survey_profile.chart = Some(c),
                    Err(e) => app.status_message = e,
                }
            }
            if let Some(c) = &app.survey_profile.chart {
                ui.painter().add(egui::Shape::mesh(c.mesh.as_ref().clone()));
            }
            if !profile.complete {
                ui.label(tr(
                    "Some terrain elevations are unavailable; gaps are preserved.",
                ));
            }
        });
    app.survey_profile.visible = visible;
    if compute {
        app.open_survey_profile();
    }
    if export {
        if let Some(path) =
            crate::dialogs::pick_save_file(&tr("Export profile CSV"), "profile.csv", "*.csv|CSV")
        {
            if let Err(e) = app.export_survey_profile(&path) {
                app.status_message = e;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monotonic_station_cursor_retains_source_bits_and_advances_once() {
        let points = [
            DVec3::ZERO,
            DVec3::ZERO,
            DVec3::new(1e-13, 0., 3.),
            DVec3::new(3., 4., 9.),
            DVec3::new(-2., 4., 1.),
        ];
        let boundary = (points[3].x - points[2].x).hypot(points[3].y - points[2].y) + 1e-13;
        let mut stations = vec![
            0.,
            1e-14,
            1e-13,
            boundary,
            boundary,
            boundary + 1e-12,
            boundary + 5.,
            boundary + 6.,
        ];
        stations.extend((0..1000).map(|i| i as f64 / 100.));
        stations.sort_by(f64::total_cmp);
        let mut cursor = StationCursor::new(&points);
        for station in stations {
            let expected = ingetrazo_core::georef::point_at_station(&points, station);
            let actual = cursor.at(station);
            assert_eq!(
                (actual.0.to_bits(), actual.1.to_bits()),
                (expected.0.to_bits(), expected.1.to_bits())
            );
        }
        assert_eq!(cursor.advances, points.len() - 1);
        assert_eq!(StationCursor::new(&[]).at(1.), (0., 0.));
    }
    fn loaded() -> IngeTrazoApp {
        let bytes = include_bytes!("../../../docs/audits/survey-profile/source-records.zip");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut file = archive.by_name("highest-gaps-input.igz").unwrap();
        let path =
            std::env::temp_dir().join(format!("ingetrazo-profile-test-{}.igz", std::process::id()));
        let mut out = std::fs::File::create(&path).unwrap();
        std::io::copy(&mut file, &mut out).unwrap();
        drop(out);
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.set_language("en");
        a.show_tray = false;
        a.document_session.show_recovery = false;
        a.open_document(&path);
        std::fs::remove_file(path).unwrap();
        a
    }
    fn frame(
        a: &mut IngeTrazoApp,
        c: &egui::Context,
        t: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        c.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
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
            .unwrap()
    }
    #[test]
    fn actual_profile_panel_is_readonly_cached_and_refusal_retains_owner() {
        let mut a = loaded();
        let before = a.scene.clone();
        a.survey_profile.visible = true;
        let ctx = egui::Context::default();
        frame(&mut a, &ctx, 0., vec![]);
        let o = frame(&mut a, &ctx, 0.1, vec![]);
        let point = text(&o, "Profile selection");
        frame(
            &mut a,
            &ctx,
            0.2,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            &mut a,
            &ctx,
            0.3,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert!(a.survey_profile.profile.is_some());
        assert_eq!(a.survey_profile.decode_count, 1);
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
        frame(&mut a, &ctx, 0.4, vec![]);
        let chart = a.survey_profile.chart.as_ref().unwrap().mesh.clone();
        let resource = Arc::downgrade(&a.survey_profile.sampler.as_ref().unwrap().survey);
        let accepted = a.survey_profile.profile.clone().unwrap();
        let bytes = a.survey_profile.resident_bytes();
        frame(&mut a, &ctx, 0.5, vec![]);
        assert!(Arc::ptr_eq(
            &chart,
            &a.survey_profile.chart.as_ref().unwrap().mesh
        ));
        assert_eq!(a.survey_profile.resident_bytes(), bytes);
        assert_eq!(a.survey_profile.decode_count, 1);
        a.survey_profile.limits.bytes = 0;
        assert!(a.compute_survey_profile().is_err());
        assert!(Arc::ptr_eq(
            &accepted,
            a.survey_profile.profile.as_ref().unwrap()
        ));
        assert!(resource.upgrade().is_some());
        assert_eq!(a.survey_profile.resident_bytes(), bytes);
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
        a.survey_profile.limits = Limits::default();
        assert!(a.compute_survey_profile().unwrap());
        assert_eq!(a.survey_profile.decode_count, 1);
        let count = a.survey_profile.decode_count;
        a.scene.geo_paths[0].points[1].x += 1.;
        a.scene.bump_version();
        a.refresh_survey_profile();
        assert_eq!(a.survey_profile.decode_count, count);
        let held = a.survey_profile.held.as_ref().unwrap().id;
        let accepted = a.survey_profile.profile.clone().unwrap();
        a.scene.geo_paths.clear();
        a.scene.bump_version();
        a.refresh_survey_profile();
        assert!(Arc::ptr_eq(
            &accepted,
            a.survey_profile.profile.as_ref().unwrap()
        ));
        let replacement = GeoPath::new(vec![DVec3::ZERO, DVec3::X], false, "new");
        assert_ne!(replacement.history_id, held);
        a.scene.geo_paths.push(replacement);
        a.scene.bump_version();
        a.refresh_survey_profile();
        assert!(Arc::ptr_eq(
            &accepted,
            a.survey_profile.profile.as_ref().unwrap()
        ));
    }
}
