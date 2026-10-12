// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! 2D Sheet Composer & Drawing Layout GUI (Track 4).
//!
//! Provides interactive paper canvas (mm units), exact architectural scales (1:50, 1:100, 1:200),
//! live 3D model viewports (MarcoVista), title blocks (Cajetín), text, north arrows, and graphic scales.

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::DVec3;
use ingetrazo_core::composition::{
    format_scale, model_height_for_frame, GraphicScaleItem, NorthArrowItem, Orientation, PaperSize,
    RectMm, Sheet, SheetItem, TitleBlock, ViewFrame,
};
use ingetrazo_core::i18n::tr;
use ingetrazo_core::scene::Scene;
use ingetrazo_render::pipeline::project_point;

use crate::app::IngeTrazoApp;
#[path="composer_pdf.rs"] pub(crate) mod pdf;
#[path="composer_text_canvas.rs"] pub(crate) mod text_canvas;
#[path="composer_text_authoring.rs"] mod text_authoring;
#[path="composer_pointer.rs"] mod pointer_transactions;

/// Interactive state for the Sheet Composer view.
#[derive(Debug, Clone)]
pub struct ComposerState {
    pub pdf_export_error: Option<String>,
    pub text_tool:bool,
    pub(super) tool_switch_rects:[egui::Rect;8],
    pub inline_text:Option<text_authoring::InlineText>,
    text_canvas:text_canvas::CanvasPainter,
    outer_border:crate::composer_border::CanvasBorder,
    paper_canvas:crate::composer_border::CanvasPaper,
    field_index_cache: Option<((u64,u64,uuid::Uuid,usize),Result<usize,String>)>,
    field_index_computations:u64,
    field_expansions:crate::composer_text_fields::CanvasExpansions,
    pub paper_text: std::rc::Rc<std::cell::RefCell<ingetrazo_formats::paper_text_painter::Painter>>,
    pub shape_tool:Option<String>,
    pub shape_start:Option<glam::DVec2>,
    pub pan_offset: Vec2,
    pub zoom: f32, // pixels per mm
    pub dragging_item_idx: Option<usize>,
    pub drag_start_pos: Option<Pos2>,
    pub drag_item_orig_pos: Option<(f64, f64)>,
    drag_item_id:Option<uuid::Uuid>,
    drag_snap_targets:Option<pointer_transactions::Targets>,
    pub interaction_sheet:Option<uuid::Uuid>,
    pub edit_before:Option<Sheet>,
    #[cfg(test)]
    edit_snapshots:u64,
    pub sheet_history:std::collections::HashMap<uuid::Uuid,(Vec<ingetrazo_core::composition::Sheet>,Vec<ingetrazo_core::composition::Sheet>)>,
}

impl Default for ComposerState {
    fn default() -> Self {
        Self {
            pdf_export_error: None,text_tool:false,tool_switch_rects:[egui::Rect::NOTHING;8],inline_text:None,text_canvas:Default::default(),outer_border:Default::default(),paper_canvas:Default::default(),field_index_cache:None,field_index_computations:0,field_expansions:Default::default(),
            paper_text: Default::default(),
            shape_tool:None,shape_start:None,
            pan_offset: Vec2::ZERO,
            zoom: 1.5, // 1.5 px per mm by default (~400mm A3 = 600px wide)
            dragging_item_idx: None,
            drag_start_pos: None,
            drag_item_orig_pos: None,drag_item_id:None,drag_snap_targets:None,
            interaction_sheet:None,edit_before:None,sheet_history:Default::default(),
            #[cfg(test)]
            edit_snapshots:0,
        }
    }
}

fn canvas_field_index(app:&mut IngeTrazoApp,sheet_idx:usize)->Result<usize,String> {
    let sheet=&app.scene.sheets[sheet_idx];
    let key=(app.model_revision,app.scene.version,sheet.id,app.scene.sheets.len());
    if app.composer_state.field_index_cache.as_ref().map(|(k,_)|*k)!=Some(key) {
        let result=crate::composer_text_fields::source_indices(&app.scene.sheets,&app.scene.sheets[sheet_idx..sheet_idx+1])
            .and_then(|indices|indices.get(&(sheet as *const _)).copied().ok_or_else(||"Paper Text context no longer exists".into()));
        app.composer_state.field_index_cache=Some((key,result));
        app.composer_state.field_index_computations=app.composer_state.field_index_computations.wrapping_add(1);
    }
    app.composer_state.field_index_cache.as_ref().unwrap().1.clone()
}

// Compare selection-independent document state using the owned transaction
// snapshot, without allocating another copy of either document.
fn sheet_model_equal(before:&mut Sheet,current:&Sheet)->bool {
 let selection=before.selected_item;
 before.selected_item=current.selected_item;
 let equal=*before==*current;
 before.selected_item=selection;
 equal
}
impl ComposerState {
 fn snapshot_sheet(&mut self,sheet:&Sheet){
  if self.edit_before.is_none(){
   self.edit_before=Some(sheet.clone());
   #[cfg(test)]{self.edit_snapshots+=1;}
  }
 }
 pub fn arm_text(&mut self){self.text_tool=true;self.shape_tool=None;self.shape_start=None;}
 pub fn arm_shape(&mut self,kind:&str){self.text_tool=false;self.shape_tool=Some(kind.into());self.shape_start=None;}
}
impl IngeTrazoApp {
 pub fn place_composer_shape(&mut self,kind:&str,start:glam::DVec2,end:glam::DVec2)->Result<uuid::Uuid,String>{
  let mut shape=ingetrazo_core::sheet_shape::SheetShape::from_drag(kind,start,end)?;
  let index=self.scene.active_sheet_index.filter(|&i|i<self.scene.sheets.len()).ok_or("No active sheet")?;
  let sheet=&self.scene.sheets[index];
  let zs=sheet.items.iter().map(|item|match item {SheetItem::Shape(f)=>f.shape.z,_=>sheet.native_items.get(&item.id()).and_then(|v|v.get("z")).and_then(|v|v.as_f64()).unwrap_or(0.)}).collect::<Vec<_>>();
  if zs.iter().any(|z|!z.is_finite()){return Err("Non-finite sheet stacking order".into());}
  shape.z=zs.into_iter().reduce(f64::max).map_or(0.,|z|z+1.);
  if !shape.z.is_finite(){return Err("Sheet stacking order exceeds numeric range".into());}
  self.finish_composer_edit();self.begin_composer_edit();
  let id=uuid::Uuid::new_v4();let sheet=&mut self.scene.sheets[index];
  sheet.add_item(SheetItem::Shape(ingetrazo_core::composition::ShapeItem{id,shape}));
  sheet.selected_item=Some(sheet.items.len()-1);self.finish_composer_edit();self.composer_state.inline_text=None;self.composer_state.shape_tool=None;self.composer_state.shape_start=None;Ok(id)
 }
 pub fn enter_composer_sheet(&mut self,index:usize){
  let Some(id)=self.scene.sheets.get(index).map(|s|s.id)else{return;};
  if self.composer_state.interaction_sheet!=Some(id){self.finish_composer_edit();self.composer_state.shape_start=None;self.composer_state.interaction_sheet=Some(id);}
 }
 pub fn begin_composer_edit(&mut self){
  if let Some(sheet)=self.scene.active_sheet_index.and_then(|i|self.scene.sheets.get(i)){
   self.composer_state.snapshot_sheet(sheet);
  }
 }
 // Original _on_page_changed/_on_border_changed are direct document properties,
 // outside ComposerHistory item commands. Replaying an item snapshot must not
 // restore the older paper settings carried by that snapshot.
 fn retain_direct_paper_properties(target:&mut Sheet,current:&Sheet){
  target.paper_size=current.paper_size;target.orientation=current.orientation;
  target.source_border=current.source_border.clone();target.source_border_baseline=current.source_border_baseline.clone();
  if let Some(value)=current.native_document.get("border"){
   if !target.native_document.is_object(){target.native_document=serde_json::json!({});}
   target.native_document["border"]=value.clone();
  }else if let Some(object)=target.native_document.as_object_mut(){object.remove("border");}
 }
 pub fn finish_composer_edit(&mut self){
  self.composer_state.dragging_item_idx=None;self.composer_state.drag_start_pos=None;self.composer_state.drag_item_orig_pos=None;self.composer_state.drag_item_id=None;self.composer_state.drag_snap_targets=None;
  let Some(mut before)=self.composer_state.edit_before.take()else{return;};
  let Some(current)=self.scene.sheets.iter().find(|s|s.id==before.id)else{return;};
  Self::retain_direct_paper_properties(&mut before,current);
  if sheet_model_equal(&mut before,current){return;}
  let history=self.composer_state.sheet_history.entry(before.id).or_default();
  history.0.push(before);history.1.clear();
  // Original ComposerHistory is independent of General undo_steps and unlimited.
  self.scene.bump_version();
 }

 pub fn undo_composer_edit(&mut self){self.finish_composer_edit();
  let Some(index)=self.scene.active_sheet_index.filter(|&i|i<self.scene.sheets.len())else{return;};let id=self.scene.sheets[index].id;
  let Some(history)=self.composer_state.sheet_history.get_mut(&id)else{return;};
  if let Some(mut before)=history.0.pop(){Self::retain_direct_paper_properties(&mut before,&self.scene.sheets[index]);history.1.push(std::mem::replace(&mut self.scene.sheets[index],before));self.scene.bump_version();}
 }
 pub fn redo_composer_edit(&mut self){self.finish_composer_edit();
  let Some(index)=self.scene.active_sheet_index.filter(|&i|i<self.scene.sheets.len())else{return;};let id=self.scene.sheets[index].id;
  let Some(history)=self.composer_state.sheet_history.get_mut(&id)else{return;};
  if let Some(mut after)=history.1.pop(){Self::retain_direct_paper_properties(&mut after,&self.scene.sheets[index]);history.0.push(std::mem::replace(&mut self.scene.sheets[index],after));self.scene.bump_version();}
 }
 pub fn cancel_composer_edit(&mut self){
  self.composer_state.shape_start=None;self.composer_state.shape_tool=None;
  if let Some(mut before)=self.composer_state.edit_before.take(){
   if let Some(current)=self.scene.sheets.iter_mut().find(|s|s.id==before.id){
    Self::retain_direct_paper_properties(&mut before,current);
    let changed=!sheet_model_equal(&mut before,current);
    *current=before;
    if changed{self.scene.bump_version();}
   }
  }
  self.composer_state.dragging_item_idx=None;self.composer_state.drag_start_pos=None;self.composer_state.drag_item_orig_pos=None;self.composer_state.drag_item_id=None;self.composer_state.drag_snap_targets=None;
 }
}

