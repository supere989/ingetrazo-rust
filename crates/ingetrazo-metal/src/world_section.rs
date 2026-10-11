//! Literal original Qt-f32 section uniform; GPU retains positive clip distance.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Section { plane: [f32;4], enabled: u32, padding: [u32;3] }
impl Section {
    pub(crate) const OFF: Self = Self { plane:[0.;4], enabled:0, padding:[0;3] };
    pub(crate) fn from_scene(scene:&ingetrazo_core::scene::Scene)->Result<Self,String>{
        if !scene.show_section_cuts { return Ok(Self::OFF); }
        let Some(p)=scene.section_planes.iter().find(|p|p.active) else { return Ok(Self::OFF); };
        let n=p.normal.as_vec3();let o=p.point.as_vec3();
        let plane=[-n.x,-n.y,-n.z,n.x*o.x+n.y*o.y+n.z*o.z];
        if !plane.iter().all(|v|v.is_finite()) { return Err("Metal section plane finite admission".into()); }
        Ok(Self {plane,enabled:1,padding:[0;3]})
    }
}
#[cfg(test)] mod section_tests {
 use super::*;
 #[test] fn section_uses_first_active_kept_halfspace_and_refuses_nonfinite() {
  use ingetrazo_core::{scene::Scene,section::SectionPlane};use glam::DVec3;
  let mut scene=Scene::new();let mut inactive=SectionPlane::new(DVec3::ZERO,DVec3::Y,"off","0");inactive.active=false;
  scene.section_planes.push(inactive);scene.section_planes.push(SectionPlane::new(DVec3::new(2.,0.,0.),DVec3::X,"active","1"));scene.show_section_cuts=true;
  let section=Section::from_scene(&scene).unwrap();assert_eq!(section.plane,[-1.,0.,0.,2.]);assert_eq!(section.enabled,1);assert_eq!(std::mem::size_of::<Section>(),32);
  // Asymmetric witness: source retains x=0.5, rejects x=5 for plane x=2.
  let distance=|p:[f32;3]|section.plane[0]*p[0]+section.plane[1]*p[1]+section.plane[2]*p[2]+section.plane[3];
  assert_eq!(distance([0.5,7.,-3.]),1.5);assert_eq!(distance([5.,7.,-3.]),-3.);
  assert!(!scene.section_planes[1].is_clipped(DVec3::new(0.5,7.,-3.)));
  assert!(scene.section_planes[1].is_clipped(DVec3::new(5.,7.,-3.)));
  scene.show_section_cuts=false;assert_eq!(Section::from_scene(&scene).unwrap().enabled,0);
  scene.show_section_cuts=true;scene.section_planes[1].point.x=f64::INFINITY;assert!(Section::from_scene(&scene).is_err());
 }
}

