// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Main eframe / egui Desktop Application state and update loop.

use std::path::{Path, PathBuf};
use glam::DVec3;
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_core::scene::{Scene, SelectionItem};
use ingetrazo_core::snap::SnapEngine;
use ingetrazo_formats::{load_file, save_file};
use ingetrazo_render::style::StyleConfig;
use ingetrazo_tools::base::{create_tool, Tool, ToolType};

use crate::dialogs::render_dialogs;
use crate::menu::render_menu_bar;
use crate::statusbar::render_status_bar;
use crate::toolbar::{render_left_cad_palette, render_top_toolbar};
use crate::tray::render_side_tray;
use crate::viewport::render_viewport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrayTab {
    #[default]
    Inspector,
    Properties,
    Bim,
    Terrain,
    Ai,
    Render,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GeometryClipboard {
    pub mesh: ingetrazo_core::mesh::Mesh,
    pub reference: DVec3,
    pub source_document: uuid::Uuid,
    pub resources: Scene,
    pub groups: Vec<ingetrazo_core::groups::Group>,
}

pub struct IngeTrazoApp {
    pub(crate) visibility_owners: std::collections::HashMap<ingetrazo_core::saved_views::VisibilityTarget, bool>,
    /// Runtime-only native GPU viewport state; never serialized into the document.
    pub native_composer:crate::composer_cad::NativeComposer,
    pub image_outlines:crate::image_outlines::ImageOutlineCache,
    #[cfg(target_os="macos")]
    pub(crate) native_metal_world:Option<crate::native_metal::world_viewport::Viewport>,
    pub native_gpu_viewport_enabled: bool,
    pub native_gpu_viewport_rect: Option<egui::Rect>,
    pub native_gpu_viewport_texture: Option<egui::TextureId>,
    pub(crate) native_gpu_annotation_shape:Option<egui::Shape>,
    pub document_session: crate::document_session::DocumentSession,
    pub component_edit_stack: Vec<crate::component_edit::ComponentEditSession>,
    pub scene: Scene,
    pub camera: OrbitCamera,
    pub(crate) cursor_navigation:crate::cursor_navigation::CursorNavigation,
    pub navigation: crate::navigation::Navigation,
    pub active_tool_type: ToolType,
    fillet_parameters:(f64,usize),
    shape_segments:[usize;3],
    arc_repeat_distance:Option<f64>,
    last_shape_tool:Option<(ToolType,Option<ingetrazo_tools::ArcMode>)>,
    pub active_tool: Box<dyn Tool>,
    pub style: StyleConfig,
    pub snap_engine: SnapEngine,
    pub(crate) annotation_pick_cache: crate::annotation_pick::Picks,
    pub vcb_text: String,
    pub status_message: String,
    pub show_toolbar: bool,
    pub show_tray: bool,
    pub shadows_enabled: bool,
    pub show_about_dialog: bool,
    pub text3d_dialog: Option<crate::text3d::Dialog>,
    pub show_simplify_dialog: bool,
    pub simplify_angle: f64,
    pub show_open_dialog: bool,
    pub show_geographic_import_dialog: bool,
    pub survey_profile:crate::survey_profile::State,
    pub base_map_requested_zoom:u8,
    pub base_map_requested_visible:bool,
    pub show_save_dialog: bool,
    pub dialog_path_input: String,
    pub active_material: String,
    pub current_file: Option<PathBuf>,
    pub undo_stack: Vec<Scene>,
    pub redo_stack: Vec<Scene>,
    pub textures: std::collections::HashMap<String, egui::TextureHandle>,
    pub language: String,
    pub icons: crate::icons::IconManager,
    pub recent_files: Vec<PathBuf>,
    pub clipboard: Option<GeometryClipboard>,
    pub system_clipboard: Option<arboard::Clipboard>,
    pub clipboard_document: uuid::Uuid,
    pub(crate) clipboard_mesh_share_imports: crate::clipboard::MeshShareImports,
    pub selected_dimension: Option<usize>,
    pub dimension_text_input: Option<(usize,String)>,
    pub annotation_editor:Option<crate::annotation_editor::Editor>,
    pub parts_options:crate::outliner::PartsOptions,
    pub parts_cache:crate::outliner::PartsCache,
    pub outliner_rename: Option<(uuid::Uuid,String)>,
    pub selected_saved_view: Option<usize>,
    pub paste_preview: Option<crate::clipboard::PastePreview>,
    pub section_name_prompt:Option<(uuid::Uuid,String,String,bool)>,
    pub active_tray_tab: TrayTab,
    pub active_layer_idx: usize,
    pub selected_mat_category: Option<String>,
    pub show_preferences_dialog: bool,
    pub show_command_search: bool,
    pub show_solid_inspector: bool,
    pub show_scripting_console: bool,
    #[cfg(feature="engineering-python")]
    pub engineering_python_enabled:bool,
    #[cfg(feature="engineering-python")]
    pub(crate) engineering_python:crate::engineering_python::Engine,
    pub show_texture_edit_dialog: bool,
    pub(crate) show_material_edit_dialog:bool,
    pub command_search_query: String,
    pub console_input: String,
    pub ai_session: ingetrazo_core::ai::Session,
    pub(crate) ai_bridge: Option<ingetrazo_core::ai_bridge::Bridge>,
    pub(crate) ai_query_cache: crate::ai_bridge::QueryCache,
    pub console_logs: Vec<String>,
    pub bim_active_class: String,
    pub bim_name_input: String,
    pub bim_active_tag_mode: bool,
    pub(crate) bim_cache: crate::bim_controller::Cache,
    pub texture_scale_u: f32,
    pub texture_scale_v: f32,
    pub texture_rot_deg: f32,
    pub texture_opacity: f32,
    pub terrain_contour_interval: f64,
    pub sheet_image:crate::sheet_image::State,
    pub composer_state: crate::composer::ComposerState,
    pub composer_compute:crate::composer_compute::ComposerCompute,
    pub model_revision:u64,
    pub native_geometry_revision:u64,
    pub native_render_revisions:ingetrazo_render::scene_revision::RenderRevisions,
    pub blender_state: crate::blender::BlenderRenderState,
    pub show_render_dialog: bool,
}

impl Default for IngeTrazoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl IngeTrazoApp {
    pub fn new() -> Self {
        let scene = Scene::new();
        let mut camera = OrbitCamera::new();
        camera.distance = 20.0;

        let env_lang = std::env::var("LANG").unwrap_or_default().to_lowercase();
        let lang = if env_lang.starts_with("es") {
            "es"
        } else if env_lang.starts_with("pt") {
            "pt-BR"
        } else if env_lang.starts_with("it") {
            "it"
        } else if env_lang.starts_with("id") {
            "id"
        } else if env_lang.starts_with("zh") {
            "zh-CN"
        } else {
            "en"
        };
        ingetrazo_core::i18n::set_language(lang);

        let mut app = Self {
            document_session: crate::document_session::DocumentSession::default(),
            #[cfg(target_os="macos")] native_metal_world:None,
            native_composer:Default::default(),image_outlines:Default::default(),native_gpu_viewport_enabled:false,native_gpu_viewport_rect:None,native_gpu_viewport_texture:None,native_gpu_annotation_shape:None,
            component_edit_stack: Vec::new(),
            scene,
            camera,
            cursor_navigation:Default::default(),
            navigation: Default::default(),
            active_tool_type: ToolType::Select,
            fillet_parameters:(0.1,8),
            shape_segments:[24,6,16],
            arc_repeat_distance:None,
            last_shape_tool:None,
            active_tool: create_tool(ToolType::Select),
            style: StyleConfig::default_shaded(),
            snap_engine: SnapEngine::default(),
            annotation_pick_cache: Default::default(),
            vcb_text: String::new(),
            status_message: String::new(),
            show_toolbar: true,
            show_tray: true,
            shadows_enabled: false,
            show_about_dialog: false,
            text3d_dialog: None,
            show_simplify_dialog: false,
            simplify_angle: 1.0,
            show_open_dialog: false,
            show_geographic_import_dialog: false,
            survey_profile:Default::default(),
            base_map_requested_zoom:16,
            base_map_requested_visible:true,
            show_save_dialog: false,
            dialog_path_input: String::new(),
            active_material: "Default Cream".to_string(),
            current_file: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            textures: std::collections::HashMap::new(),
            language: lang.to_string(),
            icons: crate::icons::IconManager::new(),
            recent_files: Vec::new(),
            clipboard: None,
            system_clipboard: None,
            clipboard_document: uuid::Uuid::new_v4(),
            clipboard_mesh_share_imports: Default::default(),
            paste_preview: None,section_name_prompt:None,
            selected_saved_view: None,
            parts_options:crate::outliner::PartsOptions::default(),
            parts_cache:crate::outliner::PartsCache::default(),
            outliner_rename: None,
            selected_dimension: None,
            dimension_text_input: None,
            annotation_editor:None,
            active_tray_tab: TrayTab::Inspector,
            active_layer_idx: 0,
            selected_mat_category: None,
            show_preferences_dialog: false,
            show_command_search: false,
            show_solid_inspector: false,
            show_scripting_console: false,
            #[cfg(feature="engineering-python")]
            engineering_python_enabled:false,
            #[cfg(feature="engineering-python")]
            engineering_python:Default::default(),
            show_texture_edit_dialog: false,show_material_edit_dialog:false,
            command_search_query: String::new(),
            console_input: String::new(),
            ai_session: Default::default(),
            ai_bridge: None,
            ai_query_cache: Default::default(),
            console_logs: vec!["IngeTrazo 0.3.15 Interactive Console initialized.".to_string()],
            bim_active_class: "IfcWall".to_string(),
            bim_name_input: String::new(),
            bim_active_tag_mode: false,
            bim_cache: Default::default(),
            texture_scale_u: 1.0,
            texture_scale_v: 1.0,
            texture_rot_deg: 0.0,
            texture_opacity: 1.0,
            terrain_contour_interval: 1.0,
            sheet_image:Default::default(),composer_state: crate::composer::ComposerState::default(),
            visibility_owners:Default::default(),composer_compute:Default::default(),model_revision:0,native_geometry_revision:0,native_render_revisions:Default::default(),
            blender_state: crate::blender::BlenderRenderState::default(),
            show_render_dialog: false,
        };

        if app.scene.sheets.is_empty() {
            app.scene.sheets.push(ingetrazo_core::composition::Sheet::new(
                "Sheet 1",
                ingetrazo_core::composition::PaperSize::A3,
                ingetrazo_core::composition::Orientation::Landscape,
            ));
        }

        app.apply_document_presentation();
        app.mark_document_clean();
        app
    }

    pub fn set_language(&mut self, lang: &str) {
        self.language = lang.to_string();
        ingetrazo_core::i18n::set_language(lang);
    }

    pub fn with_file(path: &Path) -> Self {
        let mut app = Self::new();
        app.open_document(path);
        app
    }

    /// Context-menu Change Axes uses a runtime placement identity, not a row index.
    pub fn start_change_axes(&mut self)->bool {
        let targets:Vec<_>=self.scene.selection.iter().filter_map(|item|match item{ingetrazo_core::scene::SelectionItem::Group(i)=>self.scene.groups.get(*i).map(|g|g.history_id),_=>None}).collect();
        if self.scene.selection.len()!=1||targets.len()!=1{self.status_message="Select one group or component first".into();return false;}
        self.set_tool(ToolType::ChangeAxes);self.active_tool=Box::new(ingetrazo_tools::ChangeAxesTool::new(targets[0]));self.status_message="Click the new origin".into();true
    }

    pub fn activate_arc(&mut self,mode:ingetrazo_tools::ArcMode){
        self.set_tool(ToolType::Arc);
        let mut tool=ingetrazo_tools::ArcTool::with_mode(mode);
        if mode==ingetrazo_tools::ArcMode::TwoPoint{tool.segments=self.shape_segments[2];tool.set_repeat_distance(self.arc_repeat_distance);}
        self.active_tool=Box::new(tool);self.active_tool.on_activate(&mut self.scene);
        self.last_shape_tool=Some((ToolType::Arc,Some(mode)));
    }
    pub fn set_tool(&mut self, tool_type: ToolType) {
        if let Some(n)=self.active_tool.shape_segments(){match self.active_tool_type{ToolType::Circle=>self.shape_segments[0]=n,ToolType::Polygon=>self.shape_segments[1]=n,ToolType::Arc=>self.shape_segments[2]=n,_=>{}}}
        if self.active_tool.arc_mode()==Some(ingetrazo_tools::ArcMode::TwoPoint){self.arc_repeat_distance=self.active_tool.arc_repeat_distance();}
        if let Some(parameters)=self.active_tool.fillet_parameters(){self.fillet_parameters=parameters;}
        self.cancel_blender_pick();
        self.navigation.cancel(); self.navigation.mode=None;self.cursor_navigation.cancel();
        self.paste_preview=None;
        let version=self.scene.version;let images=self.scene.image_planes.publication_revision();self.active_tool.on_deactivate(&mut self.scene);
        if self.scene.version!=version{self.bump_model_revision();}else if self.scene.image_planes.publication_revision()!=images{self.bump_image_presentation_revision();}
        self.snap_engine.reset_operation_state();
        self.active_tool_type = tool_type;
        self.active_tool = if tool_type==ToolType::Fillet{Box::new(ingetrazo_tools::fillet::FilletTool::with_parameters(self.fillet_parameters.0,self.fillet_parameters.1))}else if tool_type==ToolType::Paint{let color=self.scene.materials.get(&self.active_material).map(|m|m.color).unwrap_or([0.8,0.8,0.8,1.]);Box::new(ingetrazo_tools::paint::PaintTool::with_material(self.active_material.clone(),color))}else{create_tool(tool_type)};
        match tool_type{
            ToolType::Circle=>self.active_tool=Box::new(ingetrazo_tools::CircleTool::with_segments(self.shape_segments[0])),
            ToolType::Polygon=>{let mut t=ingetrazo_tools::PolygonTool::default();t.sides=self.shape_segments[1];self.active_tool=Box::new(t);},
            ToolType::Arc=>{let mut t=ingetrazo_tools::ArcTool::with_segments(self.shape_segments[2]);t.set_repeat_distance(self.arc_repeat_distance);self.active_tool=Box::new(t);},_=>{}
        }
        if matches!(tool_type,ToolType::Line|ToolType::Rectangle|ToolType::Freehand|ToolType::Circle|ToolType::Polygon|ToolType::Arc|ToolType::RotatedRectangle|ToolType::Move|ToolType::Rotate){self.last_shape_tool=Some((tool_type,(tool_type==ToolType::Arc).then_some(ingetrazo_tools::ArcMode::TwoPoint)));}else if tool_type!=ToolType::Select{self.last_shape_tool=None;}
        self.active_tool.on_activate(&mut self.scene);
        self.status_message.clear();
        self.vcb_text.clear();
    }


    pub fn new_document(&mut self) {self.request_document_action(crate::document_session::DocumentAction::New);}

    pub(crate) fn new_document_unchecked(&mut self) {
        if !self.component_edit_stack.is_empty() {
            self.status_message = "Finish or cancel the component edit before creating a document".into();
            return;
        }
        self.retire_current_recovery();
        self.scene = Scene::new();self.bim_active_tag_mode=false;self.apply_document_presentation();self.bump_model_revision();
        self.composer_state=Default::default();
        self.clipboard_document=uuid::Uuid::new_v4();self.clipboard_mesh_share_imports.clear();
        self.camera = self.scene.camera.clone();
        self.initialize_document_units(self.document_session.preferences.new_document_units());
        self.undo_stack.clear();self.redo_stack.clear();
        self.current_file = None;
        self.mark_document_clean();
        self.textures.clear();
        self.status_message.clear();
    }

    pub fn open_document(&mut self, path: &Path) {self.request_document_action(crate::document_session::DocumentAction::Open(path.to_path_buf()));}

    pub fn begin_open_document(&mut self) {
        self.request_document_action(crate::document_session::DocumentAction::ChooseOpen);
    }

    pub fn open_chosen_document(&mut self,path:&Path) {
        self.show_open_dialog=false;
        // File/Open has already resolved unsaved changes before the chooser.
        // Import callers and intervening edits still need their own guard.
        self.sync_document_presentation();
        let approved=self.document_session.open_dialog_revision.take().is_some_and(|(revision,version,style,shadows)|
            revision==self.model_revision && version==self.scene.version && style==self.scene.display_style && shadows==self.scene.shadows);
        if approved {
            self.open_document_unchecked(path);
        } else {self.open_document(path);}
    }

    pub fn cancel_open_document(&mut self) {
        self.show_open_dialog=false;self.document_session.open_dialog_revision=None;
    }

    pub(crate) fn open_document_unchecked(&mut self, path: &Path) {
        if !self.component_edit_stack.is_empty() {
            self.status_message = "Finish or cancel the component edit before opening a document".into();
            return;
        }
        match load_file(path) {
            Ok(loaded_scene) => {
                if let Some(capture)=&loaded_scene.tile_layer{self.base_map_requested_zoom=capture.zoom.clamp(1,21) as u8;self.base_map_requested_visible=capture.visible;}
                self.camera = loaded_scene.camera.clone();
                self.retire_current_recovery();
                self.composer_state=Default::default();
                self.scene = loaded_scene;self.bim_active_tag_mode=false;self.apply_document_presentation();self.bump_model_revision();self.clipboard_document=uuid::Uuid::new_v4();self.clipboard_mesh_share_imports.clear();
                self.undo_stack.clear();self.redo_stack.clear();
                self.current_file = Some(path.to_path_buf());
                self.remember_recent_document(path);
                self.textures.clear();
                self.mark_document_clean();
                self.persist_preferences();
                self.status_message.clear();
            }
            Err(err) => {
                self.status_message = format!("Failed to open file: {err}");
            }
        }
    }

    pub fn save_document_as(&mut self) {
        if !self.finish_contexts_for_document() { self.cancel_document_action(); return; }
        self.show_save_dialog = true;
        self.dialog_path_input = self.current_file.as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "model.igz".into());
    }

    pub fn save_document(&mut self) {
        if let Some(path) = self.current_file.clone().filter(|p|p.extension().and_then(|e|e.to_str()).map(|e|e.eq_ignore_ascii_case("igz")).unwrap_or(false)) {
            self.save_to_path(&path);
        } else {
            self.save_document_as();
        }
    }

    pub fn save_to_path(&mut self, path: &Path) {
        if self.document_session.pending.is_some() && path.extension().and_then(|e|e.to_str()).map(|e|!e.eq_ignore_ascii_case("igz")).unwrap_or(true) {
            self.status_message="Save an .igz document before continuing".into();self.document_session.confirm_unsaved=true;return;
        }
        if path.extension().and_then(|s|s.to_str()).is_some_and(|s|s.eq_ignore_ascii_case("ifc")) {
            self.status_message=match self.export_bim_ifc(path){Ok(n)=>format!("{n} IFC elements exported to {}",path.display()),Err(error)=>format!("Export IFC failed: {error}")};
            return;
        }
        if !self.finish_contexts_for_document() { self.cancel_document_action(); return; }
        if let Err(error)=self.refresh_composer_frame_anchors(){self.status_message=format!("Save failed: {error}");self.cancel_document_action();return;}
        self.sync_document_presentation();
        // Like the source camera_home update, synchronize before the write.
        // Saving must not copy every mesh just to substitute the camera.
        self.scene.camera = self.camera.clone();
        match save_file(&self.scene, path) {
            Ok(()) => {
                if !path.extension().and_then(|e|e.to_str()).map(|e|e.eq_ignore_ascii_case("igz")).unwrap_or(false) {
                    self.status_message=format!("Exported: {}",path.display());return;
                }
                self.retire_current_recovery();
                self.scene.camera = self.camera.clone();
                self.current_file = Some(path.to_path_buf());
                self.remember_recent_document(path);
                self.mark_document_clean();self.persist_preferences();
                self.finish_document_action();
                self.status_message = format!("Saved: {}", path.display());
            }
            Err(err) => {
                // A failed Save aborts the requesting New/Open/Close action.
                // The source prompt has returned by this point; retry is a
                // fresh user action, never a deferred document replacement.
                self.cancel_document_action();
                self.status_message = format!("Save failed: {err}");
            }
        }
    }

    pub fn save_undo_state(&mut self) {
        self.sync_visibility_owners();
        self.sync_document_presentation();
        self.seal_view_history();self.seal_terrain_history();self.seal_layer_history();self.seal_image_history();
        self.bump_model_revision();
        self.finish_composer_edit();
        self.scene.camera = self.camera.clone();
        self.push_prepared_undo_snapshot(self.scene.clone());
    }

    /// Existing model history ownership, with its before-state prepared by an
    /// application consumer before native publication. Callers reserve a slot.
    #[cfg(feature = "engineering-python")]
    pub(crate) fn save_prepared_undo_state(&mut self, mut before: Scene) {
        self.sync_document_presentation();
        self.seal_view_history();
        self.seal_terrain_history();
        self.seal_layer_history();
        self.seal_image_history();
        self.bump_model_revision();
        self.finish_composer_edit();
        before.camera = self.camera.clone();
        crate::document_presentation::preserve(&self.scene, &mut before);
        self.push_prepared_undo_snapshot(before);
    }
    pub(crate) fn push_prepared_undo_snapshot(&mut self, before: Scene) {
        self.undo_stack.push(before);
        let mut reclaim_images = !self.redo_stack.is_empty();
        self.redo_stack.clear();
        let cap=self.document_session.preferences.undo_steps;
        if cap>0 && self.undo_stack.len()>cap {
            self.undo_stack.drain(..self.undo_stack.len()-cap);
            reclaim_images = true;
        }
        if reclaim_images { self.collect_image_metadata(); }
    }

    pub fn set_layer_visibility(&mut self,index:usize,visible:bool)->bool{
        self.change_layer_visibility(index,visible)
    }
    pub(crate) fn bump_model_revision(&mut self){self.native_render_revisions.model_replaced();self.navigation.cancel();self.native_geometry_revision=self.native_geometry_revision.wrapping_add(1);self.bump_image_presentation_revision();}
    /// Trusted group-only preview: Vulkan still uses flattened geometry and must
    /// receive its existing invalidation. Metal static prototypes stay retained.
    pub(crate) fn bump_placement_revision(&mut self){
        self.native_render_revisions.placement_changed();
        self.navigation.cancel();
        self.native_geometry_revision=self.native_geometry_revision.wrapping_add(1);
        self.bump_image_presentation_revision();
    }
    pub(crate) fn bump_render_paint_revision(&mut self){
        self.native_render_revisions.paint_changed();
        self.navigation.cancel();self.native_geometry_revision=self.native_geometry_revision.wrapping_add(1);
        self.bump_image_presentation_revision();
    }
    pub(crate) fn bump_render_geometry_revision(&mut self){
        self.native_render_revisions.geometry_changed();
        self.navigation.cancel();self.native_geometry_revision=self.native_geometry_revision.wrapping_add(1);
        self.bump_image_presentation_revision();
    }
    pub(crate) fn bump_render_style_revision(&mut self){
        self.native_render_revisions.style_changed();
        self.native_geometry_revision=self.native_geometry_revision.wrapping_add(1);
        // Presentation is outside source modeling history; do not retire Move array commands.
        self.composer_compute.invalidate();
    }
    pub(crate) fn bump_render_layers_revision(&mut self){
        self.native_render_revisions.layers_changed();
        self.native_geometry_revision=self.native_geometry_revision.wrapping_add(1);
        self.snap_engine.invalidate_center_cache();
        self.composer_compute.invalidate();
    }
    pub(crate) fn bump_image_presentation_revision(&mut self){self.model_revision=self.model_revision.wrapping_add(1);self.composer_compute.invalidate();}
    pub fn undo(&mut self) {self.undo_outcome();}
    pub(crate) fn undo_outcome(&mut self)->bool {
        self.sync_visibility_owners();
        self.sync_document_presentation();
        if self.scene.active_sheet_index.is_some(){self.undo_composer_edit();return false;}
        self.finish_composer_edit();
        if self.active_tool.prepare_undo(!self.undo_stack.is_empty()){return false;}
        if let Some(changed)=self.material_history_step(false){return changed;}
        if let Some(changed)=self.bim_metadata_history_step(false){return changed;}
        if self.refuse_component_history_step(false){return false;}
        self.seal_view_history();self.seal_terrain_history();self.seal_layer_history();self.seal_image_history();
        let version=self.scene.version;self.active_tool.end_transient_before_history(&mut self.scene);
        if self.scene.version!=version{self.bump_model_revision();}
        if let Some(changed)=self.component_history_step(false){return changed;}
        #[cfg(feature="engineering-python")]
        let python_step = match self.restore_engineering_python_history(false) {
            Ok(step)=>step, Err(error)=>{self.status_message=error;return false;}
        };
        if let Some(mut prev) = self.undo_stack.pop() {
            if let Some(command)=prev.history_axes.take() {
                let mut candidate=self.scene.clone();
                match command.apply(&mut candidate).and_then(|()|self.prepare_axes_parked(&command,&candidate)) {
                    Ok(parked)=>{self.publish_axes_parked(parked);let mut inverse=self.scene.clone();inverse.history_axes=Some(command.inverse());self.redo_stack.push(inverse);self.scene=candidate;self.bump_model_revision();self.status_message="Undo".into();return true;},
                    Err(error)=>{prev.history_axes=Some(command);self.undo_stack.push(prev);self.status_message=error;return false;}
                }
            }
            self.scene.camera = self.camera.clone();
            if prev.history_shareback.is_some() {
                let mut inverse=self.scene.clone();inverse.history_shareback=prev.history_shareback;
                return match crate::component_edit::shareback_restore(&self.scene,&prev) {
                    Ok(restored)=>{self.redo_stack.push(inverse);self.scene=restored;self.bump_model_revision();self.status_message="Undo".into();true},
                    Err(error)=>{self.undo_stack.push(prev);self.status_message=error;false},
                };
            }
            if prev.history_annotation_create.is_some() { crate::annotation_editor::preserve_creation_selection(&self.scene, &mut prev); }
            let mut inverse=crate::history_terrain::inverse_snapshot(&mut self.scene);inverse.history_survey=prev.history_survey;prev.history_survey=false;inverse.history_annotation_create=prev.history_annotation_create.take();inverse.history_annotation_edit=prev.history_annotation_edit;prev.history_annotation_edit=false;inverse.history_annotation_move=prev.history_annotation_move;prev.history_annotation_move=false;inverse.history_annotation_delete=prev.history_annotation_delete;prev.history_annotation_delete=false;inverse.history_views.0=crate::history_views::restore(&self.scene,&mut prev);inverse.history_group_visibility=crate::history_visibility::restore(&self.scene,&mut prev,&mut self.visibility_owners);self.apply_context_visibility();inverse.history_layers.0=crate::history_layers::restore(&self.scene,&mut prev);crate::history_terrain::restore(&mut self.scene,&mut prev,&mut inverse);inverse.history_images.0=crate::image_plane_actions::restore(&self.scene,&mut prev);self.redo_stack.push(inverse);
            // Model history cannot replace later independent composer edits.
            prev.camera=self.camera.clone();
            prev.sheets=self.scene.sheets.clone();prev.active_sheet_index=self.scene.active_sheet_index;
            // Source map capture is a runtime panel publication; model commands
            // do not own it. Transfer the current capture through older history.
            prev.tile_layer=self.scene.tile_layer.take();
            crate::document_presentation::preserve(&self.scene,&mut prev);
            let python_marker=prev.history_python.take();
            if let Some(inverse)=self.redo_stack.last_mut() {inverse.history_python=python_marker;}
            self.scene = prev;self.bump_model_revision();
            #[cfg(feature="engineering-python")]
            if python_step {self.apply_engineering_python_history_groups();self.engineering_python.published(self.model_revision);}
            self.camera = self.scene.camera.clone();
            self.status_message = "Undo".to_string();
            return true;
        }
        false
    }

    pub fn redo(&mut self) {self.redo_outcome();}
    pub(crate) fn redo_outcome(&mut self)->bool {
        self.sync_visibility_owners();
        self.sync_document_presentation();
        if self.scene.active_sheet_index.is_some(){self.redo_composer_edit();return false;}
        self.finish_composer_edit();
        if let Some(changed)=self.material_history_step(true){return changed;}
        if let Some(changed)=self.bim_metadata_history_step(true){return changed;}
        if self.refuse_component_history_step(true){return false;}
        let version=self.scene.version;self.active_tool.end_transient_before_history(&mut self.scene);
        if self.scene.version!=version{self.bump_model_revision();}
        if let Some(changed)=self.component_history_step(true){return changed;}
        #[cfg(feature="engineering-python")]
        let python_step = match self.restore_engineering_python_history(true) {
            Ok(step)=>step, Err(error)=>{self.status_message=error;return false;}
        };
        if let Some(mut next) = self.redo_stack.pop() {
            if let Some(saved)=next.history_axes.take() {
                let command=match ingetrazo_core::change_axes::Command::prepare(&self.scene,saved.target,saved.frame){Ok(command)=>self.complete_axes_command(command),Err(error)=>{next.history_axes=Some(saved);self.redo_stack.push(next);self.status_message=error;return false;}};
                let mut candidate=self.scene.clone();
                match command.apply(&mut candidate).and_then(|()|self.prepare_axes_parked(&command,&candidate)) {
                    Ok(parked)=>{self.publish_axes_parked(parked);let mut inverse=self.scene.clone();inverse.history_axes=Some(command.inverse());self.undo_stack.push(inverse);self.scene=candidate;self.bump_model_revision();self.status_message="Redo".into();return true;},
                    Err(error)=>{next.history_axes=Some(command);self.redo_stack.push(next);self.status_message=error;return false;}
                }
            }
            self.scene.camera = self.camera.clone();
            if next.history_shareback.is_some() {
                let mut inverse=self.scene.clone();inverse.history_shareback=next.history_shareback;
                return match crate::component_edit::shareback_restore(&self.scene,&next) {
                    Ok(restored)=>{self.undo_stack.push(inverse);self.scene=restored;self.bump_model_revision();self.status_message="Redo".into();true},
                    Err(error)=>{self.redo_stack.push(next);self.status_message=error;false},
                };
            }
            if next.history_annotation_create.is_some() { crate::annotation_editor::preserve_creation_selection(&self.scene, &mut next); }
            let mut inverse=crate::history_terrain::inverse_snapshot(&mut self.scene);inverse.history_survey=next.history_survey;next.history_survey=false;inverse.history_annotation_create=next.history_annotation_create.take();inverse.history_annotation_edit=next.history_annotation_edit;next.history_annotation_edit=false;inverse.history_annotation_move=next.history_annotation_move;next.history_annotation_move=false;inverse.history_annotation_delete=next.history_annotation_delete;next.history_annotation_delete=false;inverse.history_views.0=crate::history_views::restore(&self.scene,&mut next);inverse.history_group_visibility=crate::history_visibility::restore(&self.scene,&mut next,&mut self.visibility_owners);self.apply_context_visibility();inverse.history_layers.0=crate::history_layers::restore(&self.scene,&mut next);crate::history_terrain::restore(&mut self.scene,&mut next,&mut inverse);inverse.history_images.0=crate::image_plane_actions::restore(&self.scene,&mut next);self.undo_stack.push(inverse);
            next.camera=self.camera.clone();
            next.sheets=self.scene.sheets.clone();next.active_sheet_index=self.scene.active_sheet_index;
            next.tile_layer=self.scene.tile_layer.take();
            crate::document_presentation::preserve(&self.scene,&mut next);
            let python_marker=next.history_python.take();
            if let Some(inverse)=self.undo_stack.last_mut() {inverse.history_python=python_marker;}
            self.scene = next;self.bump_model_revision();
            #[cfg(feature="engineering-python")]
            if python_step {self.apply_engineering_python_history_groups();self.engineering_python.published(self.model_revision);}
            self.camera = self.scene.camera.clone();
            self.status_message = "Redo".to_string();
            return true;
        }
        false
    }

    pub fn delete_selection(&mut self) {
        if self.scene.selection.iter().any(|s|matches!(s,SelectionItem::Guide(_)|SelectionItem::Dimension(_)|SelectionItem::TextLabel(_))){if let Err(error)=self.delete_annotation_selection(){self.status_message=error;}return;}
        if self.scene.selection.is_empty() {
            return;
        }
        self.save_undo_state();


        let mut faces_to_remove = Vec::new();
        let mut edges_to_remove = Vec::new();
        let mut groups_to_remove = Vec::new();let mut planes_to_remove=Vec::new();

        for item in &self.scene.selection {
            match item {
                SelectionItem::Face(f_id) => faces_to_remove.push(*f_id),
                SelectionItem::Edge(e_id) => edges_to_remove.push(*e_id),
                SelectionItem::Group(g_idx) => groups_to_remove.push(*g_idx),
                SelectionItem::SectionPlane(id)=>planes_to_remove.push(*id),
                _ => {}
            }
        }

        faces_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for f in faces_to_remove {
            if f < self.scene.mesh.faces.len() {
                self.scene.mesh.faces.remove(f);
            }
        }

        edges_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for e in edges_to_remove {
            if e < self.scene.mesh.edges.len() {
                self.scene.mesh.edges.remove(e);
            }
        }

        planes_to_remove.sort_unstable_by(|a,b|b.cmp(a));for id in planes_to_remove{if id<self.scene.section_planes.len(){self.scene.section_planes.remove(id);}}
        groups_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for g in groups_to_remove {
            if g < self.scene.groups.len() {
                self.scene.groups.remove(g);
            }
        }

        self.scene.clear_selection();
        if let Some(before)=self.undo_stack.last_mut(){before.clear_selection();}
        self.seal_snapshot_visibility();
        self.scene.mesh.rebuild_spatial_index();
        self.scene.mesh.rebuild_incidence();
        self.scene.bump_version();
    }

    pub fn select_all(&mut self) {
        for f_id in 0..self.scene.mesh.faces.len() {
            self.scene.select(SelectionItem::Face(f_id));
        }
        for e_id in 0..self.scene.mesh.edges.len() {
            self.scene.select(SelectionItem::Edge(e_id));
        }
        for g_idx in 0..self.scene.groups.len() {
            self.scene.select(SelectionItem::Group(g_idx));
        }
    }

    pub fn cut_selection(&mut self) {
        self.copy_selection();
        self.delete_selection();
        self.status_message = ingetrazo_core::i18n::tr("Cut selection");
    }

    pub fn copy_selection(&mut self) {
        self.clipboard_mesh_share_imports.clear();
        let mesh=match ingetrazo_core::containers::copy_selected_mesh(&self.scene).and_then(|mesh|ingetrazo_core::containers::transform_mesh(&mesh,self.scene.dimension_frame)){Ok(mesh)=>mesh,Err(e)=>{self.status_message=e;return;}};
        let groups=self.scene.groups.iter().enumerate().filter(|(i,_)|self.scene.selection.contains(&SelectionItem::Group(*i))).map(|(_,g)|{let mut g=g.clone();g.transform=self.scene.dimension_frame*g.transform;g}).collect::<Vec<_>>();
        let mut reference=mesh.vertices.iter().map(|v|v.position).reduce(DVec3::min);
        fn placement_min(group:&ingetrazo_core::groups::Group,parent:glam::DMat4,minimum:&mut Option<DVec3>) {
            let transform=parent*group.transform;
            for vertex in &group.mesh.vertices {let point=transform.transform_point3(vertex.position);*minimum=Some(minimum.map(|p|p.min(point)).unwrap_or(point));}
            for child in &group.children{placement_min(child,transform,minimum);}
        }
        for group in &groups{placement_min(group,glam::DMat4::IDENTITY,&mut reference);}
        let reference=reference.unwrap_or(DVec3::ZERO);
        let count=mesh.faces.len()+mesh.edges.len()+groups.len();
        if count==0{self.status_message="Nothing selected to copy".into();return;}
        let mut resources=Scene::new();resources.materials=self.scene.materials.clone();resources.layers=self.scene.layers.clone();resources.embedded_textures=self.scene.embedded_textures.clone();resources.component_definitions=self.scene.component_definitions.clone();resources.mesh_shares=self.scene.mesh_shares.clone();
        self.clipboard=Some(GeometryClipboard{mesh,reference,groups,resources,source_document:self.clipboard_document});self.write_system_clipboard();self.status_message=format!("Copied selection ({count} items)");
    }
    pub(crate) fn paste_internal(&mut self,offset:DVec3)->bool {
        let Some(mut cb)=self.clipboard.clone() else{return false;};
        if let Err(e)=crate::clipboard::validate_payload(&cb){self.status_message=e;return false;}
        if cb.mesh.faces.is_empty()&&cb.mesh.edges.is_empty()&&cb.groups.is_empty(){return false;}
        crate::clipboard::remap_resources(&mut cb,&self.scene,self.clipboard_document,&mut self.clipboard_mesh_share_imports);
        let inverse=self.scene.dimension_frame.inverse();
        let source=match ingetrazo_core::containers::transform_mesh(&cb.mesh,inverse){Ok(mesh)=>mesh,Err(e)=>{self.status_message=e;return false;}};
        let (mesh,faces,edges)=match ingetrazo_core::containers::paste_mesh(&self.scene.mesh,&source,offset){Ok(result)=>result,Err(e)=>{self.status_message=e;return false;}};
        let face_geometry:Vec<_>=faces.iter().map(|&id|{let face=&mesh.faces[id];(face.vertices.iter().map(|&v|mesh.vertices[v].position).collect::<Vec<_>>(),face.holes.iter().map(|ring|ring.iter().map(|&v|mesh.vertices[v].position).collect::<Vec<_>>()).collect::<Vec<_>>())}).collect();
        let edge_geometry:Vec<_>=edges.iter().map(|&id|{let edge=&mesh.edges[id];(mesh.vertices[edge.v0].position,mesh.vertices[edge.v1].position)}).collect();
        let mesh=match crate::clipboard::merge_pasted_planes(&self.scene.mesh,&mesh,&faces){Ok(mesh)=>mesh,Err(error)=>{self.status_message=error;return false;}};
        let mut candidate=self.scene.clone();
        if let Err(e)=crate::clipboard::merge_resources(&mut candidate,&cb.resources){self.status_message=e;return false;}
        candidate.mesh=mesh;candidate.clear_selection();
        for id in 0..candidate.mesh.faces.len(){let face=&candidate.mesh.faces[id];let outer=face.vertices.iter().map(|&v|candidate.mesh.vertices[v].position).collect::<Vec<_>>();let holes=face.holes.iter().map(|ring|ring.iter().map(|&v|candidate.mesh.vertices[v].position).collect::<Vec<_>>()).collect::<Vec<_>>();if face_geometry.iter().any(|(o,h)|*o==outer&&*h==holes){candidate.select(SelectionItem::Face(id));}}
        for id in 0..candidate.mesh.edges.len(){let edge=&candidate.mesh.edges[id];let a=candidate.mesh.vertices[edge.v0].position;let b=candidate.mesh.vertices[edge.v1].position;if edge_geometry.iter().any(|&(x,y)|(a==x&&b==y)||(a==y&&b==x)){candidate.select(SelectionItem::Edge(id));}}
        for group in cb.groups{let mut group=group.duplicate_instance();group.transform=glam::DMat4::from_translation(offset)*inverse*group.transform;let index=candidate.groups.len();candidate.groups.push(group);candidate.select(SelectionItem::Group(index));}

        self.save_undo_state();candidate.bump_version();self.scene=candidate;true
    }
    pub fn paste_selection(&mut self) {self.begin_paste_preview();}
    pub fn paste_in_place(&mut self) {self.paste_preview=None;self.read_system_clipboard();if self.paste_internal(DVec3::ZERO){self.status_message="Pasted in place".into();}}

    pub fn repeat_last_command(&mut self) {
        if let Some((tool,mode))=self.last_shape_tool{if let Some(mode)=mode{self.activate_arc(mode);}else{self.set_tool(tool);}}
        self.status_message = ingetrazo_core::i18n::tr("Repeated last command");
    }

    pub fn invert_selection(&mut self) {
        let mut new_selection = Vec::new();
        for f_id in 0..self.scene.mesh.faces.len() {
            if !self.scene.is_selected(&SelectionItem::Face(f_id)) {
                new_selection.push(SelectionItem::Face(f_id));
            }
        }
        for e_id in 0..self.scene.mesh.edges.len() {
            if !self.scene.is_selected(&SelectionItem::Edge(e_id)) {
                new_selection.push(SelectionItem::Edge(e_id));
            }
        }
        for g_idx in 0..self.scene.groups.len() {
            if !self.scene.is_selected(&SelectionItem::Group(g_idx)) {
                new_selection.push(SelectionItem::Group(g_idx));
            }
        }
        self.scene.clear_selection();
        for item in new_selection {
            self.scene.select(item);
        }
        self.status_message = ingetrazo_core::i18n::tr("Inverted selection");
    }

    pub fn make_group(&mut self) {
        let before=self.scene.clone();
        let name=format!("{} {}",ingetrazo_core::i18n::tr("Group"),self.scene.groups.len()+1);
        match self.scene.group_selection(name) {
            Ok(_)=>{self.seal_view_history_before(&before);self.seal_terrain_history_before(&before);self.seal_layer_history_before(&before);self.seal_image_history_before(&before);self.undo_stack.push(before);self.redo_stack.clear();self.status_message=ingetrazo_core::i18n::tr("Created group");},
            Err(err)=>self.status_message=err,
        }
    }

    pub fn explode_group(&mut self) {
        let before=self.scene.clone();
        match self.scene.explode_selected_groups() {
            Ok(())=>{self.seal_view_history_before(&before);self.seal_terrain_history_before(&before);self.seal_layer_history_before(&before);self.seal_image_history_before(&before);self.undo_stack.push(before);self.redo_stack.clear();self.status_message=ingetrazo_core::i18n::tr("Exploded group");},
            Err(err)=>self.status_message=err,
        }
    }

    pub fn activate_named_material(&mut self,mat_name:&str){self.active_material=mat_name.into();self.set_tool(ToolType::Paint);self.status_message=format!("Active material: {mat_name}");}
    pub fn apply_material_to_selection(&mut self,mat_name:&str){
        let result=(||->Result<usize,String>{
            let material=self.scene.materials.get(mat_name).ok_or("Select an in-model material")?.clone();let stamp=material.face_recipe();
            let mut candidate=self.scene.clone();let mut count=0;
            for selected in &self.scene.selection{match selected{
                SelectionItem::Face(i)=>{let f=candidate.mesh.faces.get_mut(*i).ok_or("Material face no longer resolves")?;for key in ["mat","color","texture","opacity"]{if let Some(v)=stamp.get(key){f.attrs.insert(key.into(),v.clone());}else{f.attrs.remove(key);}}count+=1;},
                SelectionItem::Group(i)=>{let group=candidate.groups.get_mut(*i).ok_or("Material group no longer resolves")?;if !group.native_document.is_object(){if group.native_document.is_null(){group.native_document=serde_json::json!({});}else{return Err("Material group metadata is not an object".into());}}group.native_document["material"]=stamp.clone();count+=1;},
                _=>return Err("Select faces or groups in the active context to paint".into()),
            }}
            if count==0||candidate==self.scene{return Ok(0);}
            self.undo_stack.try_reserve(1).map_err(|_|"Material history allocation refused")?;self.save_undo_state();candidate.bump_version();self.scene=candidate;self.bump_render_paint_revision();Ok(count)
        })();match result{Ok(count)=>{self.active_material=mat_name.into();self.status_message=format!("Applied {mat_name} to {count} targets");},Err(error)=>self.status_message=error}
    }

    pub fn purge_unused_layers(&mut self) {
        let mut used=self.scene.used_layer_names();for session in &self.component_edit_stack{used.extend(session.original_scene.used_layer_names());}
        let purged=self.scene.layers.iter().filter(|l|l.name!=ingetrazo_core::layers::DEFAULT_LAYER&&!used.contains(&l.name)).count();
        if purged>0{self.save_undo_state();self.scene.layers.retain(|l|l.name==ingetrazo_core::layers::DEFAULT_LAYER||used.contains(&l.name));self.scene.bump_version();}
        if self.active_layer_idx>=self.scene.layers.len(){self.active_layer_idx=0;}self.status_message=format!("Purged {purged} unused layer(s)");
    }

    pub fn assign_selection_to_active_layer(&mut self) {
        if let Some(layer)=self.scene.layers.get(self.active_layer_idx){let name=layer.name.clone();let count=self.assign_selection_to_named_layer(&name);self.status_message=format!("Assigned {count} entity/entities to layer '{name}'");}
    }

    pub fn flip_along(&mut self, axis: char) {
        self.save_undo_state();
        let mut pts = Vec::new();
        for item in &self.scene.selection {
            if let SelectionItem::Face(f_id) = item {
                if *f_id < self.scene.mesh.faces.len() {
                    for &vid in &self.scene.mesh.faces[*f_id].vertices {
                        pts.push(self.scene.mesh.vertices[vid].position);
                    }
                }
            }
        }
        if pts.is_empty() {
            self.status_message = ingetrazo_core::i18n::tr("Select geometry to flip");
            return;
        }
        let center: DVec3 = pts.iter().sum::<DVec3>() / (pts.len() as f64);
        let mut touched_verts = std::collections::HashSet::new();
        for item in &self.scene.selection {
            if let SelectionItem::Face(f_id) = item {
                if *f_id < self.scene.mesh.faces.len() {
                    for &vid in &self.scene.mesh.faces[*f_id].vertices {
                        touched_verts.insert(vid);
                    }
                    self.scene.mesh.faces[*f_id].vertices.reverse();
                }
            }
        }
        for vid in touched_verts {
            let p = &mut self.scene.mesh.vertices[vid].position;
            match axis {
                'x' | 'r' => p.x = 2.0 * center.x - p.x,
                'y' | 'g' => p.y = 2.0 * center.y - p.y,
                'z' | 'b' => p.z = 2.0 * center.z - p.z,
                _ => {}
            }
        }
        self.scene.mesh.rebuild_spatial_index();
        self.scene.bump_version();
        self.status_message = format!("Flipped selection along axis {}", axis.to_uppercase());
    }

    pub fn run_script(&mut self, code: &str) {
        let code = code.trim();
        if code.is_empty() {
            return;
        }
        self.console_logs.push(format!(">>> {code}"));
        #[cfg(feature="engineering-python")]
        if self.engineering_python_enabled{self.run_engineering_python(code);return;}
        if code == "clear" || code == "cls" {
            self.console_logs.clear();
            return;
        }
        if matches!(code, "add_cube" | "add_cube()" | "cube" | "cube()") {
            self.save_undo_state();
            let s = 1.0;
            let v0 = self.scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
            let v1 = self.scene.mesh.add_vertex(DVec3::new(s, 0.0, 0.0));
            let v2 = self.scene.mesh.add_vertex(DVec3::new(s, s, 0.0));
            let v3 = self.scene.mesh.add_vertex(DVec3::new(0.0, s, 0.0));
            let v4 = self.scene.mesh.add_vertex(DVec3::new(0.0, 0.0, s));
            let v5 = self.scene.mesh.add_vertex(DVec3::new(s, 0.0, s));
            let v6 = self.scene.mesh.add_vertex(DVec3::new(s, s, s));
            let v7 = self.scene.mesh.add_vertex(DVec3::new(0.0, s, s));
            self.scene.mesh.add_face(vec![v0, v3, v2, v1], vec![]);
            self.scene.mesh.add_face(vec![v4, v5, v6, v7], vec![]);
            self.scene.mesh.add_face(vec![v0, v1, v5, v4], vec![]);
            self.scene.mesh.add_face(vec![v1, v2, v6, v5], vec![]);
            self.scene.mesh.add_face(vec![v2, v3, v7, v6], vec![]);
            self.scene.mesh.add_face(vec![v3, v0, v4, v7], vec![]);
            self.scene.mesh.rebuild_spatial_index();
            self.scene.bump_version();
            self.console_logs.push("Added cube (1m x 1m x 1m) to model.".to_string());
        } else if code == "select_all" {
            self.select_all();
            self.console_logs.push(format!("Selected {} entities.", self.scene.selection.len()));
        } else if code == "stats" {
            self.console_logs.push(format!(
                "Mesh: {} vertices, {} edges, {} faces, {} groups, {} layers.",
                self.scene.mesh.vertices.len(),
                self.scene.mesh.edges.len(),
                self.scene.mesh.faces.len(),
                self.scene.groups.len(),
                self.scene.layers.len()
            ));
        } else {
            let error = format!("Unsupported console command: {code}");
            self.status_message = error.clone();
            self.console_logs.push(error);
        }
    }

    pub fn reverse_faces(&mut self) {
        self.save_undo_state();
        let mut count = 0;
        for item in &self.scene.selection {
            if let SelectionItem::Face(f_id) = item {
                if *f_id < self.scene.mesh.faces.len() {
                    self.scene.mesh.faces[*f_id].vertices.reverse();
                    count += 1;
                }
            }
        }
        if count > 0 {
            self.scene.bump_version();
            self.status_message = ingetrazo_core::i18n::tr("Reversed faces");
        } else {
            self.status_message = ingetrazo_core::i18n::tr("Select faces to reverse");
        }
    }

    pub fn delete_guides(&mut self) {
        self.save_undo_state();
        self.scene.guides.clear();
        self.scene.bump_version();
        self.status_message = ingetrazo_core::i18n::tr("Deleted all guides");
    }

    pub fn perform_solid_boolean(&mut self, op: ingetrazo_core::solids::SolidOp) {
        let indices = self.scene.selected_groups_in_order();
        if indices.len() < 2 || indices.iter().any(|&i| i >= self.scene.groups.len()) {
            self.status_message = "Select at least two solid groups".into();
            return;
        }
        let groups: Vec<_> = indices.iter().map(|&i| self.scene.groups[i].clone()).collect();
        match ingetrazo_core::solids::run_group_operation(op, &groups) {
            Ok(edit) => {
                // Preparation is fallible; snapshot only a successful edit. Undo/redo
                // restores inputs, their attributes/transforms and prior selection.
                self.save_undo_state();
                let mut removed: Vec<_> = edit.removed.iter().map(|&i| indices[i]).collect();
                removed.sort_unstable();
                let at = removed[0];
                for &index in removed.iter().rev() { self.scene.groups.remove(index); }
                self.scene.clear_selection();
                for (offset, group) in edit.created.into_iter().enumerate() {
                    self.scene.groups.insert(at + offset, group);
                    self.scene.select(SelectionItem::Group(at + offset));
                }
                self.scene.bump_version();
                self.status_message = format!("Solid {} completed", op.name());
            }
            Err(error) => { self.status_message = format!("Solid error: {}", error); }
        }
    }

    pub fn zoom_selection(&mut self) {
        let mut min = DVec3::splat(f64::INFINITY);
        let mut max = DVec3::splat(f64::NEG_INFINITY);

        for item in &self.scene.selection {
            match item {
                SelectionItem::Face(f_id) => {
                    if *f_id < self.scene.mesh.faces.len() {
                        for vid in &self.scene.mesh.faces[*f_id].vertices {
                            if *vid < self.scene.mesh.vertices.len() {
                                let p = self.scene.mesh.vertices[*vid].position;
                                min = min.min(p);
                                max = max.max(p);
                            }
                        }
                    }
                }
                SelectionItem::Edge(e_id) => {
                    if *e_id < self.scene.mesh.edges.len() {
                        let e = &self.scene.mesh.edges[*e_id];
                        if e.v0 < self.scene.mesh.vertices.len() && e.v1 < self.scene.mesh.vertices.len() {
                            min = min.min(self.scene.mesh.vertices[e.v0].position);
                            max = max.max(self.scene.mesh.vertices[e.v0].position);
                            min = min.min(self.scene.mesh.vertices[e.v1].position);
                            max = max.max(self.scene.mesh.vertices[e.v1].position);
                        }
                    }
                }
                SelectionItem::Group(g_idx) => {
                    if *g_idx < self.scene.groups.len() {
                        let g = &self.scene.groups[*g_idx];
                        for v in &g.mesh.vertices {
                            let wp = g.transform.transform_point3(v.position);
                            min = min.min(wp);
                            max = max.max(wp);
                        }
                    }
                }
                _ => {}
            }
        }

        if min.x.is_finite() && max.x.is_finite() {
            self.camera.fit_box(min, max, 1.35);
        } else {
            self.zoom_extents();
        }
    }

    pub fn clean_screen_toggle(&mut self) {
        if self.show_toolbar || self.show_tray {
            self.show_toolbar = false;
            self.show_tray = false;
        } else {
            self.show_toolbar = true;
            self.show_tray = true;
        }
    }

    pub fn zoom_extents(&mut self) { self.source_zoom_extents(); }
}

