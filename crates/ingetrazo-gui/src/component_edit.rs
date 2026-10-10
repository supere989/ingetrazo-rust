//! Source-compatible world-coordinate group editing with atomic document history.
use ingetrazo_core::{camera::OrbitCamera,components::ComponentDefinition,scene::{Scene,SelectionItem}};
use ingetrazo_tools::base::ToolType;
use crate::app::IngeTrazoApp;

pub struct ComponentEditSession {
    original_scene: Scene,
    original_camera: OrbitCamera,
    original_undo: Vec<Scene>,
    original_redo: Vec<Scene>,
    group_id: uuid::Uuid,
    private_container_mesh: Option<uuid::Uuid>,
    container_bake: Option<glam::DMat4>,
    container_axes: Option<glam::DMat4>,
    leaf_share_back: Option<glam::DMat4>,
    mesh_baseline: ingetrazo_core::mesh::Mesh,
    groups_baseline: Vec<ingetrazo_core::groups::Group>,
    mesh_share_baseline: std::collections::HashMap<uuid::Uuid,ingetrazo_core::mesh::Mesh>,
    definitions_baseline: std::collections::HashMap<uuid::Uuid,ComponentDefinition>,
    editing_definition: bool,
    dimension_baseline: Vec<ingetrazo_core::dimensions::Dimension>,
    section_baseline: Vec<ingetrazo_core::section::SectionPlane>,
}

impl IngeTrazoApp {
    pub fn make_component(&mut self) {
        let selected:Vec<_>=self.scene.selection.iter().filter_map(|s|if let SelectionItem::Group(i)=s {Some(*i)} else {None}).collect();
        let index=if selected.len()==1 && self.scene.selection.len()==1 {
            let i=selected[0];
            if i>=self.scene.groups.len() {return;}
            if self.scene.groups[i].component_definition.is_some() {
                self.status_message="Selection is already a component".into();return;
            }
            self.save_undo_state();i
        } else {
            let old_version=self.scene.version;self.make_group();
            if self.scene.version==old_version {return;}
            self.scene.groups.len()-1
        };
        let id=self.scene.groups[index].id;
        match self.scene.make_component(id) {
            Ok(_)=>{self.scene.clear_selection();self.scene.select(SelectionItem::Group(index));self.status_message="Created linked component".into();},
            Err(e)=>self.status_message=e,
        }
    }

    pub fn make_component_unique(&mut self) {
        let selected:Vec<_>=self.scene.selection.iter().filter_map(|s|if let SelectionItem::Group(i)=s {Some(*i)} else {None}).collect();
        if selected.len()!=1 {self.status_message="Select one component".into();return;}
        let Some(group)=self.scene.groups.get(selected[0]) else {return;};
        let id=group.id;
        if group.component_definition.is_none() {self.status_message="Selection is an independent group".into();return;}
        let before=self.scene.clone();
        match self.scene.make_component_unique(id) {
            Ok(_)=>{self.undo_stack.push(before);self.redo_stack.clear();self.status_message="Component made unique".into();},
            Err(e)=>self.status_message=e,
        }
    }

