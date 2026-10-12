//! Source chain/baseline run state; history identities are nonserialized sheet tokens.
use super::super::frame_anchors::SnapHit;
use super::{record, IngeTrazoApp, Owner};
use glam::DVec2;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Default)]
pub(super) struct State {
    pub armed: bool,
    pub baseline: bool,
    pub sheet: Option<Uuid>,
    pub points: Vec<(DVec2, Option<SnapHit>)>,
    pub sep: Option<f64>,
    pub axis: &'static str,
    pub segments: Vec<Uuid>,
}
fn active(app: &IngeTrazoApp) -> Result<usize, String> {
    super::active(app)
}
fn number(v: &serde_json::Value, key: &str, fallback: f64) -> f64 {
    v.get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(fallback)
}
fn record_at<'a>(app: &'a IngeTrazoApp, sheet: Uuid, id: Uuid) -> Option<&'a serde_json::Value> {
    let s = app.scene.sheets.iter().find(|s| s.id == sheet)?;
    let index = s.annotation_index(0, id)?;
    record(
        app,
        Owner {
            sheet,
            family: 0,
            index,
        },
    )
}
fn selected_seed(app: &IngeTrazoApp, sheet: Uuid) -> Option<Owner> {
    let state = &app.composer_state.annotation_authoring;
    let mut selected = None;
    for &owner in state.multi.members.iter().chain(state.selected.as_slice()) {
        if owner.sheet != sheet || owner.family != 0 || record(app, owner).is_none() {
            continue;
        }
        if selected.is_some_and(|old| old != owner) {
            return None;
        }
        selected = Some(owner);
    }
    selected.filter(|&owner| {
        record(app, owner)
            .is_some_and(|v| v.get("locked").and_then(serde_json::Value::as_bool) != Some(true))
    })
}
fn reserve(state: &mut State) -> Result<(), String> {
    state
        .points
        .try_reserve(1)
        .map_err(|_| "Dimension run point allocation refused")?;
    state
        .segments
        .try_reserve(1)
        .map_err(|_| "Dimension run identity allocation refused".to_string())
}
fn separation(a: DVec2, b: DVec2, p: DVec2, axis: &str) -> f64 {
    if axis == "h" {
        return p.y - a.y;
    }
    if axis == "v" {
        return p.x - a.x;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = dx.hypot(dy);
    if length < 1e-9 {
        return 0.;
    }
    (p.x - a.x) * (-dy / length) + (p.y - a.y) * (dx / length)
}
fn segment_sep(state: &State, a: DVec2, b: DVec2) -> f64 {
    let p1 = state.points[0].0;
    let p2 = state.points[1].0;
    let sep = state.sep.unwrap_or(0.);
    if state.axis == "h" {
        return p1.y + sep - a.y;
    }
    if state.axis == "v" {
        return p1.x + sep - a.x;
    }
    let d12x = p2.x - p1.x;
    let d12y = p2.y - p1.y;
    let l12 = d12x.hypot(d12y);
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = dx.hypot(dy);
    if l12 < 1e-9 || length < 1e-9 {
        return sep;
    }
    let qx = p1.x + (-d12y / l12) * sep;
    let qy = p1.y + (d12x / l12) * sep;
    let nx = -dy / length;
    let ny = dx / length;
    let den = nx * d12y - ny * d12x;
    if den.abs() < 1e-9 {
        return sep;
    }
    ((qx - a.x) * d12y - (qy - a.y) * d12x) / den
}
pub(super) fn activate(app: &mut IngeTrazoApp, baseline: bool) -> Result<(), String> {
    finish(app)?;
    let index = active(app)?;
    app.scene.sheets[index].ensure_annotation_ids()?;
    app.activate_composer_annotation(0)?;
    app.composer_state.annotation_authoring.runs = State {
        armed: true,
        baseline,
        ..Default::default()
    };
    Ok(())
}
pub(super) fn reconcile(app: &mut IngeTrazoApp) -> Result<(), String> {
    let active_id = app
        .scene
        .active_sheet_index
        .and_then(|i| app.scene.sheets.get(i))
        .map(|s| s.id);
    let state = &app.composer_state.annotation_authoring.runs;
    if state.sheet.is_some() && state.sheet != active_id {
        let armed = state.armed;
        let baseline = state.baseline;
        app.composer_state.annotation_authoring.runs = State {
            armed,
            baseline,
            ..Default::default()
        };
        return Ok(());
    }
    let Some(sheet) = state.sheet else {
        return Ok(());
    };
    if state.segments.is_empty() {
        return Ok(());
    }
    // The owning history clones tokens: equal payloads never substitute for removed entities.
    let current = app
        .scene
        .sheets
        .iter()
        .find(|s| s.id == sheet)
        .ok_or("Missing dimension run sheet")?;
    let mut live = std::collections::HashSet::new();
    live.try_reserve(current.annotation_ids[0].len())
        .map_err(|_| "Dimension run identity allocation refused")?;
    live.extend(current.annotation_ids[0].iter().copied());
    let mut state = std::mem::take(&mut app.composer_state.annotation_authoring.runs);
    let old_len = state.segments.len();
    state.segments.retain(|id| live.contains(id));
    if state.segments.len() != old_len {
        let n = if state.segments.is_empty() {
            0
        } else {
            state.segments.len() + 1
        };
        state.points.truncate(n);
        if n == 0 {
            state.sep = None;
        }
    }
    app.composer_state.annotation_authoring.runs = state;
    Ok(())
}
pub(super) fn click(
    app: &mut IngeTrazoApp,
    p: DVec2,
    hit: Option<SnapHit>,
    axis: &'static str,
    zoom: f64,
) -> Result<(), String> {
    if !p.is_finite()
        || p.abs().max_element() > 1e6
        || !zoom.is_finite()
        || zoom <= 0.
        || !matches!(axis, "" | "h" | "v")
    {
        return Err("Invalid dimension run input".into());
    }
    reconcile(app)?;
    let index = active(app)?;
    app.scene.sheets[index].ensure_annotation_ids()?;
    let sheet = app.scene.sheets[index].id;
    let mut state = std::mem::take(&mut app.composer_state.annotation_authoring.runs);
    let result: Result<bool, String> = (|| {
        if !state.armed {
            return Err("No dimension run armed".into());
        }
        reserve(&mut state)?;
        if state.points.is_empty() {
            state.sheet = Some(sheet);
            if let Some(owner) = selected_seed(app, sheet) {
                if let Some(v) = record(app, owner)
                    .filter(|v| v.get("locked").and_then(serde_json::Value::as_bool) != Some(true))
                {
                    let a = DVec2::new(number(v, "x_mm", 0.), number(v, "y_mm", 0.));
                    let b = a + DVec2::new(number(v, "dx_mm", 0.), number(v, "dy_mm", 0.));
                    let sep = number(v, "sep_mm", 0.);
                    let axis = match v.get("axis").and_then(serde_json::Value::as_str) {
                        Some("h") => "h",
                        Some("v") => "v",
                        _ => "",
                    };
                    let id = app.scene.sheets[index]
                        .annotation_id(0, owner.index)
                        .ok_or("Missing seed identity")?;
                    state
                        .points
                        .try_reserve(2)
                        .map_err(|_| "Dimension seed allocation refused")?;
                    state.points.extend([(a, None), (b, None)]);
                    state.sep = Some(sep);
                    state.axis = axis;
                    state.segments.push(id);
                }
            }
        }
        if !axis.is_empty() {
            state.axis = axis;
        }
        if let Some(last) = state.points.last() {
            if (p.x - last.0.x).abs() + (p.y - last.0.y).abs() < 4. / zoom {
                return Ok(true);
            }
        }
        if state.points.len() < 2 {
            state.points.push((p, hit));
            return Ok(false);
        }
        let (a, b, sep, hits, append_point) = if state.sep.is_none() {
            let a = &state.points[0];
            let b = &state.points[1];
            (
                a.0,
                b.0,
                separation(a.0, b.0, p, state.axis),
                [a.1.clone(), b.1.clone()],
                false,
            )
        } else {
            let previous = if state.baseline {
                &state.points[0]
            } else {
                state.points.last().unwrap()
            };
            let sep = if state.baseline {
                let step = app
                    .scene
                    .dimension_style()
                    .get("base_step_mm")
                    .and_then(serde_json::Value::as_f64)
                    .filter(|v| *v > 0.1)
                    .unwrap_or(8.)
                    * state.segments.len() as f64;
                let sep = state.sep.unwrap();
                sep + if sep >= 0. { step } else { -step }
            } else {
                segment_sep(&state, previous.0, p)
            };
            (previous.0, p, sep, [previous.1.clone(), hit.clone()], true)
        };
        let owner =
            app.place_composer_linear_segment(a.to_array(), b.to_array(), sep, state.axis, hits)?;
        let id = app.scene.sheets[index]
            .annotation_id(0, owner.index)
            .ok_or("Missing placed run identity")?;
        state.segments.push(id);
        if append_point {
            state.points.push((p, hit));
        } else {
            state.sep = Some(sep);
        }
        Ok(false)
    })();
    app.composer_state.annotation_authoring.runs = state;
    match result {
        Ok(true) => finish(app),
        Ok(false) => Ok(()),
        Err(e) => Err(e),
    }
}
pub(super) fn finish(app: &mut IngeTrazoApp) -> Result<(), String> {
    reconcile(app)?;
    let state = &app.composer_state.annotation_authoring.runs;
    if state.baseline || state.segments.len() < 2 || state.sep.is_none() {
        let armed = state.armed;
        let baseline = state.baseline;
        app.composer_state.annotation_authoring.runs = State {
            armed,
            baseline,
            ..Default::default()
        };
        return Ok(());
    }
    let sheet = state.sheet.ok_or("Missing run sheet")?;
    let first_record =
        record_at(app, sheet, state.segments[0]).ok_or("Missing first run segment")?;
    let source_sheet = app
        .scene
        .sheets
        .iter()
        .find(|s| s.id == sheet)
        .ok_or("Missing dimension run sheet")?;
    let mut indices = HashMap::new();
    indices
        .try_reserve(source_sheet.annotation_ids[0].len())
        .map_err(|_| "Dimension total identity allocation refused")?;
    indices.extend(
        source_sheet.annotation_ids[0]
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i)),
    );
    let mut text_mm: Option<f64> = None;
    for id in &state.segments {
        let index = *indices.get(id).ok_or("Missing run segment identity")?;
        let row = record(
            app,
            Owner {
                sheet,
                family: 0,
                index,
            },
        )
        .ok_or("Missing run segment")?;
        let value = number(row, "text_mm", 2.8);
        text_mm = Some(text_mm.map_or(value, |old| old.max(value)));
    }
    let step = text_mm.unwrap_or(2.8) * 2. + 2.5;
    let sep = state.sep.unwrap();
    let out = if sep >= 0. { step } else { -step };
    let a = state.points.first().ok_or("Missing run start")?;
    let b = state.points.last().ok_or("Missing run end")?;
    let x = number(first_record, "x_mm", 0.);
    let y = number(first_record, "y_mm", 0.);
    let first_sep = number(first_record, "sep_mm", 0.);
    let total_sep = if state.axis == "h" {
        y + first_sep - a.0.y + out
    } else if state.axis == "v" {
        x + first_sep - a.0.x + out
    } else {
        let dx = number(first_record, "dx_mm", 0.);
        let dy = number(first_record, "dy_mm", 0.);
        let length = dx.hypot(dy);
        let (nx, ny) = if length < 1e-9 {
            (0., -1.)
        } else {
            (-dy / length, dx / length)
        };
        let line_x = x + nx * first_sep;
        let line_y = y + ny * first_sep;
        let dx = b.0.x - a.0.x;
        let dy = b.0.y - a.0.y;
        let length = dx.hypot(dy);
        if length > 1e-9 {
            (line_x - a.0.x) * (-dy / length) + (line_y - a.0.y) * (dx / length) + out
        } else {
            sep + out
        }
    };
    let (a, b, hits, axis) = (a.0, b.0, [a.1.clone(), b.1.clone()], state.axis);
    app.place_composer_linear_segment(a.to_array(), b.to_array(), total_sep, axis, hits)?;
    let state = &mut app.composer_state.annotation_authoring.runs;
    let armed = state.armed;
    let baseline = state.baseline;
    *state = State {
        armed,
        baseline,
        ..Default::default()
    };
    Ok(())
}

