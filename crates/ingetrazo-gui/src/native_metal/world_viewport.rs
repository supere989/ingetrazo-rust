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
    labels: Option<super::distance_label::Labels>,
    pub(crate) frame: Option<Arc<Prepared>>,
    pub(crate) last_route_reason: Option<String>,
    capture_stamp: Option<(crate::ai_bridge_capture::CaptureStamp, RenderRevisions, u64, u64)>,
    unsupported: Option<(ingetrazo_render::scene_revision::Revisions, u16, String)>,
}
impl Viewport {
    pub(crate) fn capture_stamp_matches(&self, app: &IngeTrazoApp) -> bool {
        self.last_route_reason.is_none() && self.capture_stamp.as_ref().is_some_and(|(stamp, revisions, view, geometry)|
            stamp.matches(app) && *revisions == app.native_render_revisions && *view == app.scene.view_version && *geometry == app.native_geometry_revision)
    }
    pub(crate) fn capture_annotation_owner(&self, app: &IngeTrazoApp) -> Option<Arc<ingetrazo_metal::egui_painter::AdmittedCallback>> {
        self.labels.as_ref().and_then(|labels| labels.document_owner(app))
    }
    pub(crate) fn document_annotation_owner_for_probe(&self,app:&IngeTrazoApp)->bool{self.labels.as_ref().and_then(|labels|labels.document_owner(app)).is_some()}
    pub(crate) fn label_owner(&self)->Option<Arc<ingetrazo_metal::egui_painter::AdmittedCallback>>{self.labels.as_ref().and_then(|l|l.admitted())}
    pub(crate) fn label_stats(&self)->(u64,u64){self.labels.as_ref().map(|l|l.stats()).unwrap_or((0,0))}
    pub(crate) fn probe_font_budget(&mut self,refuse:bool)->Result<(),String>{self.labels.as_mut().ok_or("No prepared label owner")?.probe_font_budget(refuse)}
    pub(crate) fn shared_billboard_images(&self)->usize{self.frame.as_ref().map_or(0,|frame|self.world.shared_billboard_texture_owners(frame))}
    pub(crate) fn sweep_stats(&self)->ingetrazo_metal::world::Stats{self.world.stats}
    pub(crate) fn new(metal: Arc<NativeMetal>) -> Self {
        Self {
            metal,
            world: World::default(),
            adapter: Adapter::default(),
            labels: None,
            frame: None,
            unsupported: None,
            capture_stamp: None,
            last_route_reason: None,
        }
    }
    /// Prepare a requested capture target on the actual live World/Adapter.
    /// The returned Arc owns its target; the ordinary frame remains retained
    /// and is restored on every path. No document or camera publication occurs.
    pub(crate) fn prepare_capture(
        &mut self,
        app: &IngeTrazoApp,
        extent: (u32, u32),
        logical_height: f32,
    ) -> Result<Arc<Prepared>, String> {
        if extent.0 < 64 || extent.1 < 64 || extent.0 > 4096 || extent.1 > 4096 {
            return Err("Metal capture extent outside source admission".into());
        }
        let live_frame = self.frame.take();
        let live_stamp = self.capture_stamp.take();
        let live_unsupported = self.unsupported.take();
        let live_reason = self.last_route_reason.take();
        let annotation_owner=self.labels.as_ref().and_then(|labels|labels.document_owner(app));
        let result = self.prepare(app, extent, logical_height,annotation_owner.as_deref())
            .and_then(|accepted| {
                if accepted {
                    self.frame.take().ok_or_else(|| "Metal capture prepared no owned target".into())
                } else {
                    Err("Metal capture target owner unavailable".into())
                }
            });
        self.frame = live_frame;
        self.capture_stamp = live_stamp;
        self.unsupported = live_unsupported;
        self.last_route_reason = live_reason;
        result
    }
    pub(crate) fn catalog_builds(&self) -> u64 {
        self.adapter.catalog_builds
    }
    pub(crate) fn model_refreshes(&self) -> u64 {
        self.adapter.accepted_model_refreshes
    }
    pub(crate) fn matrix_bytes(&self) -> u64 {
        self.world.stats.instance_matrix_upload_bytes
    }
    pub(crate) fn sampled_backing_kinds(&self)->Result<Vec<&'static str>,String>{self.world.sampled_backing_kinds()}
    pub(crate) fn sampled_heaps(&self)->Result<Vec<[u64;3]>,String>{self.world.sampled_heap_allocations()}
    pub(crate) fn texture_uploads(&self)->u64{self.world.stats.texture_uploads}
    pub(crate) fn material_order_builds(&self)->u64{self.world.stats.material_order_builds}
    pub(crate) fn geometry_uploads(&self) -> u64 {
        self.world.stats.geometry_uploads
    }
    pub(crate) fn ghost_metrics(&self)->(u64,u64,u64,u64,u64){(self.world.stats.ghost_builds,self.world.stats.ghost_uploads,self.world.stats.ghost_upload_bytes,self.world.stats.wire_uploads,self.world.stats.wire_upload_bytes)}
    fn record_route_reason(&mut self,reason:Option<String>) {
        if self.last_route_reason!=reason {
            if let Some(reason)=&reason {eprintln!("Native Metal viewport route: {reason}");}
            else if self.last_route_reason.is_some(){eprintln!("Native Metal viewport route: native World accepted");}
            self.last_route_reason=reason;
        }
    }
    fn prepare(&mut self, app: &IngeTrazoApp, extent: (u32, u32),logical_height:f32,annotations:Option<&ingetrazo_metal::egui_painter::AdmittedCallback>) -> Result<bool, String> {
        // Capture metadata is optional for ordinary painting and staged before
        // any World/frame publication. Active-edit capture remains unsupported.
        let capture_stamp = if app.component_edit_stack.is_empty() {
            crate::ai_bridge_capture::CaptureStamp::for_app(app).ok().map(|stamp|
                (stamp, app.native_render_revisions, app.scene.view_version, app.native_geometry_revision))
        } else { None };
        let RenderRevisions {
            static_domains,
            poses,
        } = app.native_render_revisions;
        // Selection and nonpersistent viewport flags are not modeling epochs.
        let preview=app.active_tool.face_sweep_preview().map(|p|ingetrazo_render::face_sweep::Input{face:p.face,face_identity:p.face_identity,mesh_revision:p.mesh_revision,anchor:p.anchor,normal:p.normal,offset:p.offset,attached:p.attached,prism_cap:p.prism_cap,keep_base:p.keep_base});
        let capability = (app.style.show_edges as u16)
            | ((app.style.show_profiles as u16) << 1)
            | ((app.style.show_sky as u16) << 2)
            | ((app.style.show_grid as u16) << 3)
            | ((!app.scene.selection.is_empty() as u16) << 4)
            | ((app.scene.show_section_cuts as u16) << 5)
            | ((app.style.visual_style as u16) << 6) | ((preview.is_some() as u16)<<10) | ((app.style.show_soft_edges as u16)<<11) | ((app.style.back_edges_hidden as u16)<<12) | ((annotations.is_some() as u16)<<13);
        if let Some((revision, previous, reason)) = &self.unsupported {
            if *revision == static_domains && *previous == capability {
                return Err(reason.clone());
            }
        }
        // Frozen source ghost raster uses four samples. Refuse unsupported native
        // sample capability instead of silently substituting single-sample pixels.
        self.world.set_sample_count(&self.metal,4)?;
        self.world.set_viewport_logical_height(logical_height)?;
        // Same Qt-f32/OpenGL matrix as the Vulkan world consumer. The Metal
        // vertex shader performs the native [-w,w] -> [0,w] depth mapping.
        let source_camera=crate::navigation_ray::camera_snapshot(&app.camera)
            .ok_or("Metal source camera arithmetic admission")?;
        let camera=source_camera.matrix;
        match self.world.prepare_scene_preview_with_annotations(
            &self.metal,
            &mut self.adapter,
            &app.scene,
            static_domains,
            Some(poses),
            &app.camera,
            &app.style,
            camera,
            extent,
            preview,
            Some(glam::DVec3::from_array(source_camera.eye.map(f64::from))),
            annotations,
        ) {
            Ok(Some(frame)) => {
                self.unsupported = None;
                self.frame = Some(frame);
                self.capture_stamp = capture_stamp;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(error) => {
                if ingetrazo_metal::world_scene::unsupported_stage(&error) {
                    self.unsupported = Some((static_domains, capability, error.clone()));
                    // Retain the accepted target while compatibility rendering is selected.
                    // A rejected candidate must not retire its paired annotation owner.
                }
                Err(error)
            }
        }
    }
}
/// Called after current-frame pointer/navigation edits and before paint choice.
pub(crate) fn route(app: &mut IngeTrazoApp, rect: egui::Rect, density: f32) -> Option<String> {
    let mut viewport = app.native_metal_world.take()?;
    // Admit the complete label privately before any world publication. The
    // candidate retains native buffers but cannot alter the accepted callback.
    let candidate=(|| -> Result<super::distance_label::Candidate,String> {
        if viewport.labels.is_none(){viewport.labels=Some(super::distance_label::Labels::new()?);}
        viewport.labels.as_mut().unwrap().prepare(app,rect,density,viewport.metal.clone())
    })();
    let candidate=match candidate{Ok(c)=>c,Err(error)=>{viewport.record_route_reason(Some(error.clone()));app.native_metal_world=Some(viewport);return Some(error)}};
    let annotation_owner=candidate.document_owner();
    let result = crate::viewport_extent::scene_extent(
        [rect.width() as f64, rect.height() as f64],
        density as f64,
    )
    .map_err(|e| e.to_string())
    .and_then(|[w, h]| viewport.prepare(app, (w, h),rect.height(),annotation_owner.as_deref()));
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
    if app.native_gpu_viewport_enabled && reason.is_none() {
        app.native_gpu_annotation_shape=candidate.shape(rect);
        viewport.labels.as_mut().unwrap().publish(candidate);
    } else if !app.native_gpu_viewport_enabled { app.native_gpu_annotation_shape=None; }
    viewport.record_route_reason(reason.clone());
    app.native_metal_world = Some(viewport);
    reason
}

/// Same ingress used by the real window host and the bounded whole-app probe.
pub(crate) fn install(
    painter: &mut ingetrazo_metal::egui_painter::EguiPainter,
    metal: &NativeMetal,
    app: &IngeTrazoApp,
) -> Result<(), String> {
    painter.install_admitted_callback(app.native_metal_world.as_ref().and_then(|v|v.labels.as_ref()).and_then(|labels|labels.admitted()));
    if let Some(viewport) = app.native_metal_world.as_ref() {
        if let Some(frame) = viewport.frame.as_ref() {
            painter.install_world_texture(metal, TEXTURE, frame.clone())?;
        } else {
            painter.free_textures(&[TEXTURE]);
        }
    }
    Ok(())
}