    pub fn begin_component_edit(&mut self) {
        let selected:Vec<_>=self.scene.selection.iter().filter_map(|s|if let SelectionItem::Group(i)=s {Some(*i)} else {None}).collect();
        if selected.len()!=1 {self.status_message="Select one group or component to edit".into();return;}
        let Some(group)=self.scene.groups.get(selected[0]).cloned() else {return;};
        if group.transform.determinant().abs()<1e-12 {
            self.status_message="Cannot edit a singular group placement".into();return;
        }
        self.set_tool(ToolType::Select);
        // Retiring Move may restore its transient placement. Capture the edit
        // from the restored entity, not the validated live-preview clone.
        let Some(group)=self.scene.groups.iter().find(|item|item.id==group.id).cloned() else {return;};
        if let Err(error)=self.scene.migrate_legacy_mesh_shares() {self.status_message=error;return;}
        // Migration may add identities but does not replace placement geometry.
        let Some(group)=self.scene.groups.iter().find(|item|item.id==group.id).cloned() else {return;};
        // Operative Viewport.begin_group_edit: an ordinary placed leaf
        // copied outside a component definition becomes unique before entry.
        // Mesh tokens establish sharing; equal geometry never does.
        let inside_definition=self.component_edit_stack.last().is_some_and(|s|s.editing_definition);
        let component=group.component_definition.is_some() || group.native_document.get("component").and_then(|v|v.as_bool())==Some(true);
        let placed=group.transform!=glam::DMat4::IDENTITY || group.component_definition.is_some() || group.native_document.get("xform").is_some_and(|v|v.is_array());
        fn shared_placement(groups:&[ingetrazo_core::groups::Group],token:uuid::Uuid,id:uuid::Uuid,skip_context:Option<uuid::Uuid>)->bool {
            groups.iter().any(|g|Some(g.id)!=skip_context && ((g.id!=id && g.mesh_share==Some(token)) || shared_placement(&g.children,token,id,skip_context)))
        }
        let shared=group.mesh_share.is_some_and(|token|shared_placement(&self.scene.groups,token,group.id,None)
            || self.component_edit_stack.iter().any(|s|shared_placement(&s.original_scene.groups,token,group.id,Some(s.group_id))));
        if placed && !component && group.children.is_empty() && !inside_definition && shared {
            let world=self.scene.dimension_frame*group.transform;
            let mesh=match ingetrazo_core::containers::transform_mesh(&group.mesh,world) {Ok(mesh)=>mesh,Err(error)=>{self.status_message=error;return;}};
            let axes=world*group.axes_matrix().unwrap_or(glam::DMat4::IDENTITY);
            self.save_undo_state();
            let selected=self.scene.groups.iter_mut().find(|g|g.id==group.id).unwrap();
            selected.mesh=mesh;selected.mesh_share=None;selected.transform=glam::DMat4::IDENTITY;
            if !selected.native_document.is_object() {selected.native_document=serde_json::json!({});}
            selected.native_document["xform"]=serde_json::Value::Null;
            selected.native_document["axes"]=serde_json::json!(axes.to_cols_array());
            // An explicitly baked classic placement encodes xform=null. Its
            // model comparison baseline is now identity, keeping that source
            // representation while leaving original prototype identity intact
            // for codec detachment and the Undo snapshot.
            if !selected.native_baseline.is_object() {selected.native_baseline=serde_json::json!({});}
            selected.native_baseline["xform"]=serde_json::json!(glam::DMat4::IDENTITY.to_cols_array());
            self.scene.bump_version();self.bump_model_revision();
        }
        let Some(group)=self.scene.groups.iter().find(|item|item.id==group.id).cloned() else {return;};
        self.scene.refresh_dimensions();
        let world_frame=self.scene.dimension_frame*group.transform;
        let world_mesh=if world_frame==glam::DMat4::IDENTITY {group.mesh.clone()} else {
            match ingetrazo_core::containers::transform_mesh(&group.mesh,world_frame) {
                Ok(mesh)=>mesh,Err(error)=>{self.status_message=error;return;}
            }
        };
        let mut world_children=group.children.clone();
        for child in &mut world_children {child.transform=world_frame*child.transform;}
        let mut session=ComponentEditSession{
            original_scene:self.scene.clone(),original_camera:self.camera.clone(),
            original_undo:std::mem::take(&mut self.undo_stack),original_redo:std::mem::take(&mut self.redo_stack),
            group_id:group.id,
            private_container_mesh:if !group.children.is_empty() && !group.mesh.vertices.is_empty() && self.scene.dimension_frame*group.transform!=glam::DMat4::IDENTITY {Some(uuid::Uuid::new_v4())} else {None},
            container_axes:if !group.children.is_empty() && (group.component_definition.is_some() || group.transform!=glam::DMat4::IDENTITY || group.axes_matrix().is_some() || group.native_document.get("xform").is_some_and(|value|value.is_array())) {Some(self.scene.dimension_frame*group.transform*group.axes_matrix().unwrap_or(glam::DMat4::IDENTITY))} else {None},
            container_bake:if !group.children.is_empty() && self.scene.dimension_frame*group.transform!=glam::DMat4::IDENTITY {Some(self.scene.dimension_frame*group.transform)} else {None},
            leaf_share_back:if group.children.is_empty() {Some(world_frame.inverse())} else {None},
            mesh_baseline:world_mesh.clone(),groups_baseline:world_children.clone(),
            mesh_share_baseline:self.scene.mesh_shares.clone(),definitions_baseline:self.scene.component_definitions.clone(),
            editing_definition:component,
            dimension_baseline:vec![],section_baseline:vec![],
        };
        let drawing_frame=if group.component_definition.is_some() || group.transform!=glam::DMat4::IDENTITY || group.axes_matrix().is_some() || group.native_document.get("xform").is_some_and(|value|value.is_array()) {
            Some(self.scene.dimension_frame*group.transform*group.axes_matrix().unwrap_or(glam::DMat4::IDENTITY))
        } else {None};
        let mut edit=Scene::new();
        edit.mesh=world_mesh;edit.groups=world_children;
        edit.materials=self.scene.materials.clone();edit.layers=self.scene.layers.clone();
        edit.unit=self.scene.unit;edit.embedded_textures=self.scene.embedded_textures.clone();
        edit.component_definitions=self.scene.component_definitions.clone();
        edit.mesh_shares=self.scene.mesh_shares.clone();
        // Original Scene.begin_group_edit exposes WORLD geometry and leaves
        // the viewport camera unchanged. Drawing axes remain the raw frame read
        // before the placement bake, independent of the active coordinate map.
        edit.dimension_frame=glam::DMat4::IDENTITY;
        edit.drawing_frame=drawing_frame;
        edit.dimensions=self.scene.dimensions.clone();
        edit.section_planes=self.scene.section_planes.clone();edit.show_section_cuts=self.scene.show_section_cuts;edit.show_section_planes=self.scene.show_section_planes;edit.show_section_fill=self.scene.show_section_fill;session.section_baseline=edit.section_planes.clone();
        edit.bind_dimensions();session.dimension_baseline=edit.dimensions.clone();
        edit.camera=self.camera.clone();
        self.scene=edit;self.bump_model_revision();self.component_edit_stack.push(session);
        self.status_message="Editing group; Finish commits, Cancel restores".into();
    }

