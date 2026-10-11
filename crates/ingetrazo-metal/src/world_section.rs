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
