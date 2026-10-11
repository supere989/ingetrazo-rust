//! Runtime-only render cache domains. Camera changes do not mutate these epochs.
//! Unknown model replacements conservatively invalidate every static domain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Revisions {
    pub geometry: u64,
    pub paint: u64,
    pub style: u64,
    pub layers: u64,
    pub topology: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderRevisions {
    pub static_domains: Revisions,
    pub poses: u64,
}
impl RenderRevisions {
    pub fn model_replaced(&mut self) {
        self.static_domains.geometry = self.static_domains.geometry.wrapping_add(1);
        self.static_domains.paint = self.static_domains.paint.wrapping_add(1);
        self.static_domains.style = self.static_domains.style.wrapping_add(1);
        self.static_domains.layers = self.static_domains.layers.wrapping_add(1);
        self.static_domains.topology = self.static_domains.topology.wrapping_add(1);
        self.placement_changed();
    }
    pub fn placement_changed(&mut self) { self.poses = self.poses.wrapping_add(1); }
    pub fn paint_changed(&mut self) { self.static_domains.paint = self.static_domains.paint.wrapping_add(1); }
    pub fn geometry_changed(&mut self) { self.static_domains.geometry = self.static_domains.geometry.wrapping_add(1); }
    pub fn style_changed(&mut self) { self.static_domains.style = self.static_domains.style.wrapping_add(1); }
    pub fn layers_changed(&mut self) { self.static_domains.layers = self.static_domains.layers.wrapping_add(1); }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn camera_and_pose_keep_static_catalog_domains() {
        let mut r=RenderRevisions::default();let static_before=r.static_domains;
        r.placement_changed();assert_eq!(r.static_domains,static_before);assert_eq!(r.poses,1);
        r.style_changed();assert_eq!(r.static_domains.geometry,0);assert_eq!(r.static_domains.style,1);
        r.layers_changed();assert_eq!(r.static_domains.layers,1);assert_eq!(r.poses,1);
        r.model_replaced();assert_eq!(r.poses,2);assert_eq!(r.static_domains.geometry,1);assert_eq!(r.static_domains.topology,1);
    }
}
