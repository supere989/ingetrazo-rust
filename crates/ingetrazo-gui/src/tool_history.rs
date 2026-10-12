// SPDX-License-Identifier: GPL-3.0-or-later
//! Tool commits share one undo transaction whether entered by click or VCB.
use crate::app::IngeTrazoApp;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::base::ToolContext;

fn changed_model(before: &Scene, after: &Scene) -> bool {
    before.image_planes != after.image_planes || before.mesh != after.mesh
        || before.groups != after.groups
        || before.component_definitions != after.component_definitions
        || before.mesh_shares != after.mesh_shares
        || before.dimensions != after.dimensions
        || before.guides != after.guides
        || before.section_planes != after.section_planes
        || before.geo_points != after.geo_points
        || before.geo_paths != after.geo_paths
        || before.terrain != after.terrain
        || before.materials != after.materials
        || before.layers != after.layers
        || before.text_labels != after.text_labels
        || before.sheets != after.sheets
}

// Cold commit comparison reuses the existing history snapshot. No Scene clone,
// geometry equality walk, or registry scan is added to pointer-move handling.
fn same_group_static(a:&ingetrazo_core::groups::Group,b:&ingetrazo_core::groups::Group)->bool {
    a.id==b.id && a.mesh_share==b.mesh_share && a.component_definition==b.component_definition
    && a.definition_member==b.definition_member && a.mesh==b.mesh && a.name==b.name
    && a.native_document==b.native_document && a.native_baseline==b.native_baseline
    && a.layer==b.layer && a.hidden==b.hidden && a.ifc_tag==b.ifc_tag
    && a.billboard==b.billboard && a.billboard_texture==b.billboard_texture
    && a.children.len()==b.children.len() && a.children.iter().zip(&b.children).all(|(a,b)|same_group_static(a,b))
}
fn only_group_poses(before:&Scene,after:&Scene)->bool {
    before.groups!=after.groups && before.groups.len()==after.groups.len()
    && before.groups.iter().zip(&after.groups).all(|(a,b)|same_group_static(a,b))
    && before.image_planes==after.image_planes && before.mesh==after.mesh
    && before.component_definitions==after.component_definitions && before.mesh_shares==after.mesh_shares
    && before.dimensions==after.dimensions && before.guides==after.guides && before.section_planes==after.section_planes
    && before.geo_points==after.geo_points && before.geo_paths==after.geo_paths && before.terrain==after.terrain
    && before.materials==after.materials && before.layers==after.layers && before.text_labels==after.text_labels && before.sheets==after.sheets
}

fn same_mesh_shape(a:&ingetrazo_core::mesh::Mesh,b:&ingetrazo_core::mesh::Mesh)->bool{
    a.vertices==b.vertices && a.edges.len()==b.edges.len() && a.faces.len()==b.faces.len()
    && a.edges.iter().zip(&b.edges).all(|(a,b)|a.v0==b.v0&&a.v1==b.v1&&a.faces==b.faces&&a.soft==b.soft&&a.curve_id==b.curve_id&&a.layer==b.layer&&a.hidden==b.hidden)
    && a.faces.iter().zip(&b.faces).all(|(a,b)|a.vertices==b.vertices&&a.holes==b.holes&&a.layer==b.layer&&a.interior==b.interior)
}
fn same_mesh_paint(a:&ingetrazo_core::mesh::Mesh,b:&ingetrazo_core::mesh::Mesh)->bool{
    a.edges.len()==b.edges.len()&&a.faces.len()==b.faces.len()
    && a.edges.iter().zip(&b.edges).all(|(a,b)|a.attrs==b.attrs&&a.soft==b.soft&&a.curve_id==b.curve_id&&a.layer==b.layer&&a.hidden==b.hidden)
    && a.faces.iter().zip(&b.faces).all(|(a,b)|a.attrs==b.attrs&&a.layer==b.layer&&a.interior==b.interior)
}
fn group_domain_equal(a:&ingetrazo_core::groups::Group,b:&ingetrazo_core::groups::Group,paint:bool)->bool{
    a.id==b.id&&a.mesh_share==b.mesh_share&&a.component_definition==b.component_definition&&a.definition_member==b.definition_member
    && a.transform==b.transform&&a.name==b.name&&a.native_document==b.native_document&&a.native_baseline==b.native_baseline
    && a.layer==b.layer&&a.hidden==b.hidden&&a.ifc_tag==b.ifc_tag&&a.billboard==b.billboard&&a.billboard_texture==b.billboard_texture
    && if paint{same_mesh_shape(&a.mesh,&b.mesh)}else{same_mesh_paint(&a.mesh,&b.mesh)}
    && a.children.len()==b.children.len()&&a.children.iter().zip(&b.children).all(|(a,b)|group_domain_equal(a,b,paint))
}
fn same_scene_non_mesh(a:&Scene,b:&Scene)->bool{
    a.image_planes==b.image_planes&&a.component_definitions==b.component_definitions&&a.mesh_shares==b.mesh_shares
    && a.dimensions==b.dimensions&&a.guides==b.guides&&a.section_planes==b.section_planes
    && a.geo_points==b.geo_points&&a.geo_paths==b.geo_paths&&a.terrain==b.terrain
    && a.layers==b.layers&&a.text_labels==b.text_labels&&a.sheets==b.sheets
}
fn only_mesh_domain(a:&Scene,b:&Scene,paint:bool)->bool{
    same_scene_non_mesh(a,b)&&a.groups.len()==b.groups.len()
    && (paint||a.materials==b.materials)
    && if paint{same_mesh_shape(&a.mesh,&b.mesh)}else{same_mesh_paint(&a.mesh,&b.mesh)}
    && a.groups.iter().zip(&b.groups).all(|(a,b)|group_domain_equal(a,b,paint))
}

impl IngeTrazoApp {
    fn tool_snapshot(&self) -> Scene {
        let mut before = self.scene.clone();
        before.camera = self.camera.clone();
        self.active_tool.restore_transient_model(&mut before);
        before
    }

