//! Opt-in borrowed Scene consumer. No managed-image upload or CPU readback.
use crate::IngeTrazoApp;
use ingetrazo_metal::{
    world::{Prepared, World},
    world_scene::Adapter,
    NativeMetal,
};
use ingetrazo_render::scene_revision::RenderRevisions;
use std::sync::Arc;
pub(crate) const TEXTURE: egui::TextureId = egui::TextureId::User(u64::MAX - 41);
pub(crate) struct Viewport {
    metal: Arc<NativeMetal>,
    world: World,
    adapter: Adapter,
    pub(crate) frame: Option<Arc<Prepared>>,
    unsupported: Option<(ingetrazo_render::scene_revision::Revisions, u16, String)>,
}
impl Viewport {
    pub(crate) fn new(metal: Arc<NativeMetal>) -> Self {
        Self {
            metal,
            world: World::default(),
            adapter: Adapter::default(),
            frame: None,
            unsupported: None,
        }
    }
    fn prepare(&mut self, app: &IngeTrazoApp, extent: (u32, u32)) -> Result<bool, String> {
        let RenderRevisions {
            static_domains,
            poses,
        } = app.native_render_revisions;
        // Selection and nonpersistent viewport flags are not modeling epochs.
        let capability = (app.style.show_edges as u16)
            | ((app.style.show_profiles as u16) << 1)
            | ((app.style.show_sky as u16) << 2)
            | ((app.style.show_grid as u16) << 3)
            | ((!app.scene.selection.is_empty() as u16) << 4)
            | ((app.scene.show_section_cuts as u16) << 5)
            | ((app.style.visual_style as u16) << 6);
        if let Some((revision, previous, reason)) = &self.unsupported {
            if *revision == static_domains && *previous == capability {
                return Err(reason.clone());
            }
        }
        let camera = (app.camera.projection_matrix() * app.camera.view_matrix())
            .as_mat4()
            .to_cols_array_2d();
        match self.world.prepare_scene_placements_revisioned(
            &self.metal,
            &mut self.adapter,
            &app.scene,
            static_domains,
            Some(poses),
            &app.camera,
            &app.style,
            camera,
            extent,
        ) {
            Ok(Some(frame)) => {
                self.unsupported = None;
                self.frame = Some(frame);
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(error) => {
                if ingetrazo_metal::world_scene::unsupported_stage(&error) {
                    self.unsupported = Some((static_domains, capability, error.clone()));
                    self.frame = None;
                }
                Err(error)
            }
        }
    }
}
/// Called after current-frame pointer/navigation edits and before paint choice.
pub(crate) fn route(app: &mut IngeTrazoApp, rect: egui::Rect, density: f32) -> Option<String> {
    let mut viewport = app.native_metal_world.take()?;
    let result = crate::viewport_extent::scene_extent(
        [rect.width() as f64, rect.height() as f64],
        density as f64,
    )
    .map_err(|e| e.to_string())
    .and_then(|[w, h]| viewport.prepare(app, (w, h)));
    let reason = match result {
        Ok(true) => {
            app.native_gpu_viewport_enabled = true;
            app.native_gpu_viewport_texture = Some(TEXTURE);
            None
        }
        Ok(false) => {
            app.native_gpu_viewport_enabled = true;
            app.native_gpu_viewport_texture = viewport.frame.as_ref().map(|_| TEXTURE);
            Some("Metal viewport waiting for a completed target".into())
        }
        Err(error) => {
            let unsupported = ingetrazo_metal::world_scene::unsupported_stage(&error);
            app.native_gpu_viewport_enabled = !unsupported;
            app.native_gpu_viewport_texture =
                if app.native_gpu_viewport_enabled && viewport.frame.is_some() {
                    Some(TEXTURE)
                } else {
                    None
                };
            Some(if unsupported {
                format!("Metal viewport compatibility path: {error}")
            } else {
                format!("Metal viewport paused: {error}")
            })
        }
    };
    app.native_metal_world = Some(viewport);
    reason
}

/// Same ingress used by the real window host and the bounded whole-app probe.
pub(crate) fn install(
    painter: &mut ingetrazo_metal::egui_painter::EguiPainter,
    metal: &NativeMetal,
    app: &IngeTrazoApp,
) -> Result<(), String> {
    if let Some(viewport) = app.native_metal_world.as_ref() {
        if let Some(frame) = viewport.frame.as_ref() {
            painter.install_world_texture(metal, TEXTURE, frame.clone())?;
        } else {
            painter.free_textures(&[TEXTURE]);
        }
    }
    Ok(())
}