/// Cached six-vertex source plane, not a reconstructed polygon cap.
#[cfg(target_os = "macos")]
pub(crate) struct Fill {
    owner:std::sync::Arc<crate::world::Owner>,
    pub vertices: crate::Buffer,
    pub color: [f32; 4],
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FillKey {
    revisions: ingetrazo_render::scene_revision::Revisions,
    pose: Option<u64>,
    plane: [u32; 6],
    color: [u32; 4],
}
impl FillKey {
    pub(crate) fn from_scene(
        scene: &ingetrazo_core::scene::Scene,
        style: &ingetrazo_render::style::StyleConfig,
        revisions: ingetrazo_render::scene_revision::Revisions,
        pose: Option<u64>,
    ) -> Option<Self> {
        if !scene.show_section_cuts
            || !scene.show_section_fill
            || matches!(
                style.visual_style,
                ingetrazo_render::VisualStyle::Wireframe | ingetrazo_render::VisualStyle::XRay
            )
        {
            return None;
        }
        let p = scene.section_planes.iter().find(|p| p.active)?;
        let o = p.point.as_vec3();
        let n = p.normal.as_vec3();
        Some(Self {
            revisions,
            pose,
            plane: [
                o.x.to_bits(),
                o.y.to_bits(),
                o.z.to_bits(),
                n.x.to_bits(),
                n.y.to_bits(),
                n.z.to_bits(),
            ],
            color: style.section_fill_color.map(f32::to_bits),
        })
    }
}
#[cfg(target_os = "macos")]
impl Fill {
    pub(crate) fn validate(&self,metal:&crate::NativeMetal)->Result<(),String>{self.owner.validate(metal)}
    pub(crate) fn prepare(
        metal: &crate::NativeMetal,
        scene: &ingetrazo_core::scene::Scene,
        color: [f32; 4],
    ) -> Result<Self, String> {
        let corners = frame(scene)?;
        if !color.iter().all(|v| v.is_finite()) {
            return Err("Metal section fill color finite admission".into());
        }
        let color = [color[0], color[1], color[2], 1.];
        let vertices = [0, 1, 2, 0, 2, 3].map(|i| ingetrazo_render::buffer::Vertex {
            position: corners[i].to_array(),
            normal: [0.; 3],
            color,
            uv: [0.; 2],
        });
        Ok(Self {
            owner:std::sync::Arc::new(crate::world::Owner::new(metal)),
            vertices: crate::world::upload(metal, bytemuck::cast_slice(&vertices))?,
            color,
        })
    }
}
fn dot(a: glam::Vec3, b: glam::Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn normalize(p: glam::Vec3) -> glam::Vec3 {
    let length = ((p.x as f64).powi(2) + (p.y as f64).powi(2) + (p.z as f64).powi(2)).sqrt();
    (p.as_dvec3() / length).as_vec3()
}
/// Literal viewport.section_plane_frame, including Qt vector operation boundaries.
fn frame(scene: &ingetrazo_core::scene::Scene) -> Result<[glam::Vec3; 4], String> {
    use glam::{DVec3, Mat4, Vec3};
    let p = scene
        .section_planes
        .iter()
        .find(|p| p.active)
        .ok_or("Metal section fill has no active plane")?;
    let origin = p.point.as_vec3();
    let n = normalize(p.normal.as_vec3());
    let mut u = Vec3::X - n * dot(Vec3::X, n);
    if u.length() < 0.1 {
        u = Vec3::Y - n * dot(Vec3::Y, n);
    }
    let u = normalize(u);
    let v = normalize(n.cross(u));
    let mut bounds = None;
    fn absorb(b: &mut Option<(DVec3, DVec3)>, p: DVec3) -> Result<(), String> {
        if !p.is_finite() || !p.as_vec3().is_finite() {
            return Err("Metal section bounds nonfinite".into());
        }
        *b = Some(b.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))));
        Ok(())
    }
    for e in &scene.mesh.edges {
        if scene.is_layer_visible(e.layer.as_deref()) {
            for id in [e.v0, e.v1] {
                absorb(
                    &mut bounds,
                    scene
                        .mesh
                        .vertices
                        .get(id)
                        .ok_or("Metal section edge index invalid")?
                        .position,
                )?;
            }
        }
    }
    for (id, f) in scene.mesh.faces.iter().enumerate() {
        if scene.mesh_face_visible(&scene.mesh, id) {
            for &id in &f.vertices {
                absorb(
                    &mut bounds,
                    scene
                        .mesh
                        .vertices
                        .get(id)
                        .ok_or("Metal section face index invalid")?
                        .position,
                )?;
            }
        }
    }
    fn groups(
        scene: &ingetrazo_core::scene::Scene,
        g: &ingetrazo_core::groups::Group,
        parent: Mat4,
        b: &mut Option<(DVec3, DVec3)>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 128 {
            return Err("Metal section bounds nesting admission".into());
        }
        let m = parent * g.transform.as_mat4();
        if !m.is_finite() {
            return Err("Metal section placement nonfinite".into());
        }
        if !g.hidden && !g.billboard && scene.is_layer_visible(g.layer.as_deref()) {
            let a = m.to_cols_array_2d();
            for v in &g.mesh.vertices {
                let p = v.position.as_vec3().as_dvec3();
                absorb(
                    b,
                    DVec3::from_array(std::array::from_fn(|r| {
                        a[0][r] as f64 * p.x
                            + a[1][r] as f64 * p.y
                            + a[2][r] as f64 * p.z
                            + a[3][r] as f64
                    })),
                )?;
            }
        }
        for child in &g.children {
            groups(scene, child, m, b, depth + 1)?;
        }
        Ok(())
    }
    for g in &scene.groups {
        if !g.hidden && !g.billboard && scene.is_layer_visible(g.layer.as_deref()) {
            groups(scene, g, Mat4::IDENTITY, &mut bounds, 0)?;
        }
    }
    let (mut cu, mut cv, mut hu, mut hv) = (0., 0., 1.5, 1.5);
    if let Some((lo, hi)) = bounds {
        let lo = lo.as_vec3();
        let hi = hi.as_vec3();
        let (mut lu, mut ru, mut lv, mut rv) = (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        );
        for x in [lo.x, hi.x] {
            for y in [lo.y, hi.y] {
                for z in [lo.z, hi.z] {
                    let q = Vec3::new(x, y, z) - origin;
                    let a = dot(q, u);
                    let b = dot(q, v);
                    lu = lu.min(a);
                    ru = ru.max(a);
                    lv = lv.min(b);
                    rv = rv.max(b);
                }
            }
        }
        cu = ((lu as f64 + ru as f64) * 0.5) as f32;
        cv = ((lv as f64 + rv as f64) * 0.5) as f32;
        hu = ((ru as f64 - lu as f64) * 0.56 + 0.2) as f32;
        hv = ((rv as f64 - lv as f64) * 0.56 + 0.2) as f32;
    }
    let c = origin + u * cu + v * cv;
    let result = [
        c - u * hu - v * hv,
        c + u * hu - v * hv,
        c + u * hu + v * hv,
        c - u * hu + v * hv,
    ];
    if !result.iter().all(|p| p.is_finite()) {
        return Err("Metal section frame nonfinite".into());
    }
    Ok(result)
}
