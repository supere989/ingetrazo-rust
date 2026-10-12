// SPDX-License-Identifier: GPL-3.0-or-later
//! Physical sheet records for direct native export, independent of editor paint.
use ingetrazo_core::composition::{Sheet, SheetItem};
use ingetrazo_formats::{paper_text_painter::Painter, sheet_image::Metrics};
use ingetrazo_render::cad2d::{Batch, Geometry};
use std::sync::Arc;
pub type Records = (Arc<Geometry>, Arc<[Batch]>);
pub(crate) enum Record {
    Cad(Records),
    Picture(Arc<ingetrazo_render::paper_picture::Callback>),
}
impl From<Records> for Record {
    fn from(v: Records) -> Self {
        Self::Cad(v)
    }
}
pub struct Plan {
    pub metrics: Metrics,
    pub(crate) records: Vec<Record>,
}
/// Cold export preparation. Every visible unsupported painter is reported
/// together, before native submission or opening a destination temporary file.
pub fn unsupported(sheet: &Sheet) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["perfiles"    ] {
        if let Some(items) = sheet.native_document.get(key).and_then(|v| v.as_array()) {
            for (i, item) in items.iter().enumerate() {
                if !item
                    .get("hidden")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    out.push(format!("{key}[{i}]"));
                }
            }
        }
    }
    out
}
#[derive(Default)]
pub struct PainterCache {
    text: crate::composer::text_canvas::CanvasPainter,
    fonts: Painter,
    pub(crate) pictures: crate::composer_picture::Cache,
}
impl PainterCache {
    /// Cold picture decode/composite counters; immutable pixel owners stay private.
    pub fn picture_stats(&self) -> (u64, u64) {
        (self.pictures.decodes, self.pictures.builds)
    }

