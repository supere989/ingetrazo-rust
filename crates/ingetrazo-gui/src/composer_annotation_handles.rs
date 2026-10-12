//! Source specialized item-space gestures; retained press state, no per-motion target scan.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Handle {
    LinearEnd,
    LinearSep,
    LinearText,
    AngularRadius,
    RadialEnd,
    LabelSpot(usize),
    LabelWidth,
    CalloutBubble,
    CalloutBox,
}
fn near(p: DVec2, q: DVec2) -> bool {
    (p.x - q.x).abs() <= 3. && (p.y - q.y).abs() <= 3.
}
fn xy(v: &Value, x: &str, y: &str, dx: f64, dy: f64) -> DVec2 {
    DVec2::new(number(v, x, dx), number(v, y, dy))
}
pub(super) fn linear(v: &Value) -> (DVec2, DVec2, DVec2) {
    let d = xy(v, "dx_mm", "dy_mm", 40., 0.);
    let sep = number(v, "sep_mm", 0.);
    let n = if d.length() < 1e-9 {
        DVec2::new(0., -1.)
    } else {
        DVec2::new(-d.y, d.x) / d.length()
    };
    let (a, b) = match v.get("axis").and_then(Value::as_str).unwrap_or("") {
        "h" => (DVec2::new(0., sep), DVec2::new(d.x, sep)),
        "v" => (DVec2::new(sep, 0.), DVec2::new(sep, d.y)),
        _ => (n * sep, d + n * sep),
    };
    (a, b, n)
}
struct Strip {
    anchor: DVec2,
    reference: DVec2,
    angle: f64,
    rect: [f64; 4],
    label_w: f64,
    extent: f64,
}
fn strip(v: &Value) -> Result<Strip, String> {
    let (a, b, _) = linear(v);
    let d = b - a;
    let len = d.length();
    let size = number(v, "text_mm", 2.8);
    let gap = number(v, "offset_mm", 4.);
    let width = crate::composer_annotations::authoring_linear_label(v)?
        .chars()
        .count() as f64
        * size
        * 0.62
        + 2.;
    let height = size * 1.3 + 0.8;
    let deg = (d.y.atan2(d.x).to_degrees() + 90.).rem_euclid(180.) - 90.;
    let horizontal = v.get("text_align").and_then(Value::as_str) == Some("horizontal");
    let rad = deg.to_radians();
    let extent = if horizontal {
        width * rad.cos().abs() + height * rad.sin().abs()
    } else {
        width
    };
    let mut anchor = (a + b) / 2.;
    let along = v
        .get("text_along")
        .and_then(Value::as_str)
        .unwrap_or("middle");
    if len > 1e-9 && matches!(along, "start" | "end") {
        let mut shift = len / 2. + extent / 2. + 1.;
        if along == "start" {
            shift = -shift;
        }
        anchor += d / len * shift;
    }
    anchor += xy(v, "text_dx_mm", "text_dy_mm", 0., 0.);
    let reference = anchor;
    let angle = if horizontal { 0. } else { rad };
    let pos = v
        .get("text_pos")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("above");
    let rect = if matches!(pos, "aside" | "aside_below") {
        let mut u = DVec2::new(rad.sin(), -rad.cos());
        if pos == "aside_below" {
            u = -u;
        }
        let half = if horizontal {
            width / 2. * u.x.abs() + height / 2. * u.y.abs()
        } else {
            height / 2.
        };
        anchor += u * (gap + half);
        [-width / 2. - 1., -height / 2. - 1., width + 2., height + 2.]
    } else {
        let w = if along != "middle" {
            width
        } else {
            80_f64.min(number(v, "dx_mm", 40.).hypot(number(v, "dy_mm", 0.)) + 2. * size)
        };
        match pos {
            "below" => [-w / 2., -1., w, gap + size * 1.3 + 2.],
            "centered" => [-w / 2., -size * 0.8, w, size * 1.6],
            _ => [-w / 2., -gap - size * 1.3 - 1., w, gap + size * 1.3 + 2.],
        }
    };
    Ok(Strip {
        anchor,
        reference,
        angle,
        rect,
        label_w: width,
        extent,
    })
}
fn text_contains(v: &Value, p: DVec2) -> Result<bool, String> {
    let s = strip(v)?;
    let d = p - s.anchor;
    let (c, t) = (s.angle.cos(), s.angle.sin());
    let q = DVec2::new(d.x * c + d.y * t, -d.x * t + d.y * c);
    Ok(q.x >= s.rect[0]
        && q.x <= s.rect[0] + s.rect[2]
        && q.y >= s.rect[1]
        && q.y <= s.rect[1] + s.rect[3])
}
pub(super) fn acquire(
    app: &IngeTrazoApp,
    owner: Owner,
    p: DVec2,
) -> Result<Option<Handle>, String> {
    let v = record(app, owner).ok_or("Missing annotation")?;
    let size = if owner.family == 3 && multi::selected(app, owner) {
        source_size(app, active(app)?, 3, v)?
    } else {
        super::size(v, owner.family)
    };
    Ok(match owner.family {
        0 => {
            let (a, b, _) = linear(v);
            if near(p, (a + b) / 2.) {
                Some(Handle::LinearSep)
            } else if near(p, xy(v, "dx_mm", "dy_mm", 40., 0.)) {
                Some(Handle::LinearEnd)
            } else if text_contains(v, p)? {
                Some(Handle::LinearText)
            } else {
                None
            }
        }
        1 => {
            let a = number(v, "ay_mm", 0.).atan2(number(v, "ax_mm", 30.));
            let b = number(v, "by_mm", -30.).atan2(number(v, "bx_mm", 0.));
            let mut sweep = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            if sweep <= -std::f64::consts::PI {
                sweep = std::f64::consts::PI;
            }
            let am = a + sweep / 2.;
            near(
                p,
                DVec2::new(am.cos(), am.sin()) * number(v, "radius_mm", 15.),
            )
            .then_some(Handle::AngularRadius)
        }
        2 => {
            let a = number(v, "angle_deg", -30.).to_radians();
            near(
                p,
                DVec2::new(a.cos(), a.sin()) * number(v, "radius_mm", 15.),
            )
            .then_some(Handle::RadialEnd)
        }
        3 => {
            let mut hit =
                near(p, xy(v, "ax_mm", "ay_mm", -20., 15.)).then_some(Handle::LabelSpot(0));
            if hit.is_none() {
                if let Some(ls) = v.get("leaders").and_then(Value::as_array) {
                    for (i, l) in ls.iter().enumerate() {
                        if near(p, xy(l, "ax_mm", "ay_mm", 0., 0.)) {
                            hit = Some(Handle::LabelSpot(i + 1));
                            break;
                        }
                    }
                }
            }
            if hit.is_none() && multi::selected(app, owner) && near(p, DVec2::from_array(size)) {
                hit = Some(Handle::LabelWidth);
            }
            hit
        }
        5 => {
            let center = xy(v, "bx_mm", "by_mm", 36., -6.);
            let radius = (number(v, "size_mm", 3.) * 2.4).max(3.) / 2. + 1.5;
            if (p - center).length_squared() <= radius * radius {
                Some(Handle::CalloutBubble)
            } else if near(p, DVec2::from_array(size)) {
                Some(Handle::CalloutBox)
            } else {
                None
            }
        }
        _ => None,
    })
}
pub(super) fn bounds(app: &IngeTrazoApp, owner: Owner) -> Result<[f64; 4], String> {
    let v = record(app, owner).ok_or("Missing annotation")?;
    let size = if owner.family == 3 {
        source_size(app, active(app)?, 3, v)?
    } else {
        super::size(v, owner.family)
    };
    let r = match owner.family {
        0 => {
            let (a, b, _) = linear(v);
            let d = xy(v, "dx_mm", "dy_mm", 40., 0.);
            let s = strip(v)?;
            let mut pad = number(v, "offset_mm", 4.) + number(v, "text_mm", 2.8) + 4.;
            let pos = v.get("text_pos").and_then(Value::as_str).unwrap_or("");
            if matches!(pos, "aside" | "aside_below") {
                pad += s.label_w;
            }
            let along = v
                .get("text_along")
                .and_then(Value::as_str)
                .unwrap_or("middle");
            let drag = xy(v, "text_dx_mm", "text_dy_mm", 0., 0.);
            if along != "middle" || drag.x.abs() >= 1e-9 || drag.y.abs() >= 1e-9 {
                pad += (s.reference - (a + b) / 2.).length() + s.label_w;
            }
            let mut x0 = 0_f64.min(d.x).min(a.x).min(b.x);
            let mut y0 = 0_f64.min(d.y).min(a.y).min(b.y);
            let mut x1 = 0_f64.max(d.x).max(a.x).max(b.x);
            let mut y1 = 0_f64.max(d.y).max(a.y).max(b.y);
            if matches!(along, "start" | "end")
                && drag.x.abs() <= 1e-9
                && drag.y.abs() <= 1e-9
                && (b - a).length() > 1e-9
            {
                let u = (b - a) / (b - a).length();
                let q = if along == "start" {
                    a - u * (s.extent + 1.8)
                } else {
                    b + u * (s.extent + 1.8)
                };
                x0 = x0.min(q.x);
                x1 = x1.max(q.x);
                y0 = y0.min(q.y);
                y1 = y1.max(q.y);
            }
            [x0 - pad, y0 - pad, x1 - x0 + 2. * pad, y1 - y0 + 2. * pad]
        }
        1 => {
            let reach = (number(v, "radius_mm", 15.)
                + number(v, "offset_mm", 4.)
                + number(v, "text_mm", 2.8) * 2.5)
                .max(number(v, "ax_mm", 30.).abs())
                .max(number(v, "ay_mm", 0.).abs())
                .max(number(v, "bx_mm", 0.).abs())
                .max(number(v, "by_mm", -30.).abs())
                + 3.;
            [-reach, -reach, 2. * reach, 2. * reach]
        }
        2 => {
            let r = number(v, "radius_mm", 15.);
            let a = number(v, "angle_deg", -30.).to_radians();
            let diameter = v.get("kind").and_then(Value::as_str) == Some("diameter");
            let label_width = crate::composer_annotations::authoring_radial_label(v)?
                .chars()
                .count() as f64
                * number(v, "text_mm", 2.8)
                * 0.62
                + 2.;
            let placement = v.get("placement").and_then(Value::as_str).unwrap_or("auto");
            let outside = match placement {
                "in" => false,
                "out" => true,
                _ => {
                    label_width
                        + (1.8_f64.max(number(v, "stroke_mm", 0.25) * 6.))
                            * (if diameter { 2. } else { 1. })
                        + 1.
                        > r.abs() * (if diameter { 2. } else { 1. })
                }
            };
            let tail = if outside { r + label_width + 2.5 } else { r };
            let reach = r.max((a.cos() * tail).abs()).max((a.sin() * tail).abs())
                + number(v, "text_mm", 2.8) * 2.
                + 4.;
            [-reach, -reach, 2. * reach, 2. * reach]
        }
        3 => {
            let mut x0 = 0_f64.min(number(v, "ax_mm", -20.));
            let mut y0 = 0_f64.min(number(v, "ay_mm", 15.));
            let mut x1 = size[0].max(number(v, "ax_mm", -20.));
            let mut y1 = size[1].max(number(v, "ay_mm", 15.));
            if let Some(ls) = v.get("leaders").and_then(Value::as_array) {
                for l in ls {
                    x0 = x0.min(number(l, "ax_mm", 0.));
                    y0 = y0.min(number(l, "ay_mm", 0.));
                    x1 = x1.max(number(l, "ax_mm", 0.));
                    y1 = y1.max(number(l, "ay_mm", 0.));
                }
            }
            [x0 - 3., y0 - 3., x1 - x0 + 6., y1 - y0 + 6.]
        }
        4 => {
            let [x, y, w, h] = crate::composer_annotations::authoring_level_bounds(
                v,
                &mut app.composer_state.paper_text.borrow_mut(),
            )?;
            [x - 2., y - 2., w + 4., h + 4.]
        }
        5 => {
            let p = xy(v, "bx_mm", "by_mm", 36., -6.);
            let r = (number(v, "size_mm", 3.) * 2.4).max(3.) / 2.;
            let x0 = 0_f64.min(p.x - r) - 4.;
            let y0 = 0_f64.min(p.y - r) - 4.;
            let x1 = size[0].max(p.x + r) + 4.;
            let y1 = size[1].max(p.y + r) + 4.;
            [x0, y0, x1 - x0, y1 - y0]
        }
        _ => return Err("Unknown annotation family".into()),
    };
    if r.iter().any(|v| !v.is_finite() || v.abs() > 2e6) {
        return Err("Invalid annotation bounds".into());
    }
    Ok(r)
}

