use crate::IngeTrazoApp;use glam::DVec3;use ingetrazo_core::scene::Scene;
pub struct PastePreview {pub scene:Scene,pub reference:DVec3,pub offset:DVec3}
/// Fail before mutation when a foreign document uses a resource name for different data.
pub(crate) fn merge_resources(destination:&mut Scene,source:&Scene)->Result<(),String>{
    for(name,bytes)in &source.embedded_textures {if destination.embedded_textures.get(name).is_some_and(|existing|existing!=bytes){return Err(format!("Clipboard asset name conflict: {name}"));}}
    for(name,material)in &source.materials {if destination.materials.get(name).is_some_and(|existing|existing!=material){return Err(format!("Clipboard material name conflict: {name}"));}}
    for(name,bytes)in &source.embedded_textures{destination.embedded_textures.entry(name.clone()).or_insert_with(||bytes.clone());}
    for(name,material)in &source.materials{destination.materials.entry(name.clone()).or_insert_with(||material.clone());}
    for(id,definition)in &source.component_definitions{destination.component_definitions.entry(*id).or_insert_with(||definition.clone());}
    for layer in &source.layers{if !destination.layers.iter().any(|existing|existing.name==layer.name){destination.layers.push(layer.clone());}}
    Ok(())
}
pub(crate) fn merge_pasted_planes(original:&ingetrazo_core::mesh::Mesh,pasted:&ingetrazo_core::mesh::Mesh,faces:&[usize])->Result<ingetrazo_core::mesh::Mesh,String>{
    use ingetrazo_core::math::newell_vector;
    let mut overlaps=vec![];let mut source_planes=std::collections::HashSet::new();
    for &id in faces{let face=&pasted.faces[id];let points=face.vertices.iter().map(|&v|pasted.vertices[v].position).collect::<Vec<_>>();let normal=newell_vector(&points).normalize_or_zero();if normal.length_squared()<0.5{continue;}let mut canonical=normal;if canonical.z<0.||(canonical.z==0.&&(canonical.y<0.||(canonical.y==0.&&canonical.x<0.))){canonical=-canonical;}let d=canonical.dot(points[0]);source_planes.insert([(canonical.x*1e4).round()as i64,(canonical.y*1e4).round()as i64,(canonical.z*1e4).round()as i64,(d*1e4).round()as i64]);if source_planes.len()>24{return Ok(pasted.clone());}let lo=points.iter().copied().reduce(DVec3::min).unwrap();let hi=points.iter().copied().reduce(DVec3::max).unwrap();
        if original.faces.iter().any(|existing|{let positions=existing.vertices.iter().map(|&v|original.vertices[v].position).collect::<Vec<_>>();if positions.iter().any(|p|normal.dot(*p-points[0]).abs()>=1e-4){return false;}let Some(flo)=positions.iter().copied().reduce(DVec3::min)else{return false;};let fhi=positions.iter().copied().reduce(DVec3::max).unwrap();(0..3).all(|axis|(flo[axis]<hi[axis]-1e-4&&lo[axis]<fhi[axis]-1e-4)||((fhi[axis]-flo[axis]).abs()<1e-4&&(hi[axis]-lo[axis]).abs()<1e-4))}){overlaps.push(id);}
    }
    let mut planes=std::collections::HashSet::new();overlaps.retain(|&id|{let mut normal=pasted.face_normal(id);if normal.z<0.||(normal.z==0.&&(normal.y<0.||(normal.y==0.&&normal.x<0.))){normal=-normal;}let distance=normal.dot(pasted.vertices[pasted.faces[id].vertices[0]].position);planes.insert([(normal.x*1e4).round()as i64,(normal.y*1e4).round()as i64,(normal.z*1e4).round()as i64,(distance*1e4).round()as i64])});if planes.len()>24{return Ok(pasted.clone());}
    let mut result=pasted.clone();for id in overlaps{let face=&pasted.faces[id];let origin=pasted.vertices[face.vertices[0]].position;let normal=pasted.face_normal(id);if let Some(current)=result.faces.iter().position(|face|face.vertices.iter().all(|&v|normal.dot(result.vertices[v].position-origin).abs()<1e-4)){result=ingetrazo_core::planar::rebuild_faces(&result,&[current])?;}}Ok(result)
}
pub(crate) fn remap_resources(payload:&mut crate::app::GeometryClipboard,destination:&Scene,document:uuid::Uuid){
    use std::collections::HashMap;
    if payload.source_document==document{
        for(name,material)in &mut payload.resources.materials{if let Some(current)=destination.materials.get(name){*material=current.clone();}}
        for(name,bytes)in &mut payload.resources.embedded_textures{if let Some(current)=destination.embedded_textures.get(name){*bytes=current.clone();}}
        for(id,definition)in &mut payload.resources.component_definitions{if let Some(current)=destination.component_definitions.get(id){*definition=current.clone();}}
    }
    let mut assets=HashMap::new();let mut materials=HashMap::new();
    let asset_names:std::collections::HashSet<_>=payload.resources.embedded_textures.keys().cloned().collect();let mut allocated_assets=std::collections::HashSet::new();
    for(name,bytes)in &payload.resources.embedded_textures{let mut target=name.clone();let mut i=1;while destination.embedded_textures.get(&target).is_some_and(|existing|existing!=bytes)||(target!=*name&&asset_names.contains(&target))||allocated_assets.contains(&target){target=format!("{name} (clipboard {i})");i+=1;}allocated_assets.insert(target.clone());assets.insert(name.clone(),target);}
    fn values(value:&mut serde_json::Value,assets:&HashMap<String,String>,materials:&HashMap<String,String>){
        if let Some(object)=value.as_object_mut(){for key in ["mat","material"]{if let Some(value)=object.get_mut(key){if let Some(target)=value.as_str().and_then(|name|materials.get(name)){*value=serde_json::json!(target);}}}if let Some(texture)=object.get_mut("texture"){texture_refs(texture,assets);}}
    }
    fn texture_refs(texture:&mut serde_json::Value,assets:&HashMap<String,String>){if let Some(object)=texture.as_object_mut(){for key in ["embed","path"]{if let Some(value)=object.get_mut(key){if let Some(target)=value.as_str().and_then(|name|assets.get(name)){*value=serde_json::json!(target);}}}}}
    let material_names:std::collections::HashSet<_>=payload.resources.materials.keys().cloned().collect();let mut allocated_materials=std::collections::HashSet::new();
    for(name,material)in &mut payload.resources.materials{if let Some(embed)=&mut material.texture_embed{if let Some(target)=assets.get(embed){*embed=target.clone();}}if let Some(path)=&mut material.texture_path{if let Some(target)=assets.get(path){*path=target.clone();}}values(&mut material.native_document,&assets,&HashMap::new());let mut target=name.clone();let mut i=1;while destination.materials.get(&target).is_some_and(|existing|{let mut compared=material.clone();compared.name=target.clone();existing!=&compared})||(target!=*name&&material_names.contains(&target))||allocated_materials.contains(&target){target=format!("{name} (clipboard {i})");i+=1;}allocated_materials.insert(target.clone());materials.insert(name.clone(),target);}
    let mut definitions=HashMap::new();if payload.source_document!=document{for(id,definition)in &payload.resources.component_definitions{
        use std::hash::{Hash,Hasher};let mut hash=std::collections::hash_map::DefaultHasher::new();serde_json::to_value(definition).unwrap_or_default().to_string().hash(&mut hash);let target=uuid::Uuid::from_u128(id.as_u128()^document.as_u128()^u128::from(hash.finish()));definitions.insert(*id,target);
    }}
    fn mesh(mesh:&mut ingetrazo_core::mesh::Mesh,assets:&HashMap<String,String>,materials:&HashMap<String,String>){for face in &mut mesh.faces{for(key,value)in &mut face.attrs{if key=="mat"||key=="material"{if let Some(target)=value.as_str().and_then(|name|materials.get(name)){*value=serde_json::json!(target);}}if key=="texture"{texture_refs(value,assets);}}}}
    fn groups(groups:&mut[ingetrazo_core::groups::Group],assets:&HashMap<String,String>,materials:&HashMap<String,String>,defs:&HashMap<uuid::Uuid,uuid::Uuid>){for group in groups{if let Some(id)=group.component_definition{group.component_definition=Some(*defs.get(&id).unwrap_or(&id));}mesh(&mut group.mesh,assets,materials);values(&mut group.native_document,assets,materials);if let Some(paint)=group.native_document.get_mut("material"){values(paint,assets,materials);}if let Some(texture)=&mut group.billboard_texture{if let Some(target)=assets.get(texture){*texture=target.clone();}}groups_children(&mut group.children,assets,materials,defs);}}
    fn groups_children(children:&mut[ingetrazo_core::groups::Group],assets:&HashMap<String,String>,materials:&HashMap<String,String>,defs:&HashMap<uuid::Uuid,uuid::Uuid>){groups(children,assets,materials,defs)}
    mesh(&mut payload.mesh,&assets,&materials);groups(&mut payload.groups,&assets,&materials,&definitions);
    payload.resources.embedded_textures=std::mem::take(&mut payload.resources.embedded_textures).into_iter().map(|(name,bytes)|(assets[&name].clone(),bytes)).collect();
    payload.resources.materials=std::mem::take(&mut payload.resources.materials).into_iter().map(|(name,mut material)|{let target=materials[&name].clone();material.name=target.clone();(target,material)}).collect();
    payload.resources.component_definitions=std::mem::take(&mut payload.resources.component_definitions).into_iter().map(|(id,mut definition)|{mesh(&mut definition.mesh,&assets,&materials);groups(&mut definition.children,&assets,&materials,&definitions);(*definitions.get(&id).unwrap_or(&id),definition)}).collect();
}
pub(crate) fn validate_payload(payload:&crate::app::GeometryClipboard)->Result<(),String>{
    fn mesh(mesh:&ingetrazo_core::mesh::Mesh)->Result<(),String>{
        let n=mesh.vertices.len();if mesh.vertices.iter().any(|v|!v.position.is_finite())||mesh.edges.iter().any(|e|e.v0>=n||e.v1>=n)||mesh.faces.iter().any(|f|std::iter::once(&f.vertices).chain(&f.holes).any(|ring|ring.len()<3||ring.iter().any(|v|*v>=n))){return Err("Malformed clipboard geometry".into());}Ok(())
    }
    fn groups(groups:&[ingetrazo_core::groups::Group],defs:&std::collections::HashMap<uuid::Uuid,ingetrazo_core::components::ComponentDefinition>,stack:&mut Vec<uuid::Uuid>,depth:usize)->Result<(),String>{
        if depth>128{return Err("Clipboard nesting exceeds limit".into());}
        for g in groups{if !g.transform.is_finite()||g.transform.determinant().abs()<1e-12{return Err("Invalid clipboard transform".into());}mesh(&g.mesh)?;groups_children(&g.children,defs,stack,depth)?;if let Some(id)=g.component_definition{if stack.contains(&id){return Err("Recursive clipboard component".into());}let def=defs.get(&id).ok_or("Missing clipboard component")?;stack.push(id);mesh(&def.mesh)?;groups_children(&def.children,defs,stack,depth)?;stack.pop();}}Ok(())
    }
    fn groups_children(children:&[ingetrazo_core::groups::Group],defs:&std::collections::HashMap<uuid::Uuid,ingetrazo_core::components::ComponentDefinition>,stack:&mut Vec<uuid::Uuid>,depth:usize)->Result<(),String>{groups(children,defs,stack,depth+1)}
    if !payload.reference.is_finite(){return Err("Invalid clipboard reference".into());}mesh(&payload.mesh)?;groups(&payload.groups,&payload.resources.component_definitions,&mut vec![],0)?;
    for(id,def)in &payload.resources.component_definitions{mesh(&def.mesh)?;groups(&def.children,&payload.resources.component_definitions,&mut vec![*id],0)?;}Ok(())
}
const CLIPBOARD_PREFIX:&str="IngeTrazo-Rust-geometry-v1\n";
impl IngeTrazoApp {
    pub(crate) fn write_system_clipboard(&mut self){
        if let (Some(system),Some(payload))=(&mut self.system_clipboard,&self.clipboard){if let Ok(encoded)=serde_json::to_string(payload){let _=system.set_text(format!("{CLIPBOARD_PREFIX}{encoded}"));}}
    }
    pub(crate) fn read_system_clipboard(&mut self){
        if let Some(system)=&mut self.system_clipboard{if let Ok(text)=system.get_text(){if let Some(encoded)=text.strip_prefix(CLIPBOARD_PREFIX){self.clipboard=None;match serde_json::from_str(encoded){Ok(payload)=>match validate_payload(&payload){Ok(())=>self.clipboard=Some(payload),Err(error)=>self.status_message=error},Err(_)=>self.status_message="Malformed IngeTrazo clipboard payload".into()}}else{self.clipboard=None;}}}
    }