pub fn render_sheet_composer(app: &mut IngeTrazoApp, ui: &mut egui::Ui, sheet_idx: usize) {
    if sheet_idx >= app.scene.sheets.len() {
        app.finish_composer_edit();
        app.scene.active_sheet_index = None;
        return;
    }

    app.enter_composer_sheet(sheet_idx);
    if ui.is_enabled()&&ui.input(|i|i.key_pressed(egui::Key::Escape)){
        // A normal item grab keeps its geometry until the real release.
        // Source Select Escape only drops selection; explicit edit/placement
        // cancellation retains the existing snapshot rollback law.
        if app.composer_state.dragging_item_idx.is_some(){app.scene.sheets[sheet_idx].selected_item=None;}
        else {let select_mode=!app.composer_state.text_tool&&app.composer_state.shape_tool.is_none()&&app.composer_state.inline_text.is_none();app.cancel_composer_edit();if select_mode{app.scene.sheets[sheet_idx].selected_item=None;}}
    }

    let (response_rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    let painter = ui.painter_at(response_rect);

    // Dark drafting table workspace background
    painter.rect_filled(response_rect, 0.0, Color32::from_rgb(32, 34, 40));

    // Handle Pan and Zoom
    let pointer = ui.input(|i| i.pointer.clone());
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    let space_down = ui.input(|i| i.key_down(egui::Key::Space));

    if scroll_delta != 0.0 && response.hovered() {
        let zoom_mult = if scroll_delta > 0.0 { 1.12 } else { 0.88 };
        // Clamp zoom between 0.2 (broad view) and 8.0 (close up detail)
        let new_zoom = (app.composer_state.zoom * zoom_mult).clamp(0.2, 8.0);
        app.composer_state.zoom = new_zoom;
    }

    // Pan with middle mouse or Space+drag
    if response.dragged() && (pointer.middle_down() || space_down) {
        app.composer_state.pan_offset += response.drag_delta();
    }

    // Extract sheet metrics and items
    let (paper_w_mm, paper_h_mm) = app.scene.sheets[sheet_idx].dimensions_mm();
    let margin_mm = app.scene.sheets[sheet_idx].margin_mm;
    let selected_item = app.scene.sheets[sheet_idx].selected_item;
    let items = app.scene.sheets[sheet_idx].items.clone();
    app.composer_state.text_canvas.begin_frame();
    let field_index=if items.iter().any(|item|matches!(item,SheetItem::Text(t)if t.text.contains('{'))) {Some(canvas_field_index(app,sheet_idx))}else{None};
    let calendar_before=crate::composer_text_fields::calendar_samples();
    let calendar=field_index.as_ref().map(|_|crate::composer_text_fields::sample_calendar());
    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|{
        d.insert_temp(egui::Id::new("pdf-ui-canvas-fields"),Vec::<String>::new());
        d.insert_temp(egui::Id::new("pdf-ui-canvas-text-rects"),Vec::<egui::Rect>::new());
        d.insert_temp(egui::Id::new("pdf-ui-field-index-computations"),app.composer_state.field_index_computations);
    });}
    let zoom = app.composer_state.zoom;

    // Center paper on screen by default
    let center_x = response_rect.center().x + app.composer_state.pan_offset.x;
    let center_y = response_rect.center().y + app.composer_state.pan_offset.y;

    let paper_w_px = paper_w_mm as f32 * zoom;
    let paper_h_px = paper_h_mm as f32 * zoom;

    let paper_rect = Rect::from_center_size(
        Pos2::new(center_x, center_y),
        Vec2::new(paper_w_px, paper_h_px),
    );

    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-paper-transform"),[paper_rect.min.x,paper_rect.min.y,zoom]));}
    let source_paper=app.composer_state.paper_canvas.paint(&painter,app.native_composer.factory.as_ref(),&app.scene.sheets[sheet_idx],zoom,paper_rect.min);
    match source_paper{
        Ok(true)=>{},
        Ok(false)=>{
    // Draw Paper Shadow & Sheet
    let shadow_rect = paper_rect.translate(Vec2::new(6.0, 8.0));
    painter.rect_filled(shadow_rect, 2.0, Color32::from_black_alpha(100));

    // Paper White Canvas
    painter.rect_filled(paper_rect, 0.0, Color32::WHITE);
    painter.rect_stroke(paper_rect, 0.0, Stroke::new(1.0, Color32::from_rgb(180, 180, 185)));

    // Dashed Printable Margin
    let margin_px = margin_mm as f32 * zoom;
    let margin_rect = paper_rect.shrink(margin_px);
    painter.rect_stroke(
        margin_rect,
        0.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(160, 175, 200, 150)),
    );

        },
        Err(error)=>app.status_message=error,
    }

    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("paper-cold-builds"),[app.composer_state.paper_canvas.cold_builds,app.composer_state.outer_border.cold_builds]));}
    // Helpers to convert mm to screen Pos2 / Rect
    let mm_to_screen = |x_mm: f64, y_mm: f64| -> Pos2 {
        Pos2::new(
            paper_rect.min.x + (x_mm as f32 * zoom),
            paper_rect.min.y + (y_mm as f32 * zoom),
        )
    };

    let rect_mm_to_screen = |r: RectMm| -> Rect {
        let p0 = mm_to_screen(r.x, r.y);
        Rect::from_min_size(p0, Vec2::new(r.w as f32 * zoom, r.h as f32 * zoom))
    };

    app.native_composer.retain_items(&items);
    if app.composer_state.shape_tool.is_none()||app.composer_state.shape_start.is_none(){app.native_composer.clear_preview();}
    // Render Sheet Items
    let item_order=app.scene.sheets[sheet_idx].visible_item_indices();
    for &idx in &item_order {
        let item=&items[idx];
        let is_selected = selected_item == Some(idx);

        match item {
            SheetItem::Shape(shape) => {
                match app.native_composer.paint(&painter,shape.id,&shape.shape,zoom,paper_rect.min){
                    Ok(true)=>{},Ok(false)=>{if let Err(error)=paint_sheet_shape(&painter,&shape.shape,zoom,paper_rect.min){app.status_message=error;}},Err(error)=>app.status_message=error,
                }
                if is_selected {render_selection_box(&painter,rect_mm_to_screen(item.rect()));}
            },
            SheetItem::ViewFrame(frame) => {
                let fr = rect_mm_to_screen(frame.rect);

                // View frame background (paper white or technical background)
                painter.rect_filled(fr, 0.0, Color32::WHITE);

                // Render 3D model projection inside the view frame
                let vector=matches!(frame.style.as_str(),"technical"|"vectorial")&&!frame.perspective;
                let ink=if vector{
                    let snapshot=frame_scene(&app.scene,frame);let camera=frame_camera_live(&snapshot,frame,&app.camera);
                    match app.composer_compute.segments(&snapshot,&camera,app.clipboard_document,app.model_revision,&frame.view_key,frame.profiles,frame.hidden_lines){
                        Ok(ink)=>Some(ink),Err(error)=>{app.status_message=error;Some(std::sync::Arc::new(vec![]))}
                    }
                }else{None};
                render_model_projection_with_ink(&app.scene,&painter,fr,frame,&app.style,&app.textures.iter().map(|(k,v)|(k.clone(),v.id())).collect(),ink.as_deref().map(Vec::as_slice),Some(&app.camera));

                // View Frame border
                if frame.border {
                    painter.rect_stroke(fr, 0.0, Stroke::new(1.2, Color32::from_rgb(40, 40, 45)));
                }

                // Title label
                if frame.show_title {
                    let title_text = if !frame.title_text.is_empty() {
                        &frame.title_text
                    } else {
                        "View"
                    };
                    let label_str = format!("{title_text}  {}", format_scale(frame.scale_n));
                    let title_pos = Pos2::new(fr.min.x, fr.max.y + 4.0 * zoom);
                    painter.text(
                        title_pos,
                        egui::Align2::LEFT_TOP,
                        label_str,
                        FontId::proportional((11.0 * (zoom / 1.5)).clamp(9.0, 16.0)),
                        Color32::from_rgb(20, 20, 25),
                    );
                }

                if is_selected {
                    render_selection_box(&painter, fr);
                }
            }

            SheetItem::TitleBlock(tb) => {
                let tr = rect_mm_to_screen(tb.rect);
                render_title_block(&painter, tr, tb, zoom);
                if is_selected {
                    render_selection_box(&painter, tr);
                }
            }

            SheetItem::Text(txt) => {
                let p = mm_to_screen(txt.pos.0, txt.pos.1);
                let cached=if txt.text.contains('{') {
                    Some((||{
                        let index=field_index.as_ref().unwrap().as_ref().map_err(Clone::clone)?;
                        let date=calendar.as_ref().unwrap().as_ref().map_err(Clone::clone)?;
                        app.composer_state.field_expansions.get(app.model_revision,app.scene.version,&app.scene.sheets[sheet_idx],txt,*index,app.scene.sheets.len(),app.current_file.as_deref(),*date)
                    })())
                }else{None};
                let error;
                let display=match &cached {
                    Some(Ok(text))=>text.as_ref(),Some(Err(message))=>{error=format!("Text fields: {message}");error.as_str()},None=>txt.text.as_str(),
                };
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|{
                    let mut strings=d.get_temp::<Vec<String>>(egui::Id::new("pdf-ui-canvas-fields")).unwrap_or_default();strings.push(display.to_string());d.insert_temp(egui::Id::new("pdf-ui-canvas-fields"),strings);
                });}
                let painted_rect=if !crate::paper_raw_texture::installed(painter.ctx()) {
                    // Preserve the existing display on hosts whose native raw upload
                    // path is not implemented yet; this is not exact source parity.
                    painter.text(p,egui::Align2::LEFT_TOP,display,
                        FontId::proportional((txt.font_size_pt as f32 * zoom * 0.4).max(10.0)),
                        Color32::from_rgb(15,15,20))
                } else {match app.composer_state.text_canvas.paint_with_factory(&painter,&app.scene.sheets[sheet_idx],txt,display,app.model_revision,app.scene.version,zoom,paper_rect.min,&mut app.composer_state.paper_text.borrow_mut(),app.native_composer.factory.as_ref()) {
                    Ok(rect)=>rect,Err(error)=>{app.status_message=error.clone();painter.text(p,egui::Align2::LEFT_TOP,format!("Text painter: {error}"),FontId::proportional(10.),Color32::DARK_RED)},
                }};
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|{let mut rects=d.get_temp::<Vec<egui::Rect>>(egui::Id::new("pdf-ui-canvas-text-rects")).unwrap_or_default();rects.push(painted_rect);d.insert_temp(egui::Id::new("pdf-ui-canvas-text-rects"),rects);});}
                if is_selected {
                    let text_rect = Rect::from_min_size(p, Vec2::new(60.0 * zoom, 12.0 * zoom));
                    render_selection_box(&painter, text_rect);
                }
            }

            SheetItem::NorthArrow(na) => {
                let p = mm_to_screen(na.pos.0, na.pos.1);
                let size_px = na.size_mm as f32 * zoom;
                let na_rect = Rect::from_min_size(p, Vec2::new(size_px, size_px));
                render_north_arrow(&painter, na_rect, na.rot_deg);
                if is_selected {
                    render_selection_box(&painter, na_rect);
                }
            }

            SheetItem::GraphicScale(gs) => {
                let p = mm_to_screen(gs.pos.0, gs.pos.1);
                let len_px = gs.length_mm as f32 * zoom;
                let gs_rect = Rect::from_min_size(p, Vec2::new(len_px, 8.0 * zoom));
                render_graphic_scale(&painter, gs_rect, gs, zoom);
                if is_selected {
                    render_selection_box(&painter, gs_rect);
                }
            }
        }
    }

    // Original _SheetBorderCanvasItem z=1e6: above every document item.
    match app.composer_state.outer_border.paint(&painter,app.native_composer.factory.as_ref(),&app.scene.sheets[sheet_idx],zoom,paper_rect.min){Ok(true)=>{},Ok(false)=>app.status_message="Native source sheet border provider unavailable".into(),Err(error)=>app.status_message=error}

    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){let raw_texture_stats=crate::paper_raw_texture::stats(ui.ctx());ui.ctx().data_mut(|d|{d.insert_temp(egui::Id::new("pdf-ui-canvas-paint-stats"),app.composer_state.text_canvas.stats());d.insert_temp(egui::Id::new("pdf-ui-canvas-ordered-stats"),app.composer_state.text_canvas.ordered_stats());if let Some(stats)=raw_texture_stats{d.insert_temp(egui::Id::new("pdf-ui-raw-texture-stats"),stats);}d.insert_temp(egui::Id::new("pdf-ui-field-expansion-computations"),app.composer_state.field_expansions.computations);d.insert_temp(egui::Id::new("pdf-ui-calendar-samples"),crate::composer_text_fields::calendar_samples().wrapping_sub(calendar_before));});}

    // Top Floating Toolbar for Composer Controls
    let icon_px = crate::appearance_preferences::current_toolbar_size(ui.ctx());
    let text_icon = app.icons.get(ui.ctx(), "text").cloned();
    let toolbar_rect = Rect::from_min_size(
        Pos2::new(response_rect.min.x + 20.0, response_rect.min.y + 16.0),
        Vec2::new(580.0, icon_px + 18.0),
    );

    let saved_view_names=app.scene.saved_views.iter().map(|v|v.name.clone()).collect::<Vec<_>>();
    let geo_north = app.scene.geo_datum.as_ref().map(|d| d.north).unwrap_or(0.0);
    let mut bump_needed = false;
    let mut direct_paper_changed = false;
    let mut fit_page_needed = false;
    let mut export_view_dxf = false;
    let mut export_pdf = false;
    let mut export_image = false;
    let mut export_all_pdf = false;

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(toolbar_rect), |ui| {
        egui::Frame::window(ui.style()).show(ui, |ui| {
            ui.spacing_mut().interact_size.y = icon_px + 4.0;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr("Composer:")).strong());

                let sheet = &mut app.scene.sheets[sheet_idx];

                // Paper Size
                egui::ComboBox::from_id_salt("composer_paper_size")
                    .selected_text(format!("{:?}", sheet.paper_size))
                    .show_ui(ui, |ui| {
                        for p in &[PaperSize::A4, PaperSize::A3, PaperSize::A2, PaperSize::A1, PaperSize::Letter] {
                            if ui.selectable_value(&mut sheet.paper_size, *p, format!("{p:?}")).clicked() {
                                direct_paper_changed = true;
                            }
                        }
                    });

                // Orientation
                let orient_label = match sheet.orientation {
                    Orientation::Landscape => tr("Landscape"),
                    Orientation::Portrait => tr("Portrait"),
                };
                if ui.button(orient_label).clicked() {
                    sheet.orientation = match sheet.orientation {
                        Orientation::Landscape => Orientation::Portrait,
                        Orientation::Portrait => Orientation::Landscape,
                    };
                    direct_paper_changed = true;
                }

                ui.separator();

                let border_menu=ui.menu_button(tr("Paper border"), |ui| {
                    let mut border=sheet.effective_source_border();
                    let toggle=ui.checkbox(&mut border.on,tr("Show border"));
                    let mut changed=toggle.changed();
                    changed|=ui.add(egui::DragValue::new(&mut border.mm).speed(0.1).range(0.1..=3.0).suffix(" mm")).changed();
                    let color=ui.text_edit_singleline(&mut border.color);changed|=color.changed();
                    changed|=ui.add(egui::DragValue::new(&mut border.radius_mm).speed(0.5).range(0.0..=30.0).suffix(" mm radius")).changed();
                    egui::ComboBox::from_id_salt("paper_border_style").selected_text(&border.style).show_ui(ui,|ui| {
                        for style in ["single","double","dashed"] {changed|=ui.selectable_value(&mut border.style,style.into(),style).changed();}
                    });
                    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-border-controls"),(toggle.rect,color.rect)));}
                    if changed && sheet.set_source_border(border).is_ok(){direct_paper_changed=true;}
                });
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-border-menu"),border_menu.response.rect));}

                // Add Items Menu
                app.composer_state.tool_switch_rects=[egui::Rect::NOTHING;8];
                let text_button=crate::icons::render_icon_button(ui,text_icon.as_ref(),"T",false,&tr("Add a text block"));app.composer_state.tool_switch_rects[0]=text_button.rect;if text_button.clicked(){app.composer_state.arm_text();app.native_composer.clear_preview();}
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-add-text"),text_button.rect));}
                let shape_menu=ui.menu_button(tr("+ Shape"),|ui|{
                    for (n,(kind,label)) in [("rect","Rectangle"),("elipse","Ellipse"),("poligono","Polygon"),("linea","Line"),("flecha","Arrow"),("terreno","Terrain")].into_iter().enumerate() {
                        let button=ui.button(tr(label));app.composer_state.tool_switch_rects[n+2]=button.rect;if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some()&&kind=="rect"{ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-shape-rect"),button.rect));}if button.clicked(){app.composer_state.arm_shape(kind);app.native_composer.clear_preview();ui.close_menu();}
                    }
                });
                app.composer_state.tool_switch_rects[1]=shape_menu.response.rect;
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-add-shape"),shape_menu.response.rect));}
                if ui.button(tr("+ View")).on_hover_text(tr("Add 3D model viewport")).clicked() {
                    let mut vf = ViewFrame::default();
                    vf.rect = RectMm::new(sheet.margin_mm + 10.0, sheet.margin_mm + 10.0, 160.0, 110.0);
                    app.composer_state.snapshot_sheet(sheet);
                    sheet.add_item(SheetItem::ViewFrame(vf));
                    bump_needed = true;
                }

                if ui.button(tr("+ Cajetín")).on_hover_text(tr("Add title block")).clicked() {
                    let mut tb = TitleBlock::default();
                    let (pw, ph) = sheet.dimensions_mm();
                    tb.rect = RectMm::new(pw - sheet.margin_mm - 140.0, ph - sheet.margin_mm - 50.0, 140.0, 50.0);
                    app.composer_state.snapshot_sheet(sheet);
                    sheet.add_item(SheetItem::TitleBlock(tb));
                    bump_needed = true;
                }

                let north_button=ui.button(tr("+ North")).on_hover_text(tr("Add north arrow"));
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-add-north"),north_button.rect));}
                #[cfg(test)]
                ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-test-north"),north_button.rect));
                if north_button.clicked() {
                    app.composer_state.snapshot_sheet(sheet);
                    sheet.add_item(SheetItem::NorthArrow(NorthArrowItem {
                        id: uuid::Uuid::new_v4(),
                        pos: (sheet.margin_mm + 15.0, sheet.margin_mm + 15.0),
                        size_mm: 22.0,
                        rot_deg: geo_north,
                    }));
                    bump_needed = true;
                }

                if ui.button(tr("+ Scale")).on_hover_text(tr("Add graphic scale bar")).clicked() {
                    app.composer_state.snapshot_sheet(sheet);
                    sheet.add_item(SheetItem::GraphicScale(GraphicScaleItem {
                        id: uuid::Uuid::new_v4(),
                        pos: (sheet.margin_mm + 15.0, sheet.dimensions_mm().1 - sheet.margin_mm - 20.0),
                        scale_n: 100.0,
                        length_mm: 50.0,
                        divisions: 4,
                    }));
                    bump_needed = true;
                }

                ui.separator();

                // Zoom to Fit Page
                if ui.button(tr("Fit Page")).clicked() {
                    fit_page_needed = true;
                }

                if let Some(index)=sheet.selected_item.filter(|&i|matches!(sheet.items.get(i),Some(SheetItem::ViewFrame(_)))) {
                    let view_menu=ui.menu_button(tr("View"),|ui|{
                        let SheetItem::ViewFrame(mut frame)=sheet.items[index].clone()else{unreachable!()};
                        let mut changed=false;
                        changed |=ui.add(egui::DragValue::new(&mut frame.scale_n).range(0.01..=1e8).prefix("1:")).changed();
                        let perspective=ui.checkbox(&mut frame.perspective,tr("Perspective"));
                        changed |=perspective.changed();
                        #[cfg(test)]
                        ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-test-perspective"),perspective.rect));
                        egui::ComboBox::from_id_salt("frame-render-style").selected_text(&frame.style).show_ui(ui,|ui|{
                            for key in ["sombreado","vectorial","style:Architectural","style:Shaded","style:Hidden line","style:Monochrome","style:Wireframe","style:X-ray"]{changed |=ui.selectable_value(&mut frame.style,key.into(),key).changed();}
                        });
                        egui::ComboBox::from_id_salt("frame-view-key").selected_text(&frame.view_key).show_ui(ui,|ui|{
                            for key in ["__current__","std:top","std:bottom","std:front","std:back","std:right","std:left","std:iso"]{changed |=ui.selectable_value(&mut frame.view_key,key.into(),key).changed();}
                            for name in &saved_view_names {changed |=ui.selectable_value(&mut frame.view_key,format!("scene:{name}"),name).changed();}
                        });
                        changed |=ui.add(egui::DragValue::new(&mut frame.rot_deg).speed(1.).suffix("°")).changed();
                        changed |=ui.checkbox(&mut frame.border,tr("Border")).changed();
                        changed |=ui.checkbox(&mut frame.show_title,tr("Title")).changed();
                        let title=ui.text_edit_singleline(&mut frame.title_text);
                        changed |=title.changed();
                        #[cfg(test)]
                        ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-test-title"),title.rect));
                        if changed {
                            app.composer_state.snapshot_sheet(sheet);
                            sheet.items[index]=SheetItem::ViewFrame(frame);
                            bump_needed=true;
                        }
                    });
                    #[cfg(test)]
                    ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("composer-test-view"),view_menu.response.rect));
                    #[cfg(not(test))]
                    let _=view_menu;
                    ui.menu_button(tr("Ink"),|ui|{
                        let SheetItem::ViewFrame(mut frame)=sheet.items[index].clone()else{unreachable!()};
                        let mut changed=false;
                        changed |= ui.checkbox(&mut frame.paper_bg,tr("Paper background")).changed();
                        changed |= ui.checkbox(&mut frame.profiles,tr("Profiles")).changed();
                        changed |= ui.checkbox(&mut frame.hidden_lines,tr("Hidden lines")).changed();
                        egui::ComboBox::from_id_salt("paper-cut-fill").selected_text(&frame.cut_fill).show_ui(ui,|ui|{
                            for mode in ["solid","hatch","none"] {changed |=ui.selectable_value(&mut frame.cut_fill,mode.into(),tr(mode)).changed();}
                        });
                        changed |=ui.add(egui::DragValue::new(&mut frame.cut_hatch_mm).speed(0.1).range(0.3..=50.).suffix(" mm hatch")).changed();
                        changed |=ui.text_edit_singleline(&mut frame.cut_fill_color).changed();
                        for(label,value)in [("Cut",&mut frame.pen_cut_mm),("Profile",&mut frame.pen_profile_mm),("Edge",&mut frame.pen_edge_mm)] {
                            ui.horizontal(|ui|{ui.label(tr(label));changed |=ui.add(egui::DragValue::new(value).speed(0.01).range(0.05..=5.).suffix(" mm")).changed();});
                        }
                        if changed {
                            app.composer_state.snapshot_sheet(sheet);
                            sheet.items[index]=SheetItem::ViewFrame(frame);
                            bump_needed=true;
                        }
                    });
                }

                if sheet.selected_item.and_then(|i|sheet.items.get(i)).is_some_and(|item|matches!(item,SheetItem::ViewFrame(_))) {
                    export_view_dxf |= ui.button(tr("Export view as DXF…")).clicked();
                }

                export_image|=ui.button(tr("Export image…")).clicked();
                let current_pdf_button=ui.button(tr("Export PDF…"));
                let all_pdf_button=ui.button(tr("Export all sheets (PDF)…"));
                export_pdf |=current_pdf_button.clicked();
                export_all_pdf |=all_pdf_button.clicked();
                // Private acceptance observation only: no replacement dialog or export path.
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
                    ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-observation"),(current_pdf_button.rect,all_pdf_button.rect,export_pdf,export_all_pdf)));
                }

                // Delete selected item
                if let Some(sel_idx) = sheet.selected_item {
                    if ui.button(tr("Delete Item")).clicked() {
                        app.composer_state.snapshot_sheet(sheet);
                        sheet.remove_item(sel_idx);
                        bump_needed = true;
                    }
                }
            });
        });
    });

    if export_image {app.begin_sheet_image_export(sheet_idx);}
    if export_pdf || export_all_pdf {
        let title=if export_all_pdf{tr("Export all sheets (PDF)…")}else{tr("Export PDF…")};
        let default=if export_all_pdf{"laminas.pdf"}else{"lamina.pdf"};
        if let Some(path)=crate::dialogs::pick_save_file(&title,default,"*.pdf|PDF") {
            let result=if export_all_pdf{app.export_composer_pdf(None,&path)}else{app.export_composer_pdf(Some(sheet_idx),&path)};
            app.status_message=match result{
                Ok(count)=>{app.composer_state.pdf_export_error=None;format!("{}: {count} — {}",tr("Exported sheets"),path.display())},
                Err(error)=>{app.composer_state.pdf_export_error=Some(error.clone());error}
            };
            if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-result"),app.status_message.clone()));}
        }
    }

    if let Some(error)=app.composer_state.pdf_export_error.clone() {
        egui::Window::new(tr("PDF export failed"))
            .id(egui::Id::new("composer-pdf-export-error"))
            .collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO)
            .show(ui.ctx(),|error_ui|{
                error_ui.label(error);
                let ok=error_ui.button(tr("OK"));
                if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some(){error_ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new("pdf-ui-error-ok"),ok.rect));}
                if ok.clicked(){app.composer_state.pdf_export_error=None;}
            });
    }

    if export_view_dxf {
        if let Some(path)=crate::dialogs::pick_save_file(&tr("Export view as DXF…"),"vista.dxf","*.dxf|DXF") {
            match app.export_composer_view_dxf(sheet_idx,&path) {
                Ok(count)=>app.status_message=format!("{}: {count} — {}",tr("Exported lines"),path.display()),
                Err(error)=>app.status_message=error,
            }
        }
    }

    if fit_page_needed {
        let fit_x = (response_rect.width() * 0.85) / paper_w_mm as f32;
        let fit_y = (response_rect.height() * 0.85) / paper_h_mm as f32;
        app.composer_state.zoom = fit_x.min(fit_y);
        app.composer_state.pan_offset = Vec2::ZERO;
    }

    if direct_paper_changed {app.scene.bump_version();}
    if bump_needed {
        app.scene.bump_version();
    }

    if ui.is_enabled()&&text_authoring::render(app,ui,&painter,sheet_idx,paper_rect,toolbar_rect,&response,zoom){return;}

    // Shape placement stays transient until a second click or a completed drag.
    if let Some(kind)=app.composer_state.shape_tool.clone() {
        if ui.is_enabled()&&ui.input(|i|i.key_pressed(egui::Key::Escape)) {app.composer_state.shape_start=None;app.composer_state.shape_tool=None;}
        else if let Some(hover)=pointer.hover_pos().filter(|p|paper_rect.contains(*p)&&!toolbar_rect.contains(*p)) {
            let first_pass=ui.ctx().output(|o|o.num_completed_passes)==0;
            let page=|p:Pos2|glam::DVec2::new(f64::from((p.x-paper_rect.min.x)/zoom),f64::from((p.y-paper_rect.min.y)/zoom));
            if ui.is_enabled()&&first_pass && response.drag_started() {if let Some(start)=pointer.press_origin(){app.composer_state.shape_start=Some(page(start));}}
            if let Some(start)=app.composer_state.shape_start {
                if let Ok(preview)=ingetrazo_core::sheet_shape::SheetShape::from_drag(&kind,start,page(hover)){match app.native_composer.paint_preview(&painter,&preview,zoom,paper_rect.min){Ok(true)=>{},Ok(false)=>{if let Err(error)=paint_sheet_shape(&painter,&preview,zoom,paper_rect.min){app.status_message=error;}},Err(error)=>app.status_message=error}}
                if first_pass && (response.drag_stopped()||response.clicked()){
                    match app.place_composer_shape(&kind,start,page(hover)){Ok(_)=>{},Err(error)=>app.status_message=error}
                    app.composer_state.shape_start=None;
                }
            } else if first_pass && response.clicked(){app.composer_state.shape_start=Some(page(hover));}
        }
        if app.composer_state.shape_start.is_none(){app.native_composer.clear_preview();}
        return;
    }
    // Original Qt item mouse grab is acquired at the press, before the
    // platform drag threshold. It uses the original press throughout.
    if ui.is_enabled(){pointer_transactions::input(app,ui,sheet_idx,&response,paper_rect,toolbar_rect,zoom,&item_order,space_down);}
    if ui.is_enabled()&&app.composer_state.dragging_item_idx.is_none()&&!pointer.any_down()&&!ui.ctx().wants_keyboard_input(){app.finish_composer_edit();}
}

