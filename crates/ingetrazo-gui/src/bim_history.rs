//! Atomic direct BIM metadata across immutable active/parked mesh identities.
use crate::IngeTrazoApp;
use ingetrazo_core::{bim::{MetadataHistory,MetadataTarget},groups::Group,mesh::Mesh,scene::{Scene,SelectionItem}};
use serde_json::Value;
fn path_group<'a>(groups:&'a mut [Group],path:&[uuid::Uuid])->Option<&'a mut Group>{
    let (first,rest)=path.split_first()?;let group=groups.iter_mut().find(|g|g.id==*first)?;
    if rest.is_empty(){Some(group)}else{path_group(&mut group.children,rest)}
}
fn owner_context(scenes:&[Scene],context:&[uuid::Uuid])->Result<usize,String>{
    scenes.iter().enumerate().filter(|(_,s)|context.starts_with(&s.history_context)).max_by_key(|(_,s)|s.history_context.len()).map(|(i,_)|i).ok_or_else(||"BIM document context no longer resolves".into())
}
fn target_group<'a>(scenes:&'a mut [Scene],target:&MetadataTarget)->Result<&'a mut Group,String>{
    let owner=owner_context(scenes,&target.context)?;let prefix=scenes[owner].history_context.len();
    let groups=if prefix==target.context.len(){&mut scenes[owner].groups}else{&mut path_group(&mut scenes[owner].groups,&target.context[prefix..]).ok_or("BIM placement context no longer resolves")?.children};
    groups.iter_mut().find(|g|Some(g.id)==target.group).ok_or_else(||"BIM group target no longer resolves".into())
}
fn face_mesh<'a>(scenes:&'a mut [Scene],context:&[uuid::Uuid])->Result<(&'a mut Mesh,Option<(usize,uuid::Uuid)>),String>{
    let owner=owner_context(scenes,context)?;let prefix=scenes[owner].history_context.len();
    if prefix==context.len(){return Ok((&mut scenes[owner].mesh,None));}
    let group=path_group(&mut scenes[owner].groups,&context[prefix..]).ok_or("BIM face context no longer resolves")?;
    let token=group.mesh_share;Ok((&mut group.mesh,token.map(|t|(owner,t))))
}
fn same_ring(a:&[uuid::Uuid],b:&[uuid::Uuid])->bool{
    a.len()==b.len()&&!a.is_empty()&&(0..a.len()).any(|i|a.iter().enumerate().all(|(j,v)|*v==b[(i+j)%b.len()]))
}
fn face_index(mesh:&Mesh,vertices:&[uuid::Uuid])->Result<usize,String>{
    let mut found=None;
    for (i,f)in mesh.faces.iter().enumerate(){let ids=f.vertices.iter().map(|&v|mesh.vertices.get(v).map(|v|v.uid).ok_or("BIM face references a missing vertex; no tags changed")).collect::<Result<Vec<_>,_>>()?;if same_ring(&ids,vertices){if found.is_some(){return Err("BIM face identity is ambiguous; no tags changed".into());}found=Some(i);}}
    found.ok_or_else(||"BIM face target no longer resolves; no tags changed".into())
}
fn missing_face_target(error:&str)->bool{error=="BIM face target no longer resolves; no tags changed"}
fn read(scenes:&mut [Scene],target:&MetadataTarget)->Result<Option<Value>,String>{
    if target.group.is_some(){return Ok(target_group(scenes,target)?.ifc_tag.clone());}
    let (mesh,_)=face_mesh(scenes,&target.context)?;let face=face_index(mesh,&target.vertices)?;Ok(mesh.faces[face].attrs.get("ifc").cloned())
}
fn write(scenes:&mut [Scene],target:&MetadataTarget,new:Option<Value>)->Result<(),String>{
    if target.group.is_some(){target_group(scenes,target)?.ifc_tag=new;return Ok(());}
    let (mesh,publish)=face_mesh(scenes,&target.context)?;let face=face_index(mesh,&target.vertices)?;
    if let Some(v)=new{mesh.faces[face].attrs.insert("ifc".into(),v);}else{mesh.faces[face].attrs.remove("ifc");}
    if let Some((root,token))=publish{let mesh=mesh.clone();scenes[root].publish_mesh_share(token,mesh)?;}
    Ok(())
}
impl IngeTrazoApp{
    fn bim_history_scenes(&self)->Vec<Scene>{std::iter::once(self.scene.clone()).chain(self.component_edit_stack.iter().map(|s|s.original_scene.clone())).collect()}
    fn publish_bim_history_scenes(&mut self,mut scenes:Vec<Scene>){
        let mut active=scenes.remove(0);active.history_bim=None;active.bump_version();self.scene=active;
        for (session,mut scene)in self.component_edit_stack.iter_mut().zip(scenes){scene.history_bim=None;scene.bump_version();session.original_scene=scene;}
        self.bump_model_revision();self.sync_bim_draw_activation();
    }
    /// Only mixed targets need this direct metadata command. Existing single
    /// domain commands keep their established History policy and ownership.
    pub(crate) fn tag_mixed_bim_targets(&mut self,tag:Option<(&str,&str)>)->Option<bool>{
        let parked=self.scene.selection.iter().any(|s|matches!(s,SelectionItem::ContextFace{..}|SelectionItem::ContextGroup{..}));
        let active=self.scene.selection.iter().any(|s|matches!(s,SelectionItem::Face(_)|SelectionItem::Group(_)));
        if !parked||!active{return None;}
        let result=(||->Result<(),String>{
            if self.active_tool.placement_preview().is_some(){return Err("Finish or cancel the placement preview before tagging mixed BIM contexts".into());}
            if self.component_edit_stack.is_empty(){return Err("BIM parked target is no longer active".into());}
            let mut scenes=self.bim_history_scenes();let root=scenes.iter().position(|s|s.history_context.is_empty()).ok_or("BIM document is no longer active")?;
            let id=if tag.is_some(){Some(ingetrazo_core::bim::next_draw_object_id(&scenes[root].mesh)?)}else{None};
            let mut targets=Vec::new();
            for selected in &self.scene.selection{
                let (context,group,vertices)=match selected{
                    SelectionItem::ContextFace{context,vertices,..}=>(context.clone(),None,vertices.clone()),
                    SelectionItem::ContextGroup{context,group}=>(context.clone(),Some(*group),vec![]),
                    SelectionItem::Face(face)=>{let face=self.scene.mesh.faces.get(*face).ok_or("BIM face target no longer resolves")?;(self.scene.history_context.clone(),None,face.vertices.iter().map(|&v|self.scene.mesh.vertices.get(v).map(|v|v.uid).ok_or("BIM face references a missing vertex; no tags changed")).collect::<Result<Vec<_>,_>>()?)},
                    SelectionItem::Group(index)=>(self.scene.history_context.clone(),Some(self.scene.groups.get(*index).ok_or("BIM group target no longer resolves")?.id),vec![]),
                    _=>return Err("Select only BIM faces/groups to tag mixed contexts".into()),
                };
                let mut target=MetadataTarget{context,group,vertices,value:None,expected:None};target.value=read(&mut scenes,&target)?;
                let group_name=if group.is_some(){Some(target_group(&mut scenes,&target)?.name.clone())}else{None};
                target.expected=tag.map(|(class,name)|if group.is_some(){serde_json::json!({"class":class,"name":if name.trim().is_empty(){group_name.as_deref().unwrap()}else{name.trim()}})}else{serde_json::json!({"class":class,"name":name.trim(),"id":id.unwrap()})});
                // Resolve every target before any native publication.
                targets.push(target);
            }
            for target in &targets{write(&mut scenes,target,target.expected.clone())?;}
            if targets.iter().all(|t|t.value==t.expected){return Ok(());}
            let mut before=self.scene.clone();before.history_bim=Some(MetadataHistory{targets});
            self.undo_stack.try_reserve(1).map_err(|_|"BIM history admission failed")?;
            self.undo_stack.push(before);self.redo_stack.clear();self.publish_bim_history_scenes(scenes);Ok(())
        })();
        match result{Ok(())=>{self.status_message="Tagged active and parked BIM targets".into();Some(true)},Err(e)=>{self.status_message=e;Some(false)}}
    }
    pub(crate) fn bim_metadata_history_step(&mut self,redo:bool)->Option<bool>{
        let snapshot=(if redo{self.redo_stack.last()}else{self.undo_stack.last()})?;let history=snapshot.history_bim.clone()?;let shareback=snapshot.history_shareback;let snapshot=snapshot.clone();
        let result=(||->Result<(),String>{
            if self.active_tool.placement_preview().is_some(){return Err("Finish or cancel the placement preview before BIM metadata Undo/Redo".into());}
            let mut scenes=self.bim_history_scenes();
            for target in &history.targets{match read(&mut scenes,target){Ok(value)if value==target.expected=>(),Err(e)if missing_face_target(&e)&&shareback.is_some_and(|id|target.context.last()==Some(&id))=>(),_=>return Err("BIM metadata changed outside this command; Undo/Redo refused".into())}}
            if shareback.is_some(){scenes[0]=crate::component_edit::shareback_restore(&scenes[0],&snapshot)?;}
            for target in &history.targets{
                // A source whole-prototype Undo legitimately removes newly
                // authored faces. Its metadata target is restored by Redo's
                // retained prototype; parked targets remain mandatory.
                if shareback.is_some_and(|id|target.context.last()==Some(&id))&&read(&mut scenes,target).is_err_and(|e|missing_face_target(&e)){continue;}
                write(&mut scenes,target,target.value.clone())?;
            }
            let mut previous=self.scene.clone();previous.history_shareback=shareback;previous.history_bim=Some(MetadataHistory{targets:history.targets.iter().map(|t|MetadataTarget{context:t.context.clone(),group:t.group,vertices:t.vertices.clone(),value:t.expected.clone(),expected:t.value.clone()}).collect()});
            if redo{self.undo_stack.try_reserve(1).map_err(|_|"BIM history admission failed")?;self.redo_stack.pop();self.undo_stack.push(previous);}else{self.redo_stack.try_reserve(1).map_err(|_|"BIM history admission failed")?;self.undo_stack.pop();self.redo_stack.push(previous);}
            self.publish_bim_history_scenes(scenes);Ok(())
        })();match result{Ok(())=>Some(true),Err(e)=>{self.status_message=e;Some(false)}}
    }
}
