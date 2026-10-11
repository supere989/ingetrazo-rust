//! Source-compatible world-coordinate group editing with atomic document history.
use ingetrazo_core::{camera::OrbitCamera,components::ComponentDefinition,scene::{Scene,SelectionItem}};
use ingetrazo_tools::base::ToolType;
use crate::app::IngeTrazoApp;

pub struct ComponentEditSession {
    pub(crate) original_scene: Scene,
    pub(crate) cancel_scene: Scene,
    original_camera: OrbitCamera,
    pub(crate) original_undo: Vec<Scene>,
    pub(crate) original_redo: Vec<Scene>,
    pub(crate) group_id: uuid::Uuid,
    fold_history: bool,
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

impl ComponentEditSession {
    fn finish_copy(&self) -> Self {
        Self {
            original_scene:self.original_scene.clone(),cancel_scene:Scene::new(),original_camera:self.original_camera.clone(),
            original_undo:Vec::new(),original_redo:Vec::new(),group_id:self.group_id,fold_history:self.fold_history,
            private_container_mesh:self.private_container_mesh,container_bake:self.container_bake,container_axes:self.container_axes,
            leaf_share_back:self.leaf_share_back,mesh_baseline:self.mesh_baseline.clone(),groups_baseline:self.groups_baseline.clone(),
            mesh_share_baseline:self.mesh_share_baseline.clone(),definitions_baseline:self.definitions_baseline.clone(),
            editing_definition:self.editing_definition,dimension_baseline:self.dimension_baseline.clone(),section_baseline:self.section_baseline.clone(),
        }
    }
}

struct FinishPlan {
    candidate: Scene,
    changed: bool,
    target: Vec<uuid::Uuid>,
    fold: bool,
    folded_snapshot: Option<Scene>,
    lifted_undo: Vec<Scene>,
    lifted_redo: Vec<Scene>,
    parked_candidates: Vec<Scene>,
}
fn prepare_finish(finishing_scene: &Scene, stack: &[ComponentEditSession], undo: &[Scene], redo: &[Scene]) -> Result<FinishPlan,String> {
        let session=stack.last().unwrap();
        let (candidate,changed)=match component_candidate(session,&finishing_scene) {Ok(result)=>result,Err(error)=>return Err(error)};
        // Lift target-tagged ordinary commands before changing either stack.
        // Each snapshot receives the same source world/container publication
        // as Finish, while retaining its individual global chronological slot.
        let target=finishing_scene.history_context.clone();
        let fold=session.fold_history && session.mesh_baseline!=finishing_scene.mesh;
        let folded_snapshot=if fold {match shareback_snapshot(session,&candidate){Ok(mut scene)=>{
            let mut targets:Vec<ingetrazo_core::bim::MetadataTarget>=Vec::new();
            for entry in undo.iter().filter(|entry|entry.history_context.starts_with(&target)){if let Some(history)=&entry.history_bim{for patch in &history.targets{if let Some(old)=targets.iter_mut().find(|old|old.context==patch.context&&old.group==patch.group&&old.vertices==patch.vertices){old.expected=patch.expected.clone();}else{targets.push(patch.clone());}}}}
            if !targets.is_empty(){scene.history_bim=Some(ingetrazo_core::bim::MetadataHistory{targets});}
            let mut material:Option<ingetrazo_core::materials::RestampHistory>=None;
            for entry in undo.iter().filter(|entry|entry.history_context.starts_with(&target)){if let Some(history)=&entry.history_material{if let Some(old)=&mut material{old.absorb_inverse(history);}else{material=Some(history.clone());}}}
            scene.history_material=material;
            Some(scene)},Err(error)=>return Err(error)}}else{None};
        let mut lifted_undo=undo.to_vec();let mut lifted_redo=redo.to_vec();
        if let Some(before)=lifted_undo.last_mut(){if before.history_images.0.is_none(){before.history_images.0=Some(ingetrazo_core::image_plane::ImageDelta::between(&before.image_planes,&finishing_scene.image_planes));}}
        if let Some(before)=lifted_undo.last_mut(){if before.history_terrain.0.is_none(){before.history_terrain.0=Some(ingetrazo_core::georef::TerrainHistory::seal(&before.terrain,&finishing_scene.terrain));}}
        if let Some(before)=lifted_undo.last_mut(){if before.history_layers.0.is_none(){before.history_layers.0=Some(ingetrazo_core::layers::LayerDelta::between(&before.layers,&finishing_scene.layers));}}
        if let Some(before)=lifted_undo.last_mut(){if before.history_views.0.is_none(){before.history_views.0=Some(ingetrazo_core::saved_views::ViewMembershipDelta::between(&before.saved_views,&finishing_scene.saved_views));}}
        if !fold || !changed {
            for entry in lifted_undo.iter_mut().chain(lifted_redo.iter_mut()) {
                if entry.history_context==target {
                    *entry=match component_candidate(session,entry) {Ok((mut scene,_))=>{
                        scene.selection=session.original_scene.selection.clone();scene.selection_order=session.original_scene.selection_order.clone();
                        scene.history_annotation_delete=entry.history_annotation_delete;scene.history_annotation_move=entry.history_annotation_move;scene.history_annotation_edit=entry.history_annotation_edit;scene.history_annotation_create=entry.history_annotation_create;
                        if entry.history_annotation_delete||entry.history_annotation_move||entry.history_annotation_edit{scene.clear_selection();}
                        scene.version=session.original_scene.version;scene.view_version=session.original_scene.view_version;scene.history_shareback=entry.history_shareback;scene.history_axes=entry.history_axes.clone();scene.history_bim=entry.history_bim.clone();scene.history_material=entry.history_material.clone();scene.history_views.0=entry.history_views.0.clone();scene.history_group_visibility=entry.history_group_visibility.clone();scene.history_terrain=entry.history_terrain.clone();scene.history_layers=entry.history_layers.clone();scene.history_images=entry.history_images.clone();scene
                    },Err(error)=>return Err(error)};
                }
            }
        }

        // Apply this source entry bake to every older target view, including
        // parked ancestors. Prior Move snapshots keep inverse deltas against
        // the new mesh frame; their metadata and nested placement list survive.
        // Older root snapshots need ancestor entry bakes first, exactly as
        // the live source enters outer→inner. This preserves identity matrices
        // and prevents an unentered local frame from being treated as world.
        let bakes:Vec<_>=stack.iter().map(SourceEntryBake::from_session).collect();
        for entry in lifted_undo.iter_mut().chain(lifted_redo.iter_mut()) {
            for bake in &bakes {if let Err(error)=bake.apply(entry){return Err(error);}}
        }
        let mut parked_candidates:Vec<_>=stack[..stack.len()-1].iter().map(|parent|parent.original_scene.clone()).collect();
        for parent in &mut parked_candidates {for bake in &bakes {if let Err(error)=bake.apply(parent){return Err(error);}}}
        Ok(FinishPlan {candidate,changed,target,fold,folded_snapshot,lifted_undo,lifted_redo,parked_candidates})
}
fn publish_finish(plan: FinishPlan, scene: &mut Scene, stack: &mut Vec<ComponentEditSession>, undo: &mut Vec<Scene>, redo: &mut Vec<Scene>) {
    let FinishPlan {candidate,changed,target,fold,folded_snapshot,lifted_undo,lifted_redo,parked_candidates}=plan;
        let session=stack.pop().unwrap();
        if !changed {
            if let Some(parent)=stack.last_mut() {
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
        for (parent,candidate) in stack.iter_mut().zip(parked_candidates){parent.original_scene=candidate;}
        *scene=candidate;
        *undo=lifted_undo;*redo=lifted_redo;
        if fold && changed {
            // Sharing leaves fold only commands targeting this edit mesh.
            // Prior document commands retain their global positions.
            undo.retain(|entry|!entry.history_context.starts_with(&target));
            redo.retain(|entry|!entry.history_context.starts_with(&target));
            if changed {undo.push(folded_snapshot.unwrap());redo.clear();}
        }

}


#[derive(Clone)]
struct SourceEntryBake {id:uuid::Uuid,matrix:Option<glam::DMat4>,axes:Option<glam::DMat4>,private:Option<uuid::Uuid>}
impl SourceEntryBake {
    fn from_session(session:&ComponentEditSession)->Self {Self{id:session.group_id,matrix:session.container_bake,axes:session.container_axes,private:session.private_container_mesh}}
    fn apply(&self,scene:&mut Scene)->Result<(),String> {
        let Some(axes)=self.axes else{return Ok(());};
        fn find(groups:&mut [ingetrazo_core::groups::Group],id:uuid::Uuid)->Option<&mut ingetrazo_core::groups::Group>{
            for group in groups {if group.id==id{return Some(group);}if let Some(group)=find(&mut group.children,id){return Some(group);}}None
        }
        let Some(group)=find(&mut scene.groups,self.id) else{return Ok(());};
        // Exact raw axes mark this representation; world coordinates must
        // never be baked twice when lifting through multiple context levels.
        if group.axes_matrix()==Some(axes){return Ok(());}
        if let Some(matrix)=self.matrix {
            group.mesh=ingetrazo_core::containers::transform_mesh(&group.mesh,matrix)?;
            for child in &mut group.children{child.transform=matrix*child.transform;}
            // Prior placement commands remain inverse deltas against the
            // newly baked representation, as source MoveGroup.undo composes.
            group.transform=if group.transform.x_axis==matrix.x_axis && group.transform.y_axis==matrix.y_axis && group.transform.z_axis==matrix.z_axis {
                glam::DMat4::from_translation(group.transform.w_axis.truncate()-matrix.w_axis.truncate())
            } else {group.transform*matrix.inverse()};
            if let Some(token)=self.private {group.mesh_share=Some(token);scene.mesh_shares.insert(token,group.mesh.clone());}
        }
        if !group.native_document.is_object(){group.native_document=serde_json::json!({});}
        group.native_document["axes"]=serde_json::json!(axes.to_cols_array());Ok(())
    }
}

/// The source command restores only the prototype and its selected placement.
/// All annotation/resource commands absorbed by it retain their current state.
pub(crate) fn shareback_restore(current:&Scene,snapshot:&Scene)->Result<Scene,String> {
    let id=snapshot.history_shareback.ok_or("Missing shareback target")?;
    fn find(groups:&[ingetrazo_core::groups::Group],id:uuid::Uuid)->Option<&ingetrazo_core::groups::Group>{for group in groups {if group.id==id{return Some(group);}if let Some(found)=find(&group.children,id){return Some(found);}}None}
    fn restore(groups:&mut [ingetrazo_core::groups::Group],id:uuid::Uuid,old:&ingetrazo_core::groups::Group){for group in groups {if group.id==id{group.mesh=old.mesh.clone();group.transform=old.transform;return;}restore(&mut group.children,id,old);}}
    let old=find(&snapshot.groups,id).ok_or("Shareback target no longer exists")?;
    let mut result=current.clone();
    if let Some(token)=old.mesh_share {result.publish_mesh_share(token,old.mesh.clone())?;}
    restore(&mut result.groups,id,old);
    // restore_state replaces prototype vertex objects. Only anchors owned by
    // that prototype die; source refresh_anchors retries their cached points.
    fn affected(scene:&Scene,anchor:&ingetrazo_core::dimension_anchors::VertexAnchor,id:uuid::Uuid,token:Option<uuid::Uuid>)->bool {
        let Some(last)=anchor.path.last() else{return false;};
        *last==id || token.is_some_and(|token|find(&scene.groups,*last).is_some_and(|group|group.mesh_share==Some(token)))
    }
    let mut dimensions=std::mem::take(&mut result.dimensions);
    for dimension in &mut dimensions {for (point,anchor) in [(&dimension.start,&mut dimension.anchor_a),(&dimension.end,&mut dimension.anchor_b)] {
        if anchor.as_ref().is_some_and(|anchor|affected(current,anchor,id,old.mesh_share)) {*anchor=result.resolve_dimension_anchor(*point);}
    }}
    result.dimensions=dimensions;result.history_shareback=None;Ok(result)
}

fn shareback_snapshot(session:&ComponentEditSession,candidate:&Scene)->Result<Scene,String> {
    let mut snapshot=session.original_scene.clone();snapshot.history_shareback=Some(session.group_id);
    let mut restored=shareback_restore(candidate,&snapshot)?;
    restored.history_shareback=Some(session.group_id);Ok(restored)
}

pub(crate) fn component_candidate(session:&ComponentEditSession,scene:&Scene)->Result<(Scene,bool),String> {
        let mut candidate=session.original_scene.clone();
        let Some(group_index)=candidate.groups.iter().position(|g|g.id==session.group_id) else {
            return Err("Edited instance no longer exists".into());
        };
        let images_changed=candidate.image_planes!=scene.image_planes;
        let presentation_changed=candidate.display_style!=scene.display_style || candidate.shadows!=scene.shadows || candidate.show_hidden_objects!=scene.show_hidden_objects || candidate.show_hidden_geometry!=scene.show_hidden_geometry;
        let changed=session.mesh_baseline!=scene.mesh || session.groups_baseline!=scene.groups
            || candidate.materials!=scene.materials || candidate.layers!=scene.layers
            || candidate.embedded_textures!=scene.embedded_textures
            || candidate.tile_layer!=scene.tile_layer || candidate.unit!=scene.unit || candidate.guides!=scene.guides || candidate.text_labels!=scene.text_labels || candidate.geo_datum!=scene.geo_datum || candidate.geo_points!=scene.geo_points || candidate.geo_paths!=scene.geo_paths || candidate.terrain!=scene.terrain || candidate.sheets!=scene.sheets || candidate.active_sheet_index!=scene.active_sheet_index || candidate.saved_views!=scene.saved_views
            || session.definitions_baseline!=scene.component_definitions
            || session.mesh_share_baseline!=scene.mesh_shares
            || scene.dimensions!=session.dimension_baseline || scene.section_planes!=session.section_baseline
            || scene.show_section_cuts!=candidate.show_section_cuts||scene.show_section_planes!=candidate.show_section_planes||scene.show_section_fill!=candidate.show_section_fill;
        let image_global_only=images_changed&&session.mesh_baseline==scene.mesh&&session.groups_baseline==scene.groups&&session.definitions_baseline==scene.component_definitions&&session.mesh_share_baseline==scene.mesh_shares&&scene.dimensions==session.dimension_baseline&&scene.section_planes==session.section_baseline&&candidate.materials==scene.materials&&candidate.unit==scene.unit&&candidate.guides==scene.guides&&candidate.text_labels==scene.text_labels&&candidate.geo_datum==scene.geo_datum&&candidate.geo_points==scene.geo_points&&candidate.geo_paths==scene.geo_paths&&candidate.terrain==scene.terrain&&candidate.sheets==scene.sheets&&candidate.saved_views==scene.saved_views;
        if (changed&&!image_global_only) || session.container_axes.is_some() || session.container_bake.is_some() || session.private_container_mesh.is_some() {
            let mut replacement=ComponentDefinition{mesh_share:None,mesh:scene.mesh.clone(),children:scene.groups.clone()};
            if let Some(inverse)=session.leaf_share_back {
                // Original placed leaves edit a world copy, then inverse-map
                // only that mesh into their shared local prototype on exit.
                replacement.mesh=if session.mesh_baseline==scene.mesh {
                    // Global annotation commands do not replace source leaf
                    // vertices. Avoid inverse rounding of unchanged geometry.
                    candidate.groups[group_index].mesh.clone()
                } else {
                    let original=&candidate.groups[group_index].mesh;
                    let frame=session.original_scene.dimension_frame*candidate.groups[group_index].transform;
                    let vertices:std::collections::HashMap<_,_>=original.vertices.iter().map(|v|(v.uid,v)).collect();
                    let edges:std::collections::HashSet<_>=session.mesh_baseline.edges.iter().map(|e|e.history_id).collect();
                    // Pure removals of Qt-authored rows retain source objects.
                    // Native f64 points/frames retain the existing full precision path.
                    let source_removal=frame==frame.as_mat4().as_dmat4()
                        && original.vertices.iter().all(|v|v.position==v.position.as_vec3().as_dvec3())
                        && replacement.mesh.vertices.iter().all(|v|vertices.get(&v.uid).is_some_and(|o|frame.transform_point3(o.position)==v.position))
                        && replacement.mesh.edges.iter().all(|e|edges.contains(&e.history_id))
                        && replacement.mesh.faces.is_empty();
                    let inverse=if source_removal {
                        for vertex in &mut replacement.mesh.vertices {
                            vertex.position=ingetrazo_core::change_axes::map(frame,vertices[&vertex.uid].position);
                        }
                        ingetrazo_core::change_axes::inverse(frame)?
                    } else {inverse};
                    ingetrazo_core::containers::source_shareback_mesh(&replacement.mesh,inverse)?
                };
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
            if let Err(error)=collect(&scene.groups,&candidate.mesh_shares,&mut meshes) {return Err(error);}
            for (token,mesh) in &scene.mesh_shares {
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
                    if let Err(error)=candidate.register_mesh_share(*token,mesh.clone()) {return Err(error);}
                }
            }
            for (id,def) in &scene.component_definitions {
                candidate.component_definitions.entry(*id).or_insert_with(||def.clone());
            }
            candidate.groups[group_index].mesh=replacement.mesh;
            candidate.groups[group_index].children=replacement.children;
            for (token,mesh) in meshes {
                if let Err(error)=candidate.publish_mesh_share(token,mesh) {return Err(error);}
            }
            candidate.bump_version();
            candidate.materials=scene.materials.clone();candidate.layers=scene.layers.clone();
            // Original context entry switches geometry only. Foreign clipboard
            // images remain available after Finish and through mesh shareback
            // Undo/Redo, just as the source extracted-file texture cache does.
            candidate.embedded_textures=scene.embedded_textures.clone();
            candidate.tile_layer=scene.tile_layer.clone();
            candidate.image_planes=scene.image_planes.clone();candidate.image_objects=scene.image_objects.clone();candidate.unit=scene.unit;candidate.guides=scene.guides.clone();candidate.text_labels=scene.text_labels.clone();candidate.geo_datum=scene.geo_datum.clone();candidate.geo_points=scene.geo_points.clone();candidate.geo_paths=scene.geo_paths.clone();candidate.terrain=scene.terrain.clone();candidate.sheets=scene.sheets.clone();candidate.active_sheet_index=scene.active_sheet_index;candidate.saved_views=scene.saved_views.clone();
            if scene.dimensions!=session.dimension_baseline {
                let transform=candidate.dimension_frame.inverse();
                candidate.dimensions=scene.dimensions.clone();
                let mut dimensions=std::mem::take(&mut candidate.dimensions);
                for dimension in &mut dimensions{
                    let held=(dimension.anchor_a.is_some(),dimension.anchor_b.is_some());
                    dimension.start=transform.transform_point3(dimension.start);dimension.end=transform.transform_point3(dimension.end);dimension.offset=transform.transform_vector3(dimension.offset);dimension.measurement_frame=candidate.dimension_frame;
                    // Leaving an ancestor is a representation change, not a new
                    // AddDimension command. Never revive a hold already lost
                    // by a completed child shareback Undo.
                    dimension.anchor_a=if held.0 {candidate.resolve_dimension_anchor(dimension.start)}else{None};
                    dimension.anchor_b=if held.1 {candidate.resolve_dimension_anchor(dimension.end)}else{None};
                }
                candidate.dimensions=dimensions;
            }
            if scene.section_planes!=session.section_baseline{let transform=candidate.dimension_frame.inverse();candidate.section_planes=scene.section_planes.clone();for plane in &mut candidate.section_planes{plane.point=transform.transform_point3(plane.point);plane.normal=transform.inverse().transpose().transform_vector3(plane.normal).normalize();}}
            candidate.show_section_cuts=scene.show_section_cuts;candidate.show_section_planes=scene.show_section_planes;candidate.show_section_fill=scene.show_section_fill;
            if session.fold_history && session.mesh_baseline!=scene.mesh {
                // The original edit view keeps pre-existing dimensions anchored
                // to the parked prototype, not its temporary world mesh copy.
                // Shareback replaces vertex objects; re-take only held endpoints
                // at their cached pre-command coordinates, never follow Rust UID
                // continuity across this source identity replacement boundary.
                let mut dimensions=std::mem::take(&mut candidate.dimensions);
                let token=session.original_scene.groups[group_index].mesh_share;
                for dimension in &mut dimensions {
                    if let Some(before)=session.original_scene.dimensions.iter().find(|d|d.id==dimension.id) {
                        for (point,anchor,old_point,old_anchor) in [(&mut dimension.start,&mut dimension.anchor_a,before.start,&before.anchor_a),(&mut dimension.end,&mut dimension.anchor_b,before.end,&before.anchor_b)] {
                            let affected=old_anchor.as_ref().is_some_and(|a|a.path.last().is_some_and(|id|*id==session.group_id || token.is_some_and(|token|session.original_scene.groups.iter().any(|g|g.id==*id && g.mesh_share==Some(token)))));
                            if affected {*point=old_point;*anchor=candidate.resolve_dimension_anchor(*point);}
                        }
                    }
                }
                candidate.dimensions=dimensions;
            }
            candidate.refresh_dimensions();
        }
        candidate.image_planes=scene.image_planes.clone();candidate.image_objects=scene.image_objects.clone();if image_global_only{candidate.layers=scene.layers.clone();candidate.embedded_textures=scene.embedded_textures.clone();}
        candidate.unit=scene.unit;candidate.model_units_override=scene.model_units_override.clone();
        candidate.dimension_style_override=scene.dimension_style_override.clone();
        candidate.display_style=scene.display_style.clone();candidate.shadows=scene.shadows.clone();
        candidate.show_hidden_objects=scene.show_hidden_objects;candidate.show_hidden_geometry=scene.show_hidden_geometry;
        candidate.clear_selection();
        Ok((candidate,changed||presentation_changed||images_changed))
}

impl IngeTrazoApp {
    pub(crate) fn complete_axes_command(&self,mut command:ingetrazo_core::change_axes::Command)->ingetrazo_core::change_axes::Command{for session in &self.component_edit_stack{ingetrazo_core::change_axes::extend_parked_instances(&mut command,&self.scene,&session.original_scene);}command}
    /// All parked candidate owners are prepared before any is published.
    pub(crate) fn prepare_axes_parked(&self,command:&ingetrazo_core::change_axes::Command,active:&Scene)->Result<Vec<Scene>,String>{
        self.component_edit_stack.iter().map(|session|{let mut scene=session.original_scene.clone();ingetrazo_core::change_axes::compensate_parked(command,active,&mut scene)?;Ok(scene)}).collect()
    }
    pub(crate) fn publish_axes_parked(&mut self,candidates:Vec<Scene>){for(session,scene)in self.component_edit_stack.iter_mut().zip(candidates){session.original_scene=scene;}}

    /// Route global commands to their immutable edit mesh identity. Return
    /// Some when handled/refused, None for the normal active-scene path.
    fn component_history_target(&self,redo:bool)->Result<Option<usize>,String> {
        let Some(next)=(if redo {self.redo_stack.last()} else {self.undo_stack.last()}) else {return Ok(None);};
        if next.history_context==self.scene.history_context {return Ok(None);}
        let target=&next.history_context;
        if self.component_edit_stack.is_empty() || !self.scene.history_context.starts_with(target) {
            return Err("History target is no longer active; Undo/Redo refused".into());
        }
        let index=target.len();
        let Some(session)=self.component_edit_stack.get(index) else {
            return Err("History context identity is stale; Undo/Redo refused".into());
        };
        if session.original_scene.history_context!=*target {
            return Err("History context identity is stale; Undo/Redo refused".into());
        }
        let before=session.original_scene.groups.iter().find(|g|g.id==session.group_id);
        let after=next.groups.iter().find(|g|g.id==session.group_id);
        fn same_placement(a:&ingetrazo_core::groups::Group,b:&ingetrazo_core::groups::Group)->bool {
            // Legacy runtime identity adoption is not a reference-changing
            // model command. Every authored field and child remains compared.
            let mut a=a.clone();let mut b=b.clone();
            fn normalize(a:&mut ingetrazo_core::groups::Group,b:&mut ingetrazo_core::groups::Group) {
                if a.mesh_share.is_none() || b.mesh_share.is_none(){a.mesh_share=None;b.mesh_share=None;}
                // IFC placement metadata does not replace the active mesh,
                // transform, child list or definition identity. A parked BIM
                // tag command may be undone while this group remains open.
                a.ifc_tag=None;b.ifc_tag=None;
                for (a,b) in a.children.iter_mut().zip(&mut b.children){normalize(a,b);}
            }
            normalize(&mut a,&mut b);a==b
        }
        if !before.zip(after).is_some_and(|(a,b)|same_placement(a,b)) {
            return Err("Undo/Redo would replace the open group; finish or cancel editing first".into());
        }
        Ok(Some(index))
    }

    pub(crate) fn refuse_component_history_step(&mut self,redo:bool)->bool {
        if let Err(error)=self.component_history_target(redo){self.status_message=error;true}else{false}
    }

    pub(crate) fn component_history_step(&mut self,redo:bool)->Option<bool> {
        let index=match self.component_history_target(redo){Ok(Some(index))=>index,Ok(None)=>return None,Err(error)=>{self.status_message=error;return Some(false);}};
        let next=if redo {self.redo_stack.last()}else{self.undo_stack.last()}.unwrap();
        let session=&self.component_edit_stack[index];
        let mut replacement=if next.history_shareback.is_some() {match shareback_restore(&session.original_scene,next){Ok(scene)=>scene,Err(error)=>{self.status_message=error;return Some(false);}}}else{next.clone()};
        // Adopt only existing placement identities, never deduplicate meshes
        // by equality. A pre-migration snapshot can target the parked loose
        // mesh without removing identities needed by the open edit session.
        for group in &mut replacement.groups {
            if group.mesh_share.is_none() {
                if let Some(old)=session.original_scene.groups.iter().find(|old|old.id==group.id) {
                    group.mesh_share=old.mesh_share;
                    if let Some(token)=old.mesh_share {replacement.mesh_shares.entry(token).or_insert_with(||group.mesh.clone());}
                }
            }
        }
        crate::document_presentation::preserve(&self.scene,&mut replacement);
        let annotation_create=next.history_annotation_create;
        let annotation_delete=next.history_annotation_delete||next.history_annotation_move||next.history_annotation_edit;
        let mut previous=session.original_scene.clone();previous.history_shareback=next.history_shareback;previous.history_annotation_delete=next.history_annotation_delete;previous.history_annotation_move=next.history_annotation_move;previous.history_annotation_edit=next.history_annotation_edit;previous.history_annotation_create=annotation_create;
        if annotation_create.is_some() {
            previous.text_labels=self.scene.text_labels.clone();
            crate::annotation_editor::preserve_creation_label_selection(&self.scene, &mut previous);
            crate::annotation_editor::preserve_creation_label_selection(&self.scene, &mut replacement);
            let (selection,order)=crate::annotation_editor::creation_selection(&self.scene,&replacement.text_labels);
            self.scene.text_labels=replacement.text_labels.clone();
            self.scene.selection=selection;
            self.scene.selection_order=order;
            replacement.history_annotation_create=None;
        }
        if annotation_delete {
            previous.guides=self.scene.guides.clone();previous.dimensions=self.scene.dimensions.clone();previous.text_labels=self.scene.text_labels.clone();previous.clear_selection();
            self.scene.guides=replacement.guides.clone();self.scene.dimensions=replacement.dimensions.clone();self.scene.text_labels=replacement.text_labels.clone();self.scene.clear_selection();
            replacement.history_annotation_edit=false;replacement.history_annotation_move=false;replacement.history_annotation_delete=false;replacement.clear_selection();
        }
        if next.history_shareback.is_none(){previous.layers=self.scene.layers.clone();previous.history_layers.0=crate::history_layers::restore(&self.scene,&mut replacement);self.scene.layers=replacement.layers.clone();previous.saved_views=self.scene.saved_views.clone();previous.history_views.0=crate::history_views::restore(&self.scene,&mut replacement);self.scene.saved_views=replacement.saved_views.clone();crate::history_terrain::restore(&mut self.scene,&mut replacement,&mut previous);self.scene.terrain=replacement.terrain.clone();previous.image_planes=self.scene.image_planes.clone();previous.history_images.0=crate::image_plane_actions::restore(&self.scene,&mut replacement);self.scene.image_planes=replacement.image_planes.clone();self.scene.image_objects=replacement.image_objects.clone();}
        if redo {self.redo_stack.pop();self.undo_stack.push(previous);} else {self.undo_stack.pop();self.redo_stack.push(previous);}
        let mut cancel_replacement=replacement.clone();
        if let Some(original)=session.cancel_scene.groups.iter().find(|g|g.id==session.group_id){
            if let Some(group)=cancel_replacement.groups.iter_mut().find(|g|g.id==session.group_id){*group=original.clone();}
        }
        self.component_edit_stack[index].cancel_scene=cancel_replacement;
        self.component_edit_stack[index].original_scene=replacement;
        // Cancel is a native session rollback, not source Undo. Retain genuine
        // ancestor-history changes rather than pairing their new parked mesh
        // with the obsolete pre-entry stacks. Exclude only this session's own
        // descendant commands from its cancellation checkpoint.
        for session in self.component_edit_stack.iter_mut().skip(index) {
            if annotation_delete || annotation_create.is_some() {
                for scene in [&mut session.original_scene,&mut session.cancel_scene] {
                    scene.guides=self.scene.guides.clone();scene.dimensions=self.scene.dimensions.clone();scene.text_labels=self.scene.text_labels.clone();
                }
            }
            let mut path=session.original_scene.history_context.clone();path.push(session.group_id);
            session.original_undo=self.undo_stack.iter().filter(|entry|!entry.history_context.starts_with(&path)).cloned().collect();
            session.original_redo=self.redo_stack.iter().filter(|entry|!entry.history_context.starts_with(&path)).cloned().collect();
        }
        self.bump_model_revision();self.status_message=if redo {"Redo"} else {"Undo"}.into();Some(true)
    }

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
            Ok(_)=>{self.seal_view_history_before(&before);self.seal_terrain_history_before(&before);self.seal_layer_history_before(&before);self.seal_image_history_before(&before);self.undo_stack.push(before);self.redo_stack.clear();self.status_message="Component made unique".into();},
            Err(e)=>self.status_message=e,
        }
    }

    pub fn begin_component_edit(&mut self) {
        self.sync_document_presentation();
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
            original_scene:self.scene.clone(),cancel_scene:self.scene.clone(),original_camera:self.camera.clone(),
            original_undo:self.undo_stack.clone(),original_redo:self.redo_stack.clone(),
            group_id:group.id,
            fold_history:group.children.is_empty() && (group.transform!=glam::DMat4::IDENTITY || group.component_definition.is_some() || group.native_document.get("xform").is_some_and(|v|v.is_array())),
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
        edit.history_context=self.scene.history_context.clone();edit.history_context.push(group.id);
        edit.materials=self.scene.materials.clone();edit.layers=self.scene.layers.clone();
        edit.unit=self.scene.unit;edit.embedded_textures=self.scene.embedded_textures.clone();
        // Original begin_group_edit swaps mesh and placements only; these are
        // document-global entities in the same world frame.
        edit.tile_layer=self.scene.tile_layer.clone();
        edit.unit=self.scene.unit;edit.model_units_override=Some(self.scene.model_units().clone());
        edit.dimension_style_override=(!self.scene.dimension_style().is_null()).then(||self.scene.dimension_style().clone());
        edit.display_style=self.scene.display_style.clone();edit.shadows=self.scene.shadows.clone();
        edit.show_hidden_objects=self.scene.show_hidden_objects;edit.show_hidden_geometry=self.scene.show_hidden_geometry;
        edit.image_planes=self.scene.image_planes.clone();edit.image_objects=self.scene.image_objects.clone();edit.guides=self.scene.guides.clone();edit.text_labels=self.scene.text_labels.clone();edit.geo_datum=self.scene.geo_datum.clone();edit.geo_points=self.scene.geo_points.clone();edit.geo_paths=self.scene.geo_paths.clone();edit.terrain=self.scene.terrain.clone();edit.sheets=self.scene.sheets.clone();edit.active_sheet_index=self.scene.active_sheet_index;edit.saved_views=self.scene.saved_views.clone();
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
        self.scene=edit;self.bump_model_revision();self.component_edit_stack.push(session);self.sync_bim_draw_activation();
        self.status_message="Editing group; Finish commits, Cancel restores".into();
    }

    pub fn finish_component_edit(&mut self) {
        self.sync_document_presentation();
        if self.component_edit_stack.is_empty() {self.status_message="No group edit is active".into();return;}
        let preserve_global_preview=self.active_tool.preserves_global_preview_on_context_exit();
        let mut finishing_scene=self.scene.clone();
        if !preserve_global_preview {self.active_tool.restore_transient_model(&mut finishing_scene);}
        let plan=match prepare_finish(&finishing_scene,&self.component_edit_stack,&self.undo_stack,&self.redo_stack) {
            Ok(plan)=>plan,Err(error)=>{self.status_message=error;return;}
        };
        let changed=plan.changed;
        let history_head_retained=!plan.fold || !plan.changed;
        // Validation precedes any transient mutation. Expire an already stale
        // retype window before deciding whether its global command survives.
        self.active_tool.note_history_revision(self.model_revision,false);
        let preserve_global_preview=self.active_tool.preserves_global_preview_on_context_exit();
        let camera=self.component_edit_stack.last().unwrap().original_camera.clone();
        if !preserve_global_preview {self.set_tool(ToolType::Select);}else{self.active_tool.detach_context_preview();}
        publish_finish(plan,&mut self.scene,&mut self.component_edit_stack,&mut self.undo_stack,&mut self.redo_stack);
        self.bump_model_revision();
        self.active_tool.note_history_revision(self.model_revision,history_head_retained && preserve_global_preview);
        self.camera=camera;self.sync_bim_draw_activation();
        self.status_message=if changed {"Committed group edit"}else{"Group edit unchanged; pending history targets validated"}.into();
    }

    /// Prepare the complete exit on model/history state only. No live context,
    /// tool or inference state changes if any outer publication would refuse.
    pub(crate) fn finish_contexts_for_document(&mut self) -> bool {
        if self.component_edit_stack.is_empty() {return true;}
        self.sync_document_presentation();
        let mut scene=self.scene.clone();
        let preserve=self.active_tool.preserves_global_preview_on_context_exit();
        if !preserve {self.active_tool.restore_transient_model(&mut scene);}
        let mut stack=self.component_edit_stack.iter().map(ComponentEditSession::finish_copy).collect::<Vec<_>>();
        let first=match prepare_finish(&scene,&stack,&self.undo_stack,&self.redo_stack) {
            Ok(plan)=>plan,Err(error)=>{self.status_message=error;return false;}
        };
        let mut undo=Vec::new();let mut redo=Vec::new();
        publish_finish(first,&mut scene,&mut stack,&mut undo,&mut redo);
        while !stack.is_empty() {
            let plan=match prepare_finish(&scene,&stack,&undo,&redo) {
                Ok(plan)=>plan,Err(error)=>{self.status_message=error;return false;}
            };
            publish_finish(plan,&mut scene,&mut stack,&mut undo,&mut redo);
        }
        // Retire context-bound acquisition only after every candidate passed.
        // The selected tool and current camera stay active for the document.
        // Paste owns a separate immutable world-space preview while Select is
        // active. Save exits world-space edit contexts without cancelling the
        // original Paste tool, including picker cancellation and write failure.
        // Move that owner only after all Finish candidates have been admitted.
        let paste_preview=self.paste_preview.take();
        if preserve {self.active_tool.detach_context_preview();}else{self.set_tool(self.active_tool_type);}
        self.paste_preview=paste_preview;
        scene.clear_selection();self.scene=scene;self.undo_stack=undo;self.redo_stack=redo;
        self.component_edit_stack.clear();self.bump_model_revision();self.sync_bim_draw_activation();true
    }

    pub fn cancel_component_edit(&mut self) {
        self.preserve_blender_cancel_state();
        self.sync_document_presentation();
        if let Some(mut session)=self.component_edit_stack.pop() {
            self.set_tool(ToolType::Select);
            crate::document_presentation::preserve(&self.scene,&mut session.cancel_scene);self.scene=session.cancel_scene;self.bump_model_revision();self.camera=session.original_camera;
            self.undo_stack=session.original_undo;self.redo_stack=session.original_redo;
            self.sync_bim_draw_activation();
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
    fn shareback_retains_annotations_and_ordinary_annotation_history_stays_reversible() {
        let mut app=app();app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(DVec3::ZERO,DVec3::X,DVec3::Y));app.scene.bind_dimensions();
        app.begin_component_edit();app.save_undo_state();app.scene.mesh.vertices[1].position.x=4.;app.scene.refresh_dimensions();
        app.save_undo_state();let mut dimension=ingetrazo_core::dimensions::Dimension::new(DVec3::ZERO,DVec3::new(4.,0.,0.),DVec3::Y*2.);dimension.text_override=Some("Local <>".into());let id=dimension.id;app.scene.dimensions.push(dimension);app.scene.bind_dimensions();app.finish_component_edit();
        assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.dimensions[0].end,DVec3::X);assert!(app.scene.dimensions[0].anchor_b.is_none());
        for _ in 0..2 {app.undo();assert_eq!(app.scene.groups[0].mesh.vertices[1].position,DVec3::X);assert_eq!(app.scene.groups[1].mesh.vertices[1].position,DVec3::X);assert_eq!(app.scene.dimensions[1].id,id);assert_eq!(app.scene.dimensions[1].end,DVec3::new(4.,0.,0.));assert!(app.scene.dimensions[1].anchor_b.is_none());app.redo();assert_eq!(app.scene.groups[0].mesh.vertices[1].position,DVec3::new(4.,0.,0.));assert!(app.scene.dimensions[1].anchor_b.is_none());}
        assert!(app.edit_dimension(1,Some("Independent".into()),None,None));app.undo();assert_eq!(app.scene.dimensions[1].text_override.as_deref(),Some("Local <>"));app.redo();assert_eq!(app.scene.dimensions[1].text_override.as_deref(),Some("Independent"));
        app.undo();app.undo();assert!(app.scene.history_shareback.is_none());app.save_undo_state();app.scene.dimensions[1].text_override=Some("Branch".into());assert!(app.redo_stack.is_empty());assert!(app.undo_stack.last().unwrap().history_shareback.is_none());
        let mut snapshot=app.scene.clone();snapshot.history_shareback=Some(app.scene.groups[0].id);let encoded=serde_json::to_value(&snapshot).unwrap();assert!(encoded.get("history_shareback").is_none());let decoded:Scene=serde_json::from_value(encoded).unwrap();assert!(decoded.history_shareback.is_none());
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
        app.save_undo_state();app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(DVec3::new(3.,4.,5.),DVec3::new(4.,4.,5.),DVec3::Y));app.scene.bind_dimensions();
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
        let committed=app.scene.clone();app.undo();
        // Source Reshare restores prototype geometry; it does not restore
        // selection or rewind document version counters as a Scene snapshot.
        let mut expected=before;expected.selection=app.scene.selection.clone();expected.selection_order=app.scene.selection_order.clone();expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);assert!(app.scene.history_shareback.is_none());
        app.redo();let mut expected=committed;expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);assert!(app.scene.history_shareback.is_none());
    }
    #[test]
    fn cancel_preserves_document_and_history() {
        let mut app=app();app.save_undo_state();let before=app.scene.clone();let depth=app.undo_stack.len();
        app.begin_component_edit();app.save_undo_state();app.scene.mesh.vertices[0].position.z=9.;
        app.cancel_component_edit();assert_eq!(app.scene,before);assert_eq!(app.undo_stack.len(),depth);
    }
    #[test]
    fn save_refusal_at_outer_level_preserves_all_contexts_history_and_tool_acquisition() {
        let mut a=app();
        let mut parent=Group::new("outer");parent.children.push(a.scene.groups[0].duplicate_instance());
        a.scene.groups.push(parent);a.scene.clear_selection();a.scene.select(SelectionItem::Group(2));
        a.begin_component_edit();a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
        a.save_undo_state();a.scene.mesh.vertices[0].position.z=2.;
        a.set_tool(ToolType::Rectangle);
        let ctx=ingetrazo_tools::base::ToolContext::new(DVec3::new(3.,4.,0.),ingetrazo_core::math::Ray::new(DVec3::new(3.,4.,10.),-DVec3::Z));
        a.dispatch_tool_click(&ctx);
        let anchor=a.active_tool.snap_anchor();assert!(anchor.is_some());
        // Inner candidate is valid; only the later outer publication refuses.
        a.component_edit_stack[0].original_scene.groups.clear();
        assert!(prepare_finish(&a.scene,&a.component_edit_stack,&a.undo_stack,&a.redo_stack).is_ok());
        let before=a.scene.clone();let undo=a.undo_stack.clone();let redo=a.redo_stack.clone();let camera=a.camera.clone();
        let parents=a.component_edit_stack.iter().map(|s|(s.original_scene.clone(),s.cancel_scene.clone())).collect::<Vec<_>>();
        let path=std::env::temp_dir().join(format!("outer-save-refusal-{}.igz",uuid::Uuid::new_v4()));
        a.save_to_path(&path);
        assert!(!path.exists());assert_eq!(a.component_edit_stack.len(),2);assert!(a.status_message.contains("no longer exists"));
        assert_eq!(a.scene,before);assert_eq!(a.undo_stack,undo);assert_eq!(a.redo_stack,redo);assert_eq!(a.camera,camera);
        assert_eq!(a.active_tool_type,ToolType::Rectangle);assert_eq!(a.active_tool.snap_anchor(),anchor);
        assert_eq!(a.component_edit_stack.iter().map(|s|(s.original_scene.clone(),s.cancel_scene.clone())).collect::<Vec<_>>(),parents);
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
        let mut expected=before.clone();
        // Actual original nested-parent-bake-survives-undo-False control:
        // entry axes survive Undo of the shared leaf edit.
        expected.groups[2].native_document=serde_json::json!({"axes":DMat4::IDENTITY.to_cols_array()});
        app.undo();expected.selection=app.scene.selection.clone();expected.selection_order=app.scene.selection_order.clone();expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);assert!(app.scene.history_shareback.is_none());
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