fn frame_scene<'a>(scene:&'a Scene,frame:&ViewFrame)->std::borrow::Cow<'a,Scene>{if let Some(name)=frame.view_key.strip_prefix("scene:"){if let Some(view)=scene.saved_views.iter().find(|view|view.name==name){let mut snapshot=scene.clone();let mut camera=scene.camera.clone();view.apply_scene(&mut snapshot,&mut camera);return std::borrow::Cow::Owned(snapshot);}}std::borrow::Cow::Borrowed(scene)}
fn section_projected_edges(scene:&Scene)->Vec<(DVec3,DVec3)>{fn mesh(mesh:&ingetrazo_core::mesh::Mesh,transform:glam::DMat4,scene:&Scene,edges:&mut Vec<(DVec3,DVec3)>){for (id,edge) in mesh.edges.iter().enumerate(){if !scene.mesh_edge_visible(mesh,id){continue;}let a=transform.transform_point3(mesh.vertices[edge.v0].position);let b=transform.transform_point3(mesh.vertices[edge.v1].position);if let Some(segment)=scene.section_segment(a,b){edges.push(segment);}}
        if let Some(plane)=scene.active_section(){for id in 0..mesh.faces.len(){if !scene.mesh_face_visible(mesh,id){continue;}for(a,b,c)in mesh.triangulate_face(id){let points=[a,b,c].map(|p|transform.transform_point3(p));let d=points.map(|p|plane.signed_distance(p));if d.iter().all(|d|d.abs()<1e-7){continue;}let mut hits=vec![];for i in 0..3{let j=(i+1)%3;if d[i].abs()<1e-7{hits.push(points[i]);}if d[i]*d[j]< -1e-14{hits.push(points[i]+(points[j]-points[i])*(d[i]/(d[i]-d[j])));}}let mut unique=vec![];for p in hits{if !unique.iter().any(|other:&DVec3|(*other-p).length()<1e-7){unique.push(p);}}if unique.len()==2{edges.push((unique[0],unique[1]));}}}}
    }fn groups(groups:&[ingetrazo_core::groups::Group],parent:glam::DMat4,scene:&Scene,edges:&mut Vec<(DVec3,DVec3)>){for group in groups{if group.hidden||!scene.is_layer_visible(group.layer.as_deref()){continue;}let transform=parent*group.transform;mesh(&group.mesh,transform,scene,edges);children(&group.children,transform,scene,edges);}}fn children(children:&[ingetrazo_core::groups::Group],parent:glam::DMat4,scene:&Scene,edges:&mut Vec<(DVec3,DVec3)>){groups(children,parent,scene,edges)}let mut edges=vec![];mesh(&scene.mesh,glam::DMat4::IDENTITY,scene,&mut edges);groups(&scene.groups,glam::DMat4::IDENTITY,scene,&mut edges);edges}

