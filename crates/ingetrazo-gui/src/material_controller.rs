//! Named recipe restamping is a cold, atomic metadata command.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    groups::Group,
    materials::{FacePaintPatch, GroupPaintPatch, Material, RestampHistory},
    mesh::Mesh,
    scene::Scene,
};
use serde_json::{Map, Value};
use uuid::Uuid;
const KEYS: [&str; 3] = ["color", "texture", "opacity"];
fn paint(mesh: &Mesh, index: usize) -> Value {
    let face = &mesh.faces[index];
    let mut result = Map::new();
    for key in KEYS {
        if let Some(v) = face.attrs.get(key) {
            result.insert(key.into(), v.clone());
        }
    }
    result.into()
}
fn ring(mesh: &Mesh, index: usize) -> Result<Vec<Uuid>, String> {
    mesh.faces[index]
        .vertices
        .iter()
        .map(|&i| {
            mesh.vertices
                .get(i)
                .map(|v| v.uid)
                .ok_or_else(|| "Material target references a missing vertex".into())
        })
        .collect()
}
fn same_ring(a: &[Uuid], b: &[Uuid]) -> bool {
    a.len() == b.len()
        && !a.is_empty()
        && (0..a.len()).any(|offset| {
            a.iter()
                .enumerate()
                .all(|(i, v)| *v == b[(offset + i) % b.len()])
        })
}
fn path_group<'a>(groups: &'a mut [Group], path: &[Uuid]) -> Result<&'a mut Group, String> {
    let (first, rest) = path
        .split_first()
        .ok_or("Material placement path is empty")?;
    let group = groups
        .iter_mut()
        .find(|g| g.id == *first)
        .ok_or("Material placement no longer resolves")?;
    if rest.is_empty() {
        Ok(group)
    } else {
        path_group(&mut group.children, rest)
    }
}
fn owner(scenes: &[Scene], context: &[Uuid]) -> Result<usize, String> {
    scenes
        .iter()
        .enumerate()
        .filter(|(_, s)| context.starts_with(&s.history_context))
        .max_by_key(|(_, s)| s.history_context.len())
        .map(|(i, _)| i)
        .ok_or_else(|| "Material document no longer resolves".into())
}
fn capture_mesh(
    mesh: &Mesh,
    view: &[Uuid],
    path: &[Uuid],
    name: &str,
    stamp: &Value,
    faces: &mut Vec<FacePaintPatch>,
) -> Result<(), String> {
    let mut seen: std::collections::HashSet<Vec<Uuid>> = faces
        .iter()
        .filter(|p| p.view == view && p.context == path)
        .map(|p| ring_key(p.vertices.clone()))
        .collect();
    for (i, face) in mesh.faces.iter().enumerate() {
        if face.attrs.get("mat").and_then(Value::as_str) == Some(name) {
            let vertices = ring(mesh, i)?;
            if seen.insert(ring_key(vertices.clone())) {
                let mut after = Map::new();
                for key in KEYS {
                    if let Some(value) = stamp.get(key) {
                        after.insert(key.into(), value.clone());
                    }
                }
                faces.push(FacePaintPatch {
                    view: view.to_vec(),
                    context: path.to_vec(),
                    vertices,
                    before: paint(mesh, i),
                    after: after.into(),
                });
            }
        }
    }
    Ok(())
}

