//! Annotation selection cohorts. Preparation is gesture-local; idle paint only
//! checks retained owner IDs, and a group drag shares one furniture inventory.
use super::{
    active, admit, defaults, number, patch, raw_bytes, record, IngeTrazoApp, Owner, FAMILIES,
};
use glam::DVec2;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

const MAX_MEMBERS: usize = 10_000;
const CATEGORIES: [usize; 6] = [7, 8, 9, 10, 12, 13];
#[derive(Clone, Debug, Default)]
pub(super) struct State {
    pub members: Vec<Owner>,
    pub external: bool,
    membership: HashSet<Owner>,
    validated: Option<(Option<uuid::Uuid>, u64, Option<Owner>)>,
    pub ctrl_capture: bool,
    band: Option<Band>,
    group: Option<Group>,
    pub edit_signature: Vec<(Owner, Vec<String>)>,
}
#[derive(Clone, Debug)]
struct Band {
    start: DVec2,
    current: DVec2,
    zoom: f64,
}
#[derive(Clone, Debug)]
struct Peer {
    owner: Owner,
    at: DVec2,
    current: DVec2,
    size: [f64; 2],
}
#[derive(Clone, Debug)]
struct Group {
    pressed: Owner,
    press: DVec2,
    peers: Vec<Peer>,
    axes: [AxisIndex; 2],
}