/// Projects and renders the 3D model inside the ViewFrame at exact scale 1:N.
fn frame_bounds(scene:&Scene)->Option<(DVec3,DVec3)>{
 let mut points=vec![];for edge in &scene.mesh.edges{if scene.is_layer_visible(edge.layer.as_deref()){points.extend([scene.mesh.vertices[edge.v0].position,scene.mesh.vertices[edge.v1].position]);}}for(id,face)in scene.mesh.faces.iter().enumerate(){if scene.mesh_face_visible(&scene.mesh,id){points.extend(face.vertices.iter().map(|&i|scene.mesh.vertices[i].position));}}
 fn groups(items:&[ingetrazo_core::groups::Group],parent:glam::DMat4,scene:&Scene,points:&mut Vec<DVec3>){for group in items{if group.hidden||!scene.is_layer_visible(group.layer.as_deref()){continue;}let transform=parent*group.transform;points.extend(group.mesh.vertices.iter().map(|v|transform.transform_point3(v.position)));groups(&group.children,transform,scene,points);}}
 groups(&scene.groups,glam::DMat4::IDENTITY,scene,&mut points);if points.is_empty(){None}else{Some(points.into_iter().fold((DVec3::splat(f64::INFINITY),DVec3::splat(f64::NEG_INFINITY)),|(a,b),p|(a.min(p),b.max(p))))}
}
/// Resolve a frame without changing the live document or visible camera.
pub fn camera_for_frame(scene:&Scene,frame:&ViewFrame)->ingetrazo_core::camera::OrbitCamera{let snapshot=frame_scene(scene,frame);frame_camera(&snapshot,frame)}
fn frame_camera_live(scene:&Scene,frame:&ViewFrame,live:&ingetrazo_core::camera::OrbitCamera)->ingetrazo_core::camera::OrbitCamera{
 let bound=frame.view_key.strip_prefix("scene:").is_some_and(|name|scene.saved_views.iter().any(|view|view.name==name));
 frame_camera_with_base(scene,frame,if bound{&scene.camera}else{live})
}
fn frame_camera(scene:&Scene,frame:&ViewFrame)->ingetrazo_core::camera::OrbitCamera{
 frame_camera_with_base(scene,frame,&scene.camera)
}
fn frame_camera_with_base(scene:&Scene,frame:&ViewFrame,base:&ingetrazo_core::camera::OrbitCamera)->ingetrazo_core::camera::OrbitCamera{
 use ingetrazo_core::camera::StandardView;
 let mut camera=base.clone();let bound=frame.view_key.starts_with("scene:")&&scene.saved_views.iter().any(|v|Some(v.name.as_str())==frame.view_key.strip_prefix("scene:"));
 if frame.view_key.starts_with("std:"){camera.set_standard_view(match frame.view_key.as_str(){"std:front"=>StandardView::Front,"std:right"=>StandardView::Right,"std:back"=>StandardView::Back,"std:left"=>StandardView::Left,"std:iso"=>StandardView::Iso,"std:bottom"=>StandardView::Bottom,_=>StandardView::Top});if matches!(frame.view_key.as_str(),"std:top"|"std:bottom"){camera.pitch=if frame.view_key=="std:top"{std::f64::consts::FRAC_PI_2}else{-std::f64::consts::FRAC_PI_2};camera.up=DVec3::Y;}if let Some((lo,hi))=frame_bounds(scene){camera.target=(lo+hi)/2.;}}
 if let Some(yaw)=frame.cam_yaw{camera.yaw=yaw;}if let Some(pitch)=frame.cam_pitch{camera.pitch=pitch;if pitch.abs()<89.5_f64.to_radians(){camera.up=DVec3::Z;}}if let Some(target)=frame.cam_target{camera.target=DVec3::from_array(target);}
 let right=camera.right();let up=right.cross(camera.forward());let angle=frame.rot_deg.to_radians();camera.up=up*angle.cos()-right*angle.sin();camera.aspect=(frame.rect.w/25.4*ingetrazo_core::composition::RENDER_DPI).round_ties_even().max(1.)/(frame.rect.h/25.4*ingetrazo_core::composition::RENDER_DPI).round_ties_even().max(1.);
 camera.perspective=frame.perspective;
 if frame.perspective{if let Some(fov)=frame.cam_fov.or_else(||bound.then_some(camera.fov_deg)){camera.fov_deg=fov.clamp(5.,120.);}camera.distance=frame.cam_distance.or_else(||bound.then_some(camera.distance)).unwrap_or_else(||frame_bounds(scene).map(|(lo,hi)|(hi-lo).length()/2./(camera.fov_deg.to_radians()/2.).tan().max(1e-6)*1.15).filter(|d|*d>0.).unwrap_or(30.)).max(0.001);}else{camera.distance=ingetrazo_core::composition::ortho_distance_for_height(model_height_for_frame(frame.rect.h,frame.scale_n),camera.fov_deg);}
 camera
}
fn apply_frame_style(style:&mut ingetrazo_render::style::StyleConfig,value:&serde_json::Value) {
 use ingetrazo_render::style::VisualStyle;
 if let Some(mode)=value.get("face_mode").and_then(|v|v.as_str()) {
  style.show_textures=mode!="shaded";
  style.visual_style=match mode {"wireframe"=>VisualStyle::Wireframe,"hidden_line"=>VisualStyle::HiddenLine,"monochrome"=>VisualStyle::Monochrome,"xray"=>VisualStyle::XRay,_=>VisualStyle::ShadedWithTextures};
 }
 for(key,field)in [("edges",&mut style.show_edges),("profiles",&mut style.show_profiles),("sky",&mut style.show_sky),("back_edges",&mut style.back_edges_hidden)] {
  if let Some(v)=value.get(key).and_then(|v|v.as_bool()){*field=v;}
 }
 for(key,field)in [("edge_color",&mut style.edge_color),("front_color",&mut style.front_color),("back_color",&mut style.back_color),("background",&mut style.background_color),("sky_color",&mut style.sky_color),("ground_color",&mut style.ground_color),("section_fill_color",&mut style.section_fill_color)] {
  if let Some(v)=value.get(key).and_then(|v|v.as_array()).filter(|v|v.len()>=3){for i in 0..3 {if let Some(c)=v[i].as_f64(){field[i]=c as f32;}}}
 }
}
fn style_for_frame(scene:&Scene,frame:&ViewFrame,base:&ingetrazo_render::style::StyleConfig)->ingetrazo_render::style::StyleConfig {
 use ingetrazo_render::style::{StyleConfig,VisualStyle};let mut style=base.clone();
 if let Some(view)=frame.view_key.strip_prefix("scene:").and_then(|name|scene.saved_views.iter().find(|v|v.name==name)){if let Some(v)=&view.style {apply_frame_style(&mut style,v);}}
 if matches!(frame.style.as_str(),"tecnico"|"lineas") {style=StyleConfig::default_shaded();style.visual_style=if frame.style=="tecnico" {VisualStyle::HiddenLine}else{VisualStyle::Wireframe};style.show_sky=false;style.background_color=[1.;4];style.front_color=[1.;4];}
 else if let Some(name)=frame.style.strip_prefix("style:") {
  let known=matches!(name,"Default"|"Architectural"|"Shaded"|"Hidden line"|"Monochrome"|"Wireframe"|"X-ray");
  if known {style=StyleConfig::default_shaded();style.front_color=[1.;4];let mode=match name{"Shaded"=>"shaded","Hidden line"=>"hidden_line","Monochrome"=>"monochrome","Wireframe"=>"wireframe","X-ray"=>"xray",_=>"textures"};apply_frame_style(&mut style,&serde_json::json!({"face_mode":mode}));if matches!(name,"Architectural"|"Hidden line"|"Monochrome"|"Wireframe"){style.show_sky=false;style.background_color=[1.;4];}}
 }
 if frame.paper_bg&&!matches!(frame.style.as_str(),"tecnico"|"lineas"){style.show_sky=false;style.background_color=[1.;4];}
 style.show_grid=false;style
}

/// Paper hatch strokes at subpixel device widths use the original Qt
/// major-axis coverage rule instead of egui's perceptual feathered path.
/// Width attenuates coverage; it does not add a diagonal third pixel row.
/// Reference: https://github.com/qt/qtbase/blob/6.12/src/gui/painting/qcosmeticstroker.cpp
/// Actual original Qt 6.12 fixture: .72px diagonal pen covers two rows
/// with alpha 91/92; generic egui thin ink adds a third feathered row.
fn thin_paper_line(painter:&egui::Painter,ends:[Pos2;2],width:f32,color:Color32) {
 let ppp=painter.ctx().pixels_per_point();let mut a=ends[0]*ppp;let mut b=ends[1]*ppp;
 let vertical=(b.y-a.y).abs()>(b.x-a.x).abs();
 if vertical {a=Pos2::new(a.y,a.x);b=Pos2::new(b.y,b.x);}
 if a.x>b.x{std::mem::swap(&mut a,&mut b);}
 let fixed=|v:f32|(v*64.)as i64;let first=fixed(a.x);let last=fixed(b.x);
 if first==last{return;}
 let slope=((fixed(b.y)-fixed(a.y))*65536)/(last-first);
 let mut minor=(fixed(a.y)-32)*1024-(((first.rem_euclid(64)-32)*slope)>>6)-(slope>>1);
 let start=first-32;let end=last+32;let mut mesh=egui::Mesh::default();
 let opacity=(width*ppp*256.).clamp(0.,256.)as i64;
 let clip=painter.clip_rect();
 let clip_min=if vertical{clip.min.y}else{clip.min.x};let clip_max=if vertical{clip.max.y}else{clip.max.x};
 let begin=start.div_euclid(64).max((clip_min*ppp).floor()as i64);
 let finish=end.div_euclid(64).min((clip_max*ppp).ceil()as i64);
 minor+=(begin-start.div_euclid(64))*slope;
 for major in begin..=finish{
  let covered=(end.min((major+1)*64)-start.max(major*64)).max(0);
  let base=minor.div_euclid(65536);let fraction=(minor>>8).rem_euclid(256);
  for (row,coverage)in [(base,255-fraction),(base+1,fraction)] {
   let alpha=(((coverage*covered)>>6)*opacity>>8)as u8;if alpha==0{continue;}
   let (x,y)=if vertical{(row,major)}else{(major,row)};
   let rect=Rect::from_min_max(Pos2::new(x as f32/ppp,y as f32/ppp),Pos2::new((x+1)as f32/ppp,(y+1)as f32/ppp)).intersect(clip);
   if rect.is_positive(){mesh.add_colored_rect(rect,Color32::from_rgba_unmultiplied(color.r(),color.g(),color.b(),alpha));}
  }minor+=slope;
 }
 painter.add(egui::Shape::mesh(mesh));
}

