//! Cold acquisition and ordinary single-image gesture ownership.
use super::{pointer_transactions::Targets, IngeTrazoApp};
use egui::Pos2;
use ingetrazo_core::composition::{RectMm, Sheet};
#[derive(Clone, Debug)]
pub(super) struct Grab {
    pub index: usize,
    pub sheet: uuid::Uuid,
    pub document: uuid::Uuid,
    pub revision: u64,
    pub version: u64,
    pub press: Pos2,
    pub original: RectMm,
    pub resize: bool,
    pub targets: Targets,
    pub drawing: Vec<[f64; 2]>,
}
pub(super) fn record(sheet: &Sheet, index: usize) -> Option<&serde_json::Value> {
    sheet.native_document.get("images")?.as_array()?.get(index)
}
pub(super) fn rect(sheet: &Sheet, index: usize) -> Result<RectMm, String> {
    let v = record(sheet, index).ok_or("Picture no longer exists")?;
    let n = |k: &str| {
        v.get(k)
            .and_then(|v| v.as_f64())
            .filter(|n| n.is_finite() && n.abs() <= 1e6)
            .ok_or_else(|| format!("Invalid picture {k}"))
    };
    let r = RectMm::new(n("x_mm")?, n("y_mm")?, n("w_mm")?, n("h_mm")?);
    if r.w <= 0. || r.h <= 0. {
        return Err("Invalid picture dimensions".into());
    }
    Ok(r)
}
pub(super) fn locked(sheet: &Sheet, index: usize) -> bool {
    record(sheet, index)
        .and_then(|v| v.get("locked"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}
pub(super) fn acquire(
    app: &mut IngeTrazoApp,
    s: usize,
    index: usize,
    pos: Pos2,
    paper: egui::Rect,
    zoom: f32,
) -> Result<(), String> {
    let r = rect(&app.scene.sheets[s], index)?;
    let page = [
        f64::from(pos.x - paper.min.x) / f64::from(zoom),
        f64::from(pos.y - paper.min.y) / f64::from(zoom),
    ];
    let resize = (page[0] - (r.x + r.w)).abs() <= 3. && (page[1] - (r.y + r.h)).abs() <= 3.;
    let targets = Targets::capture_picture(&app.scene.sheets[s], index)?;
    let drawing = if resize {
        Vec::new()
    } else {
        drawing_points(app, s)?
    };
    app.composer_select_picture(Some(index))?;
    app.composer_state.picture_grab = Some(Grab {
        index,
        sheet: app.scene.sheets[s].id,
        document: app.clipboard_document,
        revision: app.model_revision,
        version: app.scene.version,
        press: pos,
        original: r,
        resize,
        targets,
        drawing,
    });
    Ok(())
}
pub(super) fn motion(
    app: &mut IngeTrazoApp,
    s: usize,
    pos: Pos2,
    paper: egui::Rect,
    zoom: f32,
) -> Result<(), String> {
    let Some(g) = app.composer_state.picture_grab.as_ref() else {
        return Ok(());
    };
    if app.scene.sheets[s].id != g.sheet
        || app.clipboard_document != g.document
        || app.model_revision != g.revision
        || app.scene.version != g.version
    {
        return Err("Picture gesture owner changed".into());
    }
    if locked(&app.scene.sheets[s], g.index) {
        return Err("Picture is locked".into());
    }
    let current = rect(&app.scene.sheets[s], g.index)?;
    if !pos.x.is_finite() || !pos.y.is_finite() || !zoom.is_finite() || zoom <= 0. {
        return Err("Invalid picture gesture position".into());
    }
    let next = if g.resize {
        let p = [
            f64::from(pos.x - paper.min.x) / f64::from(zoom),
            f64::from(pos.y - paper.min.y) / f64::from(zoom),
        ];
        let w = super::pointer_transactions::Targets::snap(
            g.original.x + (p[0] - g.original.x).max(10.),
            &g.targets.x,
        ) - g.original.x;
        let h = super::pointer_transactions::Targets::snap(
            g.original.y + (p[1] - g.original.y).max(6.),
            &g.targets.y,
        ) - g.original.y;
        RectMm::new(g.original.x, g.original.y, w, h)
    } else {
        let delta = pos - g.press;
        let (mut x, mut y) = g.targets.position(
            g.original.x + f64::from(delta.x) / f64::from(zoom),
            g.original.y + f64::from(delta.y) / f64::from(zoom),
            g.original.w,
            g.original.h,
        );
        let mut best: Option<(f64, f64, f64)> = None;
        for [cx, cy] in [
            [x, y],
            [x + g.original.w, y],
            [x, y + g.original.h],
            [x + g.original.w, y + g.original.h],
        ] {
            let mut near = None;
            let mut limit = 4.;
            for &[px, py] in &g.drawing {
                let d = (px - cx).powi(2) + (py - cy).powi(2);
                if d < limit {
                    limit = d;
                    near = Some([px, py]);
                }
            }
            if let Some([px, py]) = near {
                if best.is_none_or(|b| limit < b.0) {
                    best = Some((limit, x + (px - cx), y + (py - cy)));
                }
            }
        }
        if let Some((_, bx, by)) = best {
            x = bx;
            y = by;
        }
        RectMm::new(x, y, g.original.w, g.original.h)
    };
    if [next.x, next.y, next.w, next.h]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1e6)
        || next.w <= 0.
        || next.h <= 0.
    {
        return Err("Picture geometry exceeds admission".into());
    }
    let index = g.index;
    let owner = (g.sheet, index);
    app.composer_state.picture_authoring.selected = Some(owner);
    app.scene.sheets[s].selected_item = None;
    if next == current {
        return Ok(());
    }
    app.composer_state.snapshot_sheet(&app.scene.sheets[s]);
    let v = &mut app.scene.sheets[s].native_document["images"][index];
    for (k, n) in [
        ("x_mm", next.x),
        ("y_mm", next.y),
        ("w_mm", next.w),
        ("h_mm", next.h),
    ] {
        v[k] = serde_json::json!(n);
    }
    Ok(())
}
fn drawing_points(app: &mut IngeTrazoApp, s: usize) -> Result<Vec<[f64; 2]>, String> {
    use ingetrazo_core::composition::SheetItem;
    use ingetrazo_render::{
        paper_scene::PaperSceneView,
        vector_hlr::{LineKind, OriginalQtPaperBasis},
    };
    let mut out = Vec::new();
    for item in &app.scene.sheets[s].items {
        let SheetItem::ViewFrame(frame) = item else {
            continue;
        };
        if app.scene.sheets[s]
            .native_items
            .get(&frame.id)
            .and_then(|v| v.get("hidden"))
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            continue;
        }
        let saved = frame
            .view_key
            .strip_prefix("scene:")
            .and_then(|name| app.scene.saved_views.iter().find(|v| v.name == name));
        let view = PaperSceneView::new(&app.scene, saved);
        let mut base = app.camera.clone();
        if let Some(saved) = saved {
            saved.apply(&mut base, &mut []);
        }
        let mut camera = super::frame_camera_with_base(&app.scene, frame, &base);
        if frame.rot_deg == 0. {
            camera.up = if matches!(frame.view_key.as_str(), "std:top" | "std:bottom") {
                glam::DVec3::Y
            } else if frame
                .cam_pitch
                .is_some_and(|p| p.abs() < 89.5_f64.to_radians())
            {
                glam::DVec3::Z
            } else {
                base.up
            };
        }
        let basis = OriginalQtPaperBasis::from_camera(&camera);
        let mut edges = Vec::new();
        collect_edges(&view, &app.scene.mesh, glam::DMat4::IDENTITY, &mut edges)?;
        collect_groups(&view, &app.scene.groups, glam::DMat4::IDENTITY, &mut edges)?;
        let exact = !frame.perspective && edges.len() <= 20_000;
        let segments = if exact {
            Some(app.composer_compute.original_qt_paper_drawing_for_view(
                &view,
                &camera,
                app.clipboard_document,
                app.model_revision,
                &frame.view_key,
                false,
                false,
            )?)
        } else {
            None
        };
        let camera_point = |p: glam::DVec3| {
            let d = p - basis.eye;
            [d.dot(basis.right), d.dot(basis.up), d.dot(basis.forward)]
        };
        let project = |[cx, cy, cz]: [f64; 3]| {
            let [x, y] = if frame.perspective {
                let depth = cz.max(1e-4);
                let hh = (camera.fov_deg.to_radians() / 2.).tan() * depth;
                let ar = frame.rect.w / frame.rect.h;
                [
                    ((cx / (hh * ar) + 1.) * frame.rect.w / 2.)
                        .clamp(-10. * frame.rect.w, 11. * frame.rect.w),
                    ((1. - cy / hh) * frame.rect.h / 2.)
                        .clamp(-10. * frame.rect.h, 11. * frame.rect.h),
                ]
            } else {
                let hh = ingetrazo_core::composition::model_height_for_frame(
                    frame.rect.h,
                    frame.scale_n,
                ) / 2.;
                let hw = hh * (frame.rect.w / frame.rect.h);
                let k = if hh != 0. {
                    frame.rect.h / (2. * hh)
                } else {
                    0.
                };
                [(cx + hw) * k, (hh - cy) * k]
            };
            [frame.rect.x + x, frame.rect.y + y]
        };
        let mut append_point = |cam: [f64; 3]| -> Result<(), String> {
            let q = project(cam);
            if q.iter().any(|v| !v.is_finite()) {
                return Err("Invalid drawing snap point".into());
            }
            if q[0] >= frame.rect.x - 0.5
                && q[0] <= frame.rect.x + frame.rect.w + 0.5
                && q[1] >= frame.rect.y - 0.5
                && q[1] <= frame.rect.y + frame.rect.h + 0.5
            {
                if out.len() >= 1_000_000 {
                    return Err("Drawing snap point budget exceeded".into());
                }
                out.try_reserve(1)
                    .map_err(|_| "Drawing snap allocation failed")?;
                out.push(q);
            }
            Ok(())
        };
        if let Some(drawing) = segments {
            for e in drawing
                .segments
                .iter()
                .filter(|e| e.kind != LineKind::Hidden)
            {
                let a = camera_point(e.a);
                let b = camera_point(e.b);
                append_point(a)?;
                append_point(b)?;
                append_point(std::array::from_fn(|i| (a[i] + b[i]) / 2.))?;
            }
        } else {
            // Source fast order, streamed without a second 3N payload:
            // every first endpoint, every second endpoint, then camera midpoint.
            for part in 0..3 {
                for &(a, b) in &edges {
                    let a = camera_point(a);
                    let b = camera_point(b);
                    append_point(match part {
                        0 => a,
                        1 => b,
                        _ => std::array::from_fn(|i| (a[i] + b[i]) / 2.),
                    })?;
                }
            }
        }
    }
    Ok(out)
}
pub(super) fn collect_edges(
    view: &ingetrazo_render::paper_scene::PaperSceneView<'_>,
    mesh: &ingetrazo_core::mesh::Mesh,
    transform: glam::DMat4,
    out: &mut Vec<(glam::DVec3, glam::DVec3)>,
) -> Result<(), String> {
    for (i, e) in mesh.edges.iter().enumerate() {
        if view.mesh_edge_visible(mesh, i) {
            let a = mesh
                .vertices
                .get(e.v0)
                .ok_or("Invalid drawing edge")?
                .position;
            let b = mesh
                .vertices
                .get(e.v1)
                .ok_or("Invalid drawing edge")?
                .position;
            if out.len() >= 333_333 {
                return Err("Drawing edge budget exceeded".into());
            }
            out.try_reserve(1)
                .map_err(|_| "Drawing edge allocation failed")?;
            out.push((
                transform.transform_point3(a.as_vec3().as_dvec3()),
                transform.transform_point3(b.as_vec3().as_dvec3()),
            ));
        }
    }
    Ok(())
}
pub(super) fn collect_groups(
    view: &ingetrazo_render::paper_scene::PaperSceneView<'_>,
    groups: &[ingetrazo_core::groups::Group],
    parent: glam::DMat4,
    out: &mut Vec<(glam::DVec3, glam::DVec3)>,
) -> Result<(), String> {
    for g in groups {
        if view.group_visible(g) {
            let t = parent * g.transform;
            collect_edges(view, &g.mesh, t, out)?;
            collect_groups(view, &g.children, t, out)?;
        }
    }
    Ok(())
}
