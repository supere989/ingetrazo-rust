//! Local-coordinate group editing with one atomic document history entry.
use ingetrazo_core::{camera::OrbitCamera,components::ComponentDefinition,scene::{Scene,SelectionItem}};
use ingetrazo_tools::base::ToolType;
use crate::app::IngeTrazoApp;

pub struct ComponentEditSession {
    original_scene: Scene,
    original_camera: OrbitCamera,
    original_undo: Vec<Scene>,
    original_redo: Vec<Scene>,
    group_id: uuid::Uuid,
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
            let old=self.scene.groups.len();self.make_group();
            if self.scene.groups.len()==old {return;}
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
        let session=ComponentEditSession{
            original_scene:self.scene.clone(),original_camera:self.camera.clone(),
            original_undo:std::mem::take(&mut self.undo_stack),original_redo:std::mem::take(&mut self.redo_stack),
            group_id:group.id,
        };
        let mut edit=Scene::new();
        edit.mesh=group.mesh;edit.groups=group.children;
        edit.materials=self.scene.materials.clone();edit.layers=self.scene.layers.clone();
        edit.unit=self.scene.unit;edit.embedded_textures=self.scene.embedded_textures.clone();
        edit.component_definitions=self.scene.component_definitions.clone();
        self.camera.target=group.transform.inverse().transform_point3(self.camera.target);
        edit.camera=self.camera.clone();
        self.scene=edit;self.component_edit_stack.push(session);
        self.status_message="Editing group in local coordinates; Finish commits, Cancel restores".into();
    }

    pub fn finish_component_edit(&mut self) {
        if self.component_edit_stack.is_empty() {self.status_message="No group edit is active".into();return;}
        // A group schema currently has mesh and child containers, not document
        // annotations. Refuse unsupported new local annotations instead of losing them.
        if !self.scene.dimensions.is_empty() || !self.scene.guides.is_empty() || !self.scene.text_labels.is_empty()
            || !self.scene.section_planes.is_empty() || !self.scene.geo_points.is_empty() || !self.scene.geo_paths.is_empty()
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
        let old=&candidate.groups[group_index];
        let changed=old.mesh!=self.scene.mesh || old.children!=self.scene.groups
            || candidate.materials!=self.scene.materials || candidate.layers!=self.scene.layers
            || candidate.component_definitions!=self.scene.component_definitions;
        if changed {
            let replacement=ComponentDefinition{mesh:self.scene.mesh.clone(),children:self.scene.groups.clone()};
            let updates:Vec<_>=self.scene.component_definitions.iter().filter(|(id,def)|
                candidate.component_definitions.get(id)!=Some(*def)).map(|(id,def)|(*id,def.clone())).collect();
            for (id,def) in &updates {candidate.component_definitions.entry(*id).or_insert_with(||def.clone());}
            for (id,def) in updates {
                if let Err(e)=candidate.replace_component_definition(id,def) {self.status_message=e;return;}
            }
            if let Some(id)=candidate.groups[group_index].component_definition {
                if let Err(e)=candidate.replace_component_definition(id,replacement) {self.status_message=e;return;}
            } else {
                candidate.groups[group_index].mesh=replacement.mesh;
                candidate.groups[group_index].children=replacement.children;
                candidate.bump_version();
            }
            candidate.materials=self.scene.materials.clone();candidate.layers=self.scene.layers.clone();
        }
        let session=self.component_edit_stack.pop().unwrap();
        self.scene=candidate;self.camera=session.original_camera;
        self.undo_stack=session.original_undo;self.redo_stack=session.original_redo;
        if changed {self.undo_stack.push(session.original_scene);self.redo_stack.clear();}
        self.status_message=if changed {"Committed group edit"} else {"Group edit unchanged"}.into();
    }

    pub fn cancel_component_edit(&mut self) {
        if let Some(session)=self.component_edit_stack.pop() {
            self.set_tool(ToolType::Select);
            self.scene=session.original_scene;self.camera=session.original_camera;
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

}