#[derive(Clone, Copy, Debug)]
struct Coordinate(f64);
impl PartialEq for Coordinate {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
impl Eq for Coordinate {}
impl PartialOrd for Coordinate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Coordinate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
#[derive(Clone, Debug)]
struct AxisIndex {
    points: BTreeMap<(Coordinate, usize), Option<Owner>>,
    ranks: Vec<usize>,
}
impl AxisIndex {
    fn new(fixed: Vec<f64>, peers: &[Peer], axis: usize, ranks: Vec<usize>) -> Self {
        let mut points = fixed
            .into_iter()
            .enumerate()
            .map(|(rank, p)| ((Coordinate(p), rank), None))
            .collect::<BTreeMap<_, _>>();
        for (p, &rank) in peers.iter().zip(&ranks) {
            let x = p.at[axis];
            points.insert((Coordinate(x), rank), Some(p.owner));
            points.insert((Coordinate(x + p.size[axis]), rank + 1), Some(p.owner));
        }
        Self { points, ranks }
    }
    fn moved(&mut self, index: usize, owner: Owner, old: f64, new: f64, size: f64) {
        if old.to_bits() == new.to_bits() {
            return;
        }
        for (offset, delta) in [(0, 0.), (1, size)] {
            let rank = self.ranks[index] + offset;
            self.points.remove(&(Coordinate(old + delta), rank));
            self.points
                .insert((Coordinate(new + delta), rank), Some(owner));
        }
    }
    fn snap(&self, owner: Owner, value: f64) -> f64 {
        let key = Coordinate(value);
        let left_coordinate = self
            .points
            .range(..=(key, usize::MAX))
            .rev()
            .find(|(_, o)| **o != Some(owner))
            .map(|(k, _)| k.0);
        let left = left_coordinate.and_then(|x| {
            self.points
                .range((x, 0)..=(x, usize::MAX))
                .find(|(_, o)| **o != Some(owner))
        });
        let right = self
            .points
            .range((key, 0)..)
            .find(|(_, o)| **o != Some(owner));
        let mut best: Option<(f64, f64, usize)> = None;
        for (key, _) in left.into_iter().chain(right) {
            let (x, rank) = *key;
            let d = (x.0 - value).abs();
            if d <= 2.
                && best.is_none_or(|(_, old, old_rank)| d < old || d == old && rank < old_rank)
            {
                best = Some((x.0, d, rank));
            }
        }
        best.map_or(value, |(x, _, _)| x)
    }
}

pub(super) fn external_members(app: &mut IngeTrazoApp, members: Vec<Owner>) {
    let state = &mut app.composer_state.annotation_authoring.multi;
    state.membership = members.iter().copied().collect();
    state.members = members;
    state.external = true;
    state.validated = None;
}
pub(super) fn selected(app: &IngeTrazoApp, owner: Owner) -> bool {
    app.composer_state.annotation_authoring.selected == Some(owner)
        || app
            .composer_state
            .annotation_authoring
            .multi
            .membership
            .contains(&owner)
}
/// Escape clears selection without retiring an accepted pointer capture.
pub(super) fn clear_selection(app: &mut IngeTrazoApp) {
    let state = &mut app.composer_state.annotation_authoring.multi;
    state.members.clear();
    state.membership.clear();
    state.validated = None;
}
pub(super) fn clear(app: &mut IngeTrazoApp) {
    app.composer_state.annotation_authoring.multi = State::default();
}
pub(super) fn single(app: &mut IngeTrazoApp, owner: Option<Owner>) {
    clear(app);
    if let Some(owner) = owner {
        app.composer_state
            .annotation_authoring
            .multi
            .members
            .push(owner);
        app.composer_state
            .annotation_authoring
            .multi
            .membership
            .insert(owner);
    }
}
fn select_members(app: &mut IngeTrazoApp, members: Vec<Owner>) {
    let state = &mut app.composer_state.annotation_authoring;
    if !state.selected.is_some_and(|o| members.contains(&o)) {
        state.selected = members.first().copied();
        state.draft = None;
    }
    state.multi.membership = members.iter().copied().collect();
    state.multi.validated = None;
    state.multi.members = members;
    state.coalesce = None;
    if let Some(s) = app
        .scene
        .active_sheet_index
        .filter(|&s| s < app.scene.sheets.len())
    {
        app.scene.sheets[s].selected_item = None;
    }
    app.composer_state.picture_authoring.selected = None;
    app.composer_state.legend_authoring.selected = None;
}
pub(super) fn revalidate(app: &mut IngeTrazoApp) {
    let sheet = active(app).ok().map(|s| app.scene.sheets[s].id);
    let stamp = (
        sheet,
        app.scene.version,
        app.composer_state.annotation_authoring.selected,
    );
    if app.composer_state.annotation_authoring.multi.validated == Some(stamp) {
        return;
    }
    let mut members = std::mem::take(&mut app.composer_state.annotation_authoring.multi.members);
    members.retain(|&o| Some(o.sheet) == sheet && record(app, o).is_some());
    if app.composer_state.annotation_authoring.selected.is_none()
        && !app.composer_state.annotation_authoring.multi.external
    {
        // A different owner kind has taken selection, or a lifecycle boundary
        // explicitly cleared the primary owner. Do not resurrect stale members.
        members.clear();
    }
    let state = &mut app.composer_state.annotation_authoring.multi;
    state.membership = members.iter().copied().collect();
    state.members = members;
    state.validated = Some(stamp);
}
/// True consumes the complete Ctrl click, including pointer wobble/release.
pub(super) fn select_for_press(
    app: &mut IngeTrazoApp,
    owner: Owner,
    ctrl: bool,
    owns_ctrl_tip: bool,
    handle: bool,
) -> Result<bool, String> {
    let value = record(app, owner).ok_or("Annotation no longer exists")?;
    admit(value)?;
    app.finish_composer_edit();
    let mut members = app
        .composer_state
        .annotation_authoring
        .multi
        .members
        .clone();
    if let Some(shown) = app.composer_state.annotation_authoring.selected {
        if !members.contains(&shown) {
            members.push(shown);
        }
    }
    if ctrl && !owns_ctrl_tip {
        if let Some(i) = members.iter().position(|&o| o == owner) {
            members.remove(i);
        } else {
            if members.len() >= MAX_MEMBERS {
                return Err("Annotation selection budget exceeded".into());
            }
            members.push(owner);
        }
        select_members(app, members);
        app.composer_state.annotation_authoring.multi.ctrl_capture = true;
        return Ok(true);
    }
    // A body press on an existing member preserves both the cohort and the
    // property panel's shown owner. A specialized handle can add its owner.
    if !members.contains(&owner) {
        if handle {
            if members.len() >= MAX_MEMBERS {
                return Err("Annotation selection budget exceeded".into());
            }
            members.push(owner);
        } else {
            members.clear();
            members.push(owner);
        }
    }
    select_members(app, members);
    Ok(false)
}
pub(super) fn owns_pointer(app: &IngeTrazoApp) -> bool {
    let s = &app.composer_state.annotation_authoring.multi;
    s.ctrl_capture || s.band.is_some()
}
pub(super) fn consume_ctrl(app: &mut IngeTrazoApp, event: &egui::Event) -> bool {
    if !app.composer_state.annotation_authoring.multi.ctrl_capture {
        return false;
    }
    match event {
        egui::Event::PointerMoved(_) => true,
        egui::Event::PointerButton {
            button: egui::PointerButton::Primary,
            pressed: false,
            ..
        }
        | egui::Event::PointerGone => {
            app.composer_state.annotation_authoring.multi.ctrl_capture = false;
            true
        }
        _ => false,
    }
}
pub(super) fn begin_band(app: &mut IngeTrazoApp, point: DVec2, zoom: f32) {
    app.composer_state.annotation_authoring.multi.band = Some(Band {
        start: point,
        current: point,
        zoom: f64::from(zoom),
    });
}
pub(super) fn band_event(
    app: &mut IngeTrazoApp,
    event: &egui::Event,
    point: Option<DVec2>,
    order: &[(u8, usize, usize)],
) -> Result<bool, String> {
    let Some(mut band) = app.composer_state.annotation_authoring.multi.band.take() else {
        return Ok(false);
    };
    match event {
        egui::Event::PointerMoved(_) => {
            if let Some(p) = point {
                band.current = p;
            }
            app.composer_state.annotation_authoring.multi.band = Some(band);
            Ok(true)
        }
        egui::Event::PointerButton {
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
            ..
        } => {
            let p = point.unwrap_or(band.current);
            if (p - band.start).abs().element_sum() * band.zoom < 4. {
                if !modifiers.ctrl && !modifiers.shift {
                    select_members(app, Vec::new());
                }
                return Ok(true);
            }
            let s = active(app)?;
            let sheet = app.scene.sheets[s].id;
            let lo = p.min(band.start);
            let hi = p.max(band.start);
            let crossing = p.x < band.start.x;
            let mut members = if !modifiers.ctrl && !modifiers.shift {
                Vec::new()
            } else {
                app.composer_state
                    .annotation_authoring
                    .multi
                    .members
                    .clone()
            };
            for &(kind, index, family) in order.iter().rev() {
                if kind != 3 {
                    continue;
                }
                let owner = Owner {
                    sheet,
                    family,
                    index,
                };
                let Some(v) = record(app, owner) else {
                    continue;
                };
                if v.get("hidden").and_then(Value::as_bool) == Some(true)
                    || v.get("locked").and_then(Value::as_bool) == Some(true)
                {
                    continue;
                }
                let [x, y, w, h] = super::handles::bounds(app, owner)?;
                let a = DVec2::new(number(v, "x_mm", 0.) + x, number(v, "y_mm", 0.) + y);
                let b = a + DVec2::new(w, h);
                let found = if crossing {
                    a.x < hi.x && a.y < hi.y && b.x > lo.x && b.y > lo.y
                } else {
                    a.x >= lo.x && a.y >= lo.y && b.x <= hi.x && b.y <= hi.y
                };
                if !found {
                    continue;
                }
                let old = members.iter().position(|&o| o == owner);
                if modifiers.shift {
                    if let Some(i) = old {
                        members.remove(i);
                    } else if !modifiers.ctrl {
                        members.push(owner);
                    }
                } else if old.is_none() {
                    members.push(owner);
                }
                if members.len() > MAX_MEMBERS {
                    return Err("Annotation selection budget exceeded".into());
                }
            }
            select_members(app, members);
            Ok(true)
        }
        egui::Event::PointerGone => Ok(true),
        _ => {
            app.composer_state.annotation_authoring.multi.band = Some(band);
            Ok(false)
        }
    }
}
pub(super) fn paint_band(
    app: &IngeTrazoApp,
    painter: &egui::Painter,
    paper: egui::Pos2,
    zoom: f32,
) {
    let Some(b) = &app.composer_state.annotation_authoring.multi.band else {
        return;
    };
    let to = |p: DVec2| paper + egui::vec2(p.x as f32 * zoom, p.y as f32 * zoom);
    let r = egui::Rect::from_two_pos(to(b.start), to(b.current));
    let color = if b.current.x < b.start.x {
        egui::Color32::from_rgb(41, 158, 92)
    } else {
        egui::Color32::from_rgb(41, 128, 214)
    };
    painter.rect_filled(r, 0., color.gamma_multiply(0.16));
    painter.rect_stroke(r, 0., egui::Stroke::new(1., color));
}

pub(super) fn capture_group(
    app: &mut IngeTrazoApp,
    pressed: Owner,
    press: DVec2,
) -> Result<(), String> {
    let members = &app.composer_state.annotation_authoring.multi.members;
    if members.len() < 2 {
        app.composer_state.annotation_authoring.multi.group = None;
        return Ok(());
    }
    if members.len() > MAX_MEMBERS {
        return Err("Annotation group budget exceeded".into());
    }
    let s = active(app)?;
    let sheet = &app.scene.sheets[s];
    let indices = crate::composer_text_fields::source_indices(
        &app.scene.sheets,
        std::slice::from_ref(sheet),
    )?;
    let date = crate::composer_text_fields::today()?;
    let context = crate::composer_text_fields::Context {
        index: indices.get(&(sheet as *const _)).copied(),
        total: app.scene.sheets.len(),
        path: app.current_file.as_deref(),
        date: &date,
    };
    let mut font_painter = app.composer_state.paper_text.borrow_mut();
    let mut measure = |f: usize, v: &Value| -> Result<[f64; 2], String> {
        Ok(crate::composer_annotations::authoring_size(
            sheet,
            FAMILIES[f],
            v,
            &mut font_painter,
            &context,
        )?
        .unwrap_or_else(|| super::size(v, f)))
    };
    let mut peers = Vec::new();
    peers
        .try_reserve_exact(members.len())
        .map_err(|_| "Annotation group allocation refused")?;
    for &owner in members {
        let v = record(app, owner).ok_or("Missing group annotation")?;
        admit(v)?;
        let at = DVec2::new(number(v, "x_mm", 0.), number(v, "y_mm", 0.));
        peers.push(Peer {
            owner,
            at,
            current: at,
            size: measure(owner.family, v)?,
        });
    }
    // All annotation sizes enter the same source ordering as typed furniture.
    // Dynamic endpoints retain their rank; equidistant targets therefore keep
    // the source first-target rule while nearest lookups stay logarithmic.
    let peer_indexes: HashMap<_, _> = peers
        .iter()
        .enumerate()
        .map(|(i, p)| ((CATEGORIES[p.owner.family], p.owner.index), i))
        .collect();
    let mut extra = Vec::new();
    let sheet = &app.scene.sheets[s];
    for (f, key) in FAMILIES.iter().enumerate() {
        if let Some(rows) = sheet.native_document.get(*key).and_then(Value::as_array) {
            if extra.len().saturating_add(rows.len()) > 100_000 {
                return Err("Item snap budget exceeded".into());
            }
            extra
                .try_reserve(rows.len())
                .map_err(|_| "Snap target allocation refused")?;
            for (index, v) in rows.iter().enumerate() {
                let [w, h] = if let Some(&i) = peer_indexes.get(&(CATEGORIES[f], index)) {
                    peers[i].size
                } else {
                    admit(v)?;
                    measure(f, v)?
                };
                extra.push((
                    number(v, "z", 0.),
                    CATEGORIES[f],
                    index,
                    ingetrazo_core::composition::RectMm::new(
                        number(v, "x_mm", 0.),
                        number(v, "y_mm", 0.),
                        w,
                        h,
                    ),
                ));
            }
        }
    }
    let mut ranks = vec![[usize::MAX; 2]; peers.len()];
    let targets = super::super::pointer_transactions::Targets::capture_with_extra_raw(
        sheet,
        |k, _| FAMILIES.contains(&k),
        &extra,
        |category, index, x, y| {
            if let Some(&i) = peer_indexes.get(&(category, index)) {
                ranks[i] = [x, y];
            }
        },
    )?;
    if ranks.iter().any(|r| r.contains(&usize::MAX)) {
        return Err("Missing group snap owner".into());
    }
    let axes = [
        AxisIndex::new(targets.x, &peers, 0, ranks.iter().map(|r| r[0]).collect()),
        AxisIndex::new(targets.y, &peers, 1, ranks.iter().map(|r| r[1]).collect()),
    ];
    drop(font_painter);
    app.composer_state.annotation_authoring.multi.group = Some(Group {
        pressed,
        press,
        peers,
        axes,
    });
    Ok(())
}
pub(super) fn motion_group(app: &mut IngeTrazoApp, point: DVec2) -> Result<(), String> {
    let Some(mut g) = app.composer_state.annotation_authoring.multi.group.take() else {
        return Ok(());
    };
    let result = (|| {
        let s = active(app)?;
        let delta = point - g.press;
        // The pressed item's normal handler runs first. Update its two indexed
        // coordinates, then snap each peer against the current group positions.
        if let Some(i) = g.peers.iter().position(|p| p.owner == g.pressed) {
            let peer = &g.peers[i];
            let value = record(app, peer.owner).ok_or("Missing pressed annotation")?;
            let next = DVec2::new(number(value, "x_mm", 0.), number(value, "y_mm", 0.));
            for axis in 0..2 {
                g.axes[axis].moved(
                    i,
                    peer.owner,
                    peer.current[axis],
                    next[axis],
                    peer.size[axis],
                );
            }
            g.peers[i].current = next;
        }
        for i in 0..g.peers.len() {
            let p = &g.peers[i];
            if p.owner == g.pressed {
                continue;
            }
            let x = g.axes[0].snap(
                p.owner,
                g.axes[0].snap(p.owner, p.at.x + delta.x) + p.size[0],
            ) - p.size[0];
            let y = g.axes[1].snap(
                p.owner,
                g.axes[1].snap(p.owner, p.at.y + delta.y) + p.size[1],
            ) - p.size[1];
            if !x.is_finite() || !y.is_finite() {
                return Err("Invalid group move".into());
            }
            let v = record(app, p.owner).ok_or("Group member changed during move")?;
            if number(v, "x_mm", 0.) == x && number(v, "y_mm", 0.) == y {
                continue;
            }
            app.begin_composer_edit();
            let v =
                &mut app.scene.sheets[s].native_document[FAMILIES[p.owner.family]][p.owner.index];
            v["x_mm"] = json!(x);
            v["y_mm"] = json!(y);
            let next = DVec2::new(x, y);
            for axis in 0..2 {
                g.axes[axis].moved(i, p.owner, p.current[axis], next[axis], p.size[axis]);
            }
            g.peers[i].current = next;
        }
        Ok(())
    })();
    app.composer_state.annotation_authoring.multi.group = Some(g);
    result
}
pub(super) fn release(app: &mut IngeTrazoApp) {
    app.composer_state.annotation_authoring.multi.group = None;
}

pub(super) fn edited_peers(
    app: &IngeTrazoApp,
    primary: Owner,
    changed: &Value,
) -> Result<Vec<(Owner, Value)>, String> {
    let mut out = Vec::new();
    let mut bytes = 0usize;
    let fields = changed.as_object().ok_or("Invalid group edit")?;
    let schema = defaults(primary.family)?;
    for &owner in &app.composer_state.annotation_authoring.multi.members {
        if owner == primary || owner.family != primary.family {
            continue;
        }
        let before = record(app, owner).ok_or("Missing group annotation")?;
        if !fields
            .keys()
            .all(|k| before.get(k).is_some() || schema.get(k).is_some())
        {
            continue;
        }
        bytes = bytes
            .checked_add(raw_bytes(before)?)
            .ok_or("Group edit budget overflow")?;
        if bytes > 32 * 1024 * 1024 {
            return Err("Group edit budget exceeded".into());
        }
        let mut next = before.clone();
        patch(&mut next, changed)?;
        out.push((owner, next));
    }
    Ok(out)
}
pub(super) fn delete_selected(app: &mut IngeTrazoApp) -> Result<bool, String> {
    let mut members = app
        .composer_state
        .annotation_authoring
        .multi
        .members
        .clone();
    if let Some(primary) = app.composer_state.annotation_authoring.selected {
        if !members.contains(&primary) {
            members.push(primary);
        }
    }
    if members.len() < 2 {
        return Ok(false);
    }
    let s = active(app)?;
    let sid = app.scene.sheets[s].id;
    for &o in &members {
        if o.sheet != sid || record(app, o).is_none() {
            return Err("Selection changed before Delete".into());
        }
    }
    app.scene.sheets[s].ensure_annotation_ids()?;
    app.finish_composer_edit();
    app.begin_composer_edit();
    // RemoveItemCommand.undo appends each removed object in reverse command
    // order. Preserve that order in the history image; stable IDs allow older
    // field edits to retain this inventory order when subsequently undone.
    let removed: HashSet<_> = members.iter().map(|o| (o.family, o.index)).collect();
    if let Some(before) = app.composer_state.edit_before.as_mut() {
        for (f, key) in FAMILIES.iter().enumerate() {
            if !members.iter().any(|o| o.family == f) {
                continue;
            }
            let rows = before.native_document[*key]
                .as_array_mut()
                .ok_or("Invalid selected inventory")?;
            let mut old = std::mem::take(rows)
                .into_iter()
                .map(Some)
                .collect::<Vec<_>>();
            let old_ids = std::mem::take(&mut before.annotation_ids[f]);
            for (index, value) in old.iter_mut().enumerate() {
                if !removed.contains(&(f, index)) {
                    rows.push(value.take().unwrap());
                    before.annotation_ids[f].push(old_ids[index]);
                }
            }
            for owner in members.iter().rev().filter(|o| o.family == f) {
                if let Some(value) = old[owner.index].take() {
                    rows.push(value);
                    before.annotation_ids[f].push(old_ids[owner.index]);
                }
            }
        }
    }
    members.sort_by_key(|o| (o.family, o.index));
    members.dedup();
    for o in members.into_iter().rev() {
        app.scene.sheets[s].native_document[FAMILIES[o.family]]
            .as_array_mut()
            .ok_or("Invalid selected inventory")?
            .remove(o.index);
        app.scene.sheets[s].remove_annotation_id(o.family, o.index);
    }
    clear(app);
    app.composer_state.annotation_authoring.selected = None;
    app.composer_state.annotation_authoring.draft = None;
    app.composer_state.annotation_authoring.coalesce = None;
    app.finish_composer_edit();
    Ok(true)
}
#[derive(Default)]
pub(in crate::composer) struct HistorySelection {
    sheet: Option<uuid::Uuid>,
    primary: Option<(usize, uuid::Uuid)>,
    members: Vec<(usize, uuid::Uuid)>,
}
impl IngeTrazoApp {
    pub(in crate::composer) fn composer_annotation_history_selection(&self) -> HistorySelection {
        let Ok(s) = active(self) else {
            return HistorySelection::default();
        };
        let sheet = &self.scene.sheets[s];
        let state = &self.composer_state.annotation_authoring;
        let id = |owner: Owner| {
            (owner.sheet == sheet.id)
                .then(|| sheet.annotation_id(owner.family, owner.index))
                .flatten()
                .map(|id| (owner.family, id))
        };
        HistorySelection {
            sheet: Some(sheet.id),
            primary: state.selected.and_then(id),
            members: state.multi.members.iter().copied().filter_map(id).collect(),
        }
    }
    pub(in crate::composer) fn restore_composer_annotation_history_selection(
        &mut self,
        selected: HistorySelection,
    ) {
        let Ok(s) = active(self) else {
            clear(self);
            return;
        };
        let sheet = &self.scene.sheets[s];
        let mut indices = HashMap::new();
        if selected.sheet == Some(sheet.id) {
            for (family, ids) in sheet.annotation_ids.iter().enumerate() {
                for (index, &id) in ids.iter().enumerate() {
                    indices.insert(
                        (family, id),
                        Owner {
                            sheet: sheet.id,
                            family,
                            index,
                        },
                    );
                }
            }
        }
        let primary = selected.primary.and_then(|id| indices.get(&id).copied());
        let members = selected
            .members
            .iter()
            .filter_map(|id| indices.get(id).copied())
            .collect::<Vec<_>>();
        clear(self);
        let state = &mut self.composer_state.annotation_authoring;
        state.selected = primary;
        state.multi.membership = members.iter().copied().collect();
        state.multi.members = members;
        state.draft = None;
    }
    pub fn composer_annotation_selections(&self) -> Vec<(usize, usize)> {
        let mut out = self
            .composer_state
            .annotation_authoring
            .multi
            .members
            .iter()
            .filter(|&&o| record(self, o).is_some())
            .map(|o| (o.family, o.index))
            .collect::<Vec<_>>();
        if let Some(o) = self.composer_state.annotation_authoring.selected {
            if !out.contains(&(o.family, o.index)) {
                out.push((o.family, o.index));
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}