fn paint_paper_cut(painter:&egui::Painter,rect:Rect,frame:&ViewFrame,scene:&Scene,matrix:&glam::DMat4) {
    if frame.cut_fill=="none" {return;}
    let color=frame.cut_fill_color.trim_start_matches('#');
    let rgb=u32::from_str_radix(color,16).unwrap_or(0x595e69);
    let color=Color32::from_rgb((rgb>>16)as u8,(rgb>>8)as u8,rgb as u8);
    let triangles=ingetrazo_render::vector_hlr::paper_section_triangles(scene).into_iter().filter_map(|tri|{
        Some([project_point(tri[0],matrix,rect)?.0,project_point(tri[1],matrix,rect)?.0,project_point(tri[2],matrix,rect)?.0])
    }).collect::<Vec<_>>();
    if frame.cut_fill!="hatch" {
        let mut mesh=egui::Mesh::default();for tri in triangles {
            let base=mesh.vertices.len()as u32;for pos in tri {mesh.colored_vertex(pos,color);}mesh.add_triangle(base,base+1,base+2);
        }painter.add(egui::Shape::mesh(mesh));return;
    }
    let pixel=rect.width()/frame.rect.w as f32;
    let step=(if frame.cut_hatch_mm==0. {1.5}else{frame.cut_hatch_mm}).max(0.3)as f32*pixel;
    let width=(if frame.pen_edge_mm==0. {0.18}else{frame.pen_edge_mm}).max(0.05)as f32*pixel;
    let local=triangles.iter().map(|t|t.map(|p|p-rect.min)).collect::<Vec<_>>();
    let lo=local.iter().flatten().map(|p|p.x+p.y).fold(f32::INFINITY,f32::min);
    let hi=local.iter().flatten().map(|p|p.x+p.y).fold(f32::NEG_INFINITY,f32::max);
    let mut c=(lo/step).floor()*step;
    while c<=hi {
        let mut intervals=vec![];
        for tri in &local {
            let mut xs=vec![];
            for i in 0..3 {let a=tri[i];let b=tri[(i+1)%3];let sa=a.x+a.y-c;let sb=b.x+b.y-c;
                if sa.abs()<1e-5 {xs.push(a.x);}
                if (sa<0.)!=(sb<0.) {xs.push(a.x+(b.x-a.x)*sa/(sa-sb));}
            }
            if xs.len()>=2 {let min=xs.iter().copied().fold(f32::INFINITY,f32::min);let max=xs.iter().copied().fold(f32::NEG_INFINITY,f32::max);if max>min {intervals.push((min,max));}}
        }
        intervals.sort_by(|a,b|a.0.total_cmp(&b.0));let mut merged:Vec<(f32,f32)>=vec![];
        for (a,b)in intervals {if let Some(last)=merged.last_mut(){if a<=last.1+1e-4 {last.1=last.1.max(b);continue;}}merged.push((a,b));}
        for(a,b)in merged {let ends=[rect.min+Vec2::new(a,c-a),rect.min+Vec2::new(b,c-b)];
            if width*painter.ctx().pixels_per_point()<=1. {thin_paper_line(painter,ends,width,color);}else{painter.line_segment(ends,Stroke::new(width,color));}
        }
        c+=step;
    }
}

/// Production composer model painter, also used by native acceptance fixtures.
pub fn render_model_projection(
    scene: &Scene,
    painter: &egui::Painter,
    frame_rect: Rect,
    frame: &ViewFrame,
) {
    render_model_projection_with_resources(scene,painter,frame_rect,frame,&ingetrazo_render::style::StyleConfig::default_shaded(),&std::collections::HashMap::new());
}
pub fn render_model_projection_with_resources(scene:&Scene,painter:&egui::Painter,frame_rect:Rect,frame:&ViewFrame,base:&ingetrazo_render::style::StyleConfig,textures:&std::collections::HashMap<String,egui::TextureId>) {
    render_model_projection_with_ink(scene,painter,frame_rect,frame,base,textures,None,None);
}
fn render_model_projection_with_ink(scene:&Scene,painter:&egui::Painter,frame_rect:Rect,frame:&ViewFrame,base:&ingetrazo_render::style::StyleConfig,textures:&std::collections::HashMap<String,egui::TextureId>,prepared:Option<&[ingetrazo_render::vector_hlr::VectorSegment]>,live_camera:Option<&ingetrazo_core::camera::OrbitCamera>) {
    let painter = painter.with_clip_rect(frame_rect);
    let snapshot=frame_scene(scene,frame);let scene=snapshot.as_ref();

    let camera=live_camera.map(|live|frame_camera_live(scene,frame,live)).unwrap_or_else(||frame_camera(scene,frame));
    let view_mat=camera.view_matrix();

    // Sacred scale equation: model_height_m = frame.rect.h * scale_n / 1000
    let model_h_m = model_height_for_frame(frame.rect.h, frame.scale_n);
    let model_w_m = model_h_m * (frame.rect.w / frame.rect.h);

    let proj_mat = if camera.perspective{camera.projection_matrix()}else{glam::DMat4::orthographic_rh(
        -model_w_m * 0.5,
        model_w_m * 0.5,
        -model_h_m * 0.5,
        model_h_m * 0.5,
        -camera.zfar,
        camera.zfar,
    )};
    let view_proj = proj_mat * view_mat;

    use ingetrazo_render::vector_hlr::LineKind;
    let vector=matches!(frame.style.as_str(),"technical"|"vectorial")&&!frame.perspective;
    if !vector {
        let mut rendered=scene.clone();rendered.show_section_planes=false;rendered.clear_selection();
        let mut style=style_for_frame(scene,frame,base);let pixel=frame_rect.width()/frame.rect.w as f32;
        style.line_width=frame.pen_edge_mm.max(0.05)as f32*pixel;style.profile_width=frame.pen_profile_mm.max(0.05)as f32*pixel;
        let mut data=ingetrazo_render::pipeline::build_scene_render_data(&rendered,&camera,&style,None);
        data.guides=Default::default();data.dimension_lines=Default::default();data.dimension_labels.clear();data.text_labels.clear();data.snap_glyphs.clear();
        painter.rect_filled(frame_rect,0.,Color32::from_rgb((style.background_color[0]*255.)as u8,(style.background_color[1]*255.)as u8,(style.background_color[2]*255.)as u8));
        ingetrazo_render::pipeline::render_scene_to_egui(&data,&camera,&painter,frame_rect,&style,textures);
        return;
    }
    if vector {paint_paper_cut(&painter,frame_rect,frame,scene,&view_proj);}
    let mut segments=if vector {
        if let Some(ink)=prepared{ink.iter().map(|s|(s.a,s.b,s.kind)).collect::<Vec<_>>()}else{
        ingetrazo_render::vector_hlr::parallel_segments(scene,&camera,frame.profiles,frame.hidden_lines).into_iter().map(|s|(s.a,s.b,s.kind)).collect::<Vec<_>>()
        }
    } else {section_projected_edges(scene).into_iter().map(|(a,b)|(a,b,LineKind::Edge)).collect()};
    // Thin hidden ink first, heavy cut ink last, as original _paint_hlr_lines_mm.
    segments.sort_by_key(|s|match s.2{LineKind::Hidden=>0,LineKind::Edge=>1,LineKind::Profile=>2,LineKind::Cut=>3});
    let paper_pixel=frame_rect.width()/frame.rect.w as f32;
    for(p0,p1,kind)in segments {
        if let(Some((s0,_)),Some((s1,_)))=(project_point(p0,&view_proj,frame_rect),project_point(p1,&view_proj,frame_rect)) {
            let pen=match kind {LineKind::Cut=>frame.pen_cut_mm,LineKind::Profile=>frame.pen_profile_mm,_=>frame.pen_edge_mm};
            let default=match kind {LineKind::Cut=>0.5,LineKind::Profile=>0.35,_=>0.18};
            let pen=if pen==0. {default}else{pen};
            let stroke=Stroke::new((pen.max(0.05) as f32)*paper_pixel,Color32::from_rgb(30,36,44));
            if kind==LineKind::Hidden {
                // Original dashed ink is 1.5 mm dash / 0.8 mm gap in paper units.
                let length=s0.distance(s1); let dash=1.5*paper_pixel;let period=2.3*paper_pixel;
                if period>0. {let direction=(s1-s0)/length.max(1e-6);let mut t=0.;while t<length {
                    painter.line_segment([s0+direction*t,s0+direction*(t+dash).min(length)],stroke);t+=period;
                }}
            }else{painter.line_segment([s0,s1],stroke);
                // QPainter solid ink has RoundCap; hidden strokes stay flat.
                painter.circle_filled(s0,stroke.width/2.,stroke.color);painter.circle_filled(s1,stroke.width/2.,stroke.color);
            }
        }
    }

    // Draw traced GeoPaths if present
    for gp in &scene.geo_paths {
        for (p0, p1) in gp.segments() {
            let Some((p0,p1))=scene.section_segment(p0,p1)else{continue;};
            if let (Some((s0, _)), Some((s1, _))) = (
                project_point(p0, &view_proj, frame_rect),
                project_point(p1, &view_proj, frame_rect),
            ) {
                painter.line_segment([s0, s1], Stroke::new(1.8, Color32::from_rgb(20, 100, 200)));
            }
        }
    }
}

/// Renders classical CAD Title Block (Cajetín).
fn render_title_block(painter: &egui::Painter, rect: Rect, tb: &TitleBlock, zoom: f32) {
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.4, Color32::BLACK));

    // Internal grid dividers
    let h_half = rect.height() * 0.5;
    let w_part = rect.width() * 0.65;

    // Horizontal divider
    painter.line_segment(
        [Pos2::new(rect.min.x, rect.min.y + h_half), Pos2::new(rect.max.x, rect.min.y + h_half)],
        Stroke::new(1.0, Color32::BLACK),
    );

    // Vertical divider
    painter.line_segment(
        [Pos2::new(rect.min.x + w_part, rect.min.y), Pos2::new(rect.min.x + w_part, rect.max.y)],
        Stroke::new(1.0, Color32::BLACK),
    );

    let font_title = FontId::proportional((11.0 * (zoom / 1.5)).clamp(9.0, 15.0));
    let font_body = FontId::proportional((9.0 * (zoom / 1.5)).clamp(7.5, 12.0));

    // Project Name & Drawing Title
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0),
        egui::Align2::LEFT_TOP,
        &tb.project_name,
        font_title.clone(),
        Color32::BLACK,
    );
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + h_half + 6.0),
        egui::Align2::LEFT_TOP,
        &tb.drawing_title,
        font_title,
        Color32::from_rgb(30, 30, 40),
    );

    // Right column metadata
    painter.text(
        Pos2::new(rect.min.x + w_part + 6.0, rect.min.y + 5.0),
        egui::Align2::LEFT_TOP,
        &format!("Date: {}", tb.date),
        font_body.clone(),
        Color32::from_rgb(50, 50, 50),
    );
    painter.text(
        Pos2::new(rect.min.x + w_part + 6.0, rect.min.y + 18.0),
        egui::Align2::LEFT_TOP,
        &format!("Scale: {}", tb.scale_text),
        font_body.clone(),
        Color32::from_rgb(50, 50, 50),
    );
    painter.text(
        Pos2::new(rect.min.x + w_part + 6.0, rect.min.y + h_half + 6.0),
        egui::Align2::LEFT_TOP,
        &format!("Sheet: {}", tb.sheet_number),
        font_body,
        Color32::from_rgb(20, 20, 20),
    );
}

/// Renders architectural North Arrow.
fn render_north_arrow(painter: &egui::Painter, rect: Rect, rot_deg: f64) {
    let center = rect.center();
    let r = rect.width() * 0.45;
    let rad = rot_deg.to_radians();

    // Circle ring
    painter.circle_stroke(center, r, Stroke::new(1.2, Color32::BLACK));

    // North arrow tip pointing up
    let tip = center + Vec2::new(-(rad.sin() as f32) * r * 0.9, -(rad.cos() as f32) * r * 0.9);
    let left = center + Vec2::new(-(rad.cos() as f32) * r * 0.4, (rad.sin() as f32) * r * 0.4);
    let right = center + Vec2::new((rad.cos() as f32) * r * 0.4, -(rad.sin() as f32) * r * 0.4);

    // Black half
    painter.add(egui::Shape::convex_polygon(
        vec![center, tip, left],
        Color32::BLACK,
        Stroke::new(1.0, Color32::BLACK),
    ));
    // White half
    painter.add(egui::Shape::convex_polygon(
        vec![center, tip, right],
        Color32::WHITE,
        Stroke::new(1.0, Color32::BLACK),
    ));

    // Letter 'N'
    let n_pos = tip + Vec2::new(0.0, -8.0);
    painter.text(n_pos, egui::Align2::CENTER_CENTER, "N", FontId::proportional(11.0), Color32::BLACK);
}

/// Renders Graphic Scale Bar.
fn render_graphic_scale(painter: &egui::Painter, rect: Rect, gs: &GraphicScaleItem, zoom: f32) {
    let bar_h = 4.0 * zoom;
    let div_w = rect.width() / gs.divisions as f32;

    for i in 0..gs.divisions {
        let fill = if i % 2 == 0 { Color32::BLACK } else { Color32::WHITE };
        let div_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + i as f32 * div_w, rect.min.y),
            Vec2::new(div_w, bar_h),
        );
        painter.rect_filled(div_rect, 0.0, fill);
        painter.rect_stroke(div_rect, 0.0, Stroke::new(1.0, Color32::BLACK));
    }

    // Metric length label (e.g. 5 m)
    let total_meters = (gs.length_mm * gs.scale_n) / 1000.0;
    painter.text(
        Pos2::new(rect.max.x, rect.min.y + bar_h + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("{total_meters:.1} m"),
        FontId::proportional((9.0 * (zoom / 1.5)).clamp(8.0, 12.0)),
        Color32::BLACK,
    );
}

/// Highlights selected item with dashed border and corner grips.
fn render_selection_box(painter: &egui::Painter, rect: Rect) {
    let highlight_color = Color32::from_rgb(30, 130, 240);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.5, highlight_color));

    // Corner handle boxes
    let handle_size = 6.0;
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for c in corners {
        let h_rect = Rect::from_center_size(c, Vec2::splat(handle_size));
        painter.rect_filled(h_rect, 0.0, Color32::WHITE);
        painter.rect_stroke(h_rect, 0.0, Stroke::new(1.2, highlight_color));
    }
}