    pub fn prepare(
        &mut self,
        ctx: &egui::Context,
        sheet: &Sheet,
        dpi: u16,
    ) -> Result<Plan, String> {
        self.prepare_with_views(ctx, sheet, dpi, None, None)
    }
    fn prepare_with_views(
        &mut self,
        ctx: &egui::Context,
        sheet: &Sheet,
        dpi: u16,
        views: Option<&std::collections::HashMap<uuid::Uuid, crate::composer::pdf::ViewDraws>>,
        context: Option<&crate::composer_text_fields::Context<'_>>,
    ) -> Result<Plan, String> {
        let unsupported = unsupported(sheet);
        if !unsupported.is_empty() {
            return Err(format!(
                "Sheet image painters are not supported: {}",
                unsupported.join(", ")
            ));
        }
        let (w, h) = sheet.dimensions_mm();
        let metrics = Metrics::new([w, h], dpi)?;
        let density = f64::from(dpi) / 25.4;
        let mut records = Vec::new();
        let mut bytes = 0u64;
        let mut picture_owners = std::collections::HashSet::new();
        let mut append = |part: Record| -> Result<(), String> {
            let n = match &part {
                Record::Cad((geometry, batches)) => geometry
                    .logical_bytes()
                    .checked_add(geometry.source_spans().map_or(0, |s| s.logical_bytes()))
                    .and_then(|n| {
                        n.checked_add((batches.len() * std::mem::size_of::<Batch>()) as u64)
                    })
                    .ok_or("Sheet records size overflow")?,
                Record::Picture(callback) => {
                    let snapshot = callback.snapshot()?;
                    let sampling = &snapshot.sampling;
                    let image = if picture_owners.insert(sampling.image.id()) {
                        sampling.image.bytes()
                    } else {
                        0
                    };
                    sampling
                        .bytes()
                        .checked_add(image)
                        .ok_or("Sheet picture owner charge overflow")? as u64
                }
            };
            bytes = bytes
                .checked_add(n)
                .filter(|&n| n <= 64 * 1024 * 1024)
                .ok_or("Sheet image records exceed64MiB")?;
            records
                .try_reserve(1)
                .map_err(|_| "Sheet records allocation failed")?;
            records.push(part);
            Ok(())
        };
        for exported in sheet.physical_export_items()? {
            let (index, item) = match exported {
                ingetrazo_core::composition::PhysicalExportItem::Typed { index, item } => {
                    (index, item)
                }
                ingetrazo_core::composition::PhysicalExportItem::Picture { value, .. } => {
                    let picture = self.pictures.draws(value)?;
                    for (index, mut draw) in picture.draws.into_iter().enumerate() {
                        if let Some(image) = draw.picture.take() {
                            let sampling = ingetrazo_render::paper_picture::Sampling::new(
                                &image,
                                density,
                                metrics.pixels(),
                            )?;
                            append(Record::Picture(
                                ingetrazo_render::paper_picture::Callback::new(sampling),
                            ))?;
                            continue;
                        }
                        if picture.rectangles.binary_search(&index).is_ok() {
                            for part in self.text.physical_rectangle(&draw, density)? {
                                append(part.into())?;
                            }
                            continue;
                        }
                        let parts = if picture.source_lines.binary_search(&index).is_ok() {
                            crate::sheet_image_paths::source_line_records(
                                &draw,
                                density,
                                metrics.pixels(),
                            )?
                        } else {
                            crate::sheet_image_paths::records(
                                std::slice::from_ref(&draw),
                                density,
                                metrics.pixels(),
                            )?
                        };
                        for part in parts {
                            append(part.into())?;
                        }
                    }
                    continue;
                }
                ingetrazo_core::composition::PhysicalExportItem::Annotation{key,value,..}=>{
                    let fallback_date;
                    let fallback;
                    let fields=if let Some(fields)=context{fields}else{
                        fallback_date=crate::composer_text_fields::today()?;
                        fallback=crate::composer_text_fields::Context{index:Some(0),total:1,path:None,date:&fallback_date};
                        &fallback
                    };
                    let annotation=crate::composer_annotations::draws(sheet,key,value,&mut self.fonts,fields,false)?;
                    for (index,mut draw) in annotation.draws.into_iter().enumerate(){
                        if draw.text.is_some(){for part in self.text.physical_records(std::slice::from_ref(&draw),density)?{append(part.into())?;}continue;}
                        if annotation.rectangles.binary_search(&index).is_ok(){
                            let mut fill=draw.clone();fill.stroke=None;fill.stroke_options=None;
                            for part in self.text.physical_rectangle(&fill,density)?{append(part.into())?;}
                            draw.fill=None;
                        }
                        let parts=if annotation.source_points.binary_search(&index).is_ok(){crate::sheet_image_paths::source_point_records(&draw,density,metrics.pixels())?}
                        else if annotation.source_lines.binary_search(&index).is_ok(){crate::sheet_image_paths::source_line_records(&draw,density,metrics.pixels())?}
                        else{crate::sheet_image_paths::records(std::slice::from_ref(&draw),density,metrics.pixels())?};
                        for part in parts{append(part.into())?;}
                    }
                    continue;
                }
                ingetrazo_core::composition::PhysicalExportItem::Legend { value, .. } => {
                    let legend = crate::composer_legend::draws(value, &mut self.fonts)?;
                    for (index, mut draw) in legend.draws.into_iter().enumerate() {
                        if draw.text.is_some() {
                            for part in self
                                .text
                                .physical_records(std::slice::from_ref(&draw), density)?
                            {
                                append(part.into())?;
                            }
                        } else {
                            if legend.rectangles.binary_search(&index).is_ok() {
                                let mut fill = draw.clone();
                                fill.stroke = None;
                                fill.stroke_options = None;
                                for part in self.text.physical_rectangle(&fill, density)? {
                                    append(part.into())?;
                                }
                                draw.fill = None;
                            }
                            if draw.stroke.is_some() {
                                for part in crate::sheet_image_paths::records(
                                    std::slice::from_ref(&draw),
                                    density,
                                    metrics.pixels(),
                                )? {
                                    append(part.into())?;
                                }
                            }
                        }
                    }
                    continue;
                }
            };
            match item {
                SheetItem::Shape(item) => append(
                    crate::composer_cad::physical_shape(
                        ctx,
                        &item.shape,
                        density,
                        glam::DVec2::new(item.shape.x_mm, item.shape.y_mm),
                    )?
                    .into(),
                )?,
                SheetItem::Text(item) => {
                    let draws =
                        crate::composer::pdf::text_draws(sheet, item, &mut self.fonts, context)?;
                    for part in self.text.physical_records(&draws, density)? {
                        append(part.into())?;
                    }
                }
                SheetItem::ViewFrame(item) => {
                    let draws = views
                        .and_then(|v| v.get(&item.id))
                        .ok_or("Physical vector view requires live document context")?;
                    for (index, draw) in draws.draws.iter().enumerate() {
                        if draw.text.is_some() {
                            for part in self
                                .text
                                .physical_records(std::slice::from_ref(draw), density)?
                            {
                                append(part.into())?;
                            }
                        } else if index >= draws.caption_start
                            && draw.fill.is_some()
                            && draw.stroke.is_none()
                            && crate::sheet_image_paths::axis_rectangle(&draw.path).is_some()
                        {
                            for part in self.text.physical_rectangle(draw, density)? {
                                append(part.into())?;
                            }
                        } else if draws.source_lines.binary_search(&index).is_ok() {
                            for part in crate::sheet_image_paths::source_line_records(
                                draw,
                                density,
                                metrics.pixels(),
                            )? {
                                append(part.into())?;
                            }
                        } else {
                            for part in crate::sheet_image_paths::records(
                                std::slice::from_ref(draw),
                                density,
                                metrics.pixels(),
                            )? {
                                append(part.into())?;
                            }
                        }
                    }
                }
                SheetItem::TitleBlock(_)
                | SheetItem::NorthArrow(_)
                | SheetItem::GraphicScale(_) => {
                    let draws = crate::composer_symbols::raster_draws(
                        sheet,
                        &sheet.items[index],
                        &mut self.fonts,
                        context,
                        density,
                    )?;
                    for mut draw in draws {
                        if draw.text.is_some() {
                            for part in self
                                .text
                                .physical_records(std::slice::from_ref(&draw), density)?
                            {
                                append(part.into())?;
                            }
                        } else {
                            // Source graphic cells use QRasterPaintEngine::drawRects, whose fill
                            // uses the16.16 axis rectangle rasterizer instead of gray path mapping.
                            if matches!(sheet.items[index], SheetItem::GraphicScale(_))
                                && draw.fill.is_some()
                                && crate::sheet_image_paths::axis_rectangle(&draw.path).is_some()
                            {
                                let mut fill = draw.clone();
                                fill.stroke = None;
                                fill.stroke_options = None;
                                for part in self.text.physical_rectangle(&fill, density)? {
                                    append(part.into())?;
                                }
                                draw.fill = None;
                            }
                            for part in crate::sheet_image_paths::records(
                                std::slice::from_ref(&draw),
                                density,
                                metrics.pixels(),
                            )? {
                                append(part.into())?;
                            }
                        }
                    }
                }
            }
        }
        if sheet.effective_source_border().on {
            append(crate::composer_border::physical_geometry(sheet, density)?.into())?;
        }
        Ok(Plan { metrics, records })
    }
    pub(crate) fn prepare_scene(
        &mut self,
        ctx: &egui::Context,
        scene: &ingetrazo_core::scene::Scene,
        camera: &ingetrazo_core::camera::OrbitCamera,
        compute: &mut crate::composer_compute::ComposerCompute,
        document: uuid::Uuid,
        revision: u64,
        index: usize,
        dpi: u16,
        path: Option<&std::path::Path>,
    ) -> Result<Plan, String> {
        let sheet = scene
            .sheets
            .get(index)
            .ok_or("Export sheet no longer exists")?;
        let unsupported = unsupported(sheet);
        if !unsupported.is_empty() {
            return Err(format!(
                "Sheet image painters are not supported: {}",
                unsupported.join(", ")
            ));
        }
        let date = crate::composer_text_fields::sample_calendar()?.display();
        let indices = crate::composer_text_fields::source_indices(
            &scene.sheets,
            std::slice::from_ref(sheet),
        )?;
        let source_index = *indices
            .get(&(sheet as *const _))
            .ok_or("Image sheet field context lost")?;
        let context = crate::composer_text_fields::Context {
            index: Some(source_index),
            total: scene.sheets.len(),
            path,
            date: &date,
        };
        let mut views = std::collections::HashMap::new();
        for i in sheet.visible_item_indices() {
            if let SheetItem::ViewFrame(frame) = &sheet.items[i] {
                views.insert(
                    frame.id,
                    crate::composer::pdf::view_draws(
                        scene,
                        camera,
                        compute,
                        document,
                        revision,
                        sheet,
                        frame,
                        &mut self.fonts,
                        &context,
                    )?,
                );
            }
        }
        self.prepare_with_views(ctx, sheet, dpi, Some(&views), Some(&context))
    }
}
#[derive(Clone)]
pub struct Request {
    pub sheet_id: uuid::Uuid,
    pub path: std::path::PathBuf,
    pub sheet: usize,
    pub dpi: u16,
}
pub enum Dialog {
    Path {
        sheet_id: uuid::Uuid,
        sheet: usize,
        path: String,
        format: ingetrazo_formats::sheet_image::Format,
    },
    Resolution {
        sheet_id: uuid::Uuid,
        sheet: usize,
        path: std::path::PathBuf,
        dpi: u16,
    },
}
#[derive(Default)]
pub struct State {
    pub chooser_refreshes: u64,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) pending: Option<Request>,
    pub(crate) painters: PainterCache,
}
impl crate::app::IngeTrazoApp {
    /// Explicit diagnostic for one already-required physical export. Disabled by default.
    pub fn begin_sheet_glyph_observation(&mut self) -> Result<(), String> {
        self.sheet_image.painters.text.begin_physical_observation()
    }
    pub fn take_sheet_glyph_observation(&mut self) -> Result<serde_json::Value, String> {
        self.sheet_image.painters.text.take_physical_observation()
    }