    fn record_tool_commit(&mut self, mut before: Scene) -> Result<(),String> {
        // Grab and in-place release can leave a Scale preview live. Publishing
        // that preview as a commit would add an undo entry before acceptance.
        if self.active_tool.pending_model_preview() { return Ok(()); }
        for (id,face) in self.active_tool.take_history_acquisition_faces() {
            if let Some(before_face)=before.mesh.faces.get_mut(id) {
                if before_face.history_id==face.history_id { *before_face=face; }
            }
        }
        let axes_command=self.active_tool.take_axes_command().map(|c|self.complete_axes_command(c));
        if let Some(command)=axes_command.as_ref(){match self.prepare_axes_parked(command,&self.scene){Ok(parked)=>self.publish_axes_parked(parked),Err(error)=>{self.active_tool.axes_commit_refused(command.target);self.scene=before;self.bump_model_revision();self.status_message=error.clone();return Err(error);}}}
        let bare_edge=self.active_tool.take_bare_edge_command();
        let owned=self.active_tool.take_authored_draw_faces();
        if let Err(error)=ingetrazo_core::bim::stamp_drawn_faces_with_id(&mut self.scene.mesh,&owned,self.scene.active_ifc.as_ref(),self.scene.bim_draw_id.as_ref()){
            self.scene=before;self.bump_model_revision();self.status_message=format!("Tool error: {error}");return Err(error);
        }

        if before.mesh != self.scene.mesh && self.active_tool_type!=ingetrazo_tools::base::ToolType::Fillet {
            if let Err(error)=ingetrazo_core::heal::refresh_interior(&mut self.scene.mesh) {
                self.scene=before;self.bump_model_revision();
                self.status_message=format!("Tool error: {error}");
                return Err(error);
            }
        }
        let image_command=self.active_tool.take_image_frame_command();
        let replacing=self.active_tool.take_history_replacement();
        let transient_changed=before.version!=self.scene.version;
        let committed=changed_model(&before,&self.scene) || bare_edge.is_some();
        if committed {
            before.history_annotation_move=self.active_tool_type==ingetrazo_tools::base::ToolType::Move && (before.dimensions!=self.scene.dimensions||before.text_labels!=self.scene.text_labels);
            before.history_axes=axes_command.map(|c|c.inverse());
            // Source Fillet SnapshotMutation owns mesh only; undo does not restore selection.
            if self.active_tool_type==ingetrazo_tools::base::ToolType::Fillet { before.selection=self.scene.selection.clone(); }
            if let Some((a,b))=bare_edge {
                before.history_group_visibility=Some(ingetrazo_core::saved_views::GroupVisibilityCommand { targets:Vec::new(),hidden:false,undo:true,retain_mesh_flags:true,snapshot_after:None });
                // Reuse the admitted history snapshot. Copy only the welded
                // endpoint records, not a second complete Mesh/Scene candidate.
                for point in [a,b] {
                    let actual=self.scene.mesh.vertex_at_position(point).expect("admitted Line endpoint");
                    let vertex=&self.scene.mesh.vertices[actual];
                    let id=before.mesh.add_vertex(vertex.position);
                    before.mesh.vertices[id].uid=vertex.uid;
                }
            }
            if bare_edge.is_none() && self.active_tool_type==ingetrazo_tools::base::ToolType::Line {
                before.history_group_visibility=Some(ingetrazo_core::saved_views::GroupVisibilityCommand {targets:Vec::new(),hidden:false,undo:true,retain_mesh_flags:false,snapshot_after:Some(crate::history_visibility::mesh_snapshot_flags(&self.scene))});
            }
            self.seal_view_history_before(&before);self.seal_terrain_history_before(&before);self.seal_layer_history_before(&before);self.seal_image_history_before(&before);
            if let Some(command)=image_command{let mut delta=ingetrazo_core::image_plane::ImageDelta::between(&before.image_planes,&self.scene.image_planes);delta.frame_command=Some((command,false));before.history_images.0=Some(delta);}
            if only_group_poses(&before,&self.scene){self.bump_placement_revision();}
            else if only_mesh_domain(&before,&self.scene,true){self.bump_render_paint_revision();}
            else if only_mesh_domain(&before,&self.scene,false){self.bump_render_geometry_revision();}
            else{self.bump_model_revision();}
            if !replacing||self.undo_stack.is_empty(){self.undo_stack.push(before);}
            else if let Some(command)=before.history_images.0.as_ref().and_then(|d|d.frame_command.clone()){
                // Hot retype retains the original undo base, but the relative
                // image operation must describe the newly accepted factor.
                if let Some(delta)=self.undo_stack.last_mut().and_then(|s|s.history_images.0.as_mut()){delta.frame_command=Some(command);}
            }
            self.active_tool.note_history_revision(self.model_revision,true);
            self.redo_stack.clear();
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
        if transient_changed&&!committed{self.bump_model_revision();}
        Ok(())
    }

    pub fn dispatch_tool_control_state(&mut self,held:bool){
        let placement_before=self.active_tool.placement_only_preview();let version=self.scene.version;let images=self.scene.image_planes.publication_revision();self.active_tool.on_control_state(&mut self.scene,held);
        if self.scene.version!=version{if placement_before||self.active_tool.placement_only_preview(){self.bump_placement_revision();}else{self.bump_model_revision();}}else if self.scene.image_planes.publication_revision()!=images{self.bump_image_presentation_revision();}
    }
    pub fn dispatch_tool_cancel(&mut self){
        let placement_before=self.active_tool.placement_only_preview();let version=self.scene.version;let images=self.scene.image_planes.publication_revision();self.active_tool.on_cancel(&mut self.scene);
        if self.scene.version!=version{if placement_before||self.active_tool.placement_only_preview(){self.bump_placement_revision();}else{self.bump_model_revision();}}else if self.scene.image_planes.publication_revision()!=images{self.bump_image_presentation_revision();}
        if self.active_tool.completed(){self.set_tool(ingetrazo_tools::base::ToolType::Select);}
    }
    pub fn dispatch_tool_move(&mut self,context:&ToolContext){
        if self.active_tool_type==ingetrazo_tools::base::ToolType::Dimension {self.active_tool.note_camera_forward((self.camera.target.as_vec3()-self.camera.eye().as_vec3()).as_dvec3());}
        let placement_before=self.active_tool.placement_only_preview();let version=self.scene.version;let images=self.scene.image_planes.publication_revision();self.active_tool.on_move(&mut self.scene,context);
        if self.scene.version!=version{if placement_before||self.active_tool.placement_only_preview(){self.bump_placement_revision();}else{self.bump_model_revision();}}else if self.scene.image_planes.publication_revision()!=images{self.bump_image_presentation_revision();}
        if self.active_tool.error_message().is_none(){if let Some(status)=self.active_tool.take_status_message(){self.status_message=status;}}
    }

    pub fn dispatch_tool_click(&mut self, context: &ToolContext) {
        if self.active_tool_type==ingetrazo_tools::ToolType::Image {
            self.active_tool.on_click(&mut self.scene,context);
            if let Err(error)=self.publish_image_tool_request(){self.status_message=error;}
            return;
        }
        if self.active_tool_type==ingetrazo_tools::base::ToolType::Dimension {self.active_tool.note_camera_forward((self.camera.target.as_vec3()-self.camera.eye().as_vec3()).as_dvec3());}
        // Text acquisition and its dialog are transient; only OK owns a model snapshot.
        if self.active_tool_type == ingetrazo_tools::base::ToolType::Text {
            if self.annotation_editor.is_some() { return; }
            self.active_tool.on_click(&mut self.scene, context);
            if let Some(suggestion)=self.active_tool.take_text_suggestion() {
                match self.format_text_suggestion(suggestion,context.world.as_vec3().as_dvec3()) {
                    Ok(text)=>self.active_tool.set_text_suggestion(text),
                    Err(error)=>{self.active_tool.finish_text_creation();self.status_message=error;return;}
                }
            }
            if let Some(request) = self.active_tool.text_creation_request() { self.begin_text_creation(request); }
            return;
        }

        if matches!(self.active_tool_type,ingetrazo_tools::base::ToolType::Rotate|ingetrazo_tools::base::ToolType::Scale)
            && self.scene.selection.iter().any(|s|matches!(s,ingetrazo_core::scene::SelectionItem::Guide(_)|ingetrazo_core::scene::SelectionItem::Dimension(_)|ingetrazo_core::scene::SelectionItem::TextLabel(_))) {
            self.status_message="Annotation transform ownership is not yet supported".into();return;
        }
        if self.section_name_prompt.is_some(){return;}
        let plane_count=self.scene.section_planes.len();
        if self.prepare_bim_draw_commit().is_err(){return;}
        let before = self.tool_snapshot();
        self.active_tool.on_click(&mut self.scene, context);
        self.status_message = self
            .active_tool
            .error_message()
            .map(|error| format!("Tool error: {error}"))
            .unwrap_or_default();
        if self.active_tool.error_message().is_none(){if let Some(status)=self.active_tool.take_status_message(){self.status_message=status;}}
        if self.active_tool.error_message().is_some(){
            let unchanged_move=self.active_tool_type==ingetrazo_tools::base::ToolType::Move&&self.scene==before;
            self.scene=before;self.active_tool.restore_current_preview_model(&mut self.scene);if !unchanged_move{self.bump_model_revision();}return;
        }
        if self.record_tool_commit(before).is_ok() {
            if self.scene.section_planes.len()>plane_count { self.queue_section_name_prompt(); }
            if self.active_tool.completed() {
                let status = self.status_message.clone();
                self.set_tool(ingetrazo_tools::base::ToolType::Select);
                self.status_message = status;
            }
        }
    }

    pub fn dispatch_tool_release(&mut self, context: &ToolContext, drag_distance: f64) {
        if self.active_tool_type==ingetrazo_tools::ToolType::Image{return;}
        if self.section_name_prompt.is_some(){return;}
        if self.prepare_bim_draw_commit().is_err(){return;}
        let before = self.tool_snapshot();
        self.active_tool.on_release(&mut self.scene, context, drag_distance);
        self.status_message = self.active_tool.error_message().map(|e| format!("Tool error: {e}")).unwrap_or_default();
        if self.active_tool.error_message().is_none(){if let Some(status)=self.active_tool.take_status_message(){self.status_message=status;}}
        let _ = self.record_tool_commit(before);
    }

    pub fn dispatch_tool_double_click(&mut self, context: &ToolContext) {
        if self.active_tool_type==ingetrazo_tools::ToolType::Image{self.dispatch_tool_click(context);return;}
        if self.annotation_editor.is_some()||self.section_name_prompt.is_some(){return;}
        if let Some(target)=self.active_tool.annotation_edit_target(&self.scene,context){if let Err(error)=self.begin_annotation_edit(target){self.status_message=error;}return;}
        if self.prepare_bim_draw_commit().is_err(){return;}
        let before = self.tool_snapshot();
        self.active_tool.on_double_click(&mut self.scene, context);
        if let Some(target)=self.active_tool.take_annotation_edit_request(){if let Err(error)=self.begin_annotation_edit(target){self.status_message=error;}return;}

        self.status_message = self
            .active_tool
            .error_message()
            .map(|error| format!("Tool error: {error}"))
            .unwrap_or_default();
        if self.active_tool.error_message().is_none(){if let Some(status)=self.active_tool.take_status_message(){self.status_message=status;}}
        if self.record_tool_commit(before).is_ok()&&self.active_tool_type==ingetrazo_tools::base::ToolType::Select&&self.scene.selection.len()==1&&self.scene.selection.iter().any(|item|matches!(item,ingetrazo_core::scene::SelectionItem::Group(_))){self.begin_component_edit();}
    }

    pub fn dispatch_tool_vcb(&mut self, text: &str) -> Result<(), String> {
        if self.active_tool_type==ingetrazo_tools::ToolType::Image {
            self.active_tool.on_vcb_input(&mut self.scene,text)?;
            return self.publish_image_tool_request();
        }
        // Original Viewport parses Unicode decimal fields before dispatching
        // numeric/segment/radius/array tags. Preserve the visible entered text.
        let normalized=ingetrazo_core::units::vcb::normalize_source_numeric(text);
        let text=normalized.as_ref();
        if self.navigation.mode.is_some(){return self.navigation_height(text);}
        if self.section_name_prompt.is_some(){return Err("Finish the section naming dialog first".into());}
        self.prepare_bim_draw_commit()?;
        let before = self.tool_snapshot();
        let version_at_entry=self.scene.version;
        let images_at_entry=self.scene.image_planes.publication_revision();
        self.active_tool.note_history_revision(self.model_revision,false);
        match self.active_tool.on_vcb_input(&mut self.scene, text) {
            Ok(()) => {
                if let Some(status)=self.active_tool.take_status_message(){self.status_message=status;}
                self.record_tool_commit(before)
            }
            Err(error) => {
                // Parsing/admission refusal leaves the live preview and last
                // command identity intact. Only an actual mutation needs rollback.
                if !matches!(self.active_tool_type,ingetrazo_tools::base::ToolType::Scale|ingetrazo_tools::base::ToolType::Circle|ingetrazo_tools::base::ToolType::Polygon|ingetrazo_tools::base::ToolType::Arc|ingetrazo_tools::base::ToolType::RotatedRectangle)
                    ||self.scene.version!=version_at_entry||self.scene.image_planes.publication_revision()!=images_at_entry{
                    self.scene=before;self.active_tool.restore_current_preview_model(&mut self.scene);self.bump_model_revision();
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::{math::Ray, scene::SelectionItem};
    use ingetrazo_tools::base::{create_tool, ToolType};

    fn app() -> IngeTrazoApp {
        let mut app = IngeTrazoApp::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.), DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ids, vec![]);
        app.active_tool = create_tool(ToolType::PushPull);
        app.active_tool_type = ToolType::PushPull;
        app
    }

    fn context(z: f64) -> ToolContext {
        ToolContext::new(
            DVec3::new(0.5, 0.5, z),
            Ray::new(DVec3::new(0.5, 0.5, 5.), -DVec3::Z),
        )
    }

    #[test]
    fn paint_click_and_geometry_commit_have_distinct_static_domains(){
        let mut app=app();app.set_tool(ToolType::Paint);let before=app.native_render_revisions;
        app.dispatch_tool_click(&context(0.));
        assert!(app.native_render_revisions.static_domains.paint>before.static_domains.paint);
        assert_eq!(app.native_render_revisions.static_domains.geometry,before.static_domains.geometry);
        assert_eq!(app.native_render_revisions.poses,before.poses);
        let before=app.scene.clone();app.scene.mesh.vertices[0].position.x+=0.25;
        assert!(only_mesh_domain(&before,&app.scene,false));assert!(!only_mesh_domain(&before,&app.scene,true));
        let revisions=app.native_render_revisions;app.record_tool_commit(before).unwrap();
        assert!(app.native_render_revisions.static_domains.geometry>revisions.static_domains.geometry);
        assert_eq!(app.native_render_revisions.static_domains.paint,revisions.static_domains.paint);
        let before=app.scene.clone();app.scene.mesh.faces[0].holes.push(vec![0,1,2]);
        assert!(!only_mesh_domain(&before,&app.scene,true));
        app.scene=before.clone();app.scene.groups.push(ingetrazo_core::groups::Group::new("new"));
        assert!(!only_mesh_domain(&before,&app.scene,true));assert!(!only_mesh_domain(&before,&app.scene,false));
    }
    #[test]
    fn layer_visibility_preserves_source_move_array_window(){
        let mut app=app();app.add_layer("visible".into());app.set_tool(ToolType::Move);app.scene.select(SelectionItem::Face(0));
        let mut first=context(0.);first.ctrl_held=true;app.dispatch_tool_click(&first);app.dispatch_tool_vcb("6;0;0").unwrap();
        let revision=app.model_revision;let undo=app.undo_stack.len();app.change_layer_visibility(1,false);
        assert_eq!(app.model_revision,revision,"layer presentation is outside modeling command history");
        app.dispatch_tool_vcb("3x").unwrap();assert_eq!(app.undo_stack.len(),undo);assert_eq!(app.scene.mesh.faces.len(),4);
    }
    #[test]
    fn presentation_and_layer_callers_advance_only_their_static_domains(){
        let mut app=app();app.add_layer("visible".into());let initial=app.native_render_revisions;
        app.change_layer_visibility(1,false);assert_eq!(app.native_render_revisions.static_domains.geometry,initial.static_domains.geometry);
        assert!(app.native_render_revisions.static_domains.layers>initial.static_domains.layers);
        let before=app.native_render_revisions;let history_identity=app.model_revision;app.style.show_edges=!app.style.show_edges;app.sync_document_presentation();
        assert!(app.native_render_revisions.static_domains.style>before.static_domains.style);
        assert_eq!(app.model_revision,history_identity,"presentation must not retire array history");
        assert_eq!(app.native_render_revisions.static_domains.geometry,before.static_domains.geometry);
        let unchanged=app.native_render_revisions;app.sync_document_presentation();assert_eq!(app.native_render_revisions,unchanged);
    }
    #[test]
    fn committed_group_transform_classification_rejects_paint_topology_and_mesh_changes(){
        let mut before=Scene::new();before.groups.push(ingetrazo_core::groups::Group::new("group"));
        let mut after=before.clone();after.groups[0].transform=glam::DMat4::from_translation(DVec3::X);
        assert!(only_group_poses(&before,&after));
        after.groups[0].native_document=serde_json::json!({"material":{"color":[1,0,0]}});
        assert!(!only_group_poses(&before,&after));after.groups[0].native_document=serde_json::Value::Null;
        after.groups[0].mesh.add_vertex(DVec3::ZERO);assert!(!only_group_poses(&before,&after));
        after=before.clone();after.groups.push(after.groups[0].duplicate_instance());assert!(!only_group_poses(&before,&after));
    }
    #[test]
    fn group_move_preview_preserves_static_domains_and_invalidates_legacy_world() {
        let mut app=app();let mesh=std::mem::take(&mut app.scene.mesh);
        app.scene.groups.push(ingetrazo_core::groups::Group::with_mesh("placed",mesh));
        app.scene.select(SelectionItem::Group(0));app.set_tool(ToolType::Move);
        app.dispatch_tool_click(&context(0.));
        let domains=app.native_render_revisions.static_domains;
        let poses=app.native_render_revisions.poses;let legacy=app.native_geometry_revision;let model=app.model_revision;
        app.dispatch_tool_move(&context(1.));
        assert_eq!(app.native_render_revisions.static_domains,domains);
        assert!(app.native_render_revisions.poses>poses);
        assert!(app.native_geometry_revision>legacy);assert!(app.model_revision>model);
        let unchanged=app.native_render_revisions;app.dispatch_tool_move(&context(1.));
        assert_eq!(app.native_render_revisions,unchanged);
        app.dispatch_tool_cancel();assert_eq!(app.native_render_revisions.static_domains,domains);
        assert!(app.native_render_revisions.poses>unchanged.poses);
        assert_eq!(app.scene.groups[0].transform,glam::DMat4::IDENTITY);
        // Unknown replacement is deliberately conservative, even with same Scene.version.
        app.bump_model_revision();assert_ne!(app.native_render_revisions.static_domains,domains);
    }
    #[test]fn history_replacement_retires_move_targets_and_redo_preserves_original_geometry(){
        let mut app=app();let original=app.scene.clone();app.undo_stack.push(Scene::new());app.set_tool(ToolType::Move);app.scene.select(SelectionItem::Face(0));app.dispatch_tool_click(&context(0.));app.dispatch_tool_move(&context(1.));
        assert_ne!(app.scene.mesh,original.mesh);app.undo();assert!(app.scene.mesh.faces.is_empty());assert!(app.active_tool.snap_anchor().is_none());
        app.redo();assert_eq!(app.scene.mesh,original.mesh);assert!(app.active_tool.snap_anchor().is_none());
        app.set_tool(ToolType::Move);app.scene.select(SelectionItem::Face(0));app.dispatch_tool_click(&context(0.));app.dispatch_tool_move(&context(1.));app.redo();assert_eq!(app.scene.mesh,original.mesh);assert!(app.active_tool.snap_anchor().is_none());
    }
    #[test]fn move_live_preview_click_vcb_failure_cancel_and_tool_change_are_transactional(){
        let mut app=app();app.set_tool(ToolType::Move);app.scene.select(SelectionItem::Face(0));let before=app.scene.mesh.clone();app.dispatch_tool_click(&context(0.));
        let revision=app.model_revision;app.dispatch_tool_move(&context(1.));assert!(app.model_revision>revision);assert!(app.undo_stack.is_empty());let preview=app.scene.mesh.clone();
        let mut invalid=context(1.);invalid.world=DVec3::splat(f64::NAN);app.dispatch_tool_click(&invalid);assert_eq!(app.scene.mesh,preview);assert!(app.undo_stack.is_empty());assert!(app.status_message.contains("finite"));
        assert!(app.dispatch_tool_vcb("garbage").is_err());assert_eq!(app.scene.mesh,preview);assert!(app.dispatch_tool_vcb("NaN").is_err());assert_eq!(app.scene.mesh,preview);
        app.dispatch_tool_move(&context(1.));assert_eq!(app.scene.mesh,preview);app.dispatch_tool_click(&context(1.));assert_eq!(app.undo_stack.len(),1);let committed=app.scene.mesh.clone();app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,committed);
        app.set_tool(ToolType::Move);app.scene.select(SelectionItem::Face(0));app.dispatch_tool_click(&context(1.));app.dispatch_tool_move(&context(2.));assert_ne!(app.scene.mesh,committed);app.set_tool(ToolType::Line);assert_eq!(app.scene.mesh,committed);assert_eq!(app.undo_stack.len(),1);
    }
    #[test]fn move_array_retyping_replaces_one_undo_and_document_replacement_closes_window(){
        let mut app=app();app.set_tool(ToolType::Move);app.scene.select(SelectionItem::Face(0));let before=app.scene.clone();let mut first=context(0.);first.ctrl_held=true;
        app.dispatch_tool_click(&first);app.dispatch_tool_vcb("6;0;0").unwrap();assert_eq!(app.undo_stack.len(),1);
        for text in ["3x","/3","2x"]{app.dispatch_tool_vcb(text).unwrap();assert_eq!(app.undo_stack.len(),1);}
        assert_eq!(app.scene.mesh.faces.len(),3);let after=app.scene.clone();app.undo();assert_eq!(app.scene.mesh,before.mesh);app.redo();assert_eq!(app.scene.mesh,after.mesh);
        assert!(app.dispatch_tool_vcb("4x").is_err());
        app.set_tool(ToolType::Move);app.scene.clear_selection();app.scene.select(SelectionItem::Face(0));app.dispatch_tool_click(&first);app.dispatch_tool_vcb("6;0;0").unwrap();
        // Replacement can restore the same Scene.version; application identity
        // must still close the source array command window.
        let restored=app.scene.clone();app.scene=restored;app.bump_model_revision();let before=app.scene.clone();assert!(app.dispatch_tool_vcb("4x").is_err());assert_eq!(app.scene,before);
    }
    #[test]fn nested_mirrored_component_drafting_commits_shared_definition_once(){
        use ingetrazo_core::{groups::Group,scene::SelectionItem};use glam::DMat4;
        let mut app=app();let mesh=app.scene.mesh.clone();app.scene=Scene::new();let child=Group::with_mesh("nested",mesh);let child_id=child.id;let mut parent=Group::new("outer");parent.transform=DMat4::from_translation(DVec3::new(9.,3.,2.))*DMat4::from_rotation_z(0.4)*DMat4::from_scale(DVec3::new(-2.,3.,1.));parent.children.push(child);app.scene.groups.push(parent);app.scene.make_component(child_id).unwrap();let mut copy=app.scene.groups[0].children[0].duplicate_instance();copy.transform=DMat4::from_translation(DVec3::new(3.,0.,0.));app.scene.groups[0].children.push(copy);app.scene.select(SelectionItem::Group(0));let original=app.scene.clone();
        app.begin_component_edit();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();assert_eq!(app.component_edit_stack.len(),2);app.set_tool(ToolType::Line);let world_start=app.scene.mesh.vertices[0].position;let world_delta=app.scene.mesh.vertices[2].position-world_start;let ctx=ToolContext::new(world_start,Ray::new(world_start+DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&ctx);app.dispatch_tool_vcb(&format!("{};{};{}",world_delta.x,world_delta.y,world_delta.z)).unwrap();assert_eq!(app.scene.mesh.faces.len(),2);assert_eq!(app.undo_stack.len(),1);app.active_tool.on_cancel(&mut app.scene);app.undo();assert_eq!(app.scene.mesh.faces.len(),1);app.redo();assert_eq!(app.scene.mesh.faces.len(),2);
        app.finish_component_edit();app.finish_component_edit();assert!(app.component_edit_stack.is_empty());assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.groups[0].transform,DMat4::IDENTITY);for (child,before) in app.scene.groups[0].children.iter().zip(&original.groups[0].children){assert_eq!(child.transform,original.groups[0].transform*before.transform,"source container bake preserves composed world placement");}assert!(app.scene.groups[0].children.iter().all(|child|child.mesh.faces.len()==2));let edited=app.scene.clone();let mut baked_original=original.clone();let frame=baked_original.groups[0].transform;baked_original.groups[0].transform=DMat4::IDENTITY;baked_original.groups[0].native_document=serde_json::json!({"axes":frame.to_cols_array()});for child in &mut baked_original.groups[0].children{child.transform=frame*child.transform;}app.undo();baked_original.selection=app.scene.selection.clone();baked_original.selection_order=app.scene.selection_order.clone();baked_original.version=app.scene.version;baked_original.view_version=app.scene.view_version;assert_eq!(app.scene,baked_original);app.redo();let mut expected=edited;expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);
        app.scene.clear_selection();app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();app.begin_component_edit();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();app.set_tool(ToolType::Rectangle);let ctx=ToolContext::new(DVec3::new(0.2,0.2,0.),Ray::new(DVec3::new(0.2,0.2,2.),-DVec3::Z));app.dispatch_tool_click(&ctx);app.active_tool.on_move(&mut app.scene,&ToolContext::new(DVec3::new(0.4,0.6,0.),ctx.ray));app.dispatch_tool_vcb("0.2,0.4").unwrap();app.cancel_component_edit();app.cancel_component_edit();assert_eq!(app.scene,before);assert_eq!(app.undo_stack.len(),1);
    }
    #[test]fn duplicate_line_undo_does_not_pop_unrelated_history_and_numeric_closure_stays_active(){
        let mut app=app();let prior=app.scene.clone();let mut empty=app.scene.clone();empty.mesh=ingetrazo_core::mesh::Mesh::new();app.undo_stack.push(empty);app.set_tool(ToolType::Line);let ctx=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&ctx);app.dispatch_tool_vcb("1;0;0").unwrap();assert_eq!(app.undo_stack.len(),1);app.undo();assert_eq!(app.scene.mesh,prior.mesh);assert_eq!(app.undo_stack.len(),1);app.undo();assert_eq!(app.undo_stack.len(),1);assert!(app.dispatch_tool_vcb("1;0;0").is_err());app.undo();assert!(app.scene.mesh.faces.is_empty());
        app.set_tool(ToolType::Line);app.dispatch_tool_click(&ctx);for text in ["1;0;0","0;1;0","-1;0;0","0;-1;0"]{app.dispatch_tool_vcb(text).unwrap();}assert_eq!(app.scene.mesh.faces.len(),1);app.undo();assert!(app.scene.mesh.faces.is_empty());app.dispatch_tool_vcb("0;2;0").unwrap();assert!(app.scene.mesh.vertices.iter().any(|v|v.position==DVec3::new(0.,3.,0.)));
    }
    #[test]fn line_midchain_undo_rewinds_vcb_origin_without_consuming_prior_document_undo(){
        let mut app=app();app.scene=Scene::new();app.active_tool=create_tool(ToolType::Line);app.active_tool_type=ToolType::Line;let ctx=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&ctx);app.dispatch_tool_vcb("2;0;0").unwrap();app.dispatch_tool_vcb("0;3;0").unwrap();assert_eq!(app.scene.mesh.edges.len(),2);app.undo();assert_eq!(app.scene.mesh.edges.len(),1);app.dispatch_tool_vcb("0;-1;0").unwrap();assert!(app.scene.mesh.vertices.iter().any(|v|v.position==DVec3::new(2.,-1.,0.)));assert!(app.scene.mesh.vertices.iter().any(|v|v.position==DVec3::new(2.,3.,0.)),"bare source AddEdge Undo retains its endpoint object");app.undo();app.undo();assert!(app.scene.mesh.edges.is_empty());app.undo();assert!(app.dispatch_tool_vcb("1;0;0").is_err());assert!(app.undo_stack.is_empty());
    }
    #[test]fn freehand_preview_cancel_release_has_one_reversible_commit(){
        let mut app=app();app.scene=Scene::new();app.active_tool=create_tool(ToolType::Freehand);app.active_tool_type=ToolType::Freehand;
        let ctx=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&ctx);app.active_tool.on_move(&mut app.scene,&ToolContext::new(DVec3::X,ctx.ray));app.active_tool.on_cancel(&mut app.scene);assert!(app.scene.mesh.edges.is_empty());assert!(app.undo_stack.is_empty());
        app.dispatch_tool_click(&ctx);for p in [DVec3::X,DVec3::X+DVec3::Y,DVec3::Y,DVec3::ZERO]{app.active_tool.on_move(&mut app.scene,&ToolContext::new(p,ctx.ray));}assert!(app.scene.mesh.edges.is_empty());app.dispatch_tool_release(&ctx,100.);assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.mesh.faces.len(),1);let after=app.scene.mesh.clone();app.undo();assert!(app.scene.mesh.edges.is_empty());app.redo();assert_eq!(app.scene.mesh,after);
    }
    #[test]
    fn fillet_sizing_preview_and_invalid_radius_do_not_create_history() {
        let mut app=app();ingetrazo_tools::pushpull::PushPullTool::extrude_face(&mut app.scene,0,1.).unwrap();
        app.active_tool=create_tool(ToolType::Fillet);app.active_tool_type=ToolType::Fillet;
        let before=app.scene.mesh.clone();
        let pick=ToolContext::new(DVec3::new(0.5,0.,1.),Ray::new(DVec3::new(0.5,-2.,1.),DVec3::Y));
        app.active_tool.on_move(&mut app.scene,&pick);app.dispatch_tool_click(&pick);assert_eq!(app.scene.mesh,before);assert!(app.undo_stack.is_empty());
        assert!(app.dispatch_tool_vcb("5").is_err());assert_eq!(app.scene.mesh,before);assert!(app.undo_stack.is_empty());
        app.dispatch_tool_vcb("0.2").unwrap();assert_eq!(app.undo_stack.len(),1);let after=app.scene.mesh.clone();assert_ne!(before,after);
        app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,after);
    }

    #[test]
    fn followme_preview_cancel_and_drag_release_history_are_atomic() {
        let mut app=app();app.scene=Scene::new();
        let vertices=[DVec3::ZERO,DVec3::X,DVec3::new(1.,0.,1.),DVec3::Z].into_iter().map(|p|app.scene.mesh.add_vertex(p)).collect();app.scene.mesh.add_face(vertices,vec![]);
        let a=app.scene.mesh.add_vertex(DVec3::ZERO);let b=app.scene.mesh.add_vertex(DVec3::new(0.,2.,0.));app.scene.mesh.add_edge(a,b);
        app.active_tool=create_tool(ToolType::FollowMe);app.active_tool_type=ToolType::FollowMe;
        let before=app.scene.mesh.clone();
        let click=ToolContext::new(DVec3::new(0.5,0.,0.5),Ray::new(DVec3::new(0.5,-2.,0.5),DVec3::Y));
        let hover=ToolContext::new(DVec3::new(0.,1.,0.),Ray::new(DVec3::new(0.,1.,2.),-DVec3::Z));
        app.dispatch_tool_click(&click);app.active_tool.on_move(&mut app.scene,&hover);app.dispatch_tool_release(&hover,2.);assert_eq!(app.scene.mesh,before);assert!(app.undo_stack.is_empty());
        app.active_tool.on_cancel(&mut app.scene);assert_eq!(app.scene.mesh,before);assert!(app.undo_stack.is_empty());
        app.dispatch_tool_click(&click);app.active_tool.on_move(&mut app.scene,&hover);app.dispatch_tool_release(&hover,10.);assert_eq!(app.undo_stack.len(),1);let after=app.scene.mesh.clone();assert_ne!(before,after);
        app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,after);
    }

    #[test]
    fn ctrl_pushpull_vcb_preserves_slab_and_commits_one_undo() {
        let mut app=app();ingetrazo_tools::pushpull::PushPullTool::extrude_face(&mut app.scene,0,3.).unwrap();
        app.active_tool=create_tool(ToolType::PushPull);app.active_tool_type=ToolType::PushPull;let before=app.scene.mesh.clone();
        let mut ctx=context(3.);ctx.ctrl_held=true;app.dispatch_tool_click(&ctx);app.active_tool.on_move(&mut app.scene,&ctx);assert_eq!(app.scene.mesh,before);assert!(app.undo_stack.is_empty());
        app.dispatch_tool_vcb("2").unwrap();assert_eq!(app.scene.mesh.faces.len(),11);assert_eq!(app.scene.mesh.faces.iter().filter(|f|f.interior).count(),1);assert_eq!(app.undo_stack.len(),1);let after=app.scene.mesh.clone();app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,after);
    }

    #[test]
    fn offset_preview_failure_cancel_and_success_have_transactional_history() {
        let mut app=app();app.active_tool=create_tool(ToolType::Offset);app.active_tool_type=ToolType::Offset;
        let before=app.scene.mesh.clone();let click=context(0.);
        app.dispatch_tool_click(&click);assert!(app.undo_stack.is_empty());
        assert!(app.dispatch_tool_vcb("NaN").is_err());assert_eq!(app.scene.mesh,before);assert!(app.undo_stack.is_empty());
        app.active_tool.on_cancel(&mut app.scene);assert_eq!(app.scene.mesh,before);
        app.dispatch_tool_click(&click);app.dispatch_tool_vcb("0.2").unwrap();assert_eq!(app.undo_stack.len(),1);let after=app.scene.mesh.clone();
        app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,after);
    }

    #[test]
    fn click_and_vcb_pushpull_each_create_one_undo_commit() {
        for vcb in [false, true] {
            let mut app = app();
            let before = app.scene.mesh.clone();
            app.dispatch_tool_click(&context(0.));
            assert!(app.undo_stack.is_empty());
            if vcb {
                app.dispatch_tool_vcb("1").unwrap();
            } else {
                app.active_tool.on_move(&mut app.scene, &context(1.));
                assert!(app.undo_stack.is_empty());
                app.dispatch_tool_click(&context(1.));
            }
            assert_eq!(app.undo_stack.len(), 1);
            assert!((app.scene.mesh.volume().unwrap() - 1.).abs() < 1e-8);
            let after = app.scene.mesh.clone();
            app.undo();
            assert_eq!(app.scene.mesh, before);
            app.redo();
            assert_eq!(app.scene.mesh, after);
        }
    }

    #[test]
    fn cancel_invalid_vcb_and_selection_do_not_create_undo_commits() {
        let mut app = app();
        let before = app.scene.mesh.clone();
        app.dispatch_tool_click(&context(0.));
        assert!(app.dispatch_tool_vcb("NaN").is_err());
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.scene.mesh, before);
        app.active_tool.on_cancel(&mut app.scene);
        assert!(app.undo_stack.is_empty());
        app.active_tool = create_tool(ToolType::Select);
        app.scene.select(SelectionItem::Face(0));
        app.dispatch_tool_click(&context(0.));
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.scene.mesh, before);
    }
    #[test]
    fn source_move_live_preview_and_other_transform_previews_commit_undo_once() {
        for (tool_type, input) in [
            (ToolType::Move, "1, 2, 3"),
            (ToolType::Scale, "2"),
            (ToolType::Rotate, "90"),
        ] {
            let mut app = app();
            app.active_tool = create_tool(tool_type);
            app.active_tool_type = tool_type;
            app.scene.select(SelectionItem::Face(0));
            let before = app.scene.mesh.clone();
            let grab=if tool_type==ToolType::Scale{ToolContext::new(DVec3::new(1.,1.,0.),Ray::new(DVec3::new(1.,1.,5.),-DVec3::Z))}else{context(0.)};
            app.dispatch_tool_click(&grab);
            let hover=if tool_type==ToolType::Scale{ToolContext::new(DVec3::new(2.,2.,0.),Ray::new(DVec3::new(2.,2.,5.),-DVec3::Z))}else{context(1.)};
            app.active_tool.on_move(&mut app.scene, &hover);
            if tool_type==ToolType::Move {
                assert!(app.scene.mesh.vertices.iter().zip(&before.vertices).all(|(v,old)|v.position==old.position+DVec3::Z));
                assert!(app.undo_stack.is_empty(),"live preview must not create undo entries");
            }else if tool_type==ToolType::Scale {
                assert!(app.scene.mesh.vertices.iter().zip(&before.vertices).all(|(v,old)|v.position==old.position*2.));
                assert!(app.undo_stack.is_empty());
            }else{assert_eq!(app.scene.mesh,before,"{tool_type:?} preview mutated model");}
            app.active_tool.on_cancel(&mut app.scene);
            assert_eq!(app.scene.mesh, before);
            assert!(app.undo_stack.is_empty());
            app.dispatch_tool_click(&grab);
            if tool_type == ToolType::Rotate {
                let arm = ToolContext::new(DVec3::X, Ray::new(DVec3::X, DVec3::Z));
                app.dispatch_tool_click(&arm);
            }
            app.dispatch_tool_vcb(input).unwrap();
            assert_ne!(
                app.scene.mesh, before,
                "{tool_type:?} did not commit geometry"
            );
            assert_eq!(app.undo_stack.len(), 1);
            let after = app.scene.mesh.clone();
            app.undo();
            assert_eq!(app.scene.mesh, before);
            app.redo();
            assert_eq!(app.scene.mesh, after);
        }
    }
    #[test]
    fn scale_grip_live_release_cancel_and_hot_retype_own_one_transaction() {
        let mut app=app();app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Scale);
        let original=app.scene.mesh.clone();let p=|x,y|ToolContext::new(DVec3::new(x,y,0.),Ray::new(DVec3::new(x,y,5.),-DVec3::Z));
        assert_eq!(app.active_tool.scale_box_state().unwrap().grips.len(),8);
        app.dispatch_tool_click(&p(1.,1.));app.dispatch_tool_move(&p(2.,2.));
        assert!(app.undo_stack.is_empty());assert!(app.active_tool.pending_model_preview());
        // A release in place keeps click-move-click alive without recording it.
        app.dispatch_tool_release(&p(2.,2.),0.);assert!(app.undo_stack.is_empty());
        app.dispatch_tool_cancel();assert_eq!(app.scene.mesh,original);
        app.dispatch_tool_click(&p(1.,1.));app.dispatch_tool_move(&p(2.,2.));
        app.dispatch_tool_release(&p(2.,2.),8.);assert_eq!(app.undo_stack.len(),1);
        assert!(!app.active_tool.pending_model_preview());
        app.dispatch_tool_vcb("3").unwrap();assert_eq!(app.undo_stack.len(),1);
        assert_eq!(app.scene.mesh.vertices[2].position,DVec3::new(3.,3.,0.));
        app.undo();assert_eq!(app.scene.mesh,original);app.redo();
        assert_eq!(app.scene.mesh.vertices[2].position,DVec3::new(3.,3.,0.));
        assert!(app.dispatch_tool_vcb("4").is_err(),"Redo closed the original retype window");
    }
    #[test]
    fn scale_modifier_taps_reanchor_and_invalid_values_preserve_live_preview() {
        let mut app=app();app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Scale);
        let p=|x,y|ToolContext::new(DVec3::new(x,y,0.),Ray::new(DVec3::new(x,y,5.),-DVec3::Z));
        app.dispatch_tool_click(&p(1.,0.5));assert_eq!(app.active_tool.vcb_label(),"Red Scale");
        app.dispatch_tool_move(&p(2.,0.5));assert_eq!(app.scene.mesh.vertices[2].position,DVec3::new(2.,1.,0.));
        app.dispatch_tool_control_state(true);let centered=app.scene.mesh.clone();
        assert_eq!(centered.vertices[0].position,DVec3::new(-0.5,0.,0.));
        app.dispatch_tool_control_state(true);assert_eq!(app.scene.mesh,centered);
        assert!(app.dispatch_tool_vcb("0").is_err());assert_eq!(app.scene.mesh,centered);assert!(app.undo_stack.is_empty());
        app.active_tool.note_shift_state(true);app.active_tool.note_shift_state(false);
        assert_eq!(app.active_tool.vcb_label(),"Scale");
        app.dispatch_tool_vcb("2m").unwrap();assert_eq!(app.undo_stack.len(),1);
        assert_eq!(app.scene.mesh.vertices[0].position,DVec3::new(-0.5,-0.5,0.));
    }
    #[test]
    fn scale_loose_partial_warp_autofolds_and_retype_restores_mother_before_folding() {
        let mut app=app();let other=app.scene.mesh.add_vertex(DVec3::new(2.,0.,1.));
        let edge=app.scene.mesh.add_edge(0,other).unwrap();app.scene.select(SelectionItem::Edge(edge));
        app.scene.mesh.faces[0].attrs.insert("vendor".into(),serde_json::json!(71));
        let before=app.scene.mesh.clone();app.set_tool(ToolType::Scale);
        app.dispatch_tool_click(&ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z)));
        app.dispatch_tool_vcb("2").unwrap();assert_eq!(app.scene.mesh.faces.len(),2);
        assert!(app.scene.mesh.faces.iter().all(|f|f.attrs.get("vendor")==Some(&serde_json::json!(71))));
        app.dispatch_tool_vcb("3").unwrap();assert_eq!(app.scene.mesh.faces.len(),2);assert_eq!(app.undo_stack.len(),1);
        assert_eq!(app.scene.mesh.vertices[0].position,DVec3::new(-4.,0.,-2.));
        let after=app.scene.mesh.clone();app.undo();assert_eq!(app.scene.mesh,before);app.redo();assert_eq!(app.scene.mesh,after);
    }

    #[test]
    fn scale_shared_repeated_overflow_refuses_before_any_owner_mutates() {
        use ingetrazo_core::{groups::Group, mesh::Mesh};
        let mut app = IngeTrazoApp::new();
        app.scene = Scene::new();
        let mut mesh = Mesh::new();
        let ids = [DVec3::ZERO,DVec3::X,DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(),vec![]);
        let token = uuid::Uuid::new_v4();
        app.scene.register_mesh_share(token,mesh.clone()).unwrap();
        for name in ["first","second"] {
            let mut group = Group::with_mesh(name,mesh.clone());
            group.mesh_share=Some(token);
            app.scene.groups.push(group);
        }
        app.scene.select(SelectionItem::Group(0));
        app.scene.select(SelectionItem::Group(1));
        app.set_tool(ToolType::Scale);
        app.dispatch_tool_click(&ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z*5.,-DVec3::Z)));
        let before=app.scene.clone();
        assert!(app.dispatch_tool_vcb("1e20").unwrap_err().contains("finite coordinate"));
        assert_eq!(app.scene,before);
        assert!(app.undo_stack.is_empty());
        app.dispatch_tool_vcb("2").unwrap();
        let accepted=app.scene.clone();
        assert!(app.dispatch_tool_vcb("1e20").unwrap_err().contains("finite coordinate"));
        assert_eq!(app.scene,accepted);
        assert_eq!(app.undo_stack.len(),1);
        app.dispatch_tool_vcb("3").unwrap();
        assert_eq!(app.undo_stack.len(),1);
        app.undo();
        assert_eq!(app.scene.groups[0].mesh,mesh);
        assert_eq!(app.scene.groups[1].mesh,mesh);
        assert_eq!(app.scene.mesh_shares[&token],mesh);
    }

    #[test]
    fn scale_refused_hot_value_preserves_command_and_next_transaction() {
        let mut app=app();app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Scale);
        let p=|x|ToolContext::new(DVec3::new(x,x,0.),Ray::new(DVec3::Z,-DVec3::Z));
        app.dispatch_tool_click(&p(1.));app.dispatch_tool_vcb("2").unwrap();
        let revision=app.model_revision;assert!(app.dispatch_tool_vcb("0").is_err());assert_eq!(app.model_revision,revision);
        app.dispatch_tool_vcb("3").unwrap();assert_eq!(app.undo_stack.len(),1);
        app.dispatch_tool_vcb("3").unwrap(); // same value must retire replacement flag
        app.dispatch_tool_click(&p(3.));app.dispatch_tool_vcb("2").unwrap();
        assert_eq!(app.undo_stack.len(),2);assert_eq!(app.scene.mesh.vertices[2].position,DVec3::new(6.,6.,0.));
    }
    #[test]
    fn scale_placed_group_preview_changes_pose_revision_only() {
        let mut app=app();let mesh=std::mem::take(&mut app.scene.mesh);
        let mut group=ingetrazo_core::groups::Group::with_mesh("placed",mesh.clone());
        group.native_document=serde_json::json!({"xform":glam::DMat4::IDENTITY.to_cols_array()});
        app.scene.groups.push(group);app.scene.select(SelectionItem::Group(0));app.set_tool(ToolType::Scale);
        app.dispatch_tool_click(&ToolContext::new(DVec3::new(1.,1.,0.),Ray::new(DVec3::Z,-DVec3::Z)));
        let before=app.native_render_revisions;
        app.dispatch_tool_move(&ToolContext::new(DVec3::new(2.,2.,0.),Ray::new(DVec3::Z,-DVec3::Z)));
        assert_eq!(app.native_render_revisions.static_domains.geometry,before.static_domains.geometry);
        assert!(app.native_render_revisions.poses>before.poses);assert_eq!(app.scene.groups[0].mesh,mesh);
        app.dispatch_tool_control_state(true);app.dispatch_tool_cancel();
        assert_eq!(app.native_render_revisions.static_domains.geometry,before.static_domains.geometry);
        assert_eq!(app.scene.groups[0].transform,glam::DMat4::IDENTITY);
    }

    #[test]
    fn scale_mirror_rekeys_exchanged_vertices_without_losing_lookup_owners() {
        let mut app=app();app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Scale);
        app.dispatch_tool_click(&ToolContext::new(DVec3::new(1.,1.,0.),Ray::new(DVec3::Z,-DVec3::Z)));
        app.dispatch_tool_control_state(true);app.dispatch_tool_vcb("-1").unwrap();
        for (i,v) in app.scene.mesh.vertices.iter().enumerate(){assert_eq!(app.scene.mesh.vertex_at_position(v.position),Some(i));}
        app.undo();app.redo();
        for (i,v) in app.scene.mesh.vertices.iter().enumerate(){assert_eq!(app.scene.mesh.vertex_at_position(v.position),Some(i));}
    }

    #[test]
    fn scale_image_hot_retype_updates_relative_undo_factor() {
        let mut app=IngeTrazoApp::new();app.scene=Scene::new();
        app.scene.image_planes.push(ingetrazo_core::image_plane::ImagePlane::new("scan",DVec3::ZERO,DVec3::X,DVec3::Y));
        app.scene.select(SelectionItem::ImagePlane(0));app.set_tool(ToolType::Scale);
        let image=app.scene.image_planes[0].clone();
        app.dispatch_tool_click(&ToolContext::new(DVec3::new(1.,1.,0.),Ray::new(DVec3::Z,-DVec3::Z)));
        app.dispatch_tool_vcb("2").unwrap();app.dispatch_tool_vcb("3").unwrap();
        assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.image_planes[0].u,DVec3::X*3.);
        app.undo();assert_eq!(app.scene.image_planes[0].u,image.u);app.redo();assert_eq!(app.scene.image_planes[0].u,DVec3::X*3.);
    }

    #[test]
    fn failed_path_sweep_reports_error_without_document_or_history_mutation() {
        let mut app = app();
        app.active_tool = create_tool(ToolType::FollowMe);
        app.active_tool_type = ToolType::FollowMe;
        let points = [
            DVec3::new(0., 0., 1.),
            DVec3::new(0., 0., 2.),
            DVec3::new(1., 0., 1.),
            DVec3::new(-1., 0., 1.),
        ];
        let ids: Vec<_> = points
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        for target in &ids[1..] {
            let id = app.scene.mesh.add_edge(ids[0], *target).unwrap();
            app.scene.select(SelectionItem::Edge(id));
        }
        let before = app.scene.clone();
        app.dispatch_tool_click(&context(0.));
        assert!(app.status_message.contains("Path cannot branch"));
        assert_eq!(app.scene, before);
        assert!(app.undo_stack.is_empty());
    }
}

