//! BIM actions share application history and reuse quantities across UI frames.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    bim::{self, ObjectKey, Takeoff},
    scene::SelectionItem,
};

pub(crate) struct Cache {
    key: Option<(u64, u64, usize)>,
    pub rows: Result<Vec<Takeoff>, String>,
    pub builds: usize,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            key: None,
            rows: Ok(vec![]),
            builds: 0,
        }
    }
}
impl IngeTrazoApp {
    /// Activation is UI state; stamp commands allocate one ID per draw commit.
    pub fn set_bim_draw_mode(&mut self, enabled: bool) {
        self.bim_active_tag_mode = enabled;
        self.sync_bim_draw_activation();
    }
    pub(crate) fn prepare_bim_draw_commit(&mut self) -> Result<(), String> {
        self.sync_bim_draw_activation();
        Ok(())
    }
    pub(crate) fn sync_bim_draw_activation(&mut self) {
        self.scene.bim_draw_id=if self.bim_active_tag_mode {self.component_edit_stack.first().map(|session|bim::next_draw_object_id(&session.original_scene.mesh))}else{None};
        self.scene.active_ifc = if self.bim_active_tag_mode {
            Some(
                serde_json::json!({"class":self.bim_active_class,"name":if self.bim_name_input.trim().is_empty(){self.bim_active_class.strip_prefix("Ifc").unwrap_or(&self.bim_active_class)}else{self.bim_name_input.trim()}}),
            )
        } else {
            None
        };
    }