#[cfg(test)]mod change_axes_admission_tests{
 use super::*;use glam::{DMat4,DVec3};use ingetrazo_core::{groups::Group,mesh::Mesh,math::Ray};use ingetrazo_tools::ToolContext;
 #[test]fn parked_refusal_preserves_frame_history_and_retries(){
 let mut app=IngeTrazoApp::new();let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]);let share=uuid::Uuid::new_v4();app.scene.register_mesh_share(share,mesh.clone()).unwrap();let mut target=Group::with_mesh("Target",mesh.clone());target.mesh_share=Some(share);target.transform=DMat4::from_translation(DVec3::X);let mut sibling=target.duplicate_instance();sibling.name="Sibling".into();sibling.transform=DMat4::from_translation(DVec3::new(10.,0.,0.));let mut parent=Group::new("Parent");parent.children.push(target);app.scene.groups=vec![parent,sibling];app.scene.selection.insert(SelectionItem::Group(0));app.begin_component_edit();app.scene.selection.insert(SelectionItem::Group(0));assert!(app.start_change_axes());let click=|app:&mut IngeTrazoApp,p:DVec3|app.dispatch_tool_click(&ToolContext::new(p,Ray::new(p+DVec3::Z,-DVec3::Z)));click(&mut app,DVec3::ZERO);click(&mut app,DVec3::Y);let retained=app.component_edit_stack[0].original_scene.mesh_shares.remove(&share).unwrap();let before=app.scene.clone();let parked=app.component_edit_stack[0].original_scene.clone();click(&mut app,-DVec3::X);assert_eq!(app.scene,before);assert_eq!(app.component_edit_stack[0].original_scene,parked);assert!(app.undo_stack.is_empty());assert_eq!(app.active_tool_type,ToolType::ChangeAxes);app.component_edit_stack[0].original_scene.mesh_shares.insert(share,retained);click(&mut app,-DVec3::X);assert_eq!(app.active_tool_type,ToolType::Select);assert_eq!(app.undo_stack.len(),1);
 }
}