#[cfg(test)]mod section_group_workflow{use crate::IngeTrazoApp;use glam::{DVec3,DMat4};use ingetrazo_core::{groups::Group,mesh::Mesh,math::Ray,scene::SelectionItem,section::SectionPlane};use ingetrazo_tools::base::ToolContext;#[test]fn clipped_front_group_does_not_block_rear_group_select_and_double_click(){let mut app=IngeTrazoApp::new();let mut mesh=Mesh::new();let ids=[DVec3::new(-1.,-1.,0.),DVec3::new(1.,-1.,0.),DVec3::new(0.,1.,0.)].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]);let mut child=Group::with_mesh("child",mesh.clone());child.transform=DMat4::from_translation(-DVec3::Z);let mut front=Group::new("front");front.transform=DMat4::from_translation(DVec3::Z*2.);front.children.push(child);let mut rear=Group::with_mesh("rear",mesh);rear.transform=DMat4::from_translation(-DVec3::Z);app.scene.groups.extend([front,rear]);app.scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::Z,"A","A"));app.scene.show_section_cuts=true;app.scene.show_section_planes=false;let context=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z*3.,-DVec3::Z));app.dispatch_tool_click(&context);assert!(app.scene.selection.contains(&SelectionItem::Group(1)));assert!(app.undo_stack.is_empty());app.dispatch_tool_double_click(&context);assert_eq!(app.component_edit_stack.len(),1);assert_eq!(app.scene.mesh.faces.len(),1);assert!(app.scene.groups.is_empty());app.cancel_component_edit();app.scene.show_section_cuts=false;app.dispatch_tool_click(&context);assert!(app.scene.selection.contains(&SelectionItem::Group(0)));assert!(app.undo_stack.is_empty());}}