impl IngeTrazoApp {
    /// Backend-independent application UI and document lifecycle.
    pub fn update_ui(&mut self, ctx: &egui::Context) {
        crate::appearance_preferences::apply(&self.document_session.preferences,ctx);
        let image_owns_input=self.sheet_image.dialog.is_some();
        crate::sheet_image::dialog(self,ctx);
        let recent_warning_owns_input = self.document_session.recent_missing.is_some();
        let preferences_owns_input = self.show_preferences_dialog;
        let import_owns_input=self.document_session.unit_import.is_some()||self.document_session.survey_import.is_some();
        self.sync_document_presentation();
        self.begin_document_frame(ctx);
        self.poll_ai_bridge();
        if self.navigation_suspended(){self.navigation.cancel();self.navigation.sync_cursor(ctx);}
        if self.section_name_prompt.is_some(){
            egui::CentralPanel::default().show(ctx,|ui|{let rect=ui.max_rect();let data=ingetrazo_render::pipeline::build_scene_render_data(&self.scene,&self.camera,&self.style,None);let textures=self.textures.iter().map(|(name,texture)|(name.clone(),texture.id())).collect();ingetrazo_render::pipeline::render_scene_to_egui(&data,&self.camera,ui.painter(),rect,&self.style,&textures);});
            crate::section_actions::render_section_name_prompt(self,ctx);return;
        }
        let title = if let Some(path) = &self.current_file {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            format!("IngeTrazo — {filename}")
        } else {
            "IngeTrazo 0.3.15".to_string()
        };
        // Sending a viewport command itself requests another frame. Emit only
        // changed titles, with ownership scoped to this context and viewport.
        let title_key = egui::Id::new(("ingetrazo-window-title", ctx.viewport_id()));
        let changed = ctx.data_mut(|data| {
            if data.get_temp::<String>(title_key).as_ref() == Some(&title) {
                false
            } else {
                data.insert_temp(title_key, title.clone());
                true
            }
        });
        if changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        self.sync_blender_document();
        self.blender_state.poll_events(ctx);
        self.tick_blender_follow(ctx);

        if !image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog {
        crate::vcb_input::route(self,ctx);
        // A complete Ctrl chord can arrive in one frame; the final modifier
        // snapshot is then released. Use the key event's captured modifiers.
        let (undo_pressed,redo_pressed)=ctx.input(|i|i.events.iter().fold((false,false),|mut flags,event|{
            if let egui::Event::Key{key,pressed:true,modifiers,..}=event {
                if modifiers.ctrl || modifiers.command {
                    flags.0|=*key==egui::Key::Z&&!modifiers.shift;flags.1|=*key==egui::Key::Y||(*key==egui::Key::Z&&modifiers.shift);
                }
            }
            flags
        }));
        // Original InlineTextEditor owns its focused text Undo/Redo.
        let inline_text_owns_keys=self.composer_state.inline_text.is_some()&&ctx.wants_keyboard_input();
        if undo_pressed&&!inline_text_owns_keys {self.undo();}
        if redo_pressed&&!inline_text_owns_keys {self.redo();}

        if self.navigation.mode!=Some(crate::navigation::Mode::FirstPerson){self.navigation.sync_cursor(ctx);}
        // Global Keyboard Shortcuts
        if ctx.input(|i| i.modifiers.command || i.modifiers.ctrl) {
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                self.new_document();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::O)) {
                self.begin_open_document();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                if ctx.input(|i| i.modifiers.shift) { self.save_document_as(); }
                else { self.save_document(); }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::A)) {
                self.select_all();
            }
        }
        if !ctx.wants_keyboard_input(){
            let events=ctx.input(|i|i.events.clone());for event in events{match event{egui::Event::Copy=>self.copy_selection(),egui::Event::Cut=>self.cut_selection(),egui::Event::Paste(_)=>self.paste_selection(),_=>{}}}
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            if !self.delete_picked_blender_light() { self.delete_selection_or_annotation_hover(); }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F3)) {
            self.show_command_search = !self.show_command_search;
        }

        // Tool quick switch keys (when not typing in a text field)
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::Space)) {
                self.set_tool(ToolType::Select);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::L)) {
                self.set_tool(ToolType::Line);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::R)) {
                self.set_tool(ToolType::Rectangle);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::P)) {
                self.set_tool(ToolType::PushPull);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::M)) {
                self.set_tool(ToolType::Move);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::Q)&&!self.navigation_claims_key(egui::Key::Q,i.modifiers)) {
                self.set_tool(ToolType::Rotate);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::S)&&!self.navigation_claims_key(egui::Key::S,i.modifiers)) {
                self.set_tool(ToolType::Scale);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::T)) {
                self.set_tool(ToolType::Tape);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::E)&&!self.navigation_claims_key(egui::Key::E,i.modifiers)) {
                self.set_tool(ToolType::Eraser);
            } else if ctx.input(|i| crate::vcb_input::plain_key(i,egui::Key::B)) {
                self.set_tool(ToolType::Paint);
            }
        }

        } // 3D Text dialog owns keyboard input
        self.show_simplification_dialog(ctx);

        // 1. Menu Bar Panel
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui.set_enabled(!image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog);
            render_menu_bar(self, ui);
        });

        // 2. Toolbar Panel
        if self.show_toolbar {
            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                ui.set_enabled(!image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog);
            render_top_toolbar(self, ui);
            });
        }

        // 3. Status Bar Panel
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.set_enabled(!image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog);
            render_status_bar(self, ui);
        });

        // 4. Left CAD Tool Palette
        egui::SidePanel::left("cad_palette")
            .exact_width(crate::appearance_preferences::current_toolbar_size(ctx)+20.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_enabled(!image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog);
            render_left_cad_palette(self, ui);
            });

        // 5. Side Tray Panel
        if self.show_tray {
            egui::SidePanel::right("side_tray")
                .default_width(260.0)
                .min_width(200.0)
                .max_width(400.0)
                .show(ctx, |ui| {
                    ui.set_enabled(!image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog);
            render_side_tray(self, ui);
                });
        }

        // 6. Central 3D Viewport or 2D Sheet Composer Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(!image_owns_input&&self.sheet_image.dialog.is_none()&&!import_owns_input&&self.document_session.unit_import.is_none()&&self.document_session.survey_import.is_none()&&!preferences_owns_input&&!self.show_preferences_dialog&&!recent_warning_owns_input&&self.document_session.recent_missing.is_none()&&self.annotation_editor.is_none()&&self.text3d_dialog.is_none()&&!self.show_material_edit_dialog);
            if let Some(sheet_idx) = self.scene.active_sheet_index {
                crate::composer::render_sheet_composer(self, ui, sheet_idx);
            } else {
                self.finish_composer_edit();
                render_viewport(self, ui);
            }
        });

        // 7. Modal Dialogs
        render_dialogs(self, ctx);
        crate::text3d::render_dialog(self, ctx);
        crate::annotation_editor::render(self,ctx);
        crate::section_actions::render_section_name_prompt(self,ctx);
        crate::blender::render_blender_dialog(self, ctx);
        self.sync_document_presentation();
    }
}