#[cfg(test)]
mod transaction_snapshot_tests {
 use super::*;
 fn app()->IngeTrazoApp {
  let mut app=IngeTrazoApp::new();
  let mut sheet=Sheet::new("Active",PaperSize::A4,Orientation::Portrait);
  sheet.items.clear();
  for n in 0..32 {
   let mut sibling=sheet.clone();sibling.id=uuid::Uuid::new_v4();sibling.name=format!("Sibling {n}");
   sibling.native_document=serde_json::json!({"retained": "x".repeat(4096)});
   app.scene.sheets.push(sibling);
  }
  app.scene.sheets.insert(0,sheet);app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);app
 }
 fn frame(ctx:&egui::Context,app:&mut IngeTrazoApp,events:Vec<egui::Event>){
  let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1800.,1000.))),events,..Default::default()},|ctx|{
   egui::CentralPanel::default().show(ctx,|ui|render_sheet_composer(app,ui,0));
  });
 }
 fn button(p:Pos2,pressed:bool)->egui::Event {egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}}
 fn click(ctx:&egui::Context,app:&mut IngeTrazoApp,p:Pos2){frame(ctx,app,vec![egui::Event::PointerMoved(p),button(p,true),button(p,false)]);}
 fn control(ctx:&egui::Context,name:&str)->Pos2 {ctx.data(|d|d.get_temp::<Rect>(egui::Id::new(name)).unwrap().center())}
 #[test]
 fn idle_frames_and_real_toolbar_take_zero_then_one_active_sheet_snapshot(){
  let mut app=app();let ctx=egui::Context::default();let before=app.scene.sheets.clone();
  for _ in 0..24 {frame(&ctx,&mut app,vec![]);}
  assert_eq!(app.composer_state.edit_snapshots,0);assert!(app.composer_state.edit_before.is_none());
  click(&ctx,&mut app,control(&ctx,"composer-test-north"));
  assert_eq!(app.scene.sheets[0].items.len(),1);assert_eq!(app.composer_state.edit_snapshots,1);
  let id=app.scene.sheets[0].id;assert_eq!(app.composer_state.sheet_history[&id].0.len(),1);
  assert_eq!(app.scene.sheets[1..],before[1..]);
  app.undo_composer_edit();assert_eq!(app.scene.sheets,before);
  app.redo_composer_edit();assert_eq!(app.scene.sheets[0].items.len(),1);
  for _ in 0..24 {frame(&ctx,&mut app,vec![]);}
  assert_eq!(app.composer_state.edit_snapshots,1);
 }
 #[test]
 fn real_property_menu_edits_snapshot_before_mutation_and_coalesce_text(){
  let mut app=app();let mut view=ViewFrame::default();view.title_text="Before".into();
  app.scene.sheets[0].items.push(SheetItem::ViewFrame(view));app.scene.sheets[0].selected_item=Some(0);
  let before=app.scene.sheets[0].clone();let ctx=egui::Context::default();frame(&ctx,&mut app,vec![]);
  click(&ctx,&mut app,control(&ctx,"composer-test-view"));frame(&ctx,&mut app,vec![]);
  assert_eq!(app.composer_state.edit_snapshots,0);
  click(&ctx,&mut app,control(&ctx,"composer-test-perspective"));
  assert_eq!(app.composer_state.edit_snapshots,1);
  app.finish_composer_edit();app.undo_composer_edit();assert_eq!(app.scene.sheets[0],before);
  app.redo_composer_edit();let after_toggle=app.scene.sheets[0].clone();
  // Reopen if the checkbox closed the popup; the title control must own keyboard focus.
  frame(&ctx,&mut app,vec![]);
  click(&ctx,&mut app,control(&ctx,"composer-test-title"));
  let modifiers=egui::Modifiers{ctrl:true,command:true,..Default::default()};
  frame(&ctx,&mut app,vec![egui::Event::Key{key:egui::Key::A,physical_key:None,pressed:true,repeat:false,modifiers},egui::Event::Text("After".into())]);
  frame(&ctx,&mut app,vec![egui::Event::Text(" title".into())]);
  app.finish_composer_edit();assert_eq!(app.composer_state.edit_snapshots,2);
  let SheetItem::ViewFrame(edited)=&app.scene.sheets[0].items[0]else{panic!()};assert_eq!(edited.title_text,"After title");
  app.undo_composer_edit();assert_eq!(app.scene.sheets[0],after_toggle);
  assert_eq!(app.composer_state.sheet_history[&before.id].0.len(),1);
 }
 #[test]
 fn cancellation_and_finish_keep_reordered_and_independently_changed_siblings(){
  let mut app=app();app.scene.sheets[0].items.push(SheetItem::ViewFrame(ViewFrame::default()));
  let before=app.scene.sheets[0].clone();let id=before.id;
  app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(40.,50.);
  app.scene.sheets[1].name="Independent".into();let sibling=app.scene.sheets[1].clone();
  app.scene.sheets.swap(0,1);app.cancel_composer_edit();
  assert_eq!(app.scene.sheets[1],before);assert_eq!(app.scene.sheets[0],sibling);
  app.scene.active_sheet_index=Some(1);app.begin_composer_edit();app.scene.sheets[1].items[0].set_pos(60.,70.);
  app.scene.sheets[0].name="Later independent".into();let sibling=app.scene.sheets[0].clone();
  app.finish_composer_edit();assert_eq!(app.composer_state.sheet_history.len(),1);
  assert_eq!(app.composer_state.sheet_history[&id].0.len(),1);
  app.undo_composer_edit();assert_eq!(app.scene.sheets[1],before);assert_eq!(app.scene.sheets[0],sibling);
  app.redo_composer_edit();assert_eq!(app.scene.sheets[1].items[0].rect().x,60.);assert_eq!(app.scene.sheets[0],sibling);
 }
 #[test]
 fn continuing_real_drag_has_one_snapshot_and_one_reversible_transaction(){
  let mut app=app();app.scene.sheets[0].items.push(SheetItem::NorthArrow(NorthArrowItem{id:uuid::Uuid::new_v4(),pos:(50.,50.),size_mm:22.,rot_deg:0.}));app.scene.sheets[0].selected_item=Some(0);
  let before=app.scene.sheets[0].clone();let ctx=egui::Context::default();frame(&ctx,&mut app,vec![]);
  // Centered A4 portrait at 1.5px/mm in the real CentralPanel content rect.
  let p=Pos2::new(900.-210.*0.75+60.*1.5,500.-297.*0.75+60.*1.5);
  frame(&ctx,&mut app,vec![egui::Event::PointerMoved(p),button(p,true)]);
  for dx in [8.,12.,18.,24.,30.] {frame(&ctx,&mut app,vec![egui::Event::PointerMoved(p+Vec2::new(dx,0.))]);}
  frame(&ctx,&mut app,vec![button(p+Vec2::new(30.,0.),false)]);
  assert_eq!(app.composer_state.edit_snapshots,1);
  assert!(app.scene.sheets[0].items[0].rect().x>50.);
  assert_eq!(app.composer_state.sheet_history[&before.id].0.len(),1);
  app.undo_composer_edit();assert_eq!(app.scene.sheets[0],before);
 }
}

#[cfg(test)]mod section_frame_tests{use super::*;use ingetrazo_core::{groups::Group,mesh::Mesh,saved_views::SavedView,section::SectionPlane};#[test]fn bound_saved_scene_cut_clips_nested_edges_without_changing_live_document(){let mut mesh=Mesh::new();let a=mesh.add_vertex(DVec3::new(-2.,0.,0.));let b=mesh.add_vertex(DVec3::new(2.,0.,0.));mesh.add_edge(a,b);let mut child=Group::with_mesh("child",mesh);child.transform=glam::DMat4::from_translation(DVec3::X);let mut parent=Group::new("parent");parent.children.push(child);let mut scene=Scene::new();scene.groups.push(parent);scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::X,"Cut","A"));scene.show_section_cuts=true;let view=SavedView::capture_scene("Section",&scene,&scene.camera);scene.saved_views.push(view);scene.section_planes[0].active=false;let original=scene.clone();let mut frame=ViewFrame::default();frame.view_key="scene:Section".into();let snapshot=frame_scene(&scene,&frame);let edges=section_projected_edges(&snapshot);assert_eq!(edges.len(),1);assert_eq!(edges[0],(DVec3::new(-1.,0.,0.),DVec3::ZERO));assert_eq!(scene,original);assert_eq!(section_projected_edges(&scene)[0].1,DVec3::new(3.,0.,0.));}}

