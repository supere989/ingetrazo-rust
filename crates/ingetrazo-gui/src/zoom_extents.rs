//! Original main-window Zoom Extents, cold borrowed bounds and Qt camera boundary.
use crate::app::IngeTrazoApp;
use glam::{DVec3, Mat4, Vec3};
use ingetrazo_core::{
    camera::{OrbitCamera, MAX_DISTANCE, MIN_DISTANCE},
    groups::Group,
    scene::Scene,
};
fn absorb(bounds: &mut Option<(DVec3, DVec3)>, p: DVec3) -> Result<(), String> {
    if !p.is_finite() || !p.as_vec3().is_finite() {
        return Err("Zoom Extents contains nonfinite source coordinates".into());
    }
    *bounds = Some(match *bounds {
        Some((lo, hi)) => (lo.min(p), hi.max(p)),
        None => (p, p),
    });
    Ok(())
}
fn visible(scene: &Scene, g: &Group) -> bool {
    !g.hidden && scene.is_layer_visible(g.layer.as_deref())
}
fn groups(
    scene: &Scene,
    g: &Group,
    parent: Mat4,
    bounds: &mut Option<(DVec3, DVec3)>,
) -> Result<(), String> {
    let matrix = parent * g.transform.as_mat4();
    if !matrix.is_finite() {
        return Err("Zoom Extents placement is nonfinite".into());
    }
    if !g.billboard && visible(scene, g) {
        for v in &g.mesh.vertices {
            let a = matrix.to_cols_array_2d();
            let p = v.position.as_vec3().as_dvec3();
            let world = DVec3::from_array(std::array::from_fn(|r| {
                f64::from(a[0][r]) * p.x
                    + f64::from(a[1][r]) * p.y
                    + f64::from(a[2][r]) * p.z
                    + f64::from(a[3][r])
            }));
            absorb(bounds, world)?;
        }
    }
    // Source iter_placements traverses descendants even if an intermediate owner
    // is hidden; only a hidden top-level group suppresses the entire traversal.
    for child in &g.children {
        groups(scene, child, matrix, bounds)?
    }
    Ok(())
}
fn bounds(app: &IngeTrazoApp) -> Result<Option<(DVec3, DVec3)>, String> {
    let scene = &app.scene;
    let mesh = app.material_loose_mesh();
    let mut b = None;
    // Source entity_visible(edge) excludes layer-hidden entities, but intentionally
    // ignores Edge.hidden and the draw visibility of incident faces.
    for e in &mesh.edges {
        if scene.is_layer_visible(e.layer.as_deref()) {
            for v in [e.v0, e.v1] {
                absorb(
                    &mut b,
                    mesh.vertices
                        .get(v)
                        .ok_or("Zoom Extents edge index invalid")?
                        .position,
                )?;
            }
        }
    }
    for (i, f) in mesh.faces.iter().enumerate() {
        if scene.mesh_face_visible(mesh, i) {
            for &v in &f.vertices {
                absorb(
                    &mut b,
                    mesh.vertices
                        .get(v)
                        .ok_or("Zoom Extents face index invalid")?
                        .position,
                )?;
            }
        }
    }
    let document_groups = app.bim_document_groups();
    for g in document_groups
        .iter()
        .filter(|g| visible(scene, g) && !g.billboard)
    {
        groups(scene, g, Mat4::IDENTITY, &mut b)?;
    }
    // Source _figure_bounds is top-level only; placement_points includes children.
    fn figure(g: &Group, parent: Mat4, b: &mut Option<(DVec3, DVec3)>) -> Result<(), String> {
        let m = parent * g.transform.as_mat4();
        for v in &g.mesh.vertices {
            let a = m.to_cols_array_2d();
            let p = v.position.as_vec3().as_dvec3();
            absorb(
                b,
                DVec3::from_array(std::array::from_fn(|r| {
                    f64::from(a[0][r]) * p.x
                        + f64::from(a[1][r]) * p.y
                        + f64::from(a[2][r]) * p.z
                        + f64::from(a[3][r])
                })),
            )?;
        }
        for c in &g.children {
            figure(c, m, b)?;
        }
        Ok(())
    }
    for g in document_groups
        .iter()
        .filter(|g| visible(scene, g) && g.billboard)
    {
        figure(g, Mat4::IDENTITY, &mut b)?;
    }
    if b.is_none() {
        b = ingetrazo_formats::photo_bounds::visible_bounds(scene)?;
    }
    Ok(b.map(|(lo, hi)| (lo.as_vec3().as_dvec3(), hi.as_vec3().as_dvec3())))
}
fn basis(c: &OrbitCamera) -> Result<(Vec3, Vec3, Vec3), String> {
    let mut ordinary = c.clone();
    ordinary.two_point = false;
    let s = crate::navigation_ray::camera_snapshot(&ordinary)
        .ok_or("Zoom Extents camera is nonfinite")?;
    Ok((
        Vec3::from_array(s.forward),
        Vec3::new(s.view[0][0], s.view[1][0], s.view[2][0]),
        Vec3::new(s.view[0][1], s.view[1][1], s.view[2][1]),
    ))
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    f64::from(a.x * b.x + a.y * b.y + a.z * b.z)
}
fn fit(c: &mut OrbitCamera, lo: DVec3, hi: DVec3) -> Result<(), String> {
    if !c.distance.is_finite()
        || c.distance <= 0.
        || !c.aspect.is_finite()
        || c.aspect <= 0.
        || !c.fov_deg.is_finite()
        || c.fov_deg <= 0.
        || c.fov_deg >= 180.
    {
        return Err("Zoom Extents camera domain invalid".into());
    }
    let margin = 1.1;
    let (f, r, u) = basis(c)?;
    let lo = lo.as_vec3();
    let hi = hi.as_vec3();
    let corners: [Vec3; 8] = std::array::from_fn(|i| {
        Vec3::new(
            if i & 4 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 1 == 0 { lo.z } else { hi.z },
        )
    });
    let center = (lo + hi) * 0.5;
    let range = |axis: Vec3| {
        corners
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &p| {
                let v = dot(p - center, axis);
                (min.min(v), max.max(v))
            })
    };
    let (a, b) = range(r);
    let (d, e) = range(u);
    let target = center + r * (((b + a) * 0.5) as f32) + u * (((e + d) * 0.5) as f32);
    let t = (c.fov_deg.to_radians() / 2.).tan();
    let half = ((e - d) * 0.5)
        .max((b - a) * 0.5 / c.aspect)
        .max(MIN_DISTANCE)
        * margin;
    let mut distance = half / t;
    if c.perspective {
        distance += corners
            .iter()
            .map(|&p| -dot(p - target, f))
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.);
    }
    c.target = target.as_dvec3();
    c.distance = distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    if c.perspective {
        let goal = 1. / margin;
        for _ in 0..8 {
            let snapshot = crate::navigation_ray::camera_snapshot(c)
                .ok_or("Zoom Extents projection invalid")?;
            let m = snapshot.matrix;
            let mut xs = (f64::INFINITY, f64::NEG_INFINITY);
            let mut ys = xs;
            for p in corners {
                let v = [p.x, p.y, p.z, 1.];
                let q: [f32; 4] = std::array::from_fn(|r| {
                    m[0][r] * v[0] + m[1][r] * v[1] + m[2][r] * v[2] + m[3][r] * v[3]
                });
                if q[3] <= 1e-9 {
                    return Ok(());
                }
                let x = f64::from(q[0]) / f64::from(q[3]);
                let y = f64::from(q[1]) / f64::from(q[3]);
                xs = (xs.0.min(x), xs.1.max(x));
                ys = (ys.0.min(y), ys.1.max(y));
            }
            let cx = (xs.0 + xs.1) * 0.5;
            let cy = (ys.0 + ys.1) * 0.5;
            let reach = ((xs.1 - xs.0) * 0.5).max((ys.1 - ys.0) * 0.5);
            if reach <= 0. || (reach - goal).abs() < 0.005 && cx.abs() < 0.005 && cy.abs() < 0.005 {
                return Ok(());
            }
            let (f, r, u) = basis(c)?;
            let half_h = c.distance * t;
            c.target = (c.target.as_vec3()
                + r * ((cx * half_h * c.aspect) as f32)
                + u * ((cy * half_h) as f32))
                .as_dvec3();
            let nearest = corners
                .iter()
                .map(|&p| -dot(p - c.target.as_vec3(), f))
                .fold(f64::NEG_INFINITY, f64::max);
            c.distance =
                (nearest + (c.distance - nearest) * reach / goal).clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    }
    if !c.target.is_finite() || !c.distance.is_finite() {
        return Err("Zoom Extents result is nonfinite".into());
    }
    Ok(())
}
impl IngeTrazoApp {
    pub(crate) fn source_zoom_extents(&mut self) {
        self.source_zoom_extents_applied();
    }
    /// Whether this invocation admitted and published a camera candidate.
    pub(crate) fn source_zoom_extents_applied(&mut self) -> bool {
        let result = (|| {
            let Some((lo, hi)) = bounds(self)? else {
                return Ok(None);
            };
            let mut candidate = self.camera.clone();
            fit(&mut candidate, lo, hi)?;
            Ok::<_, String>(Some(candidate))
        })();
        match result {
            Ok(Some(camera)) => {self.camera = camera;true},
            Ok(None) => false,
            Err(error) => {self.status_message = error;false},
        }
    }
}