    pub fn finish_component_edit(&mut self) {
        if self.component_edit_stack.is_empty() {self.status_message="No group edit is active".into();return;}
        // A group schema currently has mesh and child containers, not document
        // annotations. Refuse unsupported new local annotations instead of losing them.
        if !self.scene.guides.is_empty() || !self.scene.text_labels.is_empty()
            || !self.scene.geo_points.is_empty() || !self.scene.geo_paths.is_empty()
            || self.scene.geo_datum.is_some() || self.scene.terrain.is_some() || !self.scene.sheets.is_empty()
            || !self.scene.saved_views.is_empty() {
            self.status_message="Group annotations are not supported; remove them or cancel the edit".into();return;
        }
        self.set_tool(ToolType::Select);
        let session=self.component_edit_stack.last().unwrap();
        let mut candidate=session.original_scene.clone();
        let Some(group_index)=candidate.groups.iter().position(|g|g.id==session.group_id) else {
            self.status_message="Edited instance no longer exists".into();return;
        };
        let changed=session.mesh_baseline!=self.scene.mesh || session.groups_baseline!=self.scene.groups
            || candidate.materials!=self.scene.materials || candidate.layers!=self.scene.layers
            || session.definitions_baseline!=self.scene.component_definitions
            || session.mesh_share_baseline!=self.scene.mesh_shares
            || self.scene.dimensions!=session.dimension_baseline || self.scene.section_planes!=session.section_baseline
            || self.scene.show_section_cuts!=candidate.show_section_cuts||self.scene.show_section_planes!=candidate.show_section_planes||self.scene.show_section_fill!=candidate.show_section_fill;
        if changed || session.container_axes.is_some() || session.container_bake.is_some() || session.private_container_mesh.is_some() {
            let mut replacement=ComponentDefinition{mesh_share:None,mesh:self.scene.mesh.clone(),children:self.scene.groups.clone()};
            if let Some(inverse)=session.leaf_share_back {
                // Original placed leaves edit a world copy, then inverse-map
                // only that mesh into their shared local prototype on exit.
                replacement.mesh=match ingetrazo_core::containers::transform_mesh(&replacement.mesh,inverse) {Ok(mesh)=>mesh,Err(error)=>{self.status_message=error;return;}};
            }
            if session.container_bake.is_some() {
                // Original container bake happens outside History on entry.
                // Its world mesh/child placements already form the edit view;
                // Finish persists that representation without a second bake.
                candidate.groups[group_index].transform=candidate.dimension_frame.inverse();
            }
            if let Some(axes)=session.container_axes {
                if !candidate.groups[group_index].native_document.is_object() {candidate.groups[group_index].native_document=serde_json::json!({});}
                candidate.groups[group_index].native_document["axes"]=serde_json::json!(axes.to_cols_array());
            }
            // Mesh publication and placement authoring are separate operations:
            // the original container owns its child list, while placed leaves
            // retain explicit prototype sharing (even component=false).
            let mut meshes=std::collections::HashMap::new();
            fn collect(groups:&[ingetrazo_core::groups::Group],baseline:&std::collections::HashMap<uuid::Uuid,ingetrazo_core::mesh::Mesh>,changed:&mut std::collections::HashMap<uuid::Uuid,ingetrazo_core::mesh::Mesh>)->Result<(),String> {
                for group in groups {
                    if let Some(token)=group.mesh_share {
                        if baseline.get(&token)!=Some(&group.mesh) {
                            if changed.get(&token).map(|m|m!=&group.mesh).unwrap_or(false) {return Err("Conflicting edits to shared mesh".into());}
                            changed.insert(token,group.mesh.clone());
                        }
                    }
                    collect(&group.children,baseline,changed)?;
                }
                Ok(())
            }
            if let Err(error)=collect(&self.scene.groups,&candidate.mesh_shares,&mut meshes) {self.status_message=error;return;}
            for (token,mesh) in &self.scene.mesh_shares {
                if candidate.mesh_shares.get(token)!=Some(mesh) {meshes.insert(*token,mesh.clone());}
            }
            let root_token=session.private_container_mesh.or(candidate.groups[group_index].mesh_share);
            if session.private_container_mesh.is_some() {candidate.groups[group_index].mesh_share=root_token;}
            if let Some(token)=root_token {
                if session.private_container_mesh.is_some() || candidate.groups[group_index].mesh!=replacement.mesh {meshes.insert(token,replacement.mesh.clone());}
            }
            // Import newly authored identities, never infer them from equal meshes.
            for (token,mesh) in &meshes {
                if !candidate.mesh_shares.contains_key(token) {
                    if let Err(error)=candidate.register_mesh_share(*token,mesh.clone()) {self.status_message=error;return;}
                }
            }
            for (id,def) in &self.scene.component_definitions {
                candidate.component_definitions.entry(*id).or_insert_with(||def.clone());
            }
            candidate.groups[group_index].mesh=replacement.mesh;
            candidate.groups[group_index].children=replacement.children;
            for (token,mesh) in meshes {
                if let Err(error)=candidate.publish_mesh_share(token,mesh) {self.status_message=error;return;}
            }
            candidate.bump_version();
            candidate.materials=self.scene.materials.clone();candidate.layers=self.scene.layers.clone();
            if self.scene.dimensions!=session.dimension_baseline {
                let transform=candidate.dimension_frame.inverse();
                candidate.dimensions=self.scene.dimensions.clone();
                for dimension in &mut candidate.dimensions{
                    dimension.start=transform.transform_point3(dimension.start);dimension.end=transform.transform_point3(dimension.end);dimension.offset=transform.transform_vector3(dimension.offset);dimension.anchor_a=None;dimension.anchor_b=None;dimension.measurement_frame=candidate.dimension_frame;
                }
                candidate.bind_dimensions();
            }
            if self.scene.section_planes!=session.section_baseline{let transform=candidate.dimension_frame.inverse();candidate.section_planes=self.scene.section_planes.clone();for plane in &mut candidate.section_planes{plane.point=transform.transform_point3(plane.point);plane.normal=transform.inverse().transpose().transform_vector3(plane.normal).normalize();}}
            candidate.show_section_cuts=self.scene.show_section_cuts;candidate.show_section_planes=self.scene.show_section_planes;candidate.show_section_fill=self.scene.show_section_fill;
            candidate.refresh_dimensions();
        }
        candidate.clear_selection();
        let session=self.component_edit_stack.pop().unwrap();
        if !changed {
            if let Some(parent)=self.component_edit_stack.last_mut() {
                // A child entry bake is outside the original command History.
                // Retire only its representation delta from the parent's edit
                // baseline. Never hide a parent command already changing that
                // child, or an independently authored prototype/definition.
                if let (Some(before),Some(after),Some(baseline))=(
                    session.original_scene.groups.iter().find(|g|g.id==session.group_id),
                    candidate.groups.iter().find(|g|g.id==session.group_id),
                    parent.groups_baseline.iter_mut().find(|g|g.id==session.group_id)) {
                    if baseline==before {*baseline=after.clone();}
                }
                for (token,mesh) in &candidate.mesh_shares {
                    if parent.mesh_share_baseline.get(token)==session.original_scene.mesh_shares.get(token) {
                        parent.mesh_share_baseline.insert(*token,mesh.clone());
                    }
                }
                for (id,definition) in &candidate.component_definitions {
                    if parent.definitions_baseline.get(id)==session.original_scene.component_definitions.get(id) {
                        parent.definitions_baseline.insert(*id,definition.clone());
                    }
                }
            }
        }
        self.scene=candidate;self.bump_model_revision();self.camera=session.original_camera;
        self.undo_stack=session.original_undo;self.redo_stack=session.original_redo;
        if changed {self.undo_stack.push(session.original_scene);self.redo_stack.clear();}
        self.status_message=if changed {"Committed group edit"} else {"Group edit unchanged"}.into();
    }