fn capture_groups(
    groups: &[Group],
    view: &[Uuid],
    path: &[Uuid],
    name: &str,
    stamp: &Value,
    history: &mut RestampHistory,
) -> Result<(), String> {
    for group in groups {
        let mut path = path.to_vec();
        path.push(group.id);
        capture_mesh(&group.mesh, view, &path, name, stamp, &mut history.faces)?;
        if let Some(old) = group
            .native_document
            .get("material")
            .filter(|m| m.get("mat").and_then(Value::as_str) == Some(name))
        {
            if !history.groups.iter().any(|p| p.path == path) {
                history.groups.push(GroupPaintPatch {
                    path: path.clone(),
                    before: Some(old.clone()),
                    after: Some(stamp.clone()),
                });
            }
        }
        capture_groups(&group.children, view, &path, name, stamp, history)?;
    }
    Ok(())
}
fn sync_shared(scene: &mut Scene) -> Result<(), String> {
    // Only metadata is published: placement transforms and child topology stay per instance.
    fn collect(
        groups: &[Group],
        meshes: &mut std::collections::HashMap<Uuid, Mesh>,
    ) -> Result<(), String> {
        for g in groups {
            if let Some(token) = g.mesh_share {
                if let Some(old) = meshes.get(&token) {
                    if old != &g.mesh {
                        return Err(
                            "Shared material prototype has incompatible placement payloads".into(),
                        );
                    }
                } else {
                    meshes.insert(token, g.mesh.clone());
                }
            }
            collect(&g.children, meshes)?;
        }
        Ok(())
    }
    let mut meshes = std::collections::HashMap::new();
    collect(&scene.groups, &mut meshes)?;
    for (token, mesh) in meshes {
        scene.publish_mesh_share(token, mesh)?;
    }
    Ok(())
}
fn ring_key(mut ring: Vec<Uuid>) -> Vec<Uuid> {
    if let Some((i, _)) = ring.iter().enumerate().min_by_key(|(_, v)| *v) {
        ring.rotate_left(i);
    }
    ring
}
fn apply(scenes: &mut [Scene], history: &RestampHistory) -> Result<(), String> {
    apply_candidate(scenes, history, false)
}
fn apply_candidate(
    scenes: &mut [Scene],
    history: &RestampHistory,
    destination_snapshot: bool,
) -> Result<(), String> {
    // Cold indexing is per immutable mesh representative, not per painted face.
    let mut plans: std::collections::HashMap<(usize, Vec<Uuid>), Vec<(usize, usize)>> =
        std::collections::HashMap::new();
    for (patch_index, patch) in history.faces.iter().enumerate() {
        for (scene_index, scene) in scenes
            .iter()
            .enumerate()
            .filter(|(_, s)| patch.view == s.history_context)
        {
            let _ = scene;
            plans
                .entry((scene_index, patch.context.clone()))
                .or_default()
                .push((patch_index, 0));
        }
    }
    for ((scene_index, context), targets) in &mut plans {
        let scene = &mut scenes[*scene_index];
        let prefix = scene.history_context.len();
        let mesh = if prefix == context.len() {
            &mut scene.mesh
        } else {
            &mut path_group(&mut scene.groups, &context[prefix..])?.mesh
        };
        let mut index = std::collections::HashMap::new();
        index
            .try_reserve(mesh.faces.len())
            .map_err(|_| "Material face index allocation refused")?;
        for i in 0..mesh.faces.len() {
            let key = ring_key(ring(mesh, i)?);
            let slot = index.entry(key).or_insert(Some(i));
            if *slot != Some(i) {
                *slot = None;
            }
        }
        for (patch_index, face_index) in targets {
            let patch = &history.faces[*patch_index];
            *face_index = index
                .get(&ring_key(patch.vertices.clone()))
                .copied()
                .flatten()
                .ok_or("Material face identity is missing or ambiguous")?;
            if paint(mesh, *face_index) != patch.before
                && !(destination_snapshot && paint(mesh, *face_index) == patch.after)
            {
                return Err("Material face changed outside this command".into());
            }
        }
    }
    let mut group_plans = Vec::new();
    for (context, name, before, after) in &history.registry {
        let i = owner(scenes, context)?;
        if scenes[i].materials.get(name) != before.as_ref()
            && !(destination_snapshot && scenes[i].materials.get(name) == after.as_ref())
        {
            return Err("Material registry changed outside this command".into());
        }
    }
    for (patch_index, patch) in history.groups.iter().enumerate() {
        for (i, scene) in scenes.iter_mut().enumerate().filter(|(_, s)| {
            patch.path.starts_with(&s.history_context) && s.history_context.len() < patch.path.len()
        }) {
            let prefix = scene.history_context.len();
            let group = path_group(&mut scene.groups, &patch.path[prefix..])?;
            if group.native_document.get("material") != patch.before.as_ref()
                && !(destination_snapshot
                    && group.native_document.get("material") == patch.after.as_ref())
            {
                return Err("Material placement paint changed outside this command".into());
            }
            if !group.native_document.is_object() {
                return Err("Material placement metadata is not an object".into());
            }
            group_plans.push((i, patch_index));
        }
    }
    for ((scene_index, context), targets) in plans {
        let scene = &mut scenes[scene_index];
        let prefix = scene.history_context.len();
        let mesh = if prefix == context.len() {
            &mut scene.mesh
        } else {
            &mut path_group(&mut scene.groups, &context[prefix..])?.mesh
        };
        for (patch_index, i) in targets {
            for key in KEYS {
                if let Some(v) = history.faces[patch_index].after.get(key) {
                    mesh.faces[i].attrs.insert(key.into(), v.clone());
                } else {
                    mesh.faces[i].attrs.remove(key);
                }
            }
        }
    }
    for (i, patch_index) in group_plans {
        let patch = &history.groups[patch_index];
        let prefix = scenes[i].history_context.len();
        let group = path_group(&mut scenes[i].groups, &patch.path[prefix..])?;
        let object = group.native_document.as_object_mut().unwrap();
        if let Some(v) = &patch.after {
            object.insert("material".into(), v.clone());
        } else {
            object.remove("material");
        }
    }
    for (context, name, _, after) in &history.registry {
        let i = owner(scenes, context)?;
        if let Some(v) = after {
            scenes[i].materials.insert(name.clone(), v.clone());
        } else {
            scenes[i].materials.remove(name);
        }
    }
    let versions: Vec<_> = scenes.iter().map(|s| s.version).collect();
    for scene in scenes.iter_mut() {
        sync_shared(scene)?;
    }
    // Parked scenes own the canonical local prototypes; active world copies keep
    // their geometry and receive the same authoritative resource publication.
    let root = scenes
        .iter()
        .position(|s| s.history_context.is_empty())
        .ok_or("Material root context no longer resolves")?;
    let published = scenes[root].mesh_shares.clone();
    for (i, scene) in scenes.iter_mut().enumerate() {
        if i != root {
            for (token, mesh) in &published {
                if scene.mesh_shares.get(token).is_some_and(|old| old != mesh) {
                    scene.publish_mesh_share(*token, mesh.clone())?;
                }
            }
        }
        scene.version = versions[i].wrapping_add(1);
    }
    Ok(())
}