pub(super) fn cancel(app: &mut IngeTrazoApp) -> Result<(), String> {
    if !app
        .composer_state
        .annotation_authoring
        .runs
        .points
        .is_empty()
    {
        finish(app)?;
    } else {
        app.composer_state.annotation_authoring.runs.armed = false;
        app.composer_state.annotation_authoring.armed = None;
    }
    Ok(())
}

/// Source Shift changes the measured axis, never constrains the snapped point.
pub(super) fn hover_axis(app: &mut IngeTrazoApp, p: DVec2, shift: bool) -> &'static str {
    let state = &mut app.composer_state.annotation_authoring.runs;
    if shift {
        if let Some((last, _)) = state.points.last() {
            state.axis = if (p.x - last.x).abs() >= (p.y - last.y).abs() {
                "h"
            } else {
                "v"
            };
        }
    }
    state.axis
}
/// Cold consuming-protocol observation only; canvas paint borrows State directly.
pub(super) fn observation(app: &IngeTrazoApp) -> serde_json::Value {
    let state = &app.composer_state.annotation_authoring.runs;
    let ids = state
        .sheet
        .and_then(|id| app.scene.sheets.iter().find(|s| s.id == id));
    serde_json::json!({
        "run_points": state.points.iter().map(|(p, _)| p.to_array()).collect::<Vec<_>>(),
        "run_sep": state.sep,
        "run_axis": state.axis,
        "run_cotas": state.segments.iter().filter_map(|id| ids.and_then(|s| s.annotation_index(0, *id))).collect::<Vec<_>>()
    })
}