#[cfg(test)]mod image_revision_tests {
 use super::*;use ingetrazo_core::scene::SelectionItem;use ingetrazo_tools::base::ToolType;
 #[test]fn image_only_preview_invalidates_document_without_geometry_cache(){
  let mut app=IngeTrazoApp::new();app.system_clipboard=None;
  app.scene.image_planes.push(ingetrazo_core::image_plane::ImagePlane::new("missing",glam::DVec3::ZERO,glam::DVec3::X,glam::DVec3::Y));app.scene.select(SelectionItem::ImagePlane(0));app.set_tool(ToolType::Move);
  let ctx=|p|ToolContext::new(p,ingetrazo_core::math::Ray::new(glam::DVec3::new(0.5,0.5,10.),-glam::DVec3::Z));
  app.dispatch_tool_click(&ctx(glam::DVec3::ZERO));let geometry=app.native_geometry_revision;let document=app.model_revision;let version=app.scene.version;
  app.dispatch_tool_move(&ctx(glam::DVec3::X));assert_eq!(app.scene.version,version);assert_eq!(app.native_geometry_revision,geometry);assert!(app.model_revision>document);
  let document=app.model_revision;app.dispatch_tool_move(&ctx(glam::DVec3::X));assert_eq!(app.model_revision,document);
  app.scene.mesh.add_vertex(glam::DVec3::ZERO);app.scene.clear_selection();app.scene.select(SelectionItem::Vertex(0));app.set_tool(ToolType::Move);app.dispatch_tool_click(&ctx(glam::DVec3::ZERO));let geometry=app.native_geometry_revision;app.dispatch_tool_move(&ctx(glam::DVec3::Y));assert!(app.native_geometry_revision>geometry);
 }
}