impl IngeTrazoApp {
    /// Read-only document loose mesh while a nested editing view is active.
    pub fn material_loose_mesh(&self) -> &Mesh {
        self.component_edit_stack
            .first()
            .map(|s| &s.original_scene.mesh)
            .unwrap_or(&self.scene.mesh)
    }
    fn material_scenes(&self) -> Vec<Scene> {
        std::iter::once(self.scene.clone())
            .chain(
                self.component_edit_stack
                    .iter()
                    .map(|s| s.original_scene.clone()),
            )
            .collect()
    }
    fn publish_material_scenes(&mut self, mut scenes: Vec<Scene>, geometry: bool) {
        self.scene = scenes.remove(0);
        self.scene.history_material = None;
        for (session, scene) in self.component_edit_stack.iter_mut().zip(scenes) {
            session.original_scene = scene;
        }
        if geometry {
            self.bump_model_revision();
        } else {
            self.bump_render_paint_revision();
        }
    }
    pub fn edit_named_material(&mut self, recipe: Value) -> Result<bool, String> {
        let material = Material::from_recipe(recipe)?;
        if self.active_tool.placement_preview().is_some() {
            return Err("Finish or cancel the placement preview before editing a material".into());
        }
        let mut scenes = self.material_scenes();
        let mut history = RestampHistory {
            name: material.name.clone(),
            registry: vec![],
            faces: vec![],
            groups: vec![],
        };
        let stamp = material.face_recipe();
        for scene in &scenes {
            history.registry.push((
                scene.history_context.clone(),
                material.name.clone(),
                scene.materials.get(&material.name).cloned(),
                Some(material.clone()),
            ));
            capture_mesh(
                &scene.mesh,
                &scene.history_context,
                &scene.history_context,
                &material.name,
                &stamp,
                &mut history.faces,
            )?;
            capture_groups(
                &scene.groups,
                &scene.history_context,
                &scene.history_context,
                &material.name,
                &stamp,
                &mut history,
            )?;
        }
        if history.registry.iter().all(|(_, _, a, b)| a == b)
            && history.faces.iter().all(|p| p.before == p.after)
            && history.groups.iter().all(|p| p.before == p.after)
        {
            return Ok(false);
        }
        apply(&mut scenes, &history)?;
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Material history allocation refused")?;
        let mut before = self.scene.clone();
        before.history_material = Some(history.inverse());
        self.push_prepared_undo_snapshot(before);
        self.publish_material_scenes(scenes, false);
        self.active_material = material.name;
        self.status_message = "Material updated on every matching face and placement".into();
        Ok(true)
    }
    pub fn edit_selected_texture_mapping(
        &mut self,
        sw: f64,
        sh: f64,
        rot: f64,
    ) -> Result<bool, String> {
        if !sw.is_finite() || !sh.is_finite() || !rot.is_finite() || sw <= 0. || sh <= 0. {
            return Err("Texture dimensions must be positive and finite".into());
        }
        let mut history = RestampHistory {
            name: String::new(),
            registry: vec![],
            faces: vec![],
            groups: vec![],
        };
        for selected in &self.scene.selection {
            match selected {
                ingetrazo_core::scene::SelectionItem::Face(i) => {
                    let face = self
                        .scene
                        .mesh
                        .faces
                        .get(*i)
                        .ok_or("Texture face no longer resolves")?;
                    let Some(texture) = face.attrs.get("texture") else {
                        continue;
                    };
                    let mut texture = texture
                        .as_object()
                        .cloned()
                        .ok_or("Selected texture is not an object")?;
                    texture.insert("sw".into(), sw.into());
                    texture.insert("sh".into(), sh.into());
                    texture.insert("rot".into(), rot.rem_euclid(360.).into());
                    let before = paint(&self.scene.mesh, *i);
                    let mut after = before.clone();
                    after["texture"] = texture.into();
                    history.faces.push(FacePaintPatch {
                        view: self.scene.history_context.clone(),
                        context: self.scene.history_context.clone(),
                        vertices: ring(&self.scene.mesh, *i)?,
                        before,
                        after,
                    });
                }
                _ => {
                    return Err(
                        "Select textured faces in the active context to edit mapping".into(),
                    )
                }
            }
        }
        if history.faces.iter().all(|p| p.before == p.after) {
            return Ok(false);
        }
        let mut scenes = self.material_scenes();
        apply(&mut scenes, &history)?;
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Material history allocation refused")?;
        let mut before = self.scene.clone();
        before.history_material = Some(history.inverse());
        self.push_prepared_undo_snapshot(before);
        self.publish_material_scenes(scenes, false);
        self.status_message = "Updated selected texture mapping".into();
        Ok(true)
    }
    pub(crate) fn material_history_step(&mut self, redo: bool) -> Option<bool> {
        let snapshot = (if redo {
            self.redo_stack.last()
        } else {
            self.undo_stack.last()
        })?;
        let history = snapshot.history_material.clone()?;
        let snapshot = snapshot.clone();
        let shareback = snapshot.history_shareback;
        let result = (|| -> Result<(), String> {
            let mut scenes = self.material_scenes();
            apply(&mut scenes, &history)?;
            if shareback.is_some() {
                let mut restored_snapshots = vec![snapshot.clone()];
                apply_candidate(&mut restored_snapshots, &history, true)?;
                scenes[0] =
                    crate::component_edit::shareback_restore(&scenes[0], &restored_snapshots[0])?;
            }
            let mut before = self.scene.clone();
            before.history_shareback = shareback;
            before.history_material = Some(history.inverse());
            if redo {
                self.undo_stack
                    .try_reserve(1)
                    .map_err(|_| "Material history allocation refused")?;
                self.redo_stack.pop();
                self.undo_stack.push(before);
            } else {
                self.redo_stack
                    .try_reserve(1)
                    .map_err(|_| "Material history allocation refused")?;
                self.undo_stack.pop();
                self.redo_stack.push(before);
            }
            self.publish_material_scenes(scenes, shareback.is_some());
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.status_message = if redo { "Redo" } else { "Undo" }.into();
                Some(true)
            }
            Err(error) => {
                self.status_message = error;
                Some(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, DVec3};
    use ingetrazo_core::scene::SelectionItem;
    fn mesh() -> Mesh {
        let mut m = Mesh::new();
        let v = [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| m.add_vertex(p));
        let i = m.add_face(v.to_vec(), vec![]).unwrap();
        m.faces[i].attrs=serde_json::from_value(serde_json::json!({"mat":"Oak","color":[0.1,0.2,0.3],"texture":{"path":"old.png","sw":2,"sh":3,"uvw":[1,0,0,0,0,1,0,0]},"opacity":0.4,"back":{"mat":"Oak","color":[0.8,0.7,0.6]},"vendor":{"id":71},"ifc":{"class":"IfcSlab","id":9}})).unwrap();
        m
    }
    fn app() -> IngeTrazoApp {
        let mut app = IngeTrazoApp::new();
        app.scene = Scene::new();
        app.scene.mesh = mesh();
        app.scene.materials.insert("Oak".into(),Material::from_recipe(serde_json::json!({"name":"Oak","color":[0.1,0.2,0.3],"texture":{"path":"old.png","sw":2,"sh":3},"opacity":0.4,"finish":"wood","vendor":72})).unwrap());
        let m = mesh();
        let id = Uuid::new_v4();
        app.scene.register_mesh_share(id, m.clone()).unwrap();
        let mut g = Group::with_mesh("Target", m);
        g.mesh_share = Some(id);
        g.native_document = serde_json::json!({"material":{"mat":"Oak","color":[0.1,0.2,0.3],"vendor":73},"vendor":74});
        let mut other = g.duplicate_instance();
        other.transform = DMat4::from_translation(DVec3::X * 4.);
        app.scene.groups = vec![g, other];
        app
    }
    #[test]
    fn restamp_optional_fields_backs_shared_metadata_and_history() {
        let mut app = app();
        let before = app.scene.clone();
        let rev = app.model_revision;
        assert!(app
            .edit_named_material(
                serde_json::json!({"name":"Oak","color":[0.6,0.5,0.4],"finish":"wood","vendor":72})
            )
            .unwrap());
        assert_eq!(app.model_revision, rev + 1);
        assert_eq!(app.undo_stack.len(), 1);
        for m in std::iter::once(&app.scene.mesh).chain(app.scene.groups.iter().map(|g| &g.mesh)) {
            let f = &m.faces[0];
            assert!(!f.attrs.contains_key("texture"));
            assert!(!f.attrs.contains_key("opacity"));
            assert_eq!(f.attrs["back"], before.mesh.faces[0].attrs["back"]);
            assert_eq!(f.attrs["ifc"], before.mesh.faces[0].attrs["ifc"]);
            assert_eq!(f.attrs["vendor"], before.mesh.faces[0].attrs["vendor"]);
        }
        assert_eq!(app.scene.groups[0].native_document["vendor"], 74);
        assert!(app.scene.groups[0].native_document["material"]
            .get("vendor")
            .is_none());
        let after = app.scene.clone();
        app.undo();
        assert_eq!(app.scene.mesh, before.mesh);
        assert_eq!(app.scene.groups, before.groups);
        assert_eq!(app.scene.materials, before.materials);
        app.redo();
        assert_eq!(app.scene.mesh, after.mesh);
        assert_eq!(app.scene.groups, after.groups);
        assert_eq!(app.scene.materials, after.materials);
        let depth = app.undo_stack.len();
        assert!(!app
            .edit_named_material(
                serde_json::json!({"name":"Oak","color":[0.6,0.5,0.4],"finish":"wood","vendor":72})
            )
            .unwrap());
        assert_eq!(app.undo_stack.len(), depth);
    }
    #[test]
    fn restamp_refusal_has_no_partial_model_or_history() {
        let mut app = app();
        let before = app.scene.clone();
        let revision = app.model_revision;
        assert!(app
            .edit_named_material(serde_json::json!({"name":"Oak","texture":{"sw":0}}))
            .is_err());
        assert_eq!(app.scene, before);
        assert_eq!(app.model_revision, revision);
        assert!(app.undo_stack.is_empty());
        app.edit_named_material(serde_json::json!({"name":"Oak","color":[0.6,0.5,0.4]}))
            .unwrap();
        app.scene.groups[1].mesh.faces[0]
            .attrs
            .insert("color".into(), serde_json::json!([0.2, 0.3, 0.4]));
        let before = app.scene.clone();
        let depth = app.undo_stack.len();
        app.undo();
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack.len(), depth);
        assert!(app.redo_stack.is_empty());
    }
    #[test]
    fn geometry_and_restamp_fold_restore_both_owners() {
        use ingetrazo_core::math::Ray;
        use ingetrazo_tools::base::{ToolContext, ToolType};
        let mut app = app();
        app.scene.groups[0].transform = DMat4::from_translation(DVec3::X * 2.);
        app.scene.groups[0].native_document["component"] = serde_json::json!(true);
        let original = app.scene.clone();
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        app.set_tool(ToolType::Rectangle);
        for p in [DVec3::new(5., 0., 0.), DVec3::new(6., 1., 0.)] {
            app.dispatch_tool_click(&ToolContext::new(
                p,
                Ray::new(p + DVec3::Z * 10., -DVec3::Z),
            ));
        }
        assert_eq!(app.scene.mesh.faces.len(), 2);
        assert_eq!(app.undo_stack.len(), 1);
        app.edit_named_material(serde_json::json!({"name":"Oak","color":[0.6,0.5,0.4]}))
            .unwrap();
        assert_eq!(app.undo_stack.len(), 2);
        app.finish_component_edit();
        assert!(app.component_edit_stack.is_empty());
        assert_eq!(app.undo_stack.len(), 1);
        let after = app.scene.clone();
        assert_eq!(after.groups[0].mesh.faces.len(), 2);
        app.undo();
        assert_eq!(app.status_message, "Undo");
        assert_eq!(app.scene.mesh, original.mesh);
        assert_eq!(app.scene.groups[0].mesh, original.groups[0].mesh);
        assert_eq!(app.scene.groups[1].mesh, original.groups[1].mesh);
        assert_eq!(app.scene.materials, original.materials);
        assert_eq!(app.redo_stack.len(), 1);
        app.redo();
        assert_eq!(app.status_message, "Redo");
        assert_eq!(app.scene.mesh, after.mesh);
        assert_eq!(app.scene.groups[0].mesh, after.groups[0].mesh);
        assert_eq!(app.scene.groups[1].mesh, after.groups[1].mesh);
        assert_eq!(app.scene.materials, after.materials);
    }
    #[test]
    fn restamp_active_and_parked_then_undo() {
        let mut app = app();
        app.scene.groups[0].transform = DMat4::from_translation(DVec3::X * 2.);
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        let active = app.scene.mesh.clone();
        let parked = app.component_edit_stack[0].original_scene.clone();
        app.edit_named_material(serde_json::json!({"name":"Oak","color":[0.6,0.5,0.4]}))
            .unwrap();
        assert!(!app.scene.mesh.faces[0].attrs.contains_key("texture"));
        assert!(!app.component_edit_stack[0].original_scene.mesh.faces[0]
            .attrs
            .contains_key("texture"));
        app.undo();
        assert_eq!(app.scene.mesh, active);
        assert_eq!(app.component_edit_stack[0].original_scene.mesh, parked.mesh);
        app.redo();
        assert!(!app.scene.mesh.faces[0].attrs.contains_key("texture"));
    }
}

#[derive(Clone)]
struct EditorDraft {
    recipe: Value,
    color: [f32; 3],
    has_color: bool,
    has_texture: bool,
    path: String,
    sw: f64,
    sh: f64,
    rot: f64,
    has_opacity: bool,
    opacity: f64,
    error: Option<String>,
}
impl IngeTrazoApp {
    pub(crate) fn open_material_editor(&mut self, ctx: &egui::Context) {
        let Some(material) = self.scene.materials.get(&self.active_material) else {
            self.status_message = "Select an in-model material to edit".into();
            return;
        };
        let recipe = material.registry_recipe();
        let texture = recipe.get("texture");
        let draft = EditorDraft {
            color: material.color[..3].try_into().unwrap(),
            has_color: recipe.get("color").is_some(),
            has_texture: texture.is_some(),
            path: texture
                .and_then(|v| v.get("path"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            sw: texture
                .and_then(|v| v.get("sw"))
                .and_then(Value::as_f64)
                .unwrap_or(1.),
            sh: texture
                .and_then(|v| v.get("sh"))
                .and_then(Value::as_f64)
                .unwrap_or(1.),
            rot: texture
                .and_then(|v| v.get("rot"))
                .and_then(Value::as_f64)
                .unwrap_or(0.),
            has_opacity: recipe.get("opacity").is_some(),
            opacity: recipe.get("opacity").and_then(Value::as_f64).unwrap_or(1.),
            recipe,
            error: None,
        };
        self.show_material_edit_dialog = true;
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("named_material_editor"), draft));
    }
    pub(crate) fn render_material_editor(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("named_material_editor");
        let Some(mut draft) = ctx.data_mut(|data| data.get_temp::<EditorDraft>(id)) else {
            self.show_material_edit_dialog = false;
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Edit material")
            .id(id)
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Material: {}",
                    draft.recipe["name"].as_str().unwrap_or("")
                ));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut draft.has_color, "Color");
                    ui.add_enabled_ui(draft.has_color, |ui| {
                        ui.color_edit_button_rgb(&mut draft.color);
                    });
                });
                ui.checkbox(&mut draft.has_texture, "Texture");
                ui.add_enabled_ui(draft.has_texture, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Image");
                        ui.text_edit_singleline(&mut draft.path);
                        if ui.button("Browse").clicked() {
                            if let Some(path) = crate::dialogs::pick_open_file(
                                "Material texture",
                                "Images (*.png *.jpg *.jpeg *.webp)",
                            ) {
                                draft.path = path.to_string_lossy().into();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tile width (m)");
                        ui.add(egui::DragValue::new(&mut draft.sw).speed(0.01));
                        ui.label("Height (m)");
                        ui.add(egui::DragValue::new(&mut draft.sh).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Rotation (degrees)");
                        ui.add(egui::DragValue::new(&mut draft.rot));
                    });
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut draft.has_opacity, "Opacity");
                    ui.add_enabled(
                        draft.has_opacity,
                        egui::DragValue::new(&mut draft.opacity)
                            .range(0.0..=1.0)
                            .speed(0.01),
                    );
                });
                if let Some(error) = &draft.error {
                    ui.colored_label(egui::Color32::RED, error);
                }
                ui.horizontal(|ui| {
                    apply = ui.button("Apply and restamp").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if cancel {
            open = false;
        }
        if apply {
            let mut recipe = draft.recipe.clone();
            let object = recipe.as_object_mut().unwrap();
            if draft.has_color {
                if object
                    .get("color")
                    .and_then(Value::as_array)
                    .is_none_or(|c| {
                        c.len() != 3
                            || c.iter()
                                .zip(draft.color)
                                .any(|(a, b)| a.as_f64().map(|a| a as f32) != Some(b))
                    })
                {
                    object.insert("color".into(), serde_json::json!(draft.color));
                }
            } else {
                object.remove("color");
            }
            if draft.has_texture {
                let mut texture = object
                    .get("texture")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if !draft.path.is_empty() {
                    if texture.get("path").and_then(Value::as_str) != Some(draft.path.as_str()) {
                        texture.remove("embed");
                        texture.remove("base");
                    }
                    texture.insert("path".into(), draft.path.clone().into());
                }
                texture.insert("sw".into(), draft.sw.into());
                texture.insert("sh".into(), draft.sh.into());
                texture.insert("rot".into(), draft.rot.rem_euclid(360.).into());
                object.insert("texture".into(), texture.into());
            } else {
                object.remove("texture");
            }
            if draft.has_opacity {
                object.insert("opacity".into(), draft.opacity.into());
            } else {
                object.remove("opacity");
            }
            match self.edit_named_material(recipe) {
                Ok(_) => open = false,
                Err(error) => draft.error = Some(error),
            }
        }
        if open {
            ctx.data_mut(|data| data.insert_temp(id, draft));
        } else {
            self.show_material_edit_dialog = false;
            ctx.data_mut(|data| data.remove::<EditorDraft>(id));
            ctx.request_repaint();
        }
    }
}