impl eframe::App for IngeTrazoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) { self.update_ui(ctx); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_example_pergola() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let p1 = manifest_dir.join("../../ingetrazo/examples/banca-pergola-yanque.igz");
        let p2 = manifest_dir.join("../ingetrazo/examples/banca-pergola-yanque.igz");
        let p3 = PathBuf::from("/home/raymondj/Projects/ingentrazo-RUST-Conversion/ingetrazo/examples/banca-pergola-yanque.igz");
        let target_path = if p1.exists() { p1 } else if p2.exists() { p2 } else { p3 };
        println!("Target path: {:?}, exists: {}", target_path, target_path.exists());
        if target_path.exists() {
            let app = IngeTrazoApp::with_file(&target_path);
            println!("=== PERGOLA LOAD TEST ===");
            println!("Camera target: {:?}", app.camera.target);
            println!("Camera distance: {}", app.camera.distance);
            println!("Camera yaw: {}, pitch: {}", app.camera.yaw, app.camera.pitch);
            println!("Dimensions count: {}", app.scene.dimensions.len());
            println!("Saved views count: {}", app.scene.saved_views.len());
            println!("Groups count: {}", app.scene.groups.len());

            let mut cam = app.camera.clone();
            let vp_rect = egui::Rect::from_min_max(egui::pos2(42.0, 68.0), egui::pos2(1030.0, 1180.0));
            cam.aspect = (vp_rect.width() / vp_rect.height()) as f64;
            let vp = cam.projection_matrix() * cam.view_matrix();

            let mut out = String::new();
            out.push_str(&format!("Viewport rect: {:?}\nCamera: eye={:?}, target={:?}, dist={}\n", vp_rect, cam.eye(), cam.target, cam.distance));
            for (gi, g) in app.scene.groups.iter().enumerate() {
                out.push_str(&format!("=== Group {} (faces: {}, edges: {}, transform: {:?}) ===\n", gi, g.mesh.faces.len(), g.mesh.edges.len(), g.transform));
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for (fi, _face) in g.mesh.faces.iter().enumerate() {
                    let tris = g.mesh.triangulate_face(fi);
                    for (p0, p1, p2) in tris {
                        let wp0 = g.transform.transform_point3(p0);
                        let wp1 = g.transform.transform_point3(p1);
                        let wp2 = g.transform.transform_point3(p2);
                        if let (Some((s0, _)), Some((s1, _)), Some((s2, _))) = (
                            ingetrazo_render::pipeline::project_point(wp0, &vp, vp_rect),
                            ingetrazo_render::pipeline::project_point(wp1, &vp, vp_rect),
                            ingetrazo_render::pipeline::project_point(wp2, &vp, vp_rect),
                        ) {
                            min_x = min_x.min(s0.x).min(s1.x).min(s2.x);
                            max_x = max_x.max(s0.x).max(s1.x).max(s2.x);
                            min_y = min_y.min(s0.y).min(s1.y).min(s2.y);
                            max_y = max_y.max(s0.y).max(s1.y).max(s2.y);
                        }
                    }
                }
                out.push_str(&format!("  Projected bounds: x=[{:.1}, {:.1}], y=[{:.1}, {:.1}]\n", min_x, max_x, min_y, max_y));
            }
            std::fs::write("/tmp/pillar_found.txt", out).unwrap();
            assert_eq!(app.scene.groups.len(), 9);
            println!("=== GROUP 5 TRIANGLES ===");
            let g5 = &app.scene.groups[5];
            let vp_real = egui::Rect::from_min_max(egui::pos2(42.0, 68.0), egui::pos2(1048.0, 828.0));
            cam.aspect = (vp_real.width() / vp_real.height()) as f64;
            let vp_mat = cam.projection_matrix() * cam.view_matrix();
            for (fi, _) in g5.mesh.faces.iter().enumerate() {
                let tris = g5.mesh.triangulate_face(fi);
                for (p0, p1, p2) in tris {
                    let s0 = ingetrazo_render::pipeline::project_point(p0, &vp_mat, vp_real);
                    let s1 = ingetrazo_render::pipeline::project_point(p1, &vp_mat, vp_real);
                    let s2 = ingetrazo_render::pipeline::project_point(p2, &vp_mat, vp_real);
                    println!("fi={fi}: p0={p0:.2?}, p1={p1:.2?}, p2={p2:.2?} => s0={:?}, s1={:?}, s2={:?}",
                        s0.map(|(p, d)| (p.x, p.y, d)),
                        s1.map(|(p, d)| (p.x, p.y, d)),
                        s2.map(|(p, d)| (p.x, p.y, d)));
                }
            }
            println!("=========================");
        }
    }

    #[test]
    fn test_group_3_colors() {
        use ingetrazo_formats::load_igz;
        use ingetrazo_render::pipeline::build_scene_render_data;
        use ingetrazo_render::style::StyleConfig;
        use ingetrazo_core::camera::OrbitCamera;
        use std::path::Path;
        let path = Path::new("../../ingetrazo/examples/banca-pergola-yanque.igz");
        let scene = load_igz(path).unwrap();
        let style = StyleConfig::default_shaded();
        let cam = OrbitCamera::new();
        let data = build_scene_render_data(&scene, &cam, &style, None);
        println!("opaque mesh vertices: {}", data.opaque_mesh.vertices.len());
        println!("textured_meshes keys:");
        for (k, v) in &data.textured_meshes {
            println!("  tex key: {}, vertices: {}, indices: {}", k, v.vertices.len(), v.indices.len());
            if k.contains("beech") {
                let mut min_z = f32::INFINITY;
                let mut max_z = f32::NEG_INFINITY;
                for v in &v.vertices {
                    min_z = min_z.min(v.position[2]);
                    max_z = max_z.max(v.position[2]);
                }
                println!("    beech z range: [{}, {}]", min_z, max_z);
                let mut low_z_colors = Vec::new();
                let mut high_z_colors = Vec::new();
                for v in &v.vertices {
                    if v.position[2] < 1.0 && low_z_colors.len() < 3 {
                        low_z_colors.push((v.position, v.normal, v.color, v.uv));
                    }
                    if v.position[2] > 2.0 && high_z_colors.len() < 3 {
                        high_z_colors.push((v.position, v.normal, v.color, v.uv));
                    }
                }
                println!("  Bench (z < 1.0) sample verts: {:?}", low_z_colors);
                println!("  Roof (z > 2.0) sample verts: {:?}", high_z_colors);
            }
        }
        println!("Camera target: {:?}, dist: {}, yaw: {}, pitch: {}", scene.camera.target, scene.camera.distance, scene.camera.yaw, scene.camera.pitch);
        let view_proj = scene.camera.projection_matrix() * scene.camera.view_matrix();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0));
        
        let mut beech_roof_items = Vec::new();
        let mut metal_roof_items = Vec::new();

        if let Some(buf) = data.textured_meshes.get("textures/3c7fd1a55c44e5a4-beech.jpg") {
            for chunk in buf.indices.chunks_exact(3) {
                let v0 = &buf.vertices[chunk[0] as usize];
                let v1 = &buf.vertices[chunk[1] as usize];
                let v2 = &buf.vertices[chunk[2] as usize];
                if v0.position[2] > 2.0 {
                    let p0 = glam::DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
                    let p1 = glam::DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
                    let p2 = glam::DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);
                    if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                        ingetrazo_render::pipeline::project_point(p0, &view_proj, rect),
                        ingetrazo_render::pipeline::project_point(p1, &view_proj, rect),
                        ingetrazo_render::pipeline::project_point(p2, &view_proj, rect),
                    ) {
                        let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                        let depth = (d0 + d1 + d2) / 3.0;
                        beech_roof_items.push((depth, cross, [s0, s1, s2]));
                    }
                }
            }
        }

        if let Some(buf) = data.textured_meshes.get("textures/ae1ad9f06706c193-metal_dark.png") {
            for (_ti, chunk) in buf.indices.chunks_exact(3).enumerate() {
                let v0 = &buf.vertices[chunk[0] as usize];
                let v1 = &buf.vertices[chunk[1] as usize];
                let v2 = &buf.vertices[chunk[2] as usize];
                if v0.position[2] > 2.0 || v1.position[2] > 2.0 || v2.position[2] > 2.0 {
                    let p0 = glam::DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
                    let p1 = glam::DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
                    let p2 = glam::DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);
                    if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                        ingetrazo_render::pipeline::project_point(p0, &view_proj, rect),
                        ingetrazo_render::pipeline::project_point(p1, &view_proj, rect),
                        ingetrazo_render::pipeline::project_point(p2, &view_proj, rect),
                    ) {
                        let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                        let depth = (d0 + d1 + d2) / 3.0;
                        metal_roof_items.push((depth, cross, [s0, s1, s2]));
                        if metal_roof_items.len() <= 5 {
                            println!("Metal tri matched: p0={:?}, p1={:?}, p2={:?}", p0, p1, p2);
                        }
                    }
                }
            }
        }

        let mut min_sx = f32::INFINITY;
        let mut max_sx = f32::NEG_INFINITY;
        let mut min_sy = f32::INFINITY;
        let mut max_sy = f32::NEG_INFINITY;
        for (_, _, pts) in &beech_roof_items {
            for p in pts {
                min_sx = min_sx.min(p.x);
                max_sx = max_sx.max(p.x);
                min_sy = min_sy.min(p.y);
                max_sy = max_sy.max(p.y);
            }
        }
        println!("Beech roof SCREEN BBOX: x=[{:.1}, {:.1}], y=[{:.1}, {:.1}]", min_sx, max_sx, min_sy, max_sy);

        // Check visible edges for z > 2.0
        let mut _roof_edges = 0;
        for seg in &data.visible_edges.segments {
            if seg.v0.position[2] > 2.0 || seg.v1.position[2] > 2.0 {
                _roof_edges += 1;
            }
        }
        // Let's do a point-in-triangle hit test for all items in render_items at (520.0, 350.0)
        // Function to check if point p is inside triangle [p0, p1, p2]
        fn point_in_tri(p: egui::Pos2, p0: egui::Pos2, p1: egui::Pos2, p2: egui::Pos2) -> bool {
            let d1 = (p.x - p1.x) * (p0.y - p1.y) - (p0.x - p1.x) * (p.y - p1.y);
            let d2 = (p.x - p2.x) * (p1.y - p2.y) - (p1.x - p2.x) * (p.y - p2.y);
            let d3 = (p.x - p0.x) * (p2.y - p0.y) - (p2.x - p0.x) * (p.y - p0.y);
            let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
            let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
            !(has_neg && has_pos)
        }

        let mut min_mx = f32::INFINITY;
        let mut max_mx = f32::NEG_INFINITY;
        let mut min_my = f32::INFINITY;
        let mut max_my = f32::NEG_INFINITY;
        for (_, _, pts) in &metal_roof_items {
            for p in pts {
                min_mx = min_mx.min(p.x);
                max_mx = max_mx.max(p.x);
                min_my = min_my.min(p.y);
                max_my = max_my.max(p.y);
            }
        }
        println!("Metal roof SCREEN BBOX: x=[{:.1}, {:.1}], y=[{:.1}, {:.1}]", min_mx, max_mx, min_my, max_my);

        // Now let's simulate the exact render_items from render_scene_to_egui!
        let mut full_render_items: Vec<(f64, String, egui::Color32, [egui::Pos2; 3])> = Vec::new();
        
        // Beech textured triangles
        if let Some(buf) = data.textured_meshes.get("textures/3c7fd1a55c44e5a4-beech.jpg") {
            for chunk in buf.indices.chunks_exact(3) {
                let v0 = &buf.vertices[chunk[0] as usize];
                let v1 = &buf.vertices[chunk[1] as usize];
                let v2 = &buf.vertices[chunk[2] as usize];
                let p0 = glam::DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
                let p1 = glam::DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
                let p2 = glam::DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);
                if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                    ingetrazo_render::pipeline::project_point(p0, &view_proj, rect),
                    ingetrazo_render::pipeline::project_point(p1, &view_proj, rect),
                    ingetrazo_render::pipeline::project_point(p2, &view_proj, rect),
                ) {
                    let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                    if cross.abs() >= 0.25 {
                        let depth = (d0 + d1 + d2) / 3.0;
                        full_render_items.push((depth, "Beech".to_string(), egui::Color32::WHITE, [s0, s1, s2]));
                    }
                }
            }
        }

        // Metal dark textured triangles
        if let Some(buf) = data.textured_meshes.get("textures/ae1ad9f06706c193-metal_dark.png") {
            for chunk in buf.indices.chunks_exact(3) {
                let v0 = &buf.vertices[chunk[0] as usize];
                let v1 = &buf.vertices[chunk[1] as usize];
                let v2 = &buf.vertices[chunk[2] as usize];
                let p0 = glam::DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
                let p1 = glam::DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
                let p2 = glam::DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);
                if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                    ingetrazo_render::pipeline::project_point(p0, &view_proj, rect),
                    ingetrazo_render::pipeline::project_point(p1, &view_proj, rect),
                    ingetrazo_render::pipeline::project_point(p2, &view_proj, rect),
                ) {
                    let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                    if cross.abs() >= 0.25 {
                        let depth = (d0 + d1 + d2) / 3.0;
                        full_render_items.push((depth, "MetalDark".to_string(), egui::Color32::BLACK, [s0, s1, s2]));
                    }
                }
            }
        }

        // Concrete smooth textured triangles
        if let Some(buf) = data.textured_meshes.get("textures/ea4d38dd30369ad3-concrete_smooth.png") {
            for chunk in buf.indices.chunks_exact(3) {
                let v0 = &buf.vertices[chunk[0] as usize];
                let v1 = &buf.vertices[chunk[1] as usize];
                let v2 = &buf.vertices[chunk[2] as usize];
                let p0 = glam::DVec3::new(v0.position[0] as f64, v0.position[1] as f64, v0.position[2] as f64);
                let p1 = glam::DVec3::new(v1.position[0] as f64, v1.position[1] as f64, v1.position[2] as f64);
                let p2 = glam::DVec3::new(v2.position[0] as f64, v2.position[1] as f64, v2.position[2] as f64);
                if let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
                    ingetrazo_render::pipeline::project_point(p0, &view_proj, rect),
                    ingetrazo_render::pipeline::project_point(p1, &view_proj, rect),
                    ingetrazo_render::pipeline::project_point(p2, &view_proj, rect),
                ) {
                    let cross = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
                    if cross.abs() >= 0.25 {
                        let depth = (d0 + d1 + d2) / 3.0;
                        full_render_items.push((depth, "Concrete".to_string(), egui::Color32::GRAY, [s0, s1, s2]));
                    }
                }
            }
        }

        // Sort Painter's algorithm: largest depth (furthest) drawn FIRST, smallest depth (closest) drawn LAST
        full_render_items.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        println!("Total triangles in full_render_items: {}", full_render_items.len());
        // Find which triangle is drawn LAST at (520.0, 280.0)
        let pt = egui::pos2(520.0, 280.0);
        let mut hitting = Vec::new();
        for (i, (d, name, _, pts)) in full_render_items.iter().enumerate() {
            if point_in_tri(pt, pts[0], pts[1], pts[2]) {
                hitting.push((i, d, name));
            }
        }
        println!("At pt {:?}, hitting in order of drawing:", pt);
        for (i, d, name) in hitting {
            println!("  draw order {}: name={}, depth={}", i, name, d);
        }
    }

    #[test]
    fn test_cad_clipboard_and_group_operations() {
        let mut app = IngeTrazoApp::new();
        let v0 = app.scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = app.scene.mesh.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        let v2 = app.scene.mesh.add_vertex(DVec3::new(2.0, 2.0, 0.0));
        let f0 = app.scene.mesh.add_face(vec![v0, v1, v2], vec![]).unwrap();

        // 1. Selection
        app.scene.select(SelectionItem::Face(f0));
        assert!(app.scene.is_selected(&SelectionItem::Face(f0)));

        // 2. Invert selection
        app.invert_selection();
        assert!(!app.scene.is_selected(&SelectionItem::Face(f0)));
        app.invert_selection();
        assert!(app.scene.is_selected(&SelectionItem::Face(f0)));

        // 3. Make group
        app.make_group();
        assert_eq!(app.scene.groups.len(), 1);
        assert_eq!(app.scene.mesh.faces.len(), 0);

        // 4. Copy group
        app.scene.select(SelectionItem::Group(0));
        app.copy_selection();
        assert!(app.clipboard.is_some());

        // 5. Paste selection
        app.paste_selection();
        app.update_paste_preview(DVec3::new(1.,1.,0.));app.commit_paste_preview();
        assert_eq!(app.scene.groups.len(), 2);

        // 6. Explode group
        app.scene.clear_selection();
        app.scene.select(SelectionItem::Group(0));
        app.explode_group();
        assert_eq!(app.scene.groups.len(), 1);
        assert_eq!(app.scene.mesh.faces.len(), 1);

        // 7. Cut and paste in place
        app.scene.clear_selection();
        app.scene.select(SelectionItem::Face(0));
        app.cut_selection();
        assert_eq!(app.scene.mesh.faces.len(), 0);
        app.paste_in_place();
        assert_eq!(app.scene.mesh.faces.len(), 1);
    }
    #[test]
    fn solid_boolean_prepares_before_undo_and_restores_inputs() {
        fn cube() -> ingetrazo_core::mesh::Mesh {
            let mut mesh = ingetrazo_core::mesh::Mesh::new();
            for p in [[0.,0.,0.], [1.,0.,0.], [1.,1.,0.], [0.,1.,0.],
                      [0.,0.,1.], [1.,0.,1.], [1.,1.,1.], [0.,1.,1.]] {
                mesh.add_vertex(DVec3::from_array(p));
            }
            for face in [vec![0,3,2,1], vec![4,5,6,7], vec![0,1,5,4],
                         vec![1,2,6,5], vec![2,3,7,6], vec![3,0,4,7]] {
                mesh.add_face(face, vec![]);
            }
            mesh
        }
        let mut app = IngeTrazoApp::new();
        let a = ingetrazo_core::groups::Group::with_mesh("a", cube());
        let mut b = ingetrazo_core::groups::Group::with_mesh("b", cube());
        b.transform = glam::DMat4::from_translation(DVec3::new(0.5, 0., 0.));
        app.scene.groups = vec![a, b];
        app.scene.select(SelectionItem::Group(0));
        app.scene.select(SelectionItem::Group(1));
        let before = app.scene.clone();
        app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Trim);
        assert_eq!(app.scene.groups.len(), 2);
        assert_eq!(app.scene.groups[0], before.groups[0]);
        assert!((app.scene.groups[1].mesh.volume().unwrap() - 0.5).abs() < 1e-8);
        assert_eq!(app.undo_stack.len(), 1);
        let after = app.scene.clone();
        app.undo();
        assert_eq!(app.scene.groups, before.groups);
        assert_eq!(app.scene.selection, before.selection);
        app.redo();
        assert_eq!(app.scene.groups, after.groups);
        assert_eq!(app.scene.selection, after.selection);
        app.undo();
        app.scene.groups[1].mesh.clear();
        let invalid = app.scene.clone();
        let undo_len = app.undo_stack.len();
        let redo_len = app.redo_stack.len();
        app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Union);
        assert_eq!(app.scene.groups, invalid.groups);
        assert_eq!(app.scene.selection, invalid.selection);
        assert_eq!(app.undo_stack.len(), undo_len);
        assert_eq!(app.redo_stack.len(), redo_len);
    }

}