fn dirty(app: &mut IngeTrazoApp, owner: Owner) {
    if let Some(e) = app
        .composer_state
        .annotation_authoring
        .layouts
        .entries
        .get_mut(&(owner.family, owner.index))
    {
        e.version = u64::MAX;
    }
    app.composer_state.annotation_authoring.coalesce = None;
}
pub(super) fn press(app: &mut IngeTrazoApp, grab: &mut Grab, ctrl: bool) -> Result<(), String> {
    if ctrl {
        if let Some(Handle::LabelSpot(index)) = grab.handle {
            let source = if index == 0 {
                &grab.before
            } else {
                grab.before
                    .get("leaders")
                    .and_then(Value::as_array)
                    .and_then(|v| v.get(index - 1))
                    .ok_or("Missing copied leader")?
            };
            let leader = json!({"ax_mm":number(source,"ax_mm",-20.),"ay_mm":number(source,"ay_mm",15.),"anchor_uid":"","a_world":null});
            let old = record(app, grab.owner).ok_or("Missing label")?;
            let count = old
                .get("leaders")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if count >= 10000 {
                return Err("Label leader budget exceeded".into());
            }
            let s = active(app)?;
            if let Some(ls) = app.scene.sheets[s].native_document["etiquetas"][grab.owner.index]
                .get_mut("leaders")
                .and_then(Value::as_array_mut)
            {
                ls.try_reserve(1).map_err(|_| "Leader allocation refused")?;
            }
            let mut fresh = Vec::new();
            let existing =
                app.scene.sheets[s].native_document["etiquetas"][grab.owner.index].get("leaders");
            let missing = existing.is_none_or(Value::is_null);
            if !missing && !existing.is_some_and(Value::is_array) {
                return Err("Invalid label leaders".into());
            }
            if missing {
                fresh
                    .try_reserve_exact(1)
                    .map_err(|_| "Leader allocation refused")?;
            }
            app.scene.sheets[s].ensure_annotation_ids()?;
            app.begin_composer_edit();
            let current = &mut app.scene.sheets[s].native_document["etiquetas"][grab.owner.index];
            if missing {
                current["leaders"] = Value::Array(fresh);
            }
            current["leaders"]
                .as_array_mut()
                .ok_or("Invalid label leaders")?
                .push(leader);
            grab.handle = Some(Handle::LabelSpot(count + 1));
            dirty(app, grab.owner);
        }
    }
    Ok(())
}
fn snapped(value: f64, targets: &[f64]) -> f64 {
    let mut best = None;
    for &t in targets {
        let d = (value - t).abs();
        if d <= 2. && best.is_none_or(|b: f64| d < (value - b).abs()) {
            best = Some(t);
        }
    }
    best.unwrap_or(value)
}
pub(super) fn motion(app: &mut IngeTrazoApp, grab: &Grab, page: DVec2) -> Result<bool, String> {
    let Some(handle) = grab.handle else {
        return Ok(false);
    };
    let origin = xy(&grab.before, "x_mm", "y_mm", 0., 0.);
    let p = page - origin;
    if !p.is_finite() || p.abs().max_element() > 1e6 {
        return Err("Invalid annotation handle coordinate".into());
    }
    let fields = match handle {
        Handle::LinearEnd => json!({"dx_mm":p.x,"dy_mm":p.y}),
        Handle::LinearSep => {
            let sep = match grab
                .before
                .get("axis")
                .and_then(Value::as_str)
                .unwrap_or("")
            {
                "h" => p.y,
                "v" => p.x,
                _ => {
                    let (_, _, n) = linear(&grab.before);
                    p.x * n.x + p.y * n.y
                }
            };
            json!({"sep_mm":sep})
        }
        Handle::LinearText => {
            let d = page - grab.press;
            json!({"text_dx_mm":number(&grab.before,"text_dx_mm",0.)+d.x,"text_dy_mm":number(&grab.before,"text_dy_mm",0.)+d.y})
        }
        Handle::AngularRadius => json!({"radius_mm":p.x.hypot(p.y).max(2.)}),
        Handle::RadialEnd => {
            json!({"radius_mm":p.x.hypot(p.y).max(1.),"angle_deg":p.y.atan2(p.x).to_degrees()})
        }
        Handle::LabelWidth => json!({"w_mm":p.x.max(4.)}),
        Handle::CalloutBubble => json!({"bx_mm":p.x,"by_mm":p.y}),
        Handle::CalloutBox => {
            let t = grab
                .targets
                .as_ref()
                .ok_or("Missing retained resize targets")?;
            json!({"w_mm":snapped(origin.x+p.x.max(10.),&t.x)-origin.x,"h_mm":snapped(origin.y+p.y.max(6.),&t.y)-origin.y})
        }
        Handle::LabelSpot(index) => {
            let current = record(app, grab.owner).ok_or("Missing label")?;
            let spot = if index == 0 {
                current
            } else {
                current
                    .get("leaders")
                    .and_then(Value::as_array)
                    .and_then(|a| a.get(index - 1))
                    .ok_or("Missing leader")?
            };
            let patch = json!({"ax_mm":p.x,"ay_mm":p.y,"anchor_uid":"","a_world":null});
            if patch
                .as_object()
                .unwrap()
                .iter()
                .all(|(k, v)| spot.get(k) == Some(v))
            {
                return Ok(true);
            }
            admit(&patch)?;
            let s = active(app)?;
            app.scene.sheets[s].ensure_annotation_ids()?;
            app.begin_composer_edit();
            let current = &mut app.scene.sheets[s].native_document["etiquetas"][grab.owner.index];
            let spot = if index == 0 {
                current
            } else {
                current
                    .get_mut("leaders")
                    .and_then(Value::as_array_mut)
                    .and_then(|a| a.get_mut(index - 1))
                    .ok_or("Missing leader")?
            };
            for (k, v) in patch.as_object().unwrap() {
                spot[k] = v.clone();
            }
            dirty(app, grab.owner);
            return Ok(true);
        }
    };
    admit(&fields)?;
    let current = record(app, grab.owner).ok_or("Missing handled annotation")?;
    if fields
        .as_object()
        .unwrap()
        .iter()
        .all(|(k, v)| current.get(k) == Some(v))
    {
        return Ok(true);
    }
    let s = active(app)?;
    app.scene.sheets[s].ensure_annotation_ids()?;
    app.begin_composer_edit();
    let current =
        &mut app.scene.sheets[s].native_document[FAMILIES[grab.owner.family]][grab.owner.index];
    for (k, v) in fields.as_object().unwrap() {
        current[k] = v.clone();
    }
    dirty(app, grab.owner);
    Ok(true)
}
pub(super) fn release(app: &mut IngeTrazoApp, grab: &Grab) -> Result<(), String> {
    let current = record(app, grab.owner).ok_or("Missing released annotation")?;
    if grab.owner.family == 0
        && ["x_mm", "y_mm", "dx_mm", "dy_mm"]
            .iter()
            .any(|k| number(current, k, 0.) != number(&grab.before, k, 0.))
        && current
            .get("anchor_uid")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
    {
        let s = active(app)?;
        app.scene.sheets[s].ensure_annotation_ids()?;
        app.begin_composer_edit();
        let v = &mut app.scene.sheets[s].native_document["cotas"][grab.owner.index];
        v["anchor_uid"] = json!("");
        v["a_world"] = Value::Null;
        v["b_world"] = Value::Null;
        dirty(app, grab.owner);
    }
    if let Some(Handle::LabelSpot(index)) = grab.handle {
        if index > 0 {
            let s = active(app)?;
            let current = record(app, grab.owner).ok_or("Missing released label")?;
            let size = source_size(app, s, 3, current)?;
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
            let ink = crate::composer_annotations::authoring_label_ink_width(
                sheet,
                current,
                &mut app.composer_state.paper_text.borrow_mut(),
                &context,
            )?;
            let ld = current
                .get("leaders")
                .and_then(Value::as_array)
                .and_then(|v| v.get(index - 1))
                .ok_or("Missing released leader")?;
            let p = xy(ld, "ax_mm", "ay_mm", 0., 0.);
            if p.x >= 0. && p.x <= ink && p.y >= 0. && p.y <= size[1] {
                app.scene.sheets[s].ensure_annotation_ids()?;
                app.begin_composer_edit();
                app.scene.sheets[s].native_document["etiquetas"][grab.owner.index]["leaders"]
                    .as_array_mut()
                    .unwrap()
                    .remove(index - 1);
                dirty(app, grab.owner);
            }
        }
    }
    Ok(())
}

