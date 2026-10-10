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
        for child in &mut group.children {child.definition_member=Some(child.id);}
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
        // Editing a repeated placement uses that instance's UIDs. Definition
        // child slots retain their stable member IDs, so other instance UIDs
        // survive a shared update and undo does not become an identity rewrite.
        for child in &mut definition.children {
            if let Some(member)=child.definition_member {child.id=member;}
            child.definition_member=Some(child.id);
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

    /// Match the original Make Unique contract: a leaf bakes its placement
    /// into a private classic mesh; a container keeps its transform/tree and
    /// gets private definitions for every previously linked descendant.
    pub fn make_component_unique(&mut self,group_id:Uuid)->Result<(),String> {
        fn find(groups:&mut [Group],id:Uuid)->Option<&mut Group> {
            for group in groups {
                if group.id==id {return Some(group);}
                if let Some(found)=find(&mut group.children,id) {return Some(found);}
            }
            None
        }
        let mut candidate=self.clone();
        let group=find(&mut candidate.groups,group_id).ok_or("component instance not found")?;
        if group.component_definition.is_none() {return Err("group is not a component".into());}
        fn detach_children(group:&mut Group,defs:&mut std::collections::HashMap<Uuid,ComponentDefinition>) {
            for child in &mut group.children {
                detach_children(child,defs);
                if child.component_definition.is_some() {
                    let id=Uuid::new_v4();
                    defs.insert(id,ComponentDefinition{mesh:child.mesh.clone(),children:child.children.clone()});
                    child.component_definition=Some(id);
                }
            }
        }
        if group.children.is_empty() {
            if !group.transform.is_finite() || group.transform.determinant().abs()<1e-12 {return Err("invalid component transform".into());}
            let axes=group.native_document.get("axes").and_then(|v|v.as_array()).filter(|a|a.len()==16)
                .and_then(|a|a.iter().map(|v|v.as_f64()).collect::<Option<Vec<_>>>())
                .map(|v|{let mut a=[0.;16];a.copy_from_slice(&v);glam::DMat4::from_cols_array(&a)}).unwrap_or(glam::DMat4::IDENTITY);
            let frame=group.transform*axes;
            for vertex in &mut group.mesh.vertices {vertex.position=group.transform.transform_point3(vertex.position);}
            group.mesh.rebuild_spatial_index();group.transform=glam::DMat4::IDENTITY;
            if !group.native_document.is_object() {group.native_document=serde_json::json!({});}
            group.native_document["axes"]=serde_json::json!(frame.to_cols_array());
        } else {detach_children(group,&mut candidate.component_definitions);}
        group.component_definition=None;
        candidate.bump_version();*self=candidate;Ok(())
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
        scene.make_component_unique(scene.groups[1].id).unwrap();assert_eq!(scene.groups[1].component_definition,None);
        let mut replacement=scene.component_definitions[&definition].clone();replacement.mesh.vertices[0].position.z=9.;
        scene.replace_component_definition(definition,replacement).unwrap();
        assert_eq!(scene.groups[1].mesh.vertices[0].position.z,11.);
        scene=before;assert_eq!(scene.groups[0].mesh.vertices[0].position.z,0.);
    }
    #[test]
    fn nested_unique_preserves_placements_and_detaches_entire_tree() {
        let mut scene=Scene::new();
        let mut mesh=Mesh::new();mesh.add_vertex(DVec3::new(1.,2.,3.));
        let leaf=Group::with_mesh("leaf",mesh);let leaf_id=leaf.id;scene.groups.push(leaf);
        let leaf_def=scene.make_component(leaf_id).unwrap();
        let mut middle=Group::new("middle");middle.children.push(scene.groups.remove(0));
        middle.transform=DMat4::from_translation(DVec3::X);let middle_id=middle.id;scene.groups.push(middle);
        let middle_def=scene.make_component(middle_id).unwrap();
        let mut outer=Group::new("outer");outer.children.push(scene.groups.remove(0));
        outer.transform=DMat4::from_translation(DVec3::Y);let outer_id=outer.id;scene.groups.push(outer);
        let outer_def=scene.make_component(outer_id).unwrap();
        scene.groups.push(scene.groups[0].duplicate_instance());
        let original=scene.groups[0].clone();scene.make_component_unique(outer_id).unwrap();
        let unique=&scene.groups[0];assert_eq!(unique.id,original.id);assert_eq!(unique.transform,original.transform);
        assert_eq!(unique.component_definition,None);
        let middle=&unique.children[0];assert_eq!(middle.id,middle_id);assert_eq!(middle.transform,original.children[0].transform);
        assert_ne!(middle.component_definition,Some(middle_def));
        let leaf=&middle.children[0];assert_eq!(leaf.id,leaf_id);assert_ne!(leaf.component_definition,Some(leaf_def));
        let private_leaf=leaf.component_definition.unwrap();
        let mut changed=scene.component_definitions[&leaf_def].clone();changed.mesh.vertices[0].position.z=99.;
        scene.replace_component_definition(leaf_def,changed).unwrap();
        assert_eq!(scene.groups[0].children[0].children[0].mesh.vertices[0].position.z,3.);
        assert_eq!(scene.groups[1].children[0].children[0].mesh.vertices[0].position.z,99.);
        let mut changed=scene.component_definitions[&private_leaf].clone();changed.mesh.vertices[0].position.z=17.;
        scene.replace_component_definition(private_leaf,changed).unwrap();
        assert_eq!(scene.groups[0].children[0].children[0].mesh.vertices[0].position.z,17.);
        assert_eq!(scene.groups[1].children[0].children[0].mesh.vertices[0].position.z,99.);
        assert_eq!(scene.groups[1].component_definition,Some(outer_def));
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
