//! Deferred capture on the live Metal host, including owned GPU overlays.
use super::State;
use crate::{
    ai_bridge_capture::{CaptureResult, CapturedRgba, PendingCapture},
    annotation_overlay_geometry::{Cache, Composition, Limits},
};
use egui::{Pos2, Rect};
use ingetrazo_metal::{
    NativeMetal,
    annotation::{Draw, Parameters, Visibility},
    egui_painter::{AdmittedCallback, SheetReadbackPool},
};
use ingetrazo_render::{
    annotations::Gate,
    cad_callback::{Factory, GlyphOperator},
    cad2d::Pose,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Owner {
    cache: Option<Cache>,
    factory: Factory,
    callback: Option<Arc<AdmittedCallback>>,
    canvas: ingetrazo_metal::compute_canvas::Pool,
    attachments: SheetReadbackPool,
}
impl Default for Owner {
    fn default() -> Self {
        Self {
            cache: None,
            factory: Factory::default(),
            callback: None,
            canvas: Default::default(),
            attachments: Default::default(),
        }
    }
}
fn remaining(pending: &PendingCapture) -> Result<Duration, String> {
    pending
        .request()
        .deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .map(|d| d.min(Duration::from_secs(5)))
        .ok_or_else(|| "Metal capture deadline expired".into())
}
impl Owner {
    fn overlays(
        &mut self,
        app: &crate::IngeTrazoApp,
        ctx: &egui::Context,
        pending: &PendingCapture,
        metal: Arc<NativeMetal>,
    ) -> Result<
        (
            Vec<egui::epaint::ClippedShape>,
            Option<Arc<AdmittedCallback>>,
        ),
        String,
    > {
        let request = pending.request();
        let logical = request.stamp.logical_size();
        if !request.overlays {
            return Ok((vec![], None));
        }
        if self.cache.is_none() {
            self.cache = Some(Cache::new(
                ingetrazo_formats::annotation_font::Limits {
                    faces: 32,
                    retained_font_bytes: 32 * 1024 * 1024,
                },
                Limits {
                    cells: 2_000_000,
                    spans: 200_000,
                    geometry_bytes: 32 * 1024 * 1024,
                    glyphs: 4096,
                    glyph_bytes: 16 * 1024 * 1024,
                },
            )?);
        }
        let cache = self.cache.as_mut().unwrap();
        let presentation = crate::capture_presentation::collect(app, ctx, logical)?;
        let camera = crate::navigation_ray::camera_snapshot(&app.camera)
            .ok_or("Metal capture camera admission")?;
        let matrix = std::array::from_fn(|i| camera.matrix[i / 4][i % 4]);
        let inverse = camera.inverse().ok_or("Metal capture inverse admission")?;
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(logical[0], logical[1]));
        let plan = crate::annotation_plan::build_presentation_plan(app, rect, &matrix, cache)?;
        let scale = [
            f64::from(request.width) / f64::from(logical[0]),
            f64::from(request.height) / f64::from(logical[1]),
        ];
        let mesh = cache.prepare_capture(
            &plan,
            &presentation.text,
            scale,
            96.,
            [0, 0, request.width as i32, request.height as i32],
        )?;
        if mesh.batches.is_empty() {
            return Ok((presentation.shapes, None));
        }
        let mut queries = Vec::new();
        queries
            .try_reserve_exact(plan.queries.len().max(1))
            .map_err(|_| "Metal capture query admission")?;
        let bypass = matches!(
            app.style.visual_style,
            ingetrazo_render::style::VisualStyle::XRay
                | ingetrazo_render::style::VisualStyle::Wireframe
        );
        for &point in &plan.queries {
            let mut q = ingetrazo_render::annotation_query::prepare_query(
                &matrix,
                point,
                camera.eye,
                camera.forward,
                [request.width, request.height],
            )?;
            if bypass {
                q.active = 0;
            }
            queries.push(q);
        }
        let mut draws = Vec::new();
        let mut glyphs = Vec::new();
        draws
            .try_reserve_exact(mesh.gates.len())
            .map_err(|_| "Metal capture draw admission")?;
        glyphs
            .try_reserve_exact(mesh.gates.len())
            .map_err(|_| "Metal capture glyph admission")?;
        for draw in mesh.gates.iter() {
            let gates = match draw.gate {
                Gate::Always => [u32::MAX; 2],
                Gate::Point(a) => [a, u32::MAX],
                Gate::Both(a, b) => [a, b],
            };
            let mut points = [0.; 16];
            let (kind, composition) = match draw.composition {
                Composition::SpanByte => (0, 0),
                Composition::GlyphRgba64Opaque => (0, 1),
                Composition::GlyphLcd(m) => (
                    0,
                    2 | (u32::from(m[0]) << 8) | (u32::from(m[1]) << 16) | (u32::from(m[2]) << 24),
                ),
                Composition::WidgetTriangle(bits) => {
                    for (i, b) in bits.into_iter().enumerate() {
                        points[i] = f32::from_bits(b);
                    }
                    (1, 0)
                }
                _ => return Err("Metal capture source overlay operator unsupported".into()),
            };
            draws.push(Draw {
                gates,
                kind,
                padding: 0,
                points,
            });
            glyphs.push(GlyphOperator {
                composition,
                straight_rgba: draw.straight_rgba,
            });
        }
        let sample = if queries.iter().any(|q| q.active != 0) {
            ingetrazo_metal::annotation::source_sample_policy(&metal, 4)?.0
        } else {
            0
        };
        let params = Parameters {
            inverse: std::array::from_fn(|i| inverse[i / 4][i % 4]),
            eye: [camera.eye[0], camera.eye[1], camera.eye[2], 0.],
            forward: [camera.forward[0], camera.forward[1], camera.forward[2], 0.],
            extent_count_sample: [request.width, request.height, queries.len() as u32, sample],
        };
        let visibility = Visibility::prepare(
            &metal,
            &queries,
            &draws,
            params,
            self.callback
                .as_ref()
                .and_then(|old| old.annotation_visibility()),
        )?;
        let handle = self.factory.glyph_handle(
            mesh.geometry.clone(),
            mesh.batches.clone(),
            Pose::default(),
            glyphs.into(),
        )?;
        let callback = AdmittedCallback::prepare_annotation(
            metal,
            handle.snapshot()?,
            1.,
            self.callback.as_deref(),
            Some(visibility),
        )?;
        self.callback = Some(callback.clone());
        Ok((presentation.shapes, Some(callback)))
    }
}
pub(super) fn after_ui(state: &mut State, ctx: &egui::Context) -> Result<(), String> {
    let Some(pending) = state.app.ai_bridge_capture.take() else {
        return Ok(());
    };
    let id = pending.request().id;
    let result = capture(state, ctx, &pending);
    // Readback completed, or the Metal submission quarantined its retained
    // owners; only now acknowledge the serial deferred bridge slot.
    state
        .app
        .finish_ai_bridge_capture(pending, CaptureResult { id, result })?;
    Ok(())
}
fn capture(
    state: &mut State,
    ctx: &egui::Context,
    pending: &PendingCapture,
) -> Result<CapturedRgba, String> {
    if !pending.is_live(&state.app) {
        return Err("Metal capture cancelled before preparation".into());
    }
    remaining(pending)?;
    if !state.world_submitted {
        return Err("Metal viewport has not presented".into());
    }
    if state.app.scene.active_sheet_index.is_some() {
        return Err("Metal capture requires model viewport".into());
    }
    if state.app.scene.terrain.as_ref().is_some_and(|t| t.visible)
        || state
            .app
            .scene
            .tile_layer
            .as_ref()
            .is_some_and(|t| t.visible)
        || state
            .app
            .scene
            .guides
            .iter()
            .any(|g| !matches!(&g.kind, ingetrazo_core::guides::GuideType::Point(_)))
    {
        return Err("Metal capture terrain/tile/line-guide route unsupported".into());
    }
    let request = pending.request();
    let logical = request.stamp.logical_size();
    let metal = state.gpu.shared_metal();
    let mut viewport = state
        .app
        .native_metal_world
        .take()
        .ok_or("Metal capture has no live World owner")?;
    let prepared =
        viewport.prepare_capture(&state.app, (request.width, request.height), logical[1]);
    state.app.native_metal_world = Some(viewport);
    let frame = prepared?;
    let (shapes, callback) = state
        .capture
        .overlays(&state.app, ctx, pending, metal.clone())?;
    let output = Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(request.width as f32, request.height as f32),
    );
    let transaction = (|| {
        state
            .painter
            .install_world_texture(&metal, super::world_viewport::TEXTURE, frame)?;
        state.painter.install_admitted_callback(callback.clone());
        let mut quad = egui::Mesh::with_texture(super::world_viewport::TEXTURE);
        quad.add_rect_with_uv(
            output,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::WHITE,
        );
        let mut primitives = vec![egui::ClippedPrimitive {
            clip_rect: output,
            primitive: egui::epaint::Primitive::Mesh(quad),
        }];
        let scale = [
            request.width as f32 / logical[0],
            request.height as f32 / logical[1],
        ];
        let mut overlays = ctx.tessellate(shapes, 1.);
        for primitive in &mut overlays {
            primitive.clip_rect = Rect::from_min_max(
                egui::pos2(
                    primitive.clip_rect.min.x * scale[0],
                    primitive.clip_rect.min.y * scale[1],
                ),
                egui::pos2(
                    primitive.clip_rect.max.x * scale[0],
                    primitive.clip_rect.max.y * scale[1],
                ),
            );
            if let egui::epaint::Primitive::Mesh(mesh) = &mut primitive.primitive {
                for vertex in &mut mesh.vertices {
                    vertex.pos.x *= scale[0];
                    vertex.pos.y *= scale[1];
                }
            }
        }
        primitives.extend(overlays);
        if let Some(callback) = callback {
            primitives.push(egui::ClippedPrimitive {
                clip_rect: output,
                primitive: egui::epaint::Primitive::Callback(egui::PaintCallback {
                    rect: output,
                    callback: callback.token(),
                }),
            });
        }
        let paint = state.painter.prepare(&metal, &primitives, 1.)?;
        if !pending.is_live(&state.app) {
            return Err("Metal capture cancelled before submission".into());
        }
        remaining(pending)?;
        let mut submission = state.painter.submit_sheet_readback(
            &metal,
            paint,
            request.width as usize,
            request.height as usize,
            &mut state.capture.canvas,
            &mut state.capture.attachments,
        )?;
        let rgba = submission.wait_readback(remaining(pending)?)?;
        if !pending.is_live(&state.app) {
            return Err("Metal capture cancelled after completion".into());
        }
        Ok(CapturedRgba {
            width: request.width,
            height: request.height,
            rgba,
        })
    })();
    let restored = super::world_viewport::install(&mut state.painter, &metal, &state.app);
    match (transaction, restored) {
        (result, Ok(())) => result,
        (_, Err(e)) => Err(format!("Metal capture live owner restoration failed: {e}")),
    }
}