pub(super) fn markers(v: &Value, family: usize, size: [f64; 2]) -> Result<Vec<DVec2>, String> {
    let mut out = Vec::new();
    let n = v
        .get("leaders")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if n > 10000 {
        return Err("Leader marker limit exceeded".into());
    }
    out.try_reserve(n + 4)
        .map_err(|_| "Handle marker allocation refused")?;
    match family {
        0 => {
            let (a, b, _) = linear(v);
            out.push((a + b) / 2.);
            out.push(xy(v, "dx_mm", "dy_mm", 30., 0.));
        }
        1 => {
            let a = number(v, "ay_mm", 0.).atan2(number(v, "ax_mm", 20.));
            let b = number(v, "by_mm", 20.).atan2(number(v, "bx_mm", 0.));
            let mut sweep = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            if sweep <= -std::f64::consts::PI {
                sweep = std::f64::consts::PI;
            }
            let angle = a + sweep / 2.;
            out.push(DVec2::new(angle.cos(), angle.sin()) * number(v, "radius_mm", 15.));
        }
        2 => {
            let a = number(v, "angle_deg", -30.).to_radians();
            out.push(DVec2::new(a.cos(), a.sin()) * number(v, "radius_mm", 15.));
        }
        3 => {
            out.push(xy(v, "ax_mm", "ay_mm", -20., 15.));
            if let Some(ls) = v.get("leaders").and_then(Value::as_array) {
                for ld in ls {
                    out.push(xy(ld, "ax_mm", "ay_mm", 0., 0.));
                }
            }
            out.push(DVec2::from_array(size));
        }
        5 => {
            out.push(xy(v, "bx_mm", "by_mm", 36., -6.));
            out.push(DVec2::from_array(size));
        }
        _ => {}
    }
    Ok(out)
}
