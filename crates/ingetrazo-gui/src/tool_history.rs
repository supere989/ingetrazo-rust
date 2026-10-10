// SPDX-License-Identifier: GPL-3.0-or-later
//! Tool commits share one undo transaction whether entered by click or VCB.
use crate::app::IngeTrazoApp;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::base::ToolContext;

fn changed_model(before: &Scene, after: &Scene) -> bool {
    before.mesh != after.mesh
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

impl IngeTrazoApp {
    fn tool_snapshot(&self) -> Scene {
        let mut before = self.scene.clone();
        before.camera = self.camera.clone();
        self.active_tool.restore_transient_model(&mut before);
        before
    }

    fn record_tool_commit(&mut self, before: Scene) -> Result<(),String> {
        if before.mesh != self.scene.mesh {
            if let Err(error)=ingetrazo_core::heal::refresh_interior(&mut self.scene.mesh) {
                self.scene=before;self.bump_model_revision();
                self.status_message=format!("Tool error: {error}");
                return Err(error);
            }
        }
        let transient_changed=before.version!=self.scene.version;
        let committed=changed_model(&before,&self.scene);
        if committed {
            self.seal_view_history_before(&before);self.seal_terrain_history_before(&before);
            self.bump_model_revision();
            if !self.active_tool.take_history_replacement()||self.undo_stack.is_empty(){self.undo_stack.push(before);}
            self.active_tool.note_history_revision(self.model_revision,true);
            self.redo_stack.clear();
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
        if transient_changed&&!committed{self.bump_model_revision();}
        Ok(())
    }

    pub fn dispatch_tool_move(&mut self,context:&ToolContext){
        let version=self.scene.version;self.active_tool.on_move(&mut self.scene,context);
        if self.scene.version!=version{self.bump_model_revision();}
    }

    pub fn dispatch_tool_click(&mut self, context: &ToolContext) {
        if self.section_name_prompt.is_some(){return;}
        let plane_count=self.scene.section_planes.len();
        let before = self.tool_snapshot();
        self.active_tool.on_click(&mut self.scene, context);
        self.status_message = self
            .active_tool
            .error_message()
            .map(|error| format!("Tool error: {error}"))
            .unwrap_or_default();
        if self.active_tool.error_message().is_some(){
            self.scene=before;self.active_tool.restore_current_preview_model(&mut self.scene);self.bump_model_revision();return;
        }
        if self.record_tool_commit(before).is_ok()&&self.scene.section_planes.len()>plane_count{self.queue_section_name_prompt();}
    }

    pub fn dispatch_tool_release(&mut self, context: &ToolContext, drag_distance: f64) {
        if self.section_name_prompt.is_some(){return;}
        let before = self.tool_snapshot();
        self.active_tool.on_release(&mut self.scene, context, drag_distance);
        self.status_message = self.active_tool.error_message().map(|e| format!("Tool error: {e}")).unwrap_or_default();
        let _ = self.record_tool_commit(before);
    }

    pub fn dispatch_tool_double_click(&mut self, context: &ToolContext) {
        if self.section_name_prompt.is_some(){return;}
        let before = self.tool_snapshot();
        self.active_tool.on_double_click(&mut self.scene, context);
        self.status_message = self
            .active_tool
            .error_message()
            .map(|error| format!("Tool error: {error}"))
            .unwrap_or_default();
        if self.record_tool_commit(before).is_ok()&&self.active_tool_type==ingetrazo_tools::base::ToolType::Select&&self.scene.selection.len()==1&&self.scene.selection.iter().any(|item|matches!(item,ingetrazo_core::scene::SelectionItem::Group(_))){self.begin_component_edit();}
    }

    pub fn dispatch_tool_vcb(&mut self, text: &str) -> Result<(), String> {
        if self.section_name_prompt.is_some(){return Err("Finish the section naming dialog first".into());}
        let before = self.tool_snapshot();
        self.active_tool.note_history_revision(self.model_revision,false);
        match self.active_tool.on_vcb_input(&mut self.scene, text) {
            Ok(()) => {
                self.record_tool_commit(before)
            }
            Err(error) => {
                self.scene = before;self.active_tool.restore_current_preview_model(&mut self.scene);self.bump_model_revision();
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
        let mut app=app();app.scene=Scene::new();app.active_tool=create_tool(ToolType::Line);app.active_tool_type=ToolType::Line;let ctx=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&ctx);app.dispatch_tool_vcb("2;0;0").unwrap();app.dispatch_tool_vcb("0;3;0").unwrap();assert_eq!(app.scene.mesh.edges.len(),2);app.undo();assert_eq!(app.scene.mesh.edges.len(),1);app.dispatch_tool_vcb("0;-1;0").unwrap();assert!(app.scene.mesh.vertices.iter().any(|v|v.position==DVec3::new(2.,-1.,0.)));assert!(!app.scene.mesh.vertices.iter().any(|v|v.position==DVec3::new(2.,3.,0.)));app.undo();app.undo();assert!(app.scene.mesh.edges.is_empty());app.undo();assert!(app.dispatch_tool_vcb("1;0;0").is_err());assert!(app.undo_stack.is_empty());
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
            app.dispatch_tool_click(&context(0.));
            app.active_tool.on_move(&mut app.scene, &context(1.));
            if tool_type==ToolType::Move {
                assert!(app.scene.mesh.vertices.iter().zip(&before.vertices).all(|(v,old)|v.position==old.position+DVec3::Z));
                assert!(app.undo_stack.is_empty(),"live preview must not create undo entries");
            }else{assert_eq!(app.scene.mesh,before,"{tool_type:?} preview mutated model");}
            app.active_tool.on_cancel(&mut app.scene);
            assert_eq!(app.scene.mesh, before);
            assert!(app.undo_stack.is_empty());
            app.dispatch_tool_click(&context(0.));
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