#[cfg(test)]
mod document_camera_tests {
    use super::*;
    #[test]
    fn visible_camera_is_saved_retained_through_geometry_history_and_reset_for_new_document() {
        let path = std::env::temp_dir().join(format!("camera-{}.igz", uuid::Uuid::new_v4()));
        let mut app = IngeTrazoApp::new();
        app.camera.target = DVec3::new(3.0,4.0,5.0);
        app.camera.distance = 42.0;
        app.camera.yaw = 0.6;
        app.camera.perspective = false;
        app.save_to_path(&path);
        let reopened = IngeTrazoApp::with_file(&path);
        assert_eq!(reopened.camera.target, app.camera.target);
        assert_eq!(reopened.camera.distance, 42.0);
        assert_eq!(reopened.camera.yaw, 0.6);
        assert!(!reopened.camera.perspective);
        let before_mesh = app.scene.mesh.clone();
        app.save_undo_state();
        app.scene.mesh.add_vertex(DVec3::X);
        let after_mesh = app.scene.mesh.clone();
        app.camera.target=DVec3::new(8.,9.,10.);
        app.undo();
        assert_eq!(app.scene.mesh, before_mesh);
        // Original geometry history does not own later live camera motion.
        // The independent saved-view history fixture covers this source law.
        assert_eq!(app.camera.target,DVec3::new(8.,9.,10.));
        app.redo();
        assert_eq!(app.scene.mesh, after_mesh);
        assert_eq!(app.camera.target,DVec3::new(8.,9.,10.));
        // Original views/main_window.py clears history at a document boundary.
        app.new_document();
        assert!(app.document_session.confirm_unsaved);
        app.discard_document_action();
        assert_eq!(app.camera, OrbitCamera::new());
        assert!(app.undo_stack.is_empty());
        assert!(app.redo_stack.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod ordered_solid_selection_tests {
    use super::*;
    #[test]
    fn first_selected_group_is_cutter_even_when_scene_order_is_reversed() {
        let mut mesh = ingetrazo_core::mesh::Mesh::new();
        for p in [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.],[0.,0.,1.],[1.,0.,1.],[1.,1.,1.],[0.,1.,1.]] {
            mesh.add_vertex(DVec3::from_array(p));
        }
        for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]] { mesh.add_face(f.to_vec(),vec![]); }
        let mut app = IngeTrazoApp::new();
        let receiver = ingetrazo_core::groups::Group::with_mesh("receiver",mesh.clone());
        let mut cutter = ingetrazo_core::groups::Group::with_mesh("cutter",mesh);
        cutter.transform = glam::DMat4::from_translation(DVec3::new(0.5,0.,0.));
        app.scene.groups=vec![receiver,cutter];
        app.scene.select(SelectionItem::Group(1));
        app.scene.select(SelectionItem::Group(0));
        app.scene.select(SelectionItem::Group(1)); // Reselecting doesn't reorder.
        assert_eq!(app.scene.selected_groups_in_order(),vec![1,0]);
        app.perform_solid_boolean(ingetrazo_core::solids::SolidOp::Subtract);
        assert_eq!(app.scene.groups.len(),1);
        let result=&app.scene.groups[0];
        assert!((result.mesh.volume().unwrap()-0.5).abs()<1e-8);
        assert!(result.mesh.vertices.iter().all(|v| v.position.x <= 0.5+1e-8));
        app.undo();
        assert_eq!(app.scene.selected_groups_in_order(),vec![1,0]);
        app.scene.deselect(&SelectionItem::Group(1));
        app.scene.select(SelectionItem::Group(1));
        assert_eq!(app.scene.selected_groups_in_order(),vec![0,1]);
        app.scene.clear_selection();
        assert!(app.scene.selection_order.is_empty());
    }
}

