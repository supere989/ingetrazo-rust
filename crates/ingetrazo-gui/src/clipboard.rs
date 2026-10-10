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
impl IngeTrazoApp {
    pub fn begin_paste_preview(&mut self) {
        let Some(clipboard)=self.clipboard.clone() else{return;};
        if clipboard.mesh.faces.is_empty()&&clipboard.mesh.edges.is_empty()&&clipboard.groups.is_empty(){return;}
        self.set_tool(ingetrazo_tools::base::ToolType::Select);
        let inverse=self.scene.dimension_frame.inverse();
        let mesh=match ingetrazo_core::containers::transform_mesh(&clipboard.mesh,inverse){Ok(mesh)=>mesh,Err(e)=>{self.status_message=e;return;}};
        let mut scene=Scene::new();scene.mesh=mesh;scene.groups=clipboard.groups;
        for group in &mut scene.groups{group.transform=inverse*group.transform;}
        scene.materials=self.scene.materials.clone();scene.layers=self.scene.layers.clone();
        if let Err(e)=merge_resources(&mut scene,&clipboard.resources){self.status_message=e;return;}
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
    fn foreign_clipboard_assets_preview_and_stamp_survive_roundtrip_and_undo(){
        let mut source=IngeTrazoApp::new();let a=source.scene.mesh.add_vertex(DVec3::ZERO);let b=source.scene.mesh.add_vertex(DVec3::X);source.scene.mesh.add_edge(a,b);source.scene.select(SelectionItem::Edge(0));source.scene.embedded_textures.insert("texture.png".into(),vec![1,2,3,4]);source.copy_selection();
        let encoded=serde_json::to_string(source.clipboard.as_ref().unwrap()).unwrap();let mut destination=IngeTrazoApp::new();destination.clipboard=Some(serde_json::from_str(&encoded).unwrap());let before=destination.scene.clone();destination.begin_paste_preview();assert_eq!(destination.paste_preview.as_ref().unwrap().scene.embedded_textures["texture.png"],vec![1,2,3,4]);assert_eq!(destination.scene,before);assert!(destination.commit_paste_preview());assert_eq!(destination.scene.embedded_textures["texture.png"],vec![1,2,3,4]);destination.undo();assert_eq!(destination.scene,before);
        destination.scene.embedded_textures.insert("texture.png".into(),vec![9]);let before=destination.scene.clone();let depth=destination.undo_stack.len();assert!(!destination.paste_internal(DVec3::ZERO));assert_eq!(destination.scene,before);assert_eq!(destination.undo_stack.len(),depth);
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