    pub fn cancel_component_edit(&mut self) {
        if let Some(session)=self.component_edit_stack.pop() {
            self.set_tool(ToolType::Select);
            self.scene=session.original_scene;self.bump_model_revision();self.camera=session.original_camera;
            self.undo_stack=session.original_undo;self.redo_stack=session.original_redo;
            self.status_message="Cancelled group edit".into();
        } else {self.status_message="No group edit is active".into();}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::{groups::Group,mesh::Mesh};
    use glam::{DMat4,DVec3};
    fn app()->IngeTrazoApp {
        let mut app=IngeTrazoApp::new();app.scene=Scene::new();
        let mut mesh=Mesh::new();
        let verts=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(verts.to_vec(),vec![]).unwrap();
        let group=Group::with_mesh("component",mesh);let id=group.id;app.scene.groups.push(group);app.scene.make_component(id).unwrap();
        let mut copy=app.scene.groups[0].duplicate_instance();copy.transform=DMat4::from_translation(DVec3::new(3.,4.,5.));app.scene.groups.push(copy);
        app.scene.select(SelectionItem::Group(0));app
    }
    #[test]
    fn axial_and_aligned_measurements_keep_world_frame_in_rotated_scaled_context() {
        let mut app=app();let transform=DMat4::from_translation(DVec3::new(3.,4.,5.))*DMat4::from_rotation_y(30_f64.to_radians())*DMat4::from_scale(DVec3::new(-2.,3.,4.));app.scene.groups[0].transform=transform;
        let start=transform.transform_point3(DVec3::ZERO);let end=transform.transform_point3(DVec3::X);
        let mut dimension=ingetrazo_core::dimensions::Dimension::new(start,end,DVec3::new(4.,3.,2.));dimension.axis=Some("x".into());app.scene.dimensions.push(dimension);app.scene.bind_dimensions();
        let expected=app.scene.dimensions[0].clone();let expected_lines=expected.line_points();app.begin_component_edit();
        assert!((app.scene.dimensions[0].length()-expected.length()).abs()<1e-10);
        let(world_a,world_b)=app.scene.dimensions[0].line_points();assert!((world_a-expected_lines.0).length()<1e-9);assert!((world_b-expected_lines.1).length()<1e-9);
        app.scene.dimensions[0].text_override=Some("world <>".into());
        let mut fresh=ingetrazo_core::dimensions::Dimension::new(DVec3::ZERO,DVec3::X,DVec3::Y);fresh.axis=Some("z".into());fresh.measurement_frame=app.scene.dimension_frame;app.scene.dimensions.push(fresh);app.scene.bind_dimensions();
        let expected_new=app.scene.dimensions[1].length();app.finish_component_edit();assert!(app.component_edit_stack.is_empty());assert_eq!(app.scene.dimensions.len(),2);assert!((app.scene.dimensions[0].length()-expected.length()).abs()<1e-10);assert!((app.scene.dimensions[1].length()-expected_new).abs()<1e-10);assert_eq!(app.scene.dimensions[1].axis.as_deref(),Some("z"));
    }
    #[test]
    fn world_dimensions_commit_without_second_placement_transform_and_cancel_restore() {
        let mut app=app();app.scene.clear_selection();app.scene.select(SelectionItem::Group(1));
        let original=app.scene.clone();app.begin_component_edit();
        app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(DVec3::new(3.,4.,5.),DVec3::new(4.,4.,5.),DVec3::Y));app.scene.bind_dimensions();
        app.finish_component_edit();assert_eq!(app.component_edit_stack.len(),0);assert_eq!(app.scene.dimensions.len(),1);
        assert_eq!(app.scene.dimensions[0].start,DVec3::new(3.,4.,5.));assert_eq!(app.scene.dimensions[0].end,DVec3::new(4.,4.,5.));assert_eq!(app.scene.dimensions[0].offset,DVec3::Y);
        assert_eq!(app.scene.dimensions[0].anchor_a.as_ref().unwrap().path,vec![app.scene.groups[1].id]);
        app.undo();assert_eq!(app.scene,original);app.begin_component_edit();app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(DVec3::ZERO,DVec3::Y,DVec3::X));app.cancel_component_edit();assert_eq!(app.scene,original);
    }
    #[test]
    fn component_edit_commits_all_instances_once_and_undo_redo_restores_registry() {
        let mut app=app();let before=app.scene.clone();
        app.begin_component_edit();assert_eq!(app.component_edit_stack.len(),1);
        app.save_undo_state();app.scene.mesh.vertices[0].position.z=7.;
        app.finish_component_edit();assert!(app.component_edit_stack.is_empty());
        assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.groups[0].mesh.vertices[0].position.z,7.);
        assert_eq!(app.scene.groups[1].mesh.vertices[0].position.z,7.);assert_eq!(app.scene.groups[1].transform,before.groups[1].transform);
        let committed=app.scene.clone();app.undo();assert_eq!(app.scene,before);app.redo();assert_eq!(app.scene,committed);
    }
    #[test]
    fn cancel_and_fragment_save_preserve_document_and_history() {
        let mut app=app();app.save_undo_state();let before=app.scene.clone();let depth=app.undo_stack.len();
        app.begin_component_edit();app.save_undo_state();app.scene.mesh.vertices[0].position.z=9.;
        let path=std::env::temp_dir().join(format!("component-fragment-{}.igz",uuid::Uuid::new_v4()));
        app.save_to_path(&path);assert!(!path.exists());assert!(app.status_message.contains("before saving"));
        app.cancel_component_edit();assert_eq!(app.scene,before);assert_eq!(app.undo_stack.len(),depth);
    }
    #[test]
    fn paste_retains_link_but_creates_unique_instance_uids() {
        let mut app=app();app.copy_selection();app.paste_in_place();
        assert_ne!(app.scene.groups[0].id,app.scene.groups[2].id);
        assert_eq!(app.scene.groups[0].component_definition,app.scene.groups[2].component_definition);
    }
    #[test]
    fn nested_component_edit_propagates_and_folds_into_one_outer_history_entry() {
        let mut app=app();
        let child=app.scene.groups[0].duplicate_instance();
        let mut parent=Group::new("parent");parent.children.push(child);let parent_id=parent.id;
        app.scene.groups.push(parent);app.scene.make_component(parent_id).unwrap();
        let copy=app.scene.groups[2].duplicate_instance();app.scene.groups.push(copy);
        app.scene.clear_selection();app.scene.select(SelectionItem::Group(2));
        let before=app.scene.clone();
        let child_ids=[app.scene.groups[2].children[0].id,app.scene.groups[3].children[0].id];
        app.begin_component_edit();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();
        app.scene.mesh.vertices[0].position.z=11.;app.finish_component_edit();
        assert_eq!(app.component_edit_stack.len(),1);
        app.finish_component_edit();assert!(app.component_edit_stack.is_empty());
        assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.groups[0].mesh.vertices[0].position.z,11.);
        assert_eq!(app.scene.groups[2].children[0].mesh.vertices[0].position.z,11.);
        assert_eq!(app.scene.groups[3].children[0].mesh.vertices[0].position.z,11.);
        assert_eq!(app.scene.groups[2].children[0].id,child_ids[0]);assert_eq!(app.scene.groups[3].children[0].id,child_ids[1]);
        app.undo();assert_eq!(app.scene,before);
    }

    #[test]
    fn mixed_selection_component_creation_and_explosion_are_reversible() {
        let mut app=app();
        let verts=[DVec3::new(0.,0.,2.),DVec3::new(1.,0.,2.),DVec3::new(0.,1.,2.)].map(|p|app.scene.mesh.add_vertex(p));
        let face=app.scene.mesh.add_face(verts.to_vec(),vec![]).unwrap();
        app.scene.mesh.faces[face].attrs.insert("opaque".into(),serde_json::json!(23));
        app.scene.select(SelectionItem::Face(face));let before=app.scene.clone();
        app.make_component();assert_eq!(app.scene.groups.len(),2);
        let index=app.scene.selection.iter().find_map(|s|if let SelectionItem::Group(i)=s{Some(*i)}else{None}).unwrap();
        assert!(app.scene.groups[index].component_definition.is_some());
        assert_eq!(app.scene.groups[index].children.len(),1);
        assert_eq!(app.scene.groups[index].mesh.faces[0].attrs["opaque"],23);
        let grouped=app.scene.clone();app.explode_group();
        assert_eq!(app.scene.mesh.faces[0].attrs["opaque"],23);assert_eq!(app.scene.groups.len(),2);
        app.undo();assert_eq!(app.scene,grouped);app.undo();assert_eq!(app.scene,before);
    }

}