#[cfg(test)]mod model_revision_regression {
 use super::*;
 #[test]fn history_and_context_swaps_never_reuse_gpu_model_revision(){
  let mut app=IngeTrazoApp::new();let initial=app.model_revision;
  app.save_undo_state();app.scene.mesh.add_vertex(DVec3::X);let committed=app.model_revision;assert!(committed>initial);
  app.undo();let undo=app.model_revision;assert!(undo>committed);app.redo();assert!(app.model_revision>undo);
  let mut mesh=ingetrazo_core::mesh::Mesh::new();let vertices=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(vertices.to_vec(),vec![]);
  app.scene.groups.push(ingetrazo_core::groups::Group::with_mesh("context",mesh));app.scene.selection.insert(SelectionItem::Group(0));let before=app.model_revision;
  app.begin_component_edit();assert_eq!(app.component_edit_stack.len(),1);assert!(app.model_revision>before);let entered=app.model_revision;app.cancel_component_edit();assert!(app.model_revision>entered);
 }
}

#[cfg(test)]
mod console_transaction_tests {
    use super::IngeTrazoApp;

    #[test]
    fn console_cube_is_one_reversible_transaction() {
        let mut app = IngeTrazoApp::new();
        let before = app.scene.mesh.clone();
        app.run_script("add_cube");
        assert_eq!(app.undo_stack.len(), 1);
        let after = app.scene.mesh.clone();
        assert_ne!(after, before);
        assert_eq!(after.faces.len(), 6);
        app.undo();
        assert_eq!(app.scene.mesh, before);
        assert_eq!(app.redo_stack.len(), 1);
        app.redo();
        assert_eq!(app.scene.mesh, after);
    }