    pub fn tag_selection_bim(&mut self, class: &str, name: &str) {
        if !bim::IFC_CLASSES.contains(&class) {
            self.status_message = "Unsupported BIM class".into();
            return;
        }
        if self.edit_bim_context_selection(Some((class,name))).is_some(){return;}
        let faces: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::Face(i) = s {
                    (*i < self.scene.mesh.faces.len()).then_some(*i)
                } else {
                    None
                }
            })
            .collect();
        let groups: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::Group(i) = s {
                    (*i < self.scene.groups.len()).then_some(*i)
                } else {
                    None
                }
            })
            .collect();
        if faces.is_empty() && groups.is_empty() {
            self.status_message = "Select faces or groups to tag".into();
            return;
        }
        let id = match bim::next_object_id(&self.scene) {
            Ok(id) => id,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };
        let name = name.trim();
        let tag = serde_json::json!({"id":id,"class":class,"name":name});
        self.save_undo_state();
        for &i in &faces {
            self.scene.mesh.faces[i]
                .attrs
                .insert("ifc".into(), tag.clone());
        }
        for &i in &groups {
            let g = &mut self.scene.groups[i];
            g.ifc_tag = Some(
                serde_json::json!({"class":class,"name":if name.is_empty(){g.name.as_str()}else{name}}),
            );
        }
        self.scene.bump_version();
        self.status_message = format!(
            "Tagged {} BIM objects",
            groups.len() + usize::from(!faces.is_empty())
        );
    }
    pub fn untag_selection_bim(&mut self) {
        if self.edit_bim_context_selection(None).is_some(){return;}
        let faces: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::Face(i) = s {
                    self.scene
                        .mesh
                        .faces
                        .get(*i)
                        .filter(|f| f.attrs.contains_key("ifc"))
                        .map(|_| *i)
                } else {
                    None
                }
            })
            .collect();
        let groups: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::Group(i) = s {
                    self.scene
                        .groups
                        .get(*i)
                        .filter(|g| g.ifc_tag.is_some())
                        .map(|_| *i)
                } else {
                    None
                }
            })
            .collect();
        if faces.is_empty() && groups.is_empty() {
            self.status_message = "No selected BIM tags".into();
            return;
        }
        self.save_undo_state();
        for i in faces {
            self.scene.mesh.faces[i].attrs.remove("ifc");
        }
        for i in groups {
            self.scene.groups[i].ifc_tag = None;
        }
        self.scene.bump_version();
        self.status_message = "Removed selected BIM tags".into();
    }
    // Original row selection holds real outer loose-face objects while the
    // active tool mesh is a different group. Resolve the immutable target;
    // never reinterpret an outer face number against the active mesh.
    fn edit_bim_context_selection(&mut self,tag:Option<(&str,&str)>)->Option<bool>{
        let selected:Vec<_>=self.scene.selection.iter().filter_map(|s|if let SelectionItem::ContextFace{context,face,vertices}=s {Some((context.clone(),*face,vertices.clone()))}else{None}).collect();
        let groups:Vec<_>=self.scene.selection.iter().filter_map(|s|if let SelectionItem::ContextGroup{context,group}=s {Some((context.clone(),*group))}else{None}).collect();
        if selected.is_empty()&&groups.is_empty(){return None;}
        let result=(||->Result<(),String>{
            let root=self.component_edit_stack.first().ok_or("BIM face target is no longer active")?;
            if selected.iter().any(|(context,face,vertices)|*context!=root.original_scene.history_context || root.original_scene.mesh.faces.get(*face).is_none_or(|f|f.vertices.iter().map(|&v|root.original_scene.mesh.vertices[v].uid).collect::<Vec<_>>()!=*vertices)) {return Err("BIM face target no longer resolves; no tags changed".into());}
            if groups.iter().any(|(context,id)|*context!=root.original_scene.history_context || !root.original_scene.groups.iter().any(|g|g.id==*id)){return Err("BIM group target no longer resolves; no tags changed".into());}
            if selected.len()+groups.len()!=self.scene.selection.len(){return Err("Mixed active and parked BIM targets require separate tagging".into());}
            let before=root.original_scene.clone();let mut candidate=before.clone();
            let id=if tag.is_some(){Some(bim::next_draw_object_id(&candidate.mesh)?)}else{None};
            for (_,face,_) in &selected {if let Some((class,name))=tag {candidate.mesh.faces[*face].attrs.insert("ifc".into(),serde_json::json!({"id":id.unwrap(),"class":class,"name":name.trim()}));}else{candidate.mesh.faces[*face].attrs.remove("ifc");}}
            for (_,id) in &groups {let group=candidate.groups.iter_mut().find(|g|g.id==*id).unwrap();group.ifc_tag=tag.map(|(class,name)|serde_json::json!({"class":class,"name":if name.trim().is_empty(){group.name.as_str()}else{name.trim()}}));}
            if candidate.mesh==before.mesh&&candidate.groups==before.groups{return Ok(());}
            candidate.bump_version();self.undo_stack.push(before);self.redo_stack.clear();
            self.component_edit_stack[0].original_scene=candidate;
            self.scene.bump_view_version();self.bump_model_revision();Ok(())
        })();
        match result {Ok(())=>{self.status_message=if tag.is_some(){"Tagged parked document faces"}else{"Removed parked document face tags"}.into();Some(true)},Err(error)=>{self.status_message=error;Some(false)}}
    }
    /// Source leaves the document placement list in place during group editing.
    /// Reconstruct only that cold BIM view; tools still own the isolated active
    /// mesh. No Scene clone, registry publication or warm traversal is required.
    pub fn bim_document_groups(&self)->std::borrow::Cow<'_,[ingetrazo_core::groups::Group]>{
        if self.component_edit_stack.is_empty(){return std::borrow::Cow::Borrowed(&self.scene.groups);}
        let mut children=self.scene.groups.clone();let mut mesh=&self.scene.mesh;
        for session in self.component_edit_stack.iter().rev(){
            let mut groups=session.original_scene.groups.clone();
            if let Some(group)=groups.iter_mut().find(|g|g.id==session.group_id){
                group.mesh=mesh.clone();group.transform=glam::DMat4::IDENTITY;group.children=children;
            }
            children=groups;mesh=&session.original_scene.mesh;
        }
        std::borrow::Cow::Owned(children)
    }
    pub(crate) fn refresh_bim_takeoff(&mut self) {
        let key = (
            self.model_revision,
            self.scene.version,
            self.component_edit_stack.len(),
        );
        if self.bim_cache.key == Some(key) {
            return;
        }
        let loose=self.component_edit_stack.first().map_or(&self.scene.mesh,|session|&session.original_scene.mesh);
        let groups=self.bim_document_groups();
        let rows=bim::takeoff_from(loose,&groups).and_then(|mut rows|{
            if !self.component_edit_stack.is_empty(){
                // Original BimPanel/CSV classifies a root Group by its own
                // currently exposed mesh, even though area/volume use subtree
                // world geometry. Retain that operative active-context policy.
                for row in &mut rows {if let ObjectKey::Group(index)=row.key {
                    let group=&groups[index];row.quantities=bim::class_quantities(&row.class,&group.mesh,&(0..group.mesh.faces.len()).collect::<Vec<_>>())?;
                }}
            }
            Ok(rows)
        });
        self.bim_cache.rows=rows;
        self.bim_cache.key = Some(key);
        self.bim_cache.builds += 1;
    }
    pub fn bim_takeoff_rows(&mut self)->Result<&[Takeoff],String>{
        self.refresh_bim_takeoff();self.bim_cache.rows.as_ref().map(Vec::as_slice).map_err(Clone::clone)
    }
    pub fn select_bim_object(&mut self, key: &ObjectKey) {
        self.scene.clear_selection();
        match key {
            ObjectKey::Group(i) => {
                if let Some(root)=self.component_edit_stack.first(){if let Some(group)=root.original_scene.groups.get(*i){self.scene.select(SelectionItem::ContextGroup{context:root.original_scene.history_context.clone(),group:group.id});}}
                else if *i<self.scene.groups.len(){self.scene.select(SelectionItem::Group(*i));}
            }
            ObjectKey::Faces(id) => {
                let root=self.component_edit_stack.first();
                let loose=root.map_or(&self.scene.mesh,|session|&session.original_scene.mesh);
                let ids:Vec<_>=loose.faces.iter().enumerate().filter_map(|(i,f)|{
                    let tag=f.attrs.get("ifc")?.as_object()?;
                    (!tag.is_empty() && tag.get("id").and_then(|n|n.as_i64())==*id).then_some(i)
                }).collect();
                let context=root.map(|s|s.original_scene.history_context.clone());
                let ids:Vec<_>=ids.into_iter().map(|face|(face,loose.faces[face].vertices.iter().map(|&v|loose.vertices[v].uid).collect::<Vec<_>>())).collect();
                for (face,vertices) in ids {self.scene.select(match &context {Some(context)=>SelectionItem::ContextFace{context:context.clone(),face,vertices},None=>SelectionItem::Face(face)});}
            }
        }
    }
    pub fn export_bim_csv(&mut self, path: &std::path::Path) -> Result<usize, String> {
        self.refresh_bim_takeoff();
        let rows=self.bim_cache.rows.as_ref().map_err(Clone::clone)?;
        let n=ingetrazo_formats::ifc::save_quantities_rows(rows,path).map_err(|e|e.to_string())?;
        self.status_message = format!("Exported {n} BIM objects to {}", path.display());
        Ok(n)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    fn app() -> IngeTrazoApp {
        let mut a = IngeTrazoApp::new();
        for z in [0., 1.] {
            let ids = [
                DVec3::new(0., 0., z),
                DVec3::new(2., 0., z),
                DVec3::new(0., 3., z),
            ]
            .map(|p| a.scene.mesh.add_vertex(p));
            a.scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        }
        a.scene
            .groups
            .push(ingetrazo_core::groups::Group::with_mesh(
                "Named group",
                a.scene.mesh.clone(),
            ));
        a
    }
    #[test]
    fn bim_tagged_object_history_cache_selection_and_real_exports() {
        let mut a = app();
        a.scene.select(SelectionItem::Face(0));
        a.scene.select(SelectionItem::Face(1));
        a.scene.select(SelectionItem::Group(0));
        a.tag_selection_bim("IfcWall", "  ");
        assert_eq!(a.undo_stack.len(), 1);
        assert_eq!(a.scene.mesh.faces[0].attrs["ifc"]["id"], 1);
        assert_eq!(
            a.scene.mesh.faces[0].attrs["ifc"],
            a.scene.mesh.faces[1].attrs["ifc"]
        );
        assert_eq!(
            a.scene.groups[0].ifc_tag.as_ref().unwrap()["name"],
            "Named group"
        );
        for _ in 0..100 {
            a.refresh_bim_takeoff();
        }
        assert_eq!(a.bim_cache.builds, 1);
        let rows = a.bim_cache.rows.as_ref().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].area, 6.);
        assert_eq!(rows[0].volume, None);
        a.select_bim_object(&ObjectKey::Faces(Some(1)));
        assert_eq!(a.scene.selection.len(), 2);
        assert!(!a.scene.selection.contains(&SelectionItem::Group(0)));
        let dir = std::env::temp_dir().join(format!("ingetrazo-bim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        assert_eq!(a.export_bim_csv(&dir.join("q.csv")).unwrap(), 2);
        assert!(std::fs::read_to_string(dir.join("q.csv"))
            .unwrap()
            .contains("Named group"));
        ingetrazo_formats::save_igz(&a.scene, &dir.join("model.igz")).unwrap();
        let loaded = ingetrazo_formats::load_igz(&dir.join("model.igz")).unwrap();
        assert_eq!(
            bim::takeoff(&loaded).unwrap(),
            *a.bim_cache.rows.as_ref().unwrap()
        );
        assert_eq!(
            ingetrazo_formats::ifc::save_ifc(&loaded, &dir.join("model.ifc"), "Tagged").unwrap(),
            2
        );
        a.untag_selection_bim();
        a.refresh_bim_takeoff();
        assert_eq!(a.bim_cache.rows.as_ref().unwrap().len(), 1);
        a.undo();
        a.refresh_bim_takeoff();
        assert_eq!(a.bim_cache.rows.as_ref().unwrap().len(), 2);
        a.redo();
        a.refresh_bim_takeoff();
        assert_eq!(a.bim_cache.rows.as_ref().unwrap().len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bim_empty_and_identity_overflow_preserve_document_and_redo() {
        let mut a = app();
        a.tag_selection_bim("IfcWall", "");
        a.untag_selection_bim();
        assert!(a.undo_stack.is_empty());
        a.scene.select(SelectionItem::Face(0));
        a.tag_selection_bim("IfcWall", "A");
        a.undo();
        assert_eq!(a.redo_stack.len(), 1);
        a.scene.mesh.faces[1].attrs.insert(
            "ifc".into(),
            serde_json::json!({"id":i64::MAX,"class":"IfcWall"}),
        );
        let before = a.scene.clone();
        let revision = a.model_revision;
        a.tag_selection_bim("IfcSlab", "B");
        assert_eq!(a.scene, before);
        assert_eq!(a.model_revision, revision);
        assert_eq!(a.redo_stack.len(), 1);
        assert!(a.status_message.contains("exhausted"));
    }
}

#[cfg(test)]
mod draw_tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::math::Ray;
    use ingetrazo_tools::base::{ToolContext, ToolType};
    fn point(x: f64, y: f64, z: f64) -> ToolContext {
        let p = DVec3::new(x, y, z);
        ToolContext::new(p, Ray::new(p + DVec3::Z * 10., -DVec3::Z))
    }
    fn app() -> IngeTrazoApp {
        let mut a = IngeTrazoApp::new();
        a.bim_name_input = "Muros N1".into();
        a.set_bim_draw_mode(true);
        a
    }
    fn rect(a: &mut IngeTrazoApp, x: f64, y: f64) {
        a.set_tool(ToolType::Rectangle);
        a.dispatch_tool_click(&point(x, y, 0.));
        a.dispatch_tool_click(&point(x + 2., y + 1., 0.));
    }
    #[test]
    fn bim_draw_rectangles_undo_redo_preserve_separate_trace_ids() {
        let mut a = app();
        rect(&mut a, 0., 0.);
        assert_eq!(a.undo_stack.len(), 1);
        let first = a.scene.mesh.clone();
        assert_eq!(first.faces[0].attrs["ifc"]["id"], 1);
        rect(&mut a, 0., 5.);
        assert_eq!(a.undo_stack.len(), 2);
        let second = a.scene.mesh.clone();
        assert!(second.faces.iter().any(|f| f.attrs["ifc"]["id"] == 2));
        a.undo();
        assert_eq!(a.scene.mesh, first);
        a.redo();
        assert_eq!(a.scene.mesh, second);
    }
    #[test]
    fn bim_draw_push_extends_same_tag_without_active_mode() {
        let mut a = app();
        rect(&mut a, 0., 0.);
        let tag = a.scene.mesh.faces[0].attrs["ifc"].clone();
        a.set_bim_draw_mode(false);
        a.set_tool(ToolType::PushPull);
        a.dispatch_tool_move(&point(1., 0.5, 0.));
        a.dispatch_tool_click(&point(1., 0.5, 0.));
        a.dispatch_tool_vcb("1").unwrap();
        assert_eq!(a.scene.mesh.faces.len(), 6);
        assert!(a
            .scene
            .mesh
            .faces
            .iter()
            .all(|f| f.attrs.get("ifc") == Some(&tag)));
        assert_eq!(
            bim::takeoff(&a.scene).unwrap()[0].volume,
            Some(1.9999999999999998)
        );
        let raised = a.scene.mesh.clone();
        a.undo();
        assert_eq!(a.scene.mesh.faces.len(), 1);
        a.redo();
        assert_eq!(a.scene.mesh, raised);
    }
    #[test]
    fn bim_direct_circle_commands_use_owned_faces_and_default_class_name() {
        let mut a = app();
        a.bim_name_input.clear();
        a.set_bim_draw_mode(true);
        a.set_tool(ToolType::Circle);
        a.dispatch_tool_click(&point(0., 0., 0.));
        a.dispatch_tool_click(&point(1., 0., 0.));
        assert_eq!(a.scene.mesh.faces.len(), 1);
        assert_eq!(
            a.scene.mesh.faces[0].attrs["ifc"],
            serde_json::json!({"id":1,"class":"IfcWall","name":"Wall"})
        );
        assert_eq!(a.undo_stack.len(), 1);
    }
    #[test]
    fn bim_failed_draw_refuses_without_consuming_redo() {
        let mut a = app();
        rect(&mut a, 0., 0.);
        rect(&mut a, 0., 5.);
        a.undo();
        let before = a.scene.mesh.clone();
        a.scene.mesh.faces[0].attrs.get_mut("ifc").unwrap()["id"] = serde_json::json!(i64::MAX);
        let frozen = a.scene.mesh.clone();
        rect(&mut a, 0., 10.);
        assert_eq!(a.scene.mesh, frozen);
        assert_eq!(a.redo_stack.len(), 1);
        assert!(a.status_message.contains("exhausted"));
        assert_ne!(a.scene.mesh, before);
    }
    #[test]
    fn active_draw_context_uses_root_identity_and_local_history() {
        use ingetrazo_core::{groups::Group,scene::SelectionItem};
        let mut a=app();rect(&mut a,20.,20.);
        a.scene.mesh.faces[0].attrs.insert("ifc".into(),serde_json::json!({"id":40,"class":"IfcSlab","name":"root"}));
        a.scene.groups.push(Group::new("group"));
        a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
        rect(&mut a,0.,0.);rect(&mut a,0.,5.);
        assert_eq!(a.scene.mesh.faces.len(),2);
        assert!(a.scene.mesh.faces.iter().all(|f|f.attrs["ifc"]["id"]==41));
        a.undo();assert_eq!(a.scene.mesh.faces.len(),1);a.redo();assert_eq!(a.scene.mesh.faces.len(),2);
        a.finish_component_edit();assert_eq!(a.scene.mesh.faces[0].attrs["ifc"]["id"],40);
        assert_eq!(a.scene.groups[0].mesh.faces.len(),2);
        assert!(a.scene.groups[0].mesh.faces.iter().all(|f|f.attrs["ifc"]["id"]==41));
    }
    #[test]
    fn context_takeoff_selection_tag_history_and_csv_use_parked_mesh(){
        use ingetrazo_core::{groups::Group,scene::SelectionItem};
        let mut a=app();rect(&mut a,20.,20.);a.scene.mesh.faces[0].attrs.get_mut("ifc").unwrap()["id"]=40.into();
        a.scene.groups.push(Group::new("target"));a.scene.clear_selection();a.scene.select(SelectionItem::Group(0));a.begin_component_edit();rect(&mut a,0.,0.);
        let active=a.scene.mesh.clone();let root=a.component_edit_stack[0].original_scene.mesh.clone();
        assert_eq!(a.bim_takeoff_rows().unwrap().len(),1);assert_eq!(a.bim_takeoff_rows().unwrap()[0].key,ObjectKey::Faces(Some(40)));
        let builds=a.bim_cache.builds;for _ in 0..20{a.refresh_bim_takeoff();}assert_eq!(a.bim_cache.builds,builds);
        a.select_bim_object(&ObjectKey::Faces(Some(40)));assert!(matches!(a.scene.selection.iter().next(),Some(SelectionItem::ContextFace{face:0,..})));assert!(!a.scene.is_selected(&SelectionItem::Face(0)));
        a.tag_selection_bim("IfcSlab","renamed");assert_eq!(a.scene.mesh,active);assert_eq!(a.component_edit_stack[0].original_scene.mesh.faces[0].attrs["ifc"]["name"],"renamed");
        a.undo();assert_eq!(a.scene.mesh,active);assert_eq!(a.component_edit_stack[0].original_scene.mesh,root);a.redo();assert_eq!(a.scene.mesh,active);assert_eq!(a.bim_takeoff_rows().unwrap()[0].name,"renamed");
        let path=std::env::temp_dir().join(format!("bim-context-{}.csv",uuid::Uuid::new_v4()));assert_eq!(a.export_bim_csv(&path).unwrap(),1);let csv=std::fs::read_to_string(&path).unwrap();assert!(csv.contains("renamed"));assert!(!csv.contains("context"));std::fs::remove_file(path).unwrap();
        a.select_bim_object(&ObjectKey::Faces(Some(41))); // No root row41 after source id allocation40→41.
        assert_eq!(a.scene.selection.len(),1);
        a.untag_selection_bim();assert!(a.component_edit_stack[0].original_scene.mesh.faces[0].attrs.get("ifc").is_none());assert_eq!(a.scene.mesh,active);
    }
    #[test]
    fn parked_selection_stale_context_and_topology_refuse_atomically(){
        use ingetrazo_core::{groups::Group,scene::SelectionItem};
        let mut a=app();rect(&mut a,20.,20.);a.scene.groups.push(Group::new("target"));a.scene.select(SelectionItem::Group(0));a.begin_component_edit();rect(&mut a,0.,0.);a.select_bim_object(&ObjectKey::Faces(Some(1)));
        let valid=a.scene.selection.clone();a.component_edit_stack[0].original_scene.mesh.vertices[0].uid=uuid::Uuid::new_v4();let before=a.scene.clone();let root=a.component_edit_stack[0].original_scene.clone();let undo=a.undo_stack.clone();let redo=a.redo_stack.clone();a.tag_selection_bim("IfcWall","wrong");assert_eq!(a.scene,before);assert_eq!(a.component_edit_stack[0].original_scene,root);assert_eq!(a.undo_stack,undo);assert_eq!(a.redo_stack,redo);assert!(a.status_message.contains("no longer resolves"));
        a.scene.selection=valid;let stale=a.scene.selection.iter().next().cloned().unwrap();a.cancel_component_edit();a.scene.clear_selection();a.scene.select(stale);let before=a.scene.clone();a.tag_selection_bim("IfcWall","wrong");assert_eq!(a.scene,before);assert!(a.status_message.contains("no longer active"));
    }
    #[test]
    fn context_group_row_targets_root_placement_not_active_child_index() {
        use ingetrazo_core::{groups::Group,scene::SelectionItem};
        let mut a=app();let mut parent=Group::new("parent");
        parent.ifc_tag=Some(serde_json::json!({"class":"IfcSlab","name":"parent slab"}));
        parent.children.push(Group::new("child"));a.scene.groups.push(parent);
        let parent_id=a.scene.groups[0].id;let child_id=a.scene.groups[0].children[0].id;
        a.scene.select(SelectionItem::Group(0));a.begin_component_edit();
        a.scene.select(SelectionItem::Group(0));a.begin_component_edit();rect(&mut a,0.,0.);
        let active=a.scene.mesh.clone();a.select_bim_object(&ObjectKey::Group(0));
        assert!(matches!(a.scene.selection.iter().next(),Some(SelectionItem::ContextGroup{group,..})if *group==parent_id));
        a.tag_selection_bim("IfcRoof","root only");
        let root=&a.component_edit_stack[0].original_scene;
        assert_eq!(root.groups[0].ifc_tag.as_ref().unwrap()["name"],"root only");
        assert_eq!(root.groups[0].children[0].id,child_id);assert!(root.groups[0].children[0].ifc_tag.is_none());
        assert_eq!(a.scene.mesh,active);a.undo();assert_eq!(a.scene.mesh,active);
        assert_eq!(a.component_edit_stack[0].original_scene.groups[0].ifc_tag.as_ref().unwrap()["name"],"parent slab");
        a.redo();a.untag_selection_bim();assert!(a.component_edit_stack[0].original_scene.groups[0].ifc_tag.is_none());
    }
    #[test]
    fn active_mode_is_runtime_and_does_not_revert_with_model_history() {
        let mut a = app();
        rect(&mut a, 0., 0.);
        a.set_bim_draw_mode(false);
        a.undo();
        assert!(!a.bim_active_tag_mode);
        assert!(a.scene.active_ifc.is_none());
        a.redo();
        assert!(a.scene.active_ifc.is_none());
        assert!(serde_json::to_value(&a.scene)
            .unwrap()
            .get("active_ifc")
            .is_none());
        a.set_bim_draw_mode(true);
        a.new_document_unchecked();
        assert!(!a.bim_active_tag_mode);
        assert!(a.scene.active_ifc.is_none());
    }

    #[test]
    fn flat_arc_rebuild_faces_are_not_auto_tagged() {
        let mut a = app();
        let v0 = a.scene.mesh.add_vertex(DVec3::ZERO);
        let v1 = a.scene.mesh.add_vertex(DVec3::X * 2.);
        let middle=a.scene.mesh.add_vertex(DVec3::new(1.,-1.,0.));
        a.scene.mesh.add_edge(v0,middle);a.scene.mesh.add_edge(middle,v1);
        a.set_tool(ToolType::Arc);
        a.dispatch_tool_click(&point(0., 0., 0.));
        a.dispatch_tool_click(&point(2., 0., 0.));
        a.dispatch_tool_click(&point(1., 1., 0.));
        assert_eq!(a.scene.mesh.faces.len(), 1);
        assert!(!a.scene.mesh.faces[0].attrs.contains_key("ifc"));
        assert_eq!(a.undo_stack.len(), 1);
    }
}
