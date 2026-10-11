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
    pub fn tag_selection_bim(&mut self, class: &str, name: &str) {
        if !bim::IFC_CLASSES.contains(&class) {
            self.status_message = "Unsupported BIM class".into();
            return;
        }
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
    pub(crate) fn refresh_bim_takeoff(&mut self) {
        let key = (
            self.model_revision,
            self.scene.version,
            self.component_edit_stack.len(),
        );
        if self.bim_cache.key == Some(key) {
            return;
        }
        self.bim_cache.rows = if self.component_edit_stack.is_empty() {
            bim::takeoff(&self.scene)
        } else {
            Err("Finish group editing to view document quantities".into())
        };
        self.bim_cache.key = Some(key);
        self.bim_cache.builds += 1;
    }
    pub fn select_bim_object(&mut self, key: &ObjectKey) {
        self.scene.clear_selection();
        match key {
            ObjectKey::Group(i) => {
                if *i < self.scene.groups.len() {
                    self.scene.select(SelectionItem::Group(*i));
                }
            }
            ObjectKey::Faces(id) => {
                let ids:Vec<_>=self.scene.mesh.faces.iter().enumerate().filter_map(|(i,f)|{
                    let tag=f.attrs.get("ifc")?.as_object()?;
                    (!tag.is_empty() && tag.get("id").and_then(|n|n.as_i64())==*id).then_some(i)
                }).collect();
                for i in ids {self.scene.select(SelectionItem::Face(i));}
            }
        }
    }
    pub fn export_bim_csv(&mut self, path: &std::path::Path) -> Result<usize, String> {
        if !self.component_edit_stack.is_empty() {
            return Err("Finish group editing before exporting document quantities".into());
        }
        let n = ingetrazo_formats::ifc::save_quantities_csv(&self.scene, path)
            .map_err(|e| e.to_string())?;
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
