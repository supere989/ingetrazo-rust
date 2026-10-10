//! Explicit shared definitions and transactional instance updates.
use std::collections::HashSet;
use serde::{Serialize,Deserialize};
use uuid::Uuid;
use crate::{groups::Group,mesh::Mesh,scene::Scene};

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub struct ComponentDefinition {
    pub mesh: Mesh,
    /// Child placements are part of the definition. Their own component links
    /// are retained when this definition is placed or edited.
    pub children: Vec<Group>,
}

impl Scene {
    /// Convert a group into a new linked definition; unrelated plain groups
    /// never acquire linkage merely because their geometry matches.
    pub fn make_component(&mut self, group_id:Uuid) -> Result<Uuid,String> {
        fn find(groups:&mut [Group],id:Uuid)->Option<&mut Group> {
            for group in groups {
                if group.id==id { return Some(group); }
                if let Some(found)=find(&mut group.children,id) {return Some(found);}
            }
            None
        }
        let group=find(&mut self.groups,group_id).ok_or("component instance not found")?;
        if let Some(id)=group.component_definition {return Ok(id);}
        let id=Uuid::new_v4();
        self.component_definitions.insert(id,ComponentDefinition {mesh:group.mesh.clone(),children:group.children.clone()});
        group.component_definition=Some(id);self.bump_version();Ok(id)
    }

    /// Validate the complete replacement before changing either registry or
    /// caches. Instances retain placement transforms, layer, UID and metadata.
    pub fn replace_component_definition(&mut self,id:Uuid,mut definition:ComponentDefinition)->Result<(),String> {
        if !self.component_definitions.contains_key(&id) {return Err("component definition not found".into());}
        fn references(groups:&[Group],target:Uuid,defs:&std::collections::HashMap<Uuid,ComponentDefinition>,seen:&mut HashSet<Uuid>)->bool {
            for group in groups {
                if let Some(id)=group.component_definition {
                    if id==target {return true;}
                    if seen.insert(id) {
                        if let Some(def)=defs.get(&id) {
                            if references(&def.children,target,defs,seen) {return true;}
                        }
                    }
                }
                if references(&group.children,target,defs,seen) {return true;}
            }
            false
        }
        if references(&definition.children,id,&self.component_definitions,&mut HashSet::new()) {
            return Err("recursive component definition".into());
        }
        fn check_links(groups:&[Group],defs:&std::collections::HashMap<Uuid,ComponentDefinition>)->Result<(),String> {
            for group in groups {
                if let Some(id)=group.component_definition {
                    if !defs.contains_key(&id) {return Err("nested component definition not found".into());}
                }
                check_links(&group.children,defs)?;
            }
            Ok(())
        }
        check_links(&definition.children,&self.component_definitions)?;
        definition.mesh.rebuild_spatial_index();
        fn place_children(templates:&[Group],old:&[Group])->Vec<Group> {
            templates.iter().map(|template| {
                let existing=old.iter().find(|g|g.definition_member==Some(template.id)||g.id==template.id);
                let mut placed=template.clone();
                placed.definition_member=Some(template.id);
                if let Some(existing)=existing {
                    placed.id=existing.id;
                    placed.native_document=existing.native_document.clone();
                    placed.native_baseline=existing.native_baseline.clone();
                    placed.children=place_children(&template.children,&existing.children);
                } else {placed=placed.duplicate_instance();}
                placed
            }).collect()
        }
        fn update(groups:&mut [Group],id:Uuid,definition:&ComponentDefinition) {
            for group in groups {
                if group.component_definition==Some(id) {
                    group.mesh=definition.mesh.clone();
                    group.children=place_children(&definition.children,&group.children);
                } else {update(&mut group.children,id,definition);}
            }
        }
        update(&mut self.groups,id,&definition);
        // Keep nested placements in every other definition synchronized too.
        for (other,def) in &mut self.component_definitions {
            if *other!=id {update(&mut def.children,id,&definition);}
        }
        self.component_definitions.insert(id,definition);self.bump_version();Ok(())
    }

    /// Breaking a link is local to that instance and reversible with Scene history.
    pub fn make_component_unique(&mut self,group_id:Uuid)->Result<Uuid,String> {
        fn find(groups:&mut [Group],id:Uuid)->Option<&mut Group> {
            for group in groups {
                if group.id==id {return Some(group);}
                if let Some(found)=find(&mut group.children,id) {return Some(found);}
            }
            None
        }
        let group=find(&mut self.groups,group_id).ok_or("component instance not found")?;
        let old=group.component_definition.ok_or("group is not a component")?;
        let definition=self.component_definitions.get(&old).cloned().ok_or("component definition not found")?;
        let id=Uuid::new_v4();group.component_definition=Some(id);
        self.component_definitions.insert(id,definition);self.bump_version();Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4,DVec3};
    #[test]
    fn linked_updates_preserve_placements_and_unique_instances_detach() {
        let mut scene=Scene::new();
        let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);
        let group=Group::with_mesh("component",mesh);let id=group.id;scene.groups.push(group);
        let definition=scene.make_component(id).unwrap();
        let mut copy=scene.groups[0].duplicate_instance();assert_ne!(copy.id,id);
        copy.transform=DMat4::from_translation(DVec3::new(2.,3.,4.));
        scene.groups.push(copy);let before=scene.clone();
        let mut replacement=scene.component_definitions[&definition].clone();replacement.mesh.vertices[0].position.z=7.;
        scene.replace_component_definition(definition,replacement).unwrap();
        assert_eq!(scene.groups[0].mesh.vertices[0].position.z,7.);
        assert_eq!(scene.groups[1].mesh.vertices[0].position.z,7.);
        assert_eq!(scene.groups[1].transform.w_axis.z,4.);
        let unique=scene.make_component_unique(scene.groups[1].id).unwrap();assert_ne!(unique,definition);
        let mut replacement=scene.component_definitions[&definition].clone();replacement.mesh.vertices[0].position.z=9.;
        scene.replace_component_definition(definition,replacement).unwrap();
        assert_eq!(scene.groups[1].mesh.vertices[0].position.z,7.);
        scene=before;assert_eq!(scene.groups[0].mesh.vertices[0].position.z,0.);
    }
    #[test]
    fn recursive_replacements_and_missing_ids_are_atomic() {
        let mut scene=Scene::new();let group=Group::new("component");let id=group.id;scene.groups.push(group);
        let definition=scene.make_component(id).unwrap();let before=scene.clone();
        let mut replacement=scene.component_definitions[&definition].clone();replacement.children.push(scene.groups[0].duplicate_instance());
        assert!(scene.replace_component_definition(definition,replacement).is_err());assert_eq!(scene,before);
        assert!(scene.replace_component_definition(Uuid::new_v4(),ComponentDefinition{mesh:Mesh::new(),children:vec![]}).is_err());
        assert_eq!(scene,before);
    }
}
