//! Deferred capture on the live Metal host, including owned GPU overlays.
use super::State;
use objc2_metal::MTLSharedEvent;
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
    flight: Option<Flight>,
    diagnostic: bool,
    events: Vec<serde_json::Value>,
    block_next_probe: bool,
    release_gate: Option<Box<dyn FnOnce()>>,
}
struct Flight {
    pending: PendingCapture,
    submission: ingetrazo_metal::egui_painter::DiagnosticPaint,
}
impl Default for Owner {
    fn default() -> Self {
        Self {
            cache: None,
            factory: Factory::default(),
            callback: None,
            canvas: Default::default(),
            attachments: Default::default(),
            flight: None,
            diagnostic: false,
            events: Vec::new(),
            block_next_probe: false,
            release_gate: None,
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
    pub(super) fn enable_diagnostics(&mut self) {
        self.diagnostic = true;
    }
    pub(super) fn diagnostics(&self) -> &[serde_json::Value] {
        &self.events
    }
    pub(super) fn block_next_for_probe(&mut self) {
        self.block_next_probe = true;
    }
    pub(super) fn release_gate_for_probe(&mut self) {
        if let Some(release) = self.release_gate.take() {
            release();
        }
    }
    fn overlays(
        &mut self,
        app: &crate::IngeTrazoApp,
        ctx: &egui::Context,
        pending: &PendingCapture,
        metal: Arc<NativeMetal>,
        live_extent: [u32; 2],
        live_samples: usize,
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
                live_extent,
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
            ingetrazo_metal::annotation::source_sample_policy(&metal, live_samples)?.0
        } else {
            0
        };
        let params = Parameters {
            inverse: std::array::from_fn(|i| inverse[i / 4][i % 4]),
            eye: [camera.eye[0], camera.eye[1], camera.eye[2], 0.],
            forward: [camera.forward[0], camera.forward[1], camera.forward[2], 0.],
            extent_count_sample: [live_extent[0], live_extent[1], queries.len() as u32, sample],
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
/// Deadline/cancellation cannot free the serial bridge slot while an actual
/// Metal submission is outstanding. Poll even if ordinary presentation pauses.
pub(super) fn outstanding(state: &State) -> bool {
    state.capture.flight.is_some()
}
pub(super) fn poll(state: &mut State) -> Result<(), String> {
    let retire = state.capture.flight.as_ref().is_some_and(|f| {
        f.submission.completed()
            || f.submission.failed()
            || !f.pending.is_live(&state.app)
            || Instant::now() >= f.pending.request().deadline
    });
    if retire {
        let Flight {
            pending,
            mut submission,
        } = state.capture.flight.take().unwrap();
        let request = pending.request();
        let id = request.id;
        let result = if submission.completed()
            && pending.is_live(&state.app)
            && Instant::now() < request.deadline
        {
            submission
                .wait_readback(Duration::ZERO)
                .map(|rgba| CapturedRgba {
                    width: request.width,
                    height: request.height,
                    rgba,
                })
        } else {
            Err("Metal capture failed/cancelled/expired; submission retired or quarantined".into())
        };
        // Unknown completion quarantines all command/resource owners and
        // poisons the shared backend BEFORE the serial bridge slot is freed.
        if state.capture.diagnostic {
            if let Some(event) = state.capture.events.last_mut() {
                event["retirement"] = serde_json::json!(if submission.completed() {
                    "Completed"
                } else {
                    "Quarantined"
                });
            }
        }
        submission.retire();
        state
            .app
            .finish_ai_bridge_capture(pending, CaptureResult { id, result })?;
    }
    // This path is intentionally ahead of can_prepare(): poisoned presentation
    // cannot otherwise reach after_ui and would strand newly admitted jobs.
    if let Err(error) =
        ingetrazo_metal::egui_painter::EguiPainter::capture_admission(state.gpu.metal())
    {
        if let Some(pending) = state.app.ai_bridge_capture.take() {
            let id = pending.request().id;
            state.app.finish_ai_bridge_capture(
                pending,
                CaptureResult {
                    id,
                    result: Err(error),
                },
            )?;
        }
    }
    Ok(())
}

pub(super) fn after_ui(state: &mut State, ctx: &egui::Context) -> Result<(), String> {
    if state.capture.flight.is_some() {
        return Ok(());
    }
    let Some(pending) = state.app.ai_bridge_capture.take() else {
        return Ok(());
    };
    match capture(state, ctx, &pending) {
        Ok(submission) => {
            state.capture.flight = Some(Flight {
                pending,
                submission,
            });
        }
        Err(error) => {
            let id = pending.request().id;
            state.app.finish_ai_bridge_capture(
                pending,
                CaptureResult {
                    id,
                    result: Err(error),
                },
            )?;
        }
    }
    Ok(())
}
fn capture(
    state: &mut State,
    ctx: &egui::Context,
    pending: &PendingCapture,
) -> Result<ingetrazo_metal::egui_painter::DiagnosticPaint, String> {
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
    if !state.app.component_edit_stack.is_empty() {
        return Err("Metal capture parked edit-context pass is not yet supported".into());
    }
    let request = pending.request();
    let logical = request.stamp.logical_size();
    let metal = state.gpu.shared_metal();
    let mut viewport = state
        .app
        .native_metal_world
        .take()
        .ok_or("Metal capture has no live World owner")?;
    let Some(live_depth) = viewport.frame.as_ref().cloned() else {
        state.app.native_metal_world = Some(viewport);
        return Err("Metal capture live depth owner absent".into());
    };
    let prepared =
        viewport.prepare_capture(&state.app, (request.width, request.height), logical[1]);
    state.app.native_metal_world = Some(viewport);
    let frame = prepared?;
    let live_extent = [live_depth.extent().0, live_depth.extent().1];
    let live_samples = live_depth.sample_count();
    let source_identity = format!("{:p}", Arc::as_ptr(&live_depth));
    let target_identity = format!("{:p}", Arc::as_ptr(&frame));
    let (shapes, callback) = state.capture.overlays(
        &state.app,
        ctx,
        pending,
        metal.clone(),
        [live_depth.extent().0, live_depth.extent().1],
        live_depth.sample_count(),
    )?;
    let output = Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(request.width as f32, request.height as f32),
    );
    let transaction: Result<_, String> = (|| {
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
        let paint = state
            .painter
            .prepare_viewport_capture(&metal, &primitives, 1., live_depth)?;
        Ok(paint)
    })();
    let restored = super::world_viewport::install(&mut state.painter, &metal, &state.app);
    let paint = match (transaction, restored) {
        (result, Ok(())) => result?,
        (_, Err(e)) => return Err(format!("Metal capture live owner restoration failed: {e}")),
    };
    if !pending.is_live(&state.app) {
        return Err("Metal capture cancelled before submission".into());
    }
    remaining(pending)?;
    let blocked = std::mem::take(&mut state.capture.block_next_probe);
    let submission = if blocked {
        state.painter.submit_diagnostic_with_pool(
            &metal,
            paint,
            request.width as usize,
            request.height as usize,
            true,
            &mut state.capture.canvas,
        )?
    } else {
        state.painter.submit_sheet_readback(
            &metal,
            paint,
            request.width as usize,
            request.height as usize,
            &mut state.capture.canvas,
            &mut state.capture.attachments,
        )?
    };
    if blocked {
        state.capture.release_gate = submission
            .probe_gate()
            .map(|gate| Box::new(move || gate.setSignaledValue(1)) as Box<dyn FnOnce()>);
    }
    if state.capture.diagnostic && state.capture.events.len() < 16 {
        state.capture.events.push(
            serde_json::json!({"id":request.id,"requested":[request.width,request.height],
            "logical":logical,"live_depth_extent":live_extent,"live_depth_samples":live_samples,
            "source_frame":source_identity,"target_frame":target_identity,"blocked_gate":blocked,
            "retirement":"Submitted"}),
        );
    }
    Ok(submission)
}