#[cfg(test)]mod frame_camera_tests{
 use super::*;
 #[test]fn canvas_field_index_reuses_warm_context_and_invalidates_edit_history_replacement() {
  let mut app=IngeTrazoApp::new();let mut a=ingetrazo_core::composition::Sheet::default();a.items.clear();let mut b=a.clone();b.id=uuid::Uuid::new_v4();app.scene.sheets=vec![a,b];
  assert_eq!(canvas_field_index(&mut app,1).unwrap(),0);assert_eq!(app.composer_state.field_index_computations,1);
  assert_eq!(canvas_field_index(&mut app,1).unwrap(),0);assert_eq!(app.composer_state.field_index_computations,1);
  let mut border=app.scene.sheets[1].effective_source_border();border.color="#ffffff".into();app.scene.sheets[1].set_source_border(border).unwrap();app.scene.bump_version();
  assert_eq!(canvas_field_index(&mut app,1).unwrap(),1);assert_eq!(app.composer_state.field_index_computations,2);
  let id=app.scene.sheets[1].id;app.scene.sheets[1]=app.scene.sheets[0].clone();app.scene.sheets[1].id=id;app.bump_model_revision();
  assert_eq!(canvas_field_index(&mut app,1).unwrap(),0);assert_eq!(app.composer_state.field_index_computations,3);
 }

 #[test]fn source_direct_paper_properties_do_not_create_history_or_replay_with_items() {
  let mut app=IngeTrazoApp::new();app.scene.sheets=vec![Sheet::default()];app.scene.sheets[0].items.clear();app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);
  let id=app.scene.sheets[0].id;
  app.begin_composer_edit();let mut border=app.scene.sheets[0].effective_source_border();border.color="#ffffff".into();app.scene.sheets[0].set_source_border(border).unwrap();app.finish_composer_edit();
  let direct=app.scene.sheets.clone();app.undo();app.redo();assert_eq!(app.scene.sheets,direct);assert!(app.composer_state.sheet_history.get(&id).is_none());
  app.begin_composer_edit();let item=ingetrazo_core::composition::TextItem{id:uuid::Uuid::new_v4(),pos:(20.,30.),text:"A".into(),font_size_pt:14.,bold:false};app.scene.sheets[0].items.push(SheetItem::Text(item));app.finish_composer_edit();
  app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(33.,44.);app.finish_composer_edit();assert_eq!(app.composer_state.sheet_history[&id].0.len(),2);
  let mut border=app.scene.sheets[0].effective_source_border();border.on=true;border.radius_mm=5.;app.scene.sheets[0].set_source_border(border.clone()).unwrap();app.scene.sheets[0].paper_size=PaperSize::A1;app.scene.sheets[0].orientation=Orientation::Portrait;
  app.undo();assert_eq!(app.scene.sheets[0].items[0].rect().x,20.);assert_eq!(app.scene.sheets[0].effective_source_border(),border);assert_eq!(app.scene.sheets[0].paper_size,PaperSize::A1);assert_eq!(app.scene.sheets[0].orientation,Orientation::Portrait);
  // A direct property edit preserves an already pending item Redo.
  border.on=false;border.color="#ff0000".into();app.scene.sheets[0].set_source_border(border.clone()).unwrap();app.redo();assert_eq!(app.scene.sheets[0].items[0].rect().x,33.);assert_eq!(app.scene.sheets[0].effective_source_border(),border);
  app.undo();app.undo();assert!(app.scene.sheets[0].items.is_empty());assert_eq!(app.scene.sheets[0].effective_source_border(),border);app.redo();app.redo();assert_eq!(app.scene.sheets[0].items[0].rect().x,33.);assert_eq!(app.scene.sheets[0].effective_source_border(),border);assert!(app.undo_stack.is_empty());
  app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(99.,99.);border.color="#0000ff".into();app.scene.sheets[0].set_source_border(border.clone()).unwrap();app.cancel_composer_edit();assert_eq!(app.scene.sheets[0].items[0].rect().x,33.);assert_eq!(app.scene.sheets[0].effective_source_border(),border);
 }

 #[test]fn subpixel_paper_hatch_keeps_original_two_row_coverage(){
  let ctx=egui::Context::default();ctx.begin_pass(Default::default());
  let painter=ctx.layer_painter(egui::LayerId::background());thin_paper_line(&painter,[Pos2::new(10.,30.),Pos2::new(30.,10.)],0.72,Color32::from_rgb(255,0,255));
  let output=ctx.end_pass();let egui::Shape::Mesh(mesh)=&output.shapes[0].shape else{panic!("subpixel hatch must emit device pixel coverage");};
  let mut rows=std::collections::BTreeMap::new();
  for tri in mesh.indices.chunks_exact(6){let vertices=&mesh.vertices[tri[0]as usize..tri[0]as usize+4];let x=vertices.iter().map(|v|v.pos.x).fold(f32::INFINITY,f32::min);let y=vertices.iter().map(|v|v.pos.y).fold(f32::INFINITY,f32::min);if (x-15.).abs()<1e-6{rows.insert(y as i32,vertices[0].color.a());}}
  assert_eq!(rows.len(),2);assert_eq!(rows.values().copied().collect::<Vec<_>>(),vec![91,92]);
 }
 #[test]fn model_history_preserves_later_sheet_edits_and_switch_retires_drag(){
  use ingetrazo_core::composition::{Sheet,PaperSize,Orientation};let mut app=IngeTrazoApp::new();app.scene.sheets.clear();
  let mut a=Sheet::new("A",PaperSize::A4,Orientation::Portrait);a.items.push(SheetItem::ViewFrame(ViewFrame::default()));let mut b=a.clone();b.id=uuid::Uuid::new_v4();b.name="B".into();app.scene.sheets=vec![a,b];
  app.save_undo_state();app.scene.mesh.add_vertex(DVec3::new(7.,8.,9.));app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(33.,44.);
  app.composer_state.dragging_item_idx=Some(0);app.composer_state.drag_item_orig_pos=Some((20.,20.));app.composer_state.drag_start_pos=Some(Pos2::ZERO);
  let b=app.scene.sheets[1].clone();app.scene.active_sheet_index=Some(1);app.enter_composer_sheet(1);assert!(app.composer_state.dragging_item_idx.is_none());assert!(app.composer_state.edit_before.is_none());assert_eq!(app.scene.sheets[1],b);
  let edited=app.scene.sheets.clone();app.scene.active_sheet_index=None;app.undo();assert!(app.scene.mesh.vertices.is_empty());assert_eq!(app.scene.sheets,edited);app.redo();assert_eq!(app.scene.mesh.vertices[0].position,DVec3::new(7.,8.,9.));assert_eq!(app.scene.sheets,edited);
 }
 #[test]fn sheet_gestures_have_scoped_undo_and_cancel_or_selection_has_no_history(){
  use ingetrazo_core::composition::{Sheet,PaperSize,Orientation};
  let mut app=IngeTrazoApp::new();app.scene.sheets.clear();app.document_session.preferences.undo_steps=64;
  let mut sheet=Sheet::new("Atomic",PaperSize::A4,Orientation::Portrait);sheet.items.push(SheetItem::ViewFrame(ViewFrame::default()));let id=sheet.id;app.scene.sheets.push(sheet);app.scene.active_sheet_index=Some(0);
  let initial=app.scene.sheets.clone();app.begin_composer_edit();
  for x in [30.,40.,50.] {app.scene.sheets[0].items[0].set_pos(x,60.);}
  app.finish_composer_edit();assert_eq!(app.composer_state.sheet_history[&id].0.len(),1);assert!(app.undo_stack.is_empty());app.undo();assert_eq!(app.scene.sheets,initial);app.redo();assert_eq!(app.scene.sheets[0].items[0].rect().x,50.);
  let history=app.composer_state.sheet_history[&id].0.len();app.begin_composer_edit();app.scene.sheets[0].selected_item=Some(0);app.finish_composer_edit();assert_eq!(app.composer_state.sheet_history[&id].0.len(),history);
  let before=app.scene.sheets.clone();app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(90.,100.);app.cancel_composer_edit();assert_eq!(app.scene.sheets,before);assert_eq!(app.composer_state.sheet_history[&id].0.len(),history);
  app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(70.,80.);app.undo();assert_eq!(app.scene.sheets,before);assert!(app.composer_state.edit_before.is_none());
  app.save_undo_state();app.scene.mesh.add_vertex(DVec3::new(4.,5.,6.));let model=app.scene.mesh.clone();let geometry_history=app.undo_stack.len();
  app.scene.sheets.push(Sheet::new("Neighbor",PaperSize::A4,Orientation::Portrait));app.scene.active_sheet_index=Some(1);let neighbor=app.scene.sheets[1].clone();app.undo();assert_eq!(app.scene.sheets[1],neighbor);assert_eq!(app.scene.mesh,model);assert_eq!(app.undo_stack.len(),geometry_history);
  app.scene.active_sheet_index=Some(0);app.undo();assert_eq!(app.scene.mesh,model);assert_eq!(app.scene.sheets[1],neighbor);assert_eq!(app.undo_stack.len(),geometry_history);
  // General history limits never evict the independent ComposerHistory.
  app.composer_state.sheet_history.clear();
  let baseline=app.scene.sheets[0].clone();
  for (cap,x) in [(0,110.),(1,120.),(1,130.)] {
   app.document_session.preferences.undo_steps=cap;
   app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(x,140.);app.finish_composer_edit();
  }
  assert_eq!(app.composer_state.sheet_history[&id].0.len(),3);
  for _ in 0..3 {app.undo_composer_edit();}
  assert_eq!(app.scene.sheets[0],baseline);
  assert_eq!(app.composer_state.sheet_history[&id].1.len(),3);
  for _ in 0..3 {app.redo_composer_edit();}
  assert_eq!(app.scene.sheets[0].items[0].rect().x,130.);
  assert_eq!(app.composer_state.sheet_history[&id].0.len(),3);
  assert_eq!(app.scene.mesh,model);assert_eq!(app.scene.sheets[1],neighbor);

 }
 #[test]fn raster_frame_style_uses_saved_snapshot_and_paper_without_live_mutation(){
  use ingetrazo_render::style::{StyleConfig,VisualStyle};
  let mut scene=Scene::new();let mut view=ingetrazo_core::saved_views::SavedView::capture_scene("Ink",&scene,&scene.camera);view.style=Some(serde_json::json!({"face_mode":"shaded","sky":true,"background":[0.2,0.3,0.4]}));scene.saved_views.push(view);let original=scene.clone();
  let mut frame=ViewFrame::default();frame.view_key="scene:Ink".into();frame.style="sombreado".into();let base=StyleConfig::wireframe();let style=style_for_frame(&scene,&frame,&base);assert_eq!(style.visual_style,VisualStyle::ShadedWithTextures);assert!(!style.show_textures);assert!(style.show_sky);assert_eq!(style.background_color[0],0.2);
  frame.paper_bg=true;let paper=style_for_frame(&scene,&frame,&base);assert!(!paper.show_sky);assert_eq!(paper.background_color,[1.;4]);
  frame.style="tecnico".into();let plan=style_for_frame(&scene,&frame,&base);assert_eq!(plan.visual_style,VisualStyle::HiddenLine);assert!(!plan.show_sky);assert_eq!(plan.front_color,[1.;4]);assert_eq!(scene,original);assert_eq!(base.visual_style,VisualStyle::Wireframe);
 }
 #[test]fn hidden_face_boundaries_and_contours_are_absent_from_sheet(){
  let mut scene=Scene::new();let ids=[DVec3::new(-1.,0.,0.),DVec3::new(1.,0.,0.),DVec3::new(-1.,1.,0.)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);scene.section_planes.push(ingetrazo_core::section::SectionPlane::new(DVec3::ZERO,DVec3::X,"Cut","A"));scene.show_section_cuts=true;assert!(!section_projected_edges(&scene).is_empty());scene.mesh.faces[0].attrs.insert("hidden".into(),serde_json::json!(true));assert!(section_projected_edges(&scene).is_empty());
 }
 #[test]fn frame_pixel_aspect_saved_lens_clamp_and_two_point_follow_original(){
  let mut scene=Scene::new();scene.camera.fov_deg=200.;scene.camera.two_point=true;scene.saved_views.push(ingetrazo_core::saved_views::SavedView::capture_scene("Lens",&scene,&scene.camera));let mut frame=ViewFrame::default();frame.view_key="scene:Lens".into();frame.rect.w=111.11;frame.rect.h=77.77;frame.perspective=true;let camera=camera_for_frame(&scene,&frame);assert_eq!(camera.fov_deg,120.);assert!(camera.two_point);assert!((camera.aspect-1312./919.).abs()<1e-12);
 }
 #[test]fn frame_bounds_matches_original_edge_and_group_vertex_semantics(){
  let mut scene=Scene::new();let a=scene.mesh.add_vertex(DVec3::new(10.,0.,0.));let b=scene.mesh.add_vertex(DVec3::new(12.,0.,0.));let e=scene.mesh.add_edge(a,b).unwrap();scene.mesh.edges[e].hidden=true;assert_eq!(frame_bounds(&scene),Some((DVec3::new(10.,0.,0.),DVec3::new(12.,0.,0.))));let mut group=ingetrazo_core::groups::Group::new("Vertices");group.mesh.add_vertex(DVec3::new(30.,0.,0.));scene.groups.push(group);assert_eq!(frame_bounds(&scene).unwrap().1.x,30.);scene.groups[0].hidden=true;assert_eq!(frame_bounds(&scene).unwrap().1.x,12.);
 }
 #[test]fn frame_overrides_roll_scale_and_perspective_have_original_camera_semantics(){
  let mut scene=Scene::new();let ids=[DVec3::new(10.,20.,0.),DVec3::new(12.,20.,0.),DVec3::new(10.,24.,2.)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);let mut frame=ViewFrame::default();assert_eq!(frame.view_key,"__current__");assert_eq!(frame.style,"sombreado");assert!(!frame.show_title&&!frame.border);frame.view_key="std:top".into();frame.scale_n=20.;frame.rot_deg=90.;let camera=camera_for_frame(&scene,&frame);assert_eq!(camera.target,DVec3::new(11.,22.,1.));assert!((camera.pitch-std::f64::consts::FRAC_PI_2).abs()<1e-12);assert!((camera.up+DVec3::X).length()<1e-12);assert!((camera.distance-ingetrazo_core::composition::ortho_distance_for_height(3.,45.)).abs()<1e-12);
  frame.perspective=true;frame.cam_yaw=Some(-1.2);frame.cam_pitch=Some(0.3);frame.cam_target=Some([4.,5.,6.]);frame.cam_distance=Some(7.);frame.cam_fov=Some(200.);let camera=camera_for_frame(&scene,&frame);assert_eq!(camera.target,DVec3::new(4.,5.,6.));assert_eq!(camera.distance,7.);assert_eq!(camera.fov_deg,120.);assert!(camera.perspective);frame.cam_fov=Some(60.);let camera=camera_for_frame(&scene,&frame);let matrix=camera.projection_matrix()*camera.view_matrix();let near=camera.eye()+camera.forward()*2.+camera.right();let far=camera.eye()+camera.forward()*4.+camera.right();let a=matrix*near.extend(1.);let b=matrix*far.extend(1.);assert!((a.x/a.w-2.*b.x/b.w).abs()<1e-12,"perspective must foreshorten far geometry");
 }
 #[test]fn saved_section_view_uses_original_camera_direction_without_mutating_live_state(){
  let mut scene=Scene::new();scene.camera.yaw=-1.1;scene.camera.pitch=0.42;scene.camera.target=DVec3::new(4.,5.,6.);scene.saved_views.push(ingetrazo_core::saved_views::SavedView::capture_scene("Cut",&scene,&scene.camera));let captured=scene.camera.clone();scene.camera.yaw=0.3;let original=scene.clone();let mut frame=ViewFrame::default();frame.view_key="scene:Cut".into();let snapshot=frame_scene(&scene,&frame);let camera=frame_camera(&snapshot,&frame);assert_eq!(camera.target,captured.target);assert!((camera.forward()-captured.forward()).length()<1e-12);assert!(!camera.perspective);assert_eq!(scene,original);
  frame.view_key="std:front".into();assert!(frame_camera(&scene,&frame).eye().y<0.);frame.view_key="std:right".into();assert!(frame_camera(&scene,&frame).eye().x>0.);
 }
}

/// Export the selected frame as classified orthographic model-space ink.
/// Reuses the production persistent worker/cache; no scene clone per line or subprocess per export.
impl IngeTrazoApp {
    pub fn export_composer_view_dxf(&mut self,sheet_idx:usize,path:&std::path::Path)->Result<usize,String> {
        let sheet=self.scene.sheets.get(sheet_idx).ok_or("No sheet selected")?;
        let frame=match sheet.selected_item.and_then(|i|sheet.items.get(i)) {
            Some(SheetItem::ViewFrame(frame))=>frame.clone(),
            _=>return Err(tr("Select a view frame to export")),
        };
        let snapshot=frame_scene(&self.scene,&frame);
        let camera=frame_camera_live(&snapshot,&frame,&self.camera);
        let ink=self.composer_compute.segments(&snapshot,&camera,self.clipboard_document,self.model_revision,&frame.view_key,frame.profiles,frame.hidden_lines)?;
        let right=camera.right();let up=right.cross(camera.forward());let eye=camera.eye();
        let mut classes:[Vec<[f64;4]>;4]=Default::default();
        for segment in ink.iter() {
            let index=match segment.kind {ingetrazo_render::vector_hlr::LineKind::Edge=>0,ingetrazo_render::vector_hlr::LineKind::Profile=>1,ingetrazo_render::vector_hlr::LineKind::Cut=>2,ingetrazo_render::vector_hlr::LineKind::Hidden=>3};
            if index==3&&!frame.hidden_lines {continue;}
            let a=segment.a-eye;let b=segment.b-eye;
            classes[index].push([a.dot(right),a.dot(up),b.dot(right),b.dot(up)]);
        }
        let title=if let Some(name)=frame.view_key.strip_prefix("scene:"){name.to_string()}else if let Some(key)=frame.view_key.strip_prefix("std:"){
            tr(match key {"top"=>"Top (plan)","bottom"=>"bottom","front"=>"Front","back"=>"Back","right"=>"Right","left"=>"Left","iso"=>"Isometric",_=>key})
        }else{tr("View")};
        let names=[title.clone(),format!("{title}-PERFIL"),format!("{title}-CORTE"),format!("{title}-OCULTAS")];
        let groups=(0..if frame.hidden_lines{4}else{3}).map(|i|ingetrazo_formats::sheet_dxf::ViewLayer{name:&names[i],segments:&classes[i],dashed:i==3}).collect::<Vec<_>>();
        let count=ingetrazo_formats::sheet_dxf::save_view_dxf(path,&groups).map_err(|e|e.to_string())?;
        Ok(count)
    }
}