    pub fn begin_paste_preview(&mut self) {
        self.paste_preview=None;
        self.read_system_clipboard();
        let Some(mut clipboard)=self.clipboard.clone() else{return;};
        if let Err(e)=validate_payload(&clipboard){self.status_message=e;return;}
        if clipboard.mesh.faces.is_empty()&&clipboard.mesh.edges.is_empty()&&clipboard.groups.is_empty(){return;}
        remap_resources(&mut clipboard,&self.scene,self.clipboard_document);
        self.set_tool(ingetrazo_tools::base::ToolType::Select);
        let inverse=self.scene.dimension_frame.inverse();
        let mesh=match ingetrazo_core::containers::transform_mesh(&clipboard.mesh,inverse){Ok(mesh)=>mesh,Err(e)=>{self.status_message=e;return;}};
        let mut scene=Scene::new();scene.mesh=mesh;scene.groups=clipboard.groups;
        for group in &mut scene.groups{group.transform=inverse*group.transform;}
        scene.materials=self.scene.materials.clone();scene.layers=self.scene.layers.clone();
        if let Err(e)=merge_resources(&mut scene,&clipboard.resources){self.status_message=e;return;}
        let definitions=scene.component_definitions.clone();for(id,definition)in definitions{if let Err(error)=scene.replace_component_definition(id,definition){self.status_message=error;return;}}
        self.paste_preview=Some(PastePreview{scene,reference:inverse.transform_point3(clipboard.reference),offset:DVec3::ZERO});
        self.status_message="Place copied geometry; click to stamp, Esc to cancel".into();
    }
    pub fn update_paste_preview(&mut self,world:DVec3){if world.is_finite(){if let Some(preview)=&mut self.paste_preview{preview.offset=world-preview.reference;}}}
    pub fn cancel_paste_preview(&mut self){self.paste_preview=None;self.status_message="Paste cancelled".into();}
    pub fn commit_paste_preview(&mut self)->bool {
        let Some(preview)=self.paste_preview.take() else{return false;};
        if self.paste_internal(preview.offset){self.status_message="Pasted selection".into();true}else{self.paste_preview=Some(preview);false}
    }
}