#[cfg(test)]
mod annotation_move_tests {
    use crate::IngeTrazoApp;
    use glam::DVec3;
    use ingetrazo_core::{guides::Guide, scene::SelectionItem, textlabel::TextLabel};
    use ingetrazo_tools::base::{ToolContext, ToolType};
    fn context() -> ToolContext {
        ToolContext::new(
            DVec3::ZERO,
            ingetrazo_core::math::Ray::new(DVec3::Z, -DVec3::Z),
        )
    }
    #[test]
    fn annotation_move_refuses_stale_or_locked_before_mutation() {
        for stale in [false, true] {
            let mut a = IngeTrazoApp::new();
            a.system_clipboard = None;
            a.scene
                .text_labels
                .push(TextLabel::new(DVec3::ZERO, DVec3::X, "held"));
            a.scene.select(SelectionItem::TextLabel(0));
            if stale {
                a.scene.select(SelectionItem::Dimension(99));
            } else {
                a.scene.layers[0].locked = true;
            }
            a.set_tool(ToolType::Move);
            let before = a.scene.clone();
            let revision = a.model_revision;
            a.dispatch_tool_click(&context());
            assert_eq!(a.scene, before);
            assert_eq!(a.model_revision, revision);
            assert!(a.undo_stack.is_empty());
            assert!(a.active_tool.snap_anchor().is_none());
            assert!(!a.status_message.is_empty());
        }
    }
    #[test]
    fn annotation_move_guide_only_is_source_no_acquisition() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene.guides.push(Guide::point(DVec3::ZERO));
        a.scene.select(SelectionItem::Guide(0));
        a.set_tool(ToolType::Move);
        let before = a.scene.clone();
        a.dispatch_tool_click(&context());
        let mut hover = context();
        hover.world = DVec3::X;
        a.dispatch_tool_move(&hover);
        a.dispatch_tool_click(&hover);
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
        assert!(a.active_tool.snap_anchor().is_none());
    }
    #[test]
    fn annotation_move_subthreshold_commit_has_no_history() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene
            .text_labels
            .push(TextLabel::new(DVec3::ZERO, DVec3::ZERO, "tiny"));
        a.scene.select(SelectionItem::TextLabel(0));
        a.set_tool(ToolType::Move);
        let before = a.scene.text_labels.clone();
        a.dispatch_tool_click(&context());
        let mut end = context();
        end.world = DVec3::new(1e-10, 0., 0.);
        a.dispatch_tool_move(&end);
        a.dispatch_tool_click(&end);
        assert_eq!(a.scene.text_labels, before);
        assert!(a.undo_stack.is_empty());
    }
}