    pub fn sheet_picture_cache_stats(&self) -> (u64, u64) {
        self.sheet_image.painters.picture_stats()
    }
    pub fn begin_sheet_image_export(&mut self, sheet: usize) {
        self.sheet_image.chooser_refreshes += 1;
        self.composer_compute.invalidate();
        // Like the source chooser, refresh the physical vector view cache before
        // path selection, including a canceled chooser. No scene copy or history.
        let date = match crate::composer_text_fields::today() {
            Ok(v) => v,
            Err(e) => {
                self.sheet_image_result(Err(e));
                return;
            }
        };
        let fields = crate::composer_text_fields::Context {
            index: Some(sheet),
            total: self.scene.sheets.len(),
            path: self.current_file.as_deref(),
            date: &date,
        };
        if let Some(sheet) = self.scene.sheets.get(sheet) {
            for i in sheet.visible_item_indices() {
                if let SheetItem::ViewFrame(frame) = &sheet.items[i] {
                    if let Err(error) = crate::composer::pdf::view_draws(
                        &self.scene,
                        &self.camera,
                        &mut self.composer_compute,
                        self.clipboard_document,
                        self.model_revision,
                        sheet,
                        frame,
                        &mut self.sheet_image.painters.fonts,
                        &fields,
                    ) {
                        self.sheet_image_result(Err(error));
                        return;
                    }
                }
            }
        }
        let Some(target) = self.scene.sheets.get(sheet) else {
            self.sheet_image_result(Err("Export sheet no longer exists".into()));
            return;
        };
        self.sheet_image.dialog = Some(Dialog::Path {
            sheet_id: target.id,
            sheet,
            path: "lamina.png".into(),
            format: ingetrazo_formats::sheet_image::Format::Png,
        });
    }
    pub fn choose_sheet_image_path(
        &mut self,
        sheet: usize,
        path: Option<&std::path::Path>,
        format: ingetrazo_formats::sheet_image::Format,
    ) {
        let Some(path) = path else {
            self.sheet_image.dialog = None;
            return;
        };
        let sheet_id = match self.sheet_image.dialog.as_ref() {
            Some(Dialog::Path {
                sheet_id,
                sheet: owner,
                ..
            }) if *owner == sheet => *sheet_id,
            _ => match self.scene.sheets.get(sheet) {
                Some(s) => s.id,
                None => {
                    self.sheet_image_result(Err("Export sheet no longer exists".into()));
                    return;
                }
            },
        };
        let (path, _) = ingetrazo_formats::sheet_image::resolve_path(path, format);
        let dpi = self
            .document_session
            .preferences
            .extra
            .get("composer/image_dpi")
            .and_then(|v| v.as_u64())
            .filter(|v| (50..=1200).contains(v))
            .unwrap_or(200) as u16;
        self.sheet_image.dialog = Some(Dialog::Resolution {
            sheet_id,
            sheet,
            path,
            dpi,
        });
    }
    #[doc(hidden)]
    pub fn sheet_image_resolution_default(&self) -> Option<u16> {
        match &self.sheet_image.dialog {
            Some(Dialog::Resolution { dpi, .. }) => Some(*dpi),
            _ => None,
        }
    }
    pub fn choose_sheet_image_resolution(
        &mut self,
        sheet: usize,
        path: std::path::PathBuf,
        dpi: Option<u16>,
    ) -> Result<(), String> {
        let sheet_id = match self.sheet_image.dialog.take() {
            Some(Dialog::Resolution {
                sheet_id,
                sheet: owner,
                ..
            }) if owner == sheet => sheet_id,
            _ => {
                self.scene
                    .sheets
                    .get(sheet)
                    .ok_or("Export sheet no longer exists")?
                    .id
            }
        };
        let Some(dpi) = dpi else {
            return Ok(());
        };
        if !(50..=1200).contains(&dpi) {
            return Err("Paper export resolution must be 50..1200 DPI".into());
        }
        if self
            .scene
            .sheets
            .get(sheet)
            .is_none_or(|s| s.id != sheet_id)
        {
            return Err("Export sheet identity changed during chooser".into());
        }
        // Match the original chooser: remember even when the subsequent writer fails.
        self.document_session
            .preferences
            .extra
            .insert("composer/image_dpi".into(), serde_json::json!(dpi));
        self.persist_preferences();
        self.sheet_image.pending = Some(Request {
            sheet_id,
            sheet,
            path,
            dpi,
        });
        Ok(())
    }
    #[doc(hidden)]
    pub fn prepare_sheet_image_request(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<Result<(Request, Plan), String>> {
        let request = self.sheet_image.pending.take()?;
        Some((|| {
            let sheet = self
                .scene
                .sheets
                .get(request.sheet)
                .ok_or("Export sheet no longer exists")?;
            if sheet.id != request.sheet_id {
                return Err("Export sheet identity changed".into());
            }
            let plan = self.sheet_image.painters.prepare_scene(
                ctx,
                &self.scene,
                &self.camera,
                &mut self.composer_compute,
                self.clipboard_document,
                self.model_revision,
                request.sheet,
                request.dpi,
                self.current_file.as_deref(),
            )?;
            Ok((request, plan))
        })())
    }
    #[doc(hidden)]
    pub fn sheet_image_result(&mut self, result: Result<(), String>) {
        self.status_message = match result {
            Ok(()) => "Sheet image exported".into(),
            Err(error) => format!("Sheet image export failed: {error}"),
        };
    }
}
pub(crate) fn dialog(app: &mut crate::app::IngeTrazoApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.sheet_image.dialog.take() else {
        return;
    };
    let mut accepted = false;
    let mut canceled = false;
    egui::Modal::new(egui::Id::new("sheet-image-export")).show(ctx, |ui| {
        match &mut dialog {
            Dialog::Path { path, format, .. } => {
                ui.heading("Export sheet image");
                ui.text_edit_singleline(path);
                egui::ComboBox::from_label("Format")
                    .selected_text(format!("{format:?}"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            format,
                            ingetrazo_formats::sheet_image::Format::Png,
                            "PNG",
                        );
                        ui.selectable_value(
                            format,
                            ingetrazo_formats::sheet_image::Format::Jpeg,
                            "JPEG",
                        );
                    });
            }
            Dialog::Resolution { dpi, .. } => {
                ui.heading("Image resolution (DPI)");
                ui.add(egui::DragValue::new(dpi).range(50..=1200).speed(10.));
            }
        }
        ui.horizontal(|ui| {
            accepted = ui.button("Export").clicked();
            canceled = ui.button("Cancel").clicked();
        });
        canceled |= ui.input(|i| i.key_pressed(egui::Key::Escape));
    });
    if canceled {
        return;
    }
    if !accepted {
        app.sheet_image.dialog = Some(dialog);
        return;
    }
    // Preserve the cold target identity while the public chooser transition runs.
    match dialog {
        Dialog::Path {
            sheet_id,
            sheet,
            path,
            format,
        } => {
            if path.trim().is_empty() {
                return;
            }
            app.sheet_image.dialog = Some(Dialog::Path {
                sheet_id,
                sheet,
                path: path.clone(),
                format,
            });
            app.choose_sheet_image_path(sheet, Some(std::path::Path::new(&path)), format);
        }
        Dialog::Resolution {
            sheet_id,
            sheet,
            path,
            dpi,
        } => {
            app.sheet_image.dialog = Some(Dialog::Resolution {
                sheet_id,
                sheet,
                path: path.clone(),
                dpi,
            });
            if let Err(error) = app.choose_sheet_image_resolution(sheet, path, Some(dpi)) {
                app.sheet_image_result(Err(error));
            }
        }
    }
}
#[cfg(target_os = "macos")]
pub struct MetalExporter {
    owner: Arc<ingetrazo_metal::NativeMetal>,
    painter: ingetrazo_metal::egui_painter::EguiPainter,
    canvas: ingetrazo_metal::compute_canvas::Pool,
    attachments: ingetrazo_metal::egui_painter::SheetReadbackPool,
}
#[cfg(target_os = "macos")]
impl MetalExporter {
    pub fn new(owner: Arc<ingetrazo_metal::NativeMetal>) -> Result<Self, String> {
        let painter = ingetrazo_metal::egui_painter::EguiPainter::new_compute(&owner)?;
        Ok(Self {
            owner,
            painter,
            canvas: Default::default(),
            attachments: Default::default(),
        })
    }
    pub fn picture_stats(&self) -> ingetrazo_metal::paper_picture::Stats {
        self.painter.picture_stats()
    }
    pub fn save(&mut self, plan: &Plan, path: &std::path::Path) -> Result<(), String> {
        let factory = ingetrazo_render::cad_callback::Factory::default();
        let pose = ingetrazo_render::cad2d::Pose {
            scale: [1.; 2],
            translation: [0.; 2],
        };
        let handles = plan
            .records
            .iter()
            .map(|record| match record {
                Record::Cad((g, b)) => factory.handle(g.clone(), b.clone(), pose).map(Some),
                Record::Picture(_) => Ok(None),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let [width, height] = plan.metrics.pixels();
        let tile_width = 2048u32;
        let tile_height = 128u32;
        let capacity = (width as usize)
            .checked_mul(tile_height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|&n| n <= 32 * 1024 * 1024)
            .ok_or("Native paper row band exceeds 32 MiB")?;
        let mut band = Vec::new();
        band.try_reserve_exact(capacity)
            .map_err(|_| "Native paper band allocation failed")?;
        let mut first = None;
        let mut count = 0;
        ingetrazo_formats::sheet_image::save_rows(path, plan.metrics, |y, rows| {
            if first.is_none_or(|start| y < start || y + rows > start + count) {
                let start = y / tile_height * tile_height;
                count = (height - start).min(tile_height);
                band.resize(width as usize * count as usize * 4, 255);
                for x in (0..width).step_by(tile_width as usize) {
                    let pose = ingetrazo_render::cad2d::Pose {
                        scale: [1.; 2],
                        translation: [-(x as f32), -(start as f32)],
                    };
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(tile_width as f32, tile_height as f32),
                    );
                    let mut primitives = Vec::new();
                    primitives
                        .try_reserve_exact(handles.len())
                        .map_err(|_| "Paper callback allocation failed")?;
                    for (handle, record) in handles.iter().zip(&plan.records) {
                        let callback: Arc<dyn std::any::Any + Send + Sync> = match record {
                            Record::Cad((geometry, batches)) => {
                                let handle = handle.as_ref().ok_or("Paper CAD handle missing")?;
                                handle.update(geometry.clone(), batches.clone(), pose)?;
                                handle.clone()
                            }
                            Record::Picture(picture) => {
                                picture.set_origin([x as i32, start as i32])?;
                                picture.clone()
                            }
                        };
                        primitives.push(egui::ClippedPrimitive {
                            clip_rect: rect,
                            primitive: egui::epaint::Primitive::Callback(egui::PaintCallback {
                                rect,
                                callback,
                            }),
                        });
                    }
                    let paint = self.painter.prepare(&self.owner, &primitives, 1.)?;
                    let mut submitted = self.painter.submit_sheet_readback(
                        &self.owner,
                        paint,
                        tile_width as usize,
                        tile_height as usize,
                        &mut self.canvas,
                        &mut self.attachments,
                    )?;
                    let pixels = submitted.wait_readback(std::time::Duration::from_secs(5))?;
                    let columns = (width - x).min(tile_width) as usize;
                    for row in 0..count as usize {
                        let src = row * tile_width as usize * 4;
                        let dst = (row * width as usize + x as usize) * 4;
                        band[dst..dst + columns * 4]
                            .copy_from_slice(&pixels[src..src + columns * 4]);
                    }
                }
                first = Some(start);
            }
            let offset = (y - first.unwrap()) as usize * width as usize * 4;
            Ok(band[offset..offset + rows as usize * width as usize * 4].to_vec())
        })
    }
}