// Clipboard acceptance tests, independent of serialized raw snapshots.
#[cfg(test)]
mod tests {
    use crate::IngeTrazoApp;use glam::DVec3;use ingetrazo_core::scene::SelectionItem;
    #[test]
    fn resource_allocator_reserves_incoming_names_and_leaves_opaque_fields(){
        let mut payload=crate::app::GeometryClipboard::default();payload.resources.embedded_textures.insert("foo".into(),vec![1]);payload.resources.embedded_textures.insert("foo (clipboard 1)".into(),vec![2]);payload.resources.materials.insert("Paint".into(),ingetrazo_core::materials::Material::new("Paint",[1.,0.,0.,1.]));payload.resources.materials.insert("Paint (clipboard 1)".into(),ingetrazo_core::materials::Material::new("Paint (clipboard 1)",[0.,1.,0.,1.]));let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|payload.mesh.add_vertex(p));payload.mesh.add_face(ids.to_vec(),vec![]);payload.mesh.faces[0].attrs.insert("extension".into(),serde_json::json!({"path":"foo","material":"Paint","nested":{"embed":"foo"}}));payload.mesh.faces[0].attrs.insert("texture".into(),serde_json::json!({"embed":"foo"}));let opaque=payload.mesh.faces[0].attrs["extension"].clone();let mut destination=ingetrazo_core::scene::Scene::new();destination.embedded_textures.insert("foo".into(),vec![9]);destination.materials.insert("Paint".into(),ingetrazo_core::materials::Material::new("Paint",[0.,0.,1.,1.]));super::remap_resources(&mut payload,&destination,uuid::Uuid::new_v4());assert_eq!(payload.resources.embedded_textures.len(),2);assert_eq!(payload.resources.embedded_textures["foo (clipboard 1)"],vec![2]);assert_eq!(payload.resources.embedded_textures["foo (clipboard 2)"],vec![1]);assert_eq!(payload.resources.materials.len(),2);assert!(payload.resources.materials.contains_key("Paint (clipboard 2)"));assert_eq!(payload.mesh.faces[0].attrs["extension"],opaque);assert_eq!(payload.mesh.faces[0].attrs["texture"]["embed"],"foo (clipboard 2)");
    }
    #[test]
    fn overlap_paint_matches_original_first_covering_face_priority(){
        let mut app=IngeTrazoApp::new();let ids=[(0.,0.),(2.,0.),(2.,2.),(0.,2.)].map(|(x,y)|app.scene.mesh.add_vertex(DVec3::new(x,y,0.)));app.scene.mesh.add_face(ids.to_vec(),vec![]);app.scene.mesh.faces[0].attrs.insert("mat".into(),serde_json::json!("blue"));app.scene.select(SelectionItem::Face(0));app.copy_selection();app.scene.mesh.faces[0].attrs.insert("mat".into(),serde_json::json!("red"));assert!(app.paste_internal(DVec3::X));let mut areas=std::collections::HashMap::<String,f64>::new();for id in 0..app.scene.mesh.faces.len(){let paint=app.scene.mesh.faces[id].attrs["mat"].as_str().unwrap().to_owned();*areas.entry(paint).or_default()+=app.scene.mesh.face_area(id);}assert!((areas["red"]-4.).abs()<1e-9);assert!((areas["blue"]-2.).abs()<1e-9);
    }
    #[test]
    fn overlap_stamp_rebuilds_regions_on_two_planes_with_one_undo(){
        let mut app=IngeTrazoApp::new();for z in [0.,3.]{let ids=[(0.,0.),(2.,0.),(2.,2.),(0.,2.)].map(|(x,y)|app.scene.mesh.add_vertex(DVec3::new(x,y,z)));let id=app.scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();app.scene.select(SelectionItem::Face(id));}app.copy_selection();let before=app.scene.clone();assert!(app.paste_internal(DVec3::X));let area:f64=(0..app.scene.mesh.faces.len()).map(|id|app.scene.mesh.face_area(id)).sum();assert!((area-12.).abs()<1e-8,"union area {area}");assert_eq!(app.undo_stack.len(),1);app.undo();assert_eq!(app.scene,before);
    }
    #[test]
    fn foreign_component_material_and_asset_collisions_remap_with_shared_repeat_identity(){
        use ingetrazo_core::{groups::Group,mesh::Mesh,materials::Material};
        let mut source=IngeTrazoApp::new();let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]);mesh.faces[0].attrs.insert("mat".into(),serde_json::json!("Paint"));mesh.faces[0].attrs.insert("texture".into(),serde_json::json!({"embed":"same.png","unknown":17}));let mut group=Group::with_mesh("part",mesh);group.native_document=serde_json::json!({"material":{"mat":"Paint","texture":{"embed":"same.png","vendor_note":{"path":"same.png"}}}});let uid=group.id;source.scene.groups.push(group);let definition=source.scene.make_component(uid).unwrap();let mut paint=Material::new("Paint",[1.,0.,0.,1.]);paint.texture_embed=Some("same.png".into());source.scene.materials.insert("Paint".into(),paint);source.scene.embedded_textures.insert("same.png".into(),vec![1,2]);source.scene.select(SelectionItem::Group(0));source.copy_selection();
        let mut destination=IngeTrazoApp::new();destination.scene.materials.insert("Paint".into(),Material::new("Paint",[0.,0.,1.,1.]));destination.scene.embedded_textures.insert("same.png".into(),vec![9]);destination.scene.component_definitions.insert(definition,ingetrazo_core::components::ComponentDefinition{mesh:Mesh::new(),children:vec![]});destination.clipboard=source.clipboard.clone();assert!(destination.paste_internal(DVec3::ZERO));let pasted=&destination.scene.groups[0];let new_definition=pasted.component_definition.unwrap();assert_ne!(new_definition,definition);assert_eq!(pasted.native_document["material"]["mat"],"Paint (clipboard 1)");assert_eq!(pasted.native_document["material"]["texture"]["embed"],"same.png (clipboard 1)");assert_eq!(pasted.native_document["material"]["texture"]["vendor_note"]["path"],"same.png");assert_eq!(pasted.mesh.faces[0].attrs["mat"],"Paint (clipboard 1)");assert_eq!(pasted.mesh.faces[0].attrs["texture"]["embed"],"same.png (clipboard 1)");assert_eq!(pasted.mesh.faces[0].attrs["texture"]["unknown"],17);assert_eq!(destination.scene.materials["Paint (clipboard 1)"].texture_embed.as_deref(),Some("same.png (clipboard 1)"));assert!(destination.paste_internal(DVec3::X));assert_eq!(destination.scene.groups[1].component_definition,Some(new_definition));assert_ne!(destination.scene.groups[0].id,destination.scene.groups[1].id);
    }
    #[test]
    fn malformed_mesh_and_recursive_definitions_leave_document_and_history_unchanged(){
        let mut app=IngeTrazoApp::new();let mut payload=crate::app::GeometryClipboard::default();payload.mesh.add_vertex(DVec3::ZERO);payload.mesh.add_vertex(DVec3::X);payload.mesh.add_edge(0,1);payload.mesh.edges[0].v1=999;app.clipboard=Some(payload.clone());let before=app.scene.clone();assert!(!app.paste_internal(DVec3::ZERO));assert_eq!(app.scene,before);assert!(app.undo_stack.is_empty());app.begin_paste_preview();assert!(app.paste_preview.is_none());
        payload.mesh.edges[0].v1=1;let id=uuid::Uuid::new_v4();let mut group=ingetrazo_core::groups::Group::new("recursive");group.component_definition=Some(id);payload.resources.component_definitions.insert(id,ingetrazo_core::components::ComponentDefinition{mesh:Default::default(),children:vec![group.clone()]});payload.groups.push(group);assert!(super::validate_payload(&payload).is_err());
    }
    #[test]
    #[ignore="requires isolated X11 clipboard server"]
    fn system_clipboard_transfers_geometry_between_native_owners(){
        let mut source=IngeTrazoApp::new();source.system_clipboard=Some(arboard::Clipboard::new().unwrap());let a=source.scene.mesh.add_vertex(DVec3::ZERO);let b=source.scene.mesh.add_vertex(DVec3::X);source.scene.mesh.add_edge(a,b);source.scene.select(SelectionItem::Edge(0));source.copy_selection();
        let mut destination=IngeTrazoApp::new();destination.system_clipboard=Some(arboard::Clipboard::new().unwrap());destination.read_system_clipboard();assert_eq!(destination.clipboard.as_ref().unwrap().mesh.edges.len(),1);destination.paste_selection();assert!(destination.paste_preview.is_some());source.system_clipboard.as_mut().unwrap().set_text(format!("{}{{broken",super::CLIPBOARD_PREFIX)).unwrap();let before=destination.scene.clone();destination.paste_selection();assert!(destination.paste_preview.is_none());assert!(destination.clipboard.is_none());assert!(!destination.commit_paste_preview());assert_eq!(destination.scene,before);
        source.write_system_clipboard();destination.paste_in_place();assert_eq!(destination.scene.mesh.edges.len(),1);
    }
    #[test]
    fn foreign_clipboard_assets_preview_and_stamp_survive_roundtrip_and_undo(){
        let mut source=IngeTrazoApp::new();let a=source.scene.mesh.add_vertex(DVec3::ZERO);let b=source.scene.mesh.add_vertex(DVec3::X);source.scene.mesh.add_edge(a,b);source.scene.select(SelectionItem::Edge(0));source.scene.embedded_textures.insert("texture.png".into(),vec![1,2,3,4]);source.copy_selection();
        let encoded=serde_json::to_string(source.clipboard.as_ref().unwrap()).unwrap();let mut destination=IngeTrazoApp::new();destination.clipboard=Some(serde_json::from_str(&encoded).unwrap());let before=destination.scene.clone();destination.begin_paste_preview();assert_eq!(destination.paste_preview.as_ref().unwrap().scene.embedded_textures["texture.png"],vec![1,2,3,4]);assert_eq!(destination.scene,before);assert!(destination.commit_paste_preview());assert_eq!(destination.scene.embedded_textures["texture.png"],vec![1,2,3,4]);destination.undo();assert_eq!(destination.scene,before);
        destination.scene.embedded_textures.insert("texture.png".into(),vec![9]);let before=destination.scene.clone();let depth=destination.undo_stack.len();assert!(destination.paste_internal(DVec3::ZERO));assert_eq!(destination.scene.embedded_textures["texture.png"],vec![9]);assert_eq!(destination.scene.embedded_textures["texture.png (clipboard 1)"],vec![1,2,3,4]);assert_eq!(destination.undo_stack.len(),depth+1);destination.undo();assert_eq!(destination.scene,before);
    }
    #[test]
    fn copy_crosses_nested_edit_frames_without_shifting_geometry_or_texture(){
        use ingetrazo_core::{groups::Group,mesh::Mesh};use glam::DMat4;
        let mut app=IngeTrazoApp::new();let mut mesh=Mesh::new();let points=[DVec3::ZERO,DVec3::X,DVec3::Y];let ids=points.map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]);mesh.faces[0].attrs.insert("texture".into(),serde_json::json!({"uvw":[1.,0.,0.,3.,0.,1.,0.,4.]}));
        let mut group=Group::with_mesh("source",mesh);let source_frame=DMat4::from_translation(DVec3::new(3.,4.,5.))*DMat4::from_rotation_z(0.4)*DMat4::from_scale(DVec3::new(2.,3.,4.));group.transform=source_frame;app.scene.groups.push(group);app.scene.select(SelectionItem::Group(0));app.begin_component_edit();app.scene.select(SelectionItem::Face(0));app.copy_selection();app.cancel_component_edit();app.paste_in_place();
        let face=&app.scene.mesh.faces[0];for(i,vertex)in face.vertices.iter().enumerate(){let world=app.scene.mesh.vertices[*vertex].position;assert!((world-source_frame.transform_point3(points[i])).length()<1e-9);let uvw=face.attrs["texture"]["uvw"].as_array().unwrap();let u=DVec3::new(uvw[0].as_f64().unwrap(),uvw[1].as_f64().unwrap(),uvw[2].as_f64().unwrap()).dot(world)+uvw[3].as_f64().unwrap();assert!((u-(points[i].x+3.)).abs()<1e-9);}
        let mut destination=Group::new("destination");let destination_frame=DMat4::from_translation(DVec3::new(-10.,2.,1.))*DMat4::from_rotation_y(0.7);destination.transform=destination_frame;app.scene.groups.push(destination);app.scene.clear_selection();app.scene.select(SelectionItem::Group(1));app.begin_component_edit();app.paste_in_place();let mesh=&app.scene.mesh;for(i,vertex)in mesh.faces[0].vertices.iter().enumerate(){assert!((destination_frame.transform_point3(mesh.vertices[*vertex].position)-source_frame.transform_point3(points[i])).length()<1e-9);}app.finish_component_edit();assert_eq!(app.scene.groups[1].mesh.faces.len(),1);
    }
    #[test]
    fn preview_cancel_and_commit_are_one_atomic_stamp() {
        let mut app=IngeTrazoApp::new();let a=app.scene.mesh.add_vertex(DVec3::new(0.,0.,8.));let b=app.scene.mesh.add_vertex(DVec3::new(1.,0.,8.));app.scene.mesh.add_edge(a,b);app.scene.select(SelectionItem::Edge(0));app.copy_selection();
        let before=app.scene.clone();let depth=app.undo_stack.len();app.paste_selection();app.update_paste_preview(DVec3::new(5.,6.,8.));
        assert_eq!(app.scene,before);assert_eq!(app.undo_stack.len(),depth);assert_eq!(app.paste_preview.as_ref().unwrap().offset,DVec3::new(5.,6.,0.));
        app.cancel_paste_preview();assert_eq!(app.scene,before);assert_eq!(app.undo_stack.len(),depth);
        app.paste_selection();app.update_paste_preview(DVec3::new(5.,6.,8.));assert!(app.commit_paste_preview());assert!(!app.commit_paste_preview());assert_eq!(app.undo_stack.len(),depth+1);assert_eq!(app.scene.mesh.edges.len(),2);app.undo();assert_eq!(app.scene,before);
    }
    #[test]
    fn loose_copy_paste_preserves_holes_opaque_data_uvs_curves_and_one_history_step() {
        let mut app=IngeTrazoApp::new();
        let outer=[(0.,0.),(4.,0.),(4.,4.),(0.,4.)].map(|(x,y)|app.scene.mesh.add_vertex(DVec3::new(x,y,0.)));
        let hole=[(1.,1.),(1.,2.),(2.,2.),(2.,1.)].map(|(x,y)|app.scene.mesh.add_vertex(DVec3::new(x,y,0.)));
        let face=app.scene.mesh.add_face(outer.to_vec(),vec![hole.to_vec()]).unwrap();
        app.scene.mesh.faces[face].layer=Some("Face layer".into());
        app.scene.mesh.faces[face].attrs.insert("texture".into(),serde_json::json!({"uvw":[1.,0.,0.,3.,0.,2.,0.,4.],"path":"opaque.png","unknown":42}));
        app.scene.mesh.faces[face].attrs.insert("mat".into(),serde_json::json!("paint"));
        let a=app.scene.mesh.add_vertex(DVec3::new(10.,0.,0.));let b=app.scene.mesh.add_vertex(DVec3::new(11.,0.,0.));
        let edge=app.scene.mesh.add_edge(a,b).unwrap();app.scene.mesh.edges[edge].curve_id=Some(73);
        app.scene.mesh.edges[edge].attrs.insert("curve".into(),serde_json::json!(73));app.scene.mesh.edges[edge].attrs.insert("opaque".into(),serde_json::json!({"keep":true}));
        app.scene.mesh.edges[edge].soft=true;app.scene.mesh.edges[edge].hidden=true;app.scene.mesh.edges[edge].layer=Some("Edge layer".into());
        app.scene.select(SelectionItem::Face(face));app.scene.select(SelectionItem::Edge(edge));app.copy_selection();
        let before=app.scene.clone();let depth=app.undo_stack.len();assert!(app.paste_internal(DVec3::new(20.,30.,40.)));
        assert_eq!(app.undo_stack.len(),depth+1);assert_eq!(app.scene.mesh.faces.len(),2);
        let pasted=&app.scene.mesh.faces[1];assert_eq!(pasted.holes.len(),1);assert_eq!(pasted.layer.as_deref(),Some("Face layer"));assert_eq!(pasted.attrs["mat"],"paint");
        let uvw=pasted.attrs["texture"]["uvw"].as_array().unwrap();assert_eq!(uvw[3],-17.);assert_eq!(uvw[7],-56.);assert_eq!(pasted.attrs["texture"]["unknown"],42);
        let pasted_edge=app.scene.mesh.edges.iter().find(|e|app.scene.mesh.vertices[e.v0].position==DVec3::new(30.,30.,40.)).unwrap();
        assert!(pasted_edge.soft&&pasted_edge.hidden);assert_eq!(pasted_edge.layer.as_deref(),Some("Edge layer"));assert_ne!(pasted_edge.curve_id,Some(73));assert_eq!(pasted_edge.attrs["opaque"],serde_json::json!({"keep":true}));
        app.undo();assert_eq!(app.scene,before);app.redo();assert_eq!(app.scene.mesh.faces.len(),2);
        let snapshot=app.scene.clone();let depth=app.undo_stack.len();assert!(!app.paste_internal(DVec3::splat(f64::NAN)));assert_eq!(app.scene,snapshot);assert_eq!(app.undo_stack.len(),depth);
    }
}