    #[test]
    fn console_refusals_and_inspection_preserve_model_and_redo() {
        let mut app = IngeTrazoApp::new();
        app.run_script("cube");
        app.undo();
        let before = app.scene.clone();
        let undo = app.undo_stack.len();
        let redo = app.redo_stack.len();
        for command in ["cube_invalid", "add_cube(2)", "print(1)", "cube(); delete_all"] {
            app.run_script(command);
            assert_eq!(app.scene, before);
            assert_eq!(app.undo_stack.len(), undo);
            assert_eq!(app.redo_stack.len(), redo);
            assert!(app.console_logs.last().unwrap().starts_with("Unsupported console command:"));
        }
        for command in ["stats", "clear", "", "  "] {
            app.run_script(command);
            assert_eq!(app.scene, before);
            assert_eq!(app.undo_stack.len(), undo);
            assert_eq!(app.redo_stack.len(), redo);
        }
    }
}

#[cfg(test)]
mod released_history_chord_tests {
    use super::*;
    #[test]
    fn released_ctrl_chords_use_key_event_modifiers_and_route_sheet_history() {
        let mut app=IngeTrazoApp::new();app.scene.sheets=vec![ingetrazo_core::composition::Sheet::default()];app.scene.active_sheet_index=Some(0);app.enter_composer_sheet(0);
        app.scene.sheets[0].items.clear();app.scene.sheets[0].items.push(ingetrazo_core::composition::SheetItem::Text(ingetrazo_core::composition::TextItem{id:uuid::Uuid::new_v4(),pos:(20.,30.),text:"History".into(),font_size_pt:14.,bold:false}));
        let before=app.scene.sheets.clone();app.begin_composer_edit();app.scene.sheets[0].items[0].set_pos(33.,44.);app.finish_composer_edit();let after=app.scene.sheets.clone();
        let ctx=egui::Context::default();
        let frame=|app:&mut IngeTrazoApp,key,modifiers|{let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1600.,1000.))),modifiers:egui::Modifiers::NONE,events:vec![egui::Event::Key{key,physical_key:None,pressed:true,repeat:false,modifiers},egui::Event::Key{key,physical_key:None,pressed:false,repeat:false,modifiers:egui::Modifiers::NONE}],..Default::default()},|ctx|app.update_ui(ctx));};
        frame(&mut app,egui::Key::Z,egui::Modifiers::CTRL);assert_eq!(app.scene.sheets,before);
        frame(&mut app,egui::Key::Y,egui::Modifiers::CTRL);assert_eq!(app.scene.sheets,after);
        frame(&mut app,egui::Key::Z,egui::Modifiers::NONE);assert_eq!(app.scene.sheets,after);
        frame(&mut app,egui::Key::Z,egui::Modifiers::COMMAND);assert_eq!(app.scene.sheets,before);
        frame(&mut app,egui::Key::Y,egui::Modifiers::NONE);assert_eq!(app.scene.sheets,before);
        frame(&mut app,egui::Key::Y,egui::Modifiers::COMMAND);assert_eq!(app.scene.sheets,after);
        assert!(app.undo_stack.is_empty());
    }
}

#[cfg(test)]
mod title_frame_regression {
    use super::*;
    #[test]
    fn title_commands_only_on_first_or_changed_document_frame() {
        let mut app = IngeTrazoApp::new();
        let ctx = egui::Context::default();
        let frame = |app: &mut IngeTrazoApp| {
            let output = ctx.run(egui::RawInput::default(), |ctx| app.update_ui(ctx));
            output.viewport_output[&egui::ViewportId::ROOT].commands.iter()
                .filter_map(|command| match command {
                    egui::ViewportCommand::Title(title) => Some(title.clone()),
                    _ => None,
                }).collect::<Vec<_>>()
        };
        assert_eq!(frame(&mut app), vec!["IngeTrazo 0.3.15"]);
        assert!(frame(&mut app).is_empty());
        app.current_file = Some(std::path::PathBuf::from("changed.igz"));
        assert_eq!(frame(&mut app), vec!["IngeTrazo — changed.igz"]);
        assert!(frame(&mut app).is_empty());
        app.current_file = None;
        assert_eq!(frame(&mut app), vec!["IngeTrazo 0.3.15"]);
    }
}