#[cfg(test)] mod live_composer_camera_tests {
 use super::*;
 #[test] fn current_and_missing_saved_use_live_camera_while_bound_view_uses_snapshot(){
 let mut scene=Scene::new();let mut live=scene.camera.clone();live.yaw=1.2;live.pitch=0.4;live.target=DVec3::new(4.,5.,6.);let before=scene.clone();let mut frame=ViewFrame::default();frame.view_key="__current__".into();
 let current=frame_camera_live(&scene,&frame,&live);assert_eq!(current.target,live.target);assert!((current.forward()-live.forward()).length()<1e-12);assert_eq!(scene,before);
 frame.view_key="scene:missing".into();assert_eq!(frame_camera_live(&scene,&frame,&live).target,live.target);
 scene.saved_views.push(ingetrazo_core::saved_views::SavedView::capture_scene("Saved",&scene,&scene.camera));frame.view_key="scene:Saved".into();let snapshot=frame_scene(&scene,&frame);assert_eq!(frame_camera_live(&snapshot,&frame,&live).target,scene.camera.target);
 }
}

/// Resolve authored colors before geometry submission, preserving encoded RGB.
/// Terrain band opacity replaces QColor alpha, matching source setAlphaF(0.6).
pub fn precise_shape_palette(commands:&[ingetrazo_core::sheet_shape::ShapePaint])->Result<Vec<(Option<ingetrazo_render::cad2d::EncodedRgba>,Option<ingetrazo_render::cad2d::EncodedRgba>)>,String> {
 use ingetrazo_render::cad2d::EncodedRgba;
 fn color(value:&str,alpha:Option<f64>)->Result<EncodedRgba,String>{
  let bytes=ingetrazo_core::sheet_color::parse_sheet_color(value)?;
  let mut value=EncodedRgba::from_bytes(bytes,1.)?;
  if let Some(alpha)=alpha {value.0[3]=alpha as f32;value.validate()?;}
  Ok(value)
 }
 commands.iter().map(|c|Ok((
  c.stroke.as_ref().map(|(s,_)|color(s,None)).transpose()?,
  c.fill.as_ref().map(|(s,a)|color(s,if *a==1. {None}else{Some(*a)})).transpose()?
 ))).collect()
}

/// Physical shape commands are shared with export; screen projection happens once here.
pub fn paint_sheet_shape(painter:&egui::Painter,shape:&ingetrazo_core::sheet_shape::SheetShape,zoom:f32,origin:Pos2)->Result<(),String> {
 use ingetrazo_core::sheet_shape::ShapePath;
 fn color(value:&str,alpha:f64)->Result<Color32,String> {
  let rgba=ingetrazo_core::sheet_color::parse_sheet_color(value)?;
  let opacity=if alpha==1. {rgba[3]}else {(alpha*255.).round()as u8};
  Ok(Color32::from_rgba_unmultiplied(rgba[0],rgba[1],rgba[2],opacity))
 }
 let point=|p:glam::DVec2|origin+Vec2::new(((shape.x_mm+p.x)*f64::from(zoom))as f32,((shape.y_mm+p.y)*f64::from(zoom))as f32);
 let commands=shape.paint_commands(100_000)?;
 // Resolve all palette entries before touching the painter: a bad colour cannot half-paint an item.
 let prepared=commands.iter().map(|c|Ok((c.stroke.as_ref().map(|(s,w)|color(s,1.).map(|v|Stroke::new(*w as f32*zoom,v))).transpose()?.unwrap_or(Stroke::NONE),c.fill.as_ref().map(|(s,a)|color(s,*a)).transpose()?.unwrap_or(Color32::TRANSPARENT)))).collect::<Result<Vec<_>,String>>()?;
 for (command,(stroke,fill)) in commands.into_iter().zip(prepared) {
  match command.path {
   ShapePath::RoundedRect{size,radius}=>{
    // QPainter strokes are centered on the physical path. egui RectShape
    // and convex_polygon default to outside strokes, shifting the document ink.
    let points=if radius<=0. {vec![point(glam::DVec2::ZERO),point(glam::DVec2::new(size.x,0.)),point(size),point(glam::DVec2::new(0.,size.y))]}else {
     let mut points=Vec::with_capacity(100);
     for (center,start) in [(glam::DVec2::new(size.x-radius,radius),-std::f64::consts::FRAC_PI_2),(size-glam::DVec2::splat(radius),0.),(glam::DVec2::new(radius,size.y-radius),std::f64::consts::FRAC_PI_2),(glam::DVec2::splat(radius),std::f64::consts::PI)] {
      for i in 0..=24 {let angle=start+std::f64::consts::FRAC_PI_2*f64::from(i)/24.;points.push(point(center+glam::DVec2::new(angle.cos(),angle.sin())*radius));}
     }points
    };
    painter.add(egui::epaint::PathShape{points,closed:true,fill,stroke:stroke.into()});
   },
   ShapePath::Ellipse{size}=>{let center=size/2.;let points=(0..96).map(|i|{let t=std::f64::consts::TAU*f64::from(i)/96.;point(center+glam::DVec2::new(center.x*t.cos(),center.y*t.sin()))}).collect();painter.add(egui::epaint::PathShape{points,closed:true,fill,stroke:stroke.into()});},
   ShapePath::Polygon(points)=>{painter.add(egui::epaint::PathShape{points:points.into_iter().map(point).collect(),closed:true,fill,stroke:stroke.into()});},
   ShapePath::Line(mut a,mut b)=>{
    if let Some(clip)=command.clip {
     let delta=b-a;let mut lo=0_f64;let mut hi=1_f64;
     let area=clip.iter().zip(clip.iter().cycle().skip(1)).map(|(a,b)|a.perp_dot(*b)).sum::<f64>();let sign=area.signum();
     for i in 0..clip.len() {let edge=clip[(i+1)%clip.len()]-clip[i];let base=sign*edge.perp_dot(a-clip[i]);let slope=sign*edge.perp_dot(delta);if slope.abs()<1e-12 {if base<0. {lo=1.;hi=0.;break;}}else {let t=-base/slope;if slope>0. {lo=lo.max(t);}else {hi=hi.min(t);}}}
     if lo>hi {continue;}b=a+delta*hi;a+=delta*lo;
    }
    let points=[point(a),point(b)];painter.line_segment(points,stroke);
    if stroke.width>0. {for p in points {painter.circle_filled(p,stroke.width/2.,stroke.color);}}
   },
  }
 }
 Ok(())
}

#[cfg(test)]mod shape_authoring_tests {
 use super::*;
 #[test]fn precise_palette_preserves_bytes_and_source_alpha_replacement(){
  use ingetrazo_core::sheet_shape::{ShapePaint,ShapePath};
  let c=ShapePaint{path:ShapePath::Line(glam::DVec2::ZERO,glam::DVec2::X),stroke:Some(("#80255d9d".into(),0.35)),fill:Some(("#20255d9d".into(),0.6)),clip:None};
  let p=precise_shape_palette(&[c.clone()]).unwrap();
  assert_eq!(p[0].0.unwrap().0,[37./255.,93./255.,157./255.,128./255.]);
  assert_eq!(p[0].1.unwrap().0,[37./255.,93./255.,157./255.,0.6]);
  let mut invalid=c;invalid.fill=Some(("not-a-color".into(),0.6));
  assert!(precise_shape_palette(&[invalid]).is_err());
 }

 #[test]fn shape_placement_is_atomic_undoable_and_invalid_input_preserves_history(){
  let mut app=IngeTrazoApp::new();app.scene.sheets.push(ingetrazo_core::composition::Sheet::new("Shapes",PaperSize::A4,Orientation::Portrait));app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);
  let before=app.scene.sheets.clone();let id=app.place_composer_shape("linea",glam::DVec2::new(2.,3.),glam::DVec2::new(8.,3.)).unwrap();
  let after=app.scene.sheets.clone();let item=after[0].items.last().unwrap();assert_eq!(item.id(),id);let SheetItem::Shape(item)=item else{panic!()};assert_eq!(item.shape.h_mm,0.);assert_eq!(item.shape.z,1.);
  assert_eq!(app.composer_state.sheet_history[&after[0].id].0.len(),1);
  assert!(app.place_composer_shape("rect",glam::DVec2::splat(f64::NAN),glam::DVec2::ONE).is_err());assert_eq!(app.scene.sheets,after);
  app.undo_composer_edit();assert_eq!(app.scene.sheets,before);app.redo_composer_edit();assert_eq!(app.scene.sheets,after);
  let state=app.scene.clone();app.composer_state.shape_tool=Some("rect".into());app.composer_state.shape_start=Some(glam::DVec2::ZERO);app.cancel_composer_edit();assert_eq!(app.scene,state);assert!(app.composer_state.shape_start.is_none());assert!(app.composer_state.shape_tool.is_none());
 }
 #[test]fn real_composer_pointer_events_preview_then_commit_one_history_item(){
  let mut app=IngeTrazoApp::new();let mut sheet=ingetrazo_core::composition::Sheet::new("Pointer",PaperSize::A4,Orientation::Portrait);sheet.items.clear();app.scene.sheets.push(sheet);app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);app.composer_state.shape_tool=Some("rect".into());
  app.scene.sheets.remove(0);app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);
  let ctx=egui::Context::default();
  fn frame(ctx:&egui::Context,app:&mut IngeTrazoApp,events:Vec<egui::Event>){let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.,800.))),events,..Default::default()},|ctx|{egui::CentralPanel::default().show(ctx,|ui|render_sheet_composer(app,ui,0));});}
  fn click(p:Pos2)->Vec<egui::Event>{vec![egui::Event::PointerMoved(p),egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()},egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]}
  frame(&ctx,&mut app,vec![]);frame(&ctx,&mut app,click(Pos2::new(430.,330.)));assert!(app.composer_state.shape_start.is_some());assert!(app.scene.sheets[0].items.is_empty(),"unexpected items {:?}",app.scene.sheets[0].items);assert!(app.composer_state.sheet_history.is_empty());
  frame(&ctx,&mut app,vec![egui::Event::PointerMoved(Pos2::new(520.,420.))]);assert!(app.scene.sheets[0].items.is_empty());
  frame(&ctx,&mut app,click(Pos2::new(520.,420.)));assert_eq!(app.scene.sheets[0].items.len(),1);assert_eq!(app.composer_state.sheet_history[&app.scene.sheets[0].id].0.len(),1);assert!(app.composer_state.shape_start.is_none());
  app.undo_composer_edit();assert!(app.scene.sheets[0].items.is_empty());
 }

}

impl IngeTrazoApp {
    /// Prepare every page before opening the destination. Unsupported visible
    /// painters report an error rather than silently exporting incomplete sheets.
    pub fn export_composer_pdf(&mut self,sheet_idx:Option<usize>,path:&std::path::Path)->Result<usize,String>{
        let sheets=match sheet_idx{Some(index)=>std::slice::from_ref(self.scene.sheets.get(index).ok_or("Sheet no longer exists")?),None=>self.scene.sheets.as_slice()};
        let scene=&self.scene;let camera=&self.camera;let compute=&mut self.composer_compute;let document=self.clipboard_document;let revision=self.model_revision;
        let text_resources=&self.composer_state.paper_text;
        let current_file=self.current_file.as_deref();
        if sheets.is_empty() || sheets.len()>10_000 {return Err("PDF requires 1..10000 pages".into());}
        let indices=crate::composer_text_fields::source_indices(&scene.sheets,sheets)?;
        let mut painter=|sheet:&Sheet,frame:&ViewFrame|{
            let date=crate::composer_text_fields::today()?;
            let index=*indices.get(&(sheet as *const _)).ok_or("View sheet context lost")?;
            let fields=crate::composer_text_fields::Context{index:Some(index),total:scene.sheets.len(),path:current_file,date:&date};
            pdf::view_draws(scene,camera,compute,document,revision,sheet,frame,&mut text_resources.borrow_mut(),&fields).map(|v|v.draws)
        };
        let mut texts=|sheet:&ingetrazo_core::composition::Sheet,item:&ingetrazo_core::composition::TextItem|{
            if item.text.contains('{'){
                // Source field_values samples the native local date for each Text expansion.
                let date=crate::composer_text_fields::today()?;
                let fields=crate::composer_text_fields::Context{index:Some(*indices.get(&(sheet as *const _)).ok_or("Paper Text context no longer exists")?),total:scene.sheets.len(),path:current_file,date:&date};
                pdf::text_draws(sheet,item,&mut text_resources.borrow_mut(),Some(&fields))
            }else{pdf::text_draws(sheet,item,&mut text_resources.borrow_mut(),None)}
        };
        let mut symbols=|sheet:&Sheet,item:&SheetItem|{
            let date=crate::composer_text_fields::today()?;
            let fields=crate::composer_text_fields::Context{index:Some(*indices.get(&(sheet as *const _)).ok_or("Symbol sheet context lost")?),total:scene.sheets.len(),path:current_file,date:&date};
            crate::composer_symbols::draws(sheet,item,&mut text_resources.borrow_mut(),Some(&fields))
        };
        let mut legends=|_:&Sheet,value:&serde_json::Value|crate::composer_legend::draws(value,&mut text_resources.borrow_mut()).map(|v|v.draws);
        let pages=ingetrazo_formats::sheet_pdf_shapes::prepare_all_with_legends(sheets,&mut painter,&mut texts,&mut symbols,&mut legends)?;
        let has_view=sheets.iter().any(|s|s.items.iter().any(|item|matches!(item,SheetItem::ViewFrame(_))));
        if has_view || sheets.iter().any(|s|s.items.iter().any(|item|!matches!(item,SheetItem::Shape(_)))){ingetrazo_formats::sheet_pdf::save_original_qt_paper(path,&pages)?;}else{ingetrazo_formats::sheet_pdf::save(path,&pages)?;}Ok(pages.len())
    }
}

/// Engineering observation of production native Canvas tiles; no GPU/WSI claim.
#[cfg(target_os="macos")]
#[doc(hidden)]
pub fn probe_native_font_tiles(root: &std::path::Path) -> Result<(), String> {
    text_canvas::run_native_font_tiles(root)
}
