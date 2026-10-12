//! Requested viewport capture on the existing System device/painter boundary.
use super::{capture_target, System};
use crate::ai_bridge_capture::{CapturedRgba, OwnedCapture, PendingCapture};
use ash::vk;
use egui::{Pos2, Rect};
use std::time::Instant;

fn remaining(request: &OwnedCapture) -> Result<u64, String> {
    let remaining = request
        .deadline
        .checked_duration_since(Instant::now())
        .ok_or("Capture deadline expired")?;
    Ok(u64::try_from(remaining.as_nanos())
        .unwrap_or(u64::MAX)
        .min(super::lifetime::WAIT_NS))
}
fn submit(
    system: &mut System,
    app: &crate::IngeTrazoApp,
    pending: &PendingCapture,
    record: impl FnOnce(&mut System, vk::CommandBuffer, u64) -> Result<(), String>,
) -> Result<(), String> {
    let request = pending.request();
    if !pending.is_live(app) {
        return Err("Capture cancelled before GPU submission".into());
    }
    let wait = remaining(request)?;
    system
        .vulkan_context
        .lifetime
        .wait(system.fence, wait)
        .map_err(|e| e.to_string())?;
    let command = system.command_buffer;
    let serial = system
        .draw_serial
        .checked_add(1)
        .ok_or("Capture serial overflow")?;
    unsafe {
        system
            .vulkan_context
            .device
            .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
            .map_err(|e| e.to_string())?;
        system
            .vulkan_context
            .device
            .begin_command_buffer(
                command,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .map_err(|e| e.to_string())?;
    }
    record(system, command, serial)?;
    if !pending.is_live(app) {
        return Err("Capture cancelled before GPU submission".into());
    }
    let wait = remaining(request)?;
    unsafe {
        system
            .vulkan_context
            .device
            .end_command_buffer(command)
            .map_err(|e| e.to_string())?;
        system
            .vulkan_context
            .device
            .reset_fences(&[system.fence])
            .map_err(|e| e.to_string())?;
        system
            .vulkan_context
            .device
            .queue_submit(
                system.vulkan_context.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&[command])],
                system.fence,
            )
            .map_err(|e| {
                system.vulkan_context.lifetime.quarantine();
                e.to_string()
            })?;
    }
    system.draw_serial = serial;
    system
        .vulkan_context
        .lifetime
        .wait(system.fence, wait)
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl System {
    /// Called by the UI owner after its previous frame completed. The live
    /// descriptor is never replaced; capture owns an independent temporary ID.
    pub(crate) fn capture_viewport(
        &mut self,
        app: &mut crate::IngeTrazoApp,
        pending: &PendingCapture,
    ) -> Result<CapturedRgba, String> {
        let request = pending.request();
        if !pending.is_live(app) {
            return Err("Capture cancelled before preparation".into());
        }
        self.vulkan_context.lifetime.check()?;
        remaining(request)?;
        if !request.stamp.matches(app) {
            return Err("Capture context changed before rendering".into());
        }
        if app.scene.active_sheet_index.is_some() {
            return Err("Viewport capture requires model viewport".into());
        }
        if self.world.is_none() || !self.world_presented {
            return Err("Native world has not presented a viewport".into());
        }
        self.vulkan_context
            .lifetime
            .wait(self.fence, remaining(request)?)
            .map_err(|e| e.to_string())?;
        let logical = request.stamp.logical_size();
        let logical_rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(logical[0], logical[1]));
        let extent = vk::Extent2D {
            width: request.width,
            height: request.height,
        };
        // QPainter scales in qreal: retain the requested/logical ratio in f64
        // through physical path and glyph rasterization, before GPU vertex conversion.
        let raster_scale = [
            f64::from(request.width) / f64::from(logical[0]),
            f64::from(request.height) / f64::from(logical[1]),
        ];
        let scale = raster_scale.map(|v| v as f32);
        let output_rect = Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(request.width as f32, request.height as f32),
        );
        if app
            .scene
            .guides
            .iter()
            .any(|g| !matches!(&g.kind, ingetrazo_core::guides::GuideType::Point(_)))
        {
            return Err("Native capture line-guide depth pass is not yet supported".into());
        }
        if app.scene.terrain.as_ref().is_some_and(|t| t.visible)
            || app.scene.tile_layer.as_ref().is_some_and(|t| t.visible)
        {
            return Err("Native capture terrain/tile world pass is not yet supported".into());
        }
        let mut presentation = if request.overlays {
            crate::capture_presentation::collect(app, &self.egui_ctx, logical)?
        } else {
            crate::capture_presentation::Presentation {
                shapes: Vec::new(),
                text: Vec::new(),
            }
        };
        if !matches!(
            self.swapchain.format,
            vk::Format::R8G8B8A8_UNORM
                | vk::Format::R8G8B8A8_SRGB
                | vk::Format::B8G8R8A8_UNORM
                | vk::Format::B8G8R8A8_SRGB
        ) {
            return Err("Capture output format is not admitted RGBA8".into());
        }
        let target = capture_target::Target::new(
            &self.vulkan_context,
            self.memory_budget.clone(),
            extent,
            self.swapchain.format,
            self.swapchain.integer_cad,
            self.swapchain.stencil_enabled,
        )?;
        // Prepare annotation queries against the retained LIVE depth target,
        // before requested world rendering swaps targets (Qt render_image law).
        let mut styled_text = None;
        let annotation = if request.overlays {
            let factory = self.annotation_factory()?;
            let mut owner = match self.annotation_viewport.take() {
                Some(owner) => owner,
                None => super::annotation_viewport::ViewportAnnotations::new(
                    self.swapchain.stencil_enabled,
                )?,
            };
            if !presentation.text.is_empty() {
                let mesh = owner.presentation_text(&presentation.text,raster_scale)?;
                styled_text = Some(factory.annotation_handle(
                    mesh.geometry.clone(),
                    mesh.batches.clone(),
                    ingetrazo_render::cad2d::Pose {
                        scale: [1.;2],
                        translation: [0.; 2],
                    },
                    mesh.gates.clone(),
                )?);
            }
            let prepared = owner
                .prepare_presentation(app, &factory, logical_rect, 1., raster_scale)
                .map(|p| {
                    p.map(|p| {
                        (
                            p.handle.clone(),
                            p.queries.clone(),
                            p.inverse,
                            p.eye,
                            p.forward,
                        )
                    })
                });
            self.annotation_viewport = Some(owner);
            if let Some((handle, queries, inverse, eye, forward)) = prepared? {
                let renderer = self
                    .annotation_cad
                    .as_mut()
                    .ok_or("Capture annotation renderer absent")?;
                self.world
                    .as_mut()
                    .unwrap()
                    .prepare_annotation_overlay_transaction(
                        renderer,
                        &handle,
                        &self.vulkan_context,
                        queries,
                        inverse,
                        eye,
                        forward,
                        self.annotation_sample.unwrap_or(0),
                        self.fence,
                        self.draw_serial,
                        self.draw_serial + 1,
                        1.,
                        |_| Ok(()),
                    )?;
                submit(self, app, pending, |system, command, serial| {
                    system
                        .world
                        .as_mut()
                        .unwrap()
                        .record_annotations_if_prepared(command, serial)
                })?;
                Some(handle)
            } else if let Some(handle) = &styled_text {
                let snapshot = crate::navigation_ray::camera_snapshot(&app.camera)
                    .ok_or("Capture annotation camera invalid")?;
                let inverse = snapshot
                    .inverse()
                    .ok_or("Capture annotation inverse invalid")?;
                let inverse = std::array::from_fn(|i| inverse[i / 4][i % 4]);
                let query = super::annotation_visibility::Query {
                    pixel: [0; 2],
                    normalized: [0.; 2],
                    threshold: 0.,
                    active: 0,
                    _padding: [0; 2],
                };
                self.world
                    .as_mut()
                    .unwrap()
                    .prepare_annotation_overlay_transaction(
                        self.annotation_cad.as_mut().unwrap(),
                        handle,
                        &self.vulkan_context,
                        vec![query].into(),
                        inverse,
                        snapshot.eye,
                        snapshot.forward,
                        0,
                        self.fence,
                        self.draw_serial,
                        self.draw_serial + 1,
                        1.,
                        |_| Ok(()),
                    )?;
                submit(self, app, pending, |system, command, serial| {
                    system
                        .world
                        .as_mut()
                        .unwrap()
                        .record_annotations_if_prepared(command, serial)
                })?;
                None
            } else {
                None
            }
        } else {
            None
        };
        // Reversible native painter bindings keep accepted live pass objects,
        // including visibility descriptors, until capture completion is known.
        let mut ordinary_pass = None;
        let mut annotation_pass = None;
        let result = (|| {
            if let Some(cad) = &mut self.cad2d {
                let prepared = cad
                    .prepare_render_pass(target.pass, &[target.view()])
                    .map_err(|e| e.to_string())?;
                ordinary_pass = Some(cad.stage_capture_pass(prepared)?);
                cad.set_input_image(0)?;
            }
            if let Some(cad) = &mut self.annotation_cad {
                let prepared = cad
                    .prepare_render_pass(target.pass, &[target.view()])
                    .map_err(|e| e.to_string())?;
                annotation_pass = Some(cad.stage_capture_pass(prepared)?);
                cad.set_input_image(0)?;
            }
            if annotation.is_some() || styled_text.is_some() {
                self.world.as_ref().unwrap().bind_annotation_overlay(
                    self.annotation_cad.as_mut().unwrap(),
                    &self.vulkan_context,
                    self.fence,
                    self.draw_serial,
                )?;
            }
            let mut world = self.world.take().unwrap();
            let world_target = match world.stage_capture_target(&self.vulkan_context, extent) {
                Ok(target) => target,
                Err(error) => {
                    self.world = Some(world);
                    return Err(error);
                }
            };
            let captured = (|| {
                // Reuse the actual World preparation, including depth-tested
                // tool solids retained by source paintGL during export. Only
                // presentation snap/hover/rubber-band painters are absent.
                super::prepare_app_world(
                    app,
                    &mut world,
                    &self.vulkan_context,
                    self.fence,
                    self.draw_serial,
                )?;
                submit(self, app, pending, |_, command, serial| {
                    world.record(command, serial)
                })?;
                let texture = self
                    .renderer
                    .add_bottom_origin_encoded_opaque_texture(world.capture_descriptor()?);
                let composited = (|| {
                    let mut quad = egui::Mesh::with_texture(texture);
                    quad.add_rect_with_uv(
                        output_rect,
                        Rect::from_min_max(Pos2::ZERO, egui::pos2(1., 1.)),
                        egui::Color32::WHITE,
                    );
                    let mut primitives = vec![egui::ClippedPrimitive {
                        clip_rect: output_rect,
                        primitive: egui::epaint::Primitive::Mesh(quad),
                    }];
                    let mut overlays = self
                        .egui_ctx
                        .tessellate(std::mem::take(&mut presentation.shapes), 1.);
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
                    if let Some(handle) = &annotation {
                        let egui::Shape::Callback(callback) = handle.shape(output_rect) else {
                            return Err("Capture annotation callback absent".into());
                        };
                        primitives.push(egui::ClippedPrimitive {
                            clip_rect: output_rect,
                            primitive: egui::epaint::Primitive::Callback(callback),
                        });
                    }
                    if let Some(handle) = &styled_text {
                        let egui::Shape::Callback(callback) = handle.shape(output_rect) else {
                            return Err("Capture styled text callback absent".into());
                        };
                        primitives.push(egui::ClippedPrimitive {
                            clip_rect: output_rect,
                            primitive: egui::epaint::Primitive::Callback(callback),
                        });
                    }
                    let delta = self.egui_ctx.tex_manager().write().take_delta();
                    super::retain_frame_texture_deltas(
                        &mut self.pending_texture_updates,
                        &mut self.textures_to_free,
                        delta.set,
                        &delta.free,
                    );
                    self.upload_pending_textures()?;
                    if let Some(cad) = &mut self.cad2d {
                        cad.prepare(
                            &self.vulkan_context,
                            &primitives,
                            self.fence,
                            self.draw_serial,
                            self.draw_serial + 1,
                            1.,
                        )?;
                    }
                    if let Some(cad) = &mut self.annotation_cad {
                        if annotation.is_some() || styled_text.is_some() {
                            cad.prepare(
                                &self.vulkan_context,
                                &primitives,
                                self.fence,
                                self.draw_serial,
                                self.draw_serial + 1,
                                1.,
                            )?;
                        }
                    }
                    submit(self, app, pending, |system, command, _| {
                        let clear = [
                            vk::ClearValue {
                                color: vk::ClearColorValue {
                                    float32: [0., 0., 0., 1.],
                                },
                            },
                            vk::ClearValue {
                                depth_stencil: vk::ClearDepthStencilValue {
                                    depth: 1.,
                                    stencil: 0,
                                },
                            },
                        ];
                        let begin = vk::RenderPassBeginInfo::default()
                            .render_pass(target.pass)
                            .framebuffer(target.framebuffer)
                            .render_area(vk::Rect2D {
                                offset: vk::Offset2D { x: 0, y: 0 },
                                extent,
                            })
                            .clear_values(&clear);
                        unsafe {
                            system.vulkan_context.device.cmd_begin_render_pass(
                                command,
                                &begin,
                                vk::SubpassContents::INLINE,
                            );
                        }
                        let draw = system
                            .renderer
                            .cmd_draw(command, extent, 1., &primitives)
                            .map_err(|e| e.to_string());
                        unsafe {
                            system.vulkan_context.device.cmd_end_render_pass(command);
                        }
                        draw
                    })?;
                    let mut resources = super::resources::Resources::new(
                        &self.vulkan_context,
                        self.memory_budget.clone(),
                    );
                    let mut rgba = resources.readback_image_until(
                        &self.vulkan_context,
                        target.image(),
                        extent,
                        request.deadline,
                    )?;
                    if matches!(
                        target.format,
                        vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
                    ) {
                        for pixel in rgba.chunks_exact_mut(4) {
                            pixel.swap(0, 2);
                        }
                    }
                    if !request.stamp.matches(app) || Instant::now() >= request.deadline {
                        return Err("Capture retired before publication".into());
                    }
                    Ok(CapturedRgba {
                        width: request.width,
                        height: request.height,
                        rgba,
                    })
                })();
                self.renderer.remove_user_texture(texture);
                composited
            })();
            let restore = world.finish_capture_target(&self.vulkan_context, world_target);
            self.world = Some(world);
            restore?;
            captured
        })();
        if self.vulkan_context.lifetime.uncertain() {
            if let Some(pass) = ordinary_pass.take() {
                self.vulkan_context.lifetime.retain(pass);
            }
            if let Some(pass) = annotation_pass.take() {
                self.vulkan_context.lifetime.retain(pass);
            }
            return Err("Capture GPU completion unknown; owners quarantined".into());
        }
        let mut restoration = Ok(());
        if let Some(pass) = annotation_pass {
            if let Some(cad) = &mut self.annotation_cad {
                restoration = cad.restore_capture_pass(pass).map(drop);
            }
        }
        if let Some(pass) = ordinary_pass {
            if let Some(cad) = &mut self.cad2d {
                let restored = cad.restore_capture_pass(pass).map(drop);
                if restoration.is_ok() {
                    restoration = restored;
                }
            }
        }
        restoration?;
        result
    }
}
