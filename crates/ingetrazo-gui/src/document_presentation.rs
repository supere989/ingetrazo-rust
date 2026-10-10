//! The document owns precise presentation; renderer controls are a float32 view.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    presentation::{DocumentStyle, FaceMode, ShadowSettings},
    scene::Scene,
};
use ingetrazo_render::style::VisualStyle;
fn colors(source: &[f64; 3], target: &mut [f32; 4]) {
    for i in 0..3 {
        target[i] = source[i] as f32;
    }
}
fn sync_color(source: &mut [f64; 3], live: &[f32; 4]) {
    for i in 0..3 {
        if source[i] as f32 != live[i] && live[i].is_finite() {
            source[i] = live[i] as f64;
        }
    }
}
fn adopted(scene: &Scene) -> Option<[f64; 3]> {
    let a = scene.native_document.get("back_face_color")?.as_array()?;
    Some([
        a.first()?.as_f64()?,
        a.get(1)?.as_f64()?,
        a.get(2)?.as_f64()?,
    ])
}
pub(crate) fn preserve(current: &Scene, target: &mut Scene) {
    target.display_style = current.display_style.clone();
    target.shadows = current.shadows.clone();
    target.show_section_fill = current.show_section_fill;
}
impl IngeTrazoApp {
    pub(crate) fn presentation_controls_match(&self)->bool{
        let d=&self.scene.display_style;
        let mode=match self.style.visual_style{VisualStyle::Wireframe=>FaceMode::Wireframe,VisualStyle::HiddenLine=>FaceMode::HiddenLine,VisualStyle::Monochrome=>FaceMode::Monochrome,VisualStyle::XRay=>FaceMode::Xray,_=>if self.style.show_textures{FaceMode::Textures}else{FaceMode::Shaded}};
        fn rgb(a:&[f64;3],b:&[f32;4])->bool{(0..3).all(|i|!b[i].is_finite()||a[i]as f32==b[i])}
        mode==d.face_mode&&self.style.show_edges==d.edges&&self.style.show_profiles==d.profiles&&self.style.back_edges_hidden==d.back_edges&&self.style.show_sky==d.sky&&self.scene.show_section_fill==d.section_fill&&self.shadows_enabled==self.scene.shadows.enabled&&rgb(&d.edge_color,&self.style.edge_color)&&rgb(&d.front_color,&self.style.front_color)&&rgb(&d.background,&self.style.background_color)&&rgb(&d.sky_color,&self.style.sky_color)&&rgb(&d.ground_color,&self.style.ground_color)&&rgb(&d.section_fill_color,&self.style.section_fill_color)&&rgb(&d.effective_back_color(adopted(&self.scene)),&self.style.back_color)
    }
    pub(crate) fn live_presentation(&self) -> (DocumentStyle, ShadowSettings) {
        let mut d = self.scene.display_style.clone();
        d.face_mode = match self.style.visual_style {
            VisualStyle::Wireframe => FaceMode::Wireframe,
            VisualStyle::HiddenLine => FaceMode::HiddenLine,
            VisualStyle::Monochrome => FaceMode::Monochrome,
            VisualStyle::XRay => FaceMode::Xray,
            _ => {
                if self.style.show_textures {
                    FaceMode::Textures
                } else {
                    FaceMode::Shaded
                }
            }
        };
        d.edges = self.style.show_edges;
        d.profiles = self.style.show_profiles;
        d.back_edges = self.style.back_edges_hidden;
        d.sky = self.style.show_sky;
        d.section_fill = self.scene.show_section_fill;
        sync_color(&mut d.edge_color, &self.style.edge_color);
        sync_color(&mut d.front_color, &self.style.front_color);
        sync_color(&mut d.background, &self.style.background_color);
        sync_color(&mut d.sky_color, &self.style.sky_color);
        sync_color(&mut d.ground_color, &self.style.ground_color);
        sync_color(&mut d.section_fill_color, &self.style.section_fill_color);
        let effective = d.effective_back_color(adopted(&self.scene));
        if (0..3).any(|i| self.style.back_color[i].is_finite() && effective[i] as f32 != self.style.back_color[i]) {
            let mut rgb = effective;
            sync_color(&mut rgb, &self.style.back_color);
            d.back_color = Some(rgb);
        }
        let mut shadows = self.scene.shadows.clone();
        shadows.enabled = self.shadows_enabled;
        (d, shadows)
    }
    pub(crate) fn sync_document_presentation(&mut self) {
        if self.presentation_controls_match(){return;}
        let (d, s) = self.live_presentation();
        if self.scene.display_style != d || self.scene.shadows != s {
            self.scene.display_style = d;
            self.scene.shadows = s;
            self.scene.bump_view_version();
        }
    }
    pub(crate) fn apply_document_presentation(&mut self) {
        let d = &self.scene.display_style;
        self.style.visual_style = match d.face_mode {
            FaceMode::Wireframe => VisualStyle::Wireframe,
            FaceMode::HiddenLine => VisualStyle::HiddenLine,
            FaceMode::Monochrome => VisualStyle::Monochrome,
            FaceMode::Xray => VisualStyle::XRay,
            _ => VisualStyle::ShadedWithTextures,
        };
        self.style.show_textures = d.face_mode == FaceMode::Textures;
        self.style.show_edges = d.edges;
        self.style.show_profiles = d.profiles;
        self.style.back_edges_hidden = d.back_edges;
        self.style.show_sky = d.sky;
        self.scene.show_section_fill = d.section_fill;
        colors(&d.edge_color, &mut self.style.edge_color);
        colors(&d.front_color, &mut self.style.front_color);
        colors(&d.background, &mut self.style.background_color);
        colors(&d.sky_color, &mut self.style.sky_color);
        colors(&d.ground_color, &mut self.style.ground_color);
        colors(&d.section_fill_color, &mut self.style.section_fill_color);
        colors(
            &d.effective_back_color(adopted(&self.scene)),
            &mut self.style.back_color,
        );
        self.shadows_enabled = self.scene.shadows.enabled;
    }
    pub fn set_document_style(&mut self, style: DocumentStyle) {
        if self.scene.display_style != style {
            self.scene.display_style = style;
            self.apply_document_presentation();
            self.scene.bump_view_version();
        }
    }
    pub fn set_shadow_settings(&mut self, settings: ShadowSettings) {
        if self.scene.shadows != settings {
            self.scene.shadows = settings;
            self.apply_document_presentation();
            self.scene.bump_view_version();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, DVec3};
    use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
    #[test]
    fn ui_changes_dirty_document_without_quantizing_untouched_storage() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let mut d = DocumentStyle::default();
        d.edge_color = [0.123456789123456, 0.234567891234567, 0.345678912345678];
        d.back_color = None;
        a.set_document_style(d.clone());
        a.mark_document_clean();
        assert!(!a.document_dirty());
        a.style.visual_style = VisualStyle::Wireframe;
        assert!(a.document_dirty());
        a.sync_document_presentation();
        assert_eq!(a.scene.display_style.edge_color, d.edge_color);
        assert!(a.scene.display_style.back_color.is_none());
        a.save_undo_state();
        a.scene.mesh.add_vertex(DVec3::X);
        let mut s = a.scene.shadows.clone();
        s.month = 7;
        s.minute = 25;
        s.utc_offset = Some(-5);
        a.set_shadow_settings(s.clone());
        a.undo();
        assert!(a.scene.mesh.vertices.is_empty());
        assert_eq!(a.scene.shadows, s);
        assert_eq!(a.scene.display_style.face_mode, FaceMode::Wireframe);
        a.redo();
        assert_eq!(a.scene.shadows, s);
    }
    #[test]
    fn presentation_only_leaf_finish_retains_exact_private_geometry_and_no_history() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        let mut m = Mesh::new();
        m.add_vertex(DVec3::new(
            0.123456789123456,
            2.23456789123456,
            3.3456789123456,
        ));
        let mut g = Group::with_mesh("leaf", m.clone());
        g.transform = DMat4::from_rotation_z(0.23456789)
            * DMat4::from_translation(DVec3::new(7.123456789, 8., 9.));
        a.scene.groups.push(g);
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        a.style.visual_style = VisualStyle::Wireframe;
        a.finish_component_edit();
        assert!(a.undo_stack.is_empty());
        assert_eq!(a.scene.groups[0].mesh, m);
        assert_eq!(a.scene.display_style.face_mode, FaceMode::Wireframe);
    }
    #[test]
    fn complete_saved_view_recalls_precision_recipe_sky_and_time() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let mut d = DocumentStyle::default();
        d.name = "Authored".into();
        d.face_mode = FaceMode::Shaded;
        d.edge_color = [0.123456789, 0.234567891, 0.345678912];
        d.sky = false;
        d.back_edges = true;
        d.section_fill = false;
        a.set_document_style(d.clone());
        let s = ShadowSettings {
            enabled: true,
            month: 7,
            day: 19,
            hour: 13,
            minute: 25,
            darkness: 0.63,
            utc_offset: Some(-5),
            ..Default::default()
        };
        a.set_shadow_settings(s.clone());
        a.add_saved_view("View".into());
        assert_eq!(
            a.scene.saved_views[0].style,
            Some(serde_json::to_value(&d).unwrap())
        );
        assert_eq!(
            a.scene.saved_views[0].shadows,
            Some(serde_json::to_value(&s).unwrap())
        );
        a.set_document_style(Default::default());
        a.set_shadow_settings(Default::default());
        a.apply_saved_view(0);
        assert_eq!(a.scene.display_style, d);
        assert_eq!(a.scene.shadows, s);
        assert!(!a.style.show_textures && !a.style.show_sky && !a.scene.show_section_fill);
    }
}
