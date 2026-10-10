//! LayersPanel registry edits stay outside History; imports/purge own membership.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    layers::{Layer, LayerDelta},
    scene::Scene,
};
pub(crate) fn restore(current: &Scene, target: &mut Scene) -> Option<LayerDelta> {
    let delta = target.history_layers.0.take();
    target.layers = delta
        .as_ref()
        .map_or_else(|| current.layers.clone(), |d| d.restore(&current.layers));
    if let Some((name, true, entities)) = delta.as_ref().and_then(|d| d.assignment.as_ref()) {
        let selection = std::mem::take(&mut target.selection);
        target.selection = entities.iter().cloned().collect();
        target.assign_selection_to_layer(name);
        target.selection = selection;
    }
    delta.map(|d| d.inverse())
}
impl IngeTrazoApp {
    pub(crate) fn seal_layer_history(&mut self) {
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_context == self.scene.history_context
                && before.history_layers.0.is_none()
            {
                before.history_layers.0 =
                    Some(LayerDelta::between(&before.layers, &self.scene.layers));
            }
        }
    }
    pub(crate) fn seal_layer_history_before(&mut self, current: &Scene) {
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_layers.0.is_none() {
                before.history_layers.0 =
                    Some(LayerDelta::between(&before.layers, &current.layers));
            }
        }
    }
    fn refresh_history_layer(&mut self, layer: &Layer) {
        for s in self.undo_stack.iter_mut().chain(&mut self.redo_stack) {
            if let Some(d) = &mut s.history_layers.0 {
                d.refresh(layer);
            }
        }
        for session in &mut self.component_edit_stack {
            for s in session
                .original_undo
                .iter_mut()
                .chain(&mut session.original_redo)
            {
                if let Some(d) = &mut s.history_layers.0 {
                    d.refresh(layer);
                }
            }
        }
    }
    fn prune_layer_selection(&mut self, name: &str) {
        use ingetrazo_core::scene::SelectionItem;
        let dead: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter(|item| match item {
                SelectionItem::Face(i) => {
                    self.scene.mesh.faces.get(*i).and_then(|f| {
                        f.layer
                            .as_deref()
                            .or_else(|| f.attrs.get("layer").and_then(|v| v.as_str()))
                    }) == Some(name)
                }
                SelectionItem::Edge(i) => {
                    self.scene
                        .mesh
                        .edges
                        .get(*i)
                        .and_then(|e| e.layer.as_deref())
                        == Some(name)
                }
                SelectionItem::Group(i) => {
                    self.scene.groups.get(*i).and_then(|g| g.layer.as_deref()) == Some(name)
                }
                _ => false,
            })
            .cloned()
            .collect();
        for item in dead {
            self.scene.deselect(&item);
        }
    }
    pub fn add_layer(&mut self, name: String) -> bool {
        if name.trim().is_empty() || self.scene.layers.iter().any(|l| l.name == name) {
            return false;
        }
        self.seal_layer_history();
        self.scene.layers.push(Layer::new(name));
        self.scene.bump_view_version();
        true
    }
    pub fn set_layer_locked(&mut self, index: usize, locked: bool) -> bool {
        if self
            .scene
            .layers
            .get(index)
            .is_none_or(|l| l.locked == locked)
        {
            return false;
        }
        self.seal_layer_history();
        self.scene.layers[index].locked = locked;
        let layer = self.scene.layers[index].clone();
        self.refresh_history_layer(&layer);
        if !layer.visible || layer.locked {
            self.prune_layer_selection(&layer.name);
        }
        self.scene.bump_view_version();
        true
    }
    pub(crate) fn change_layer_visibility(&mut self, index: usize, visible: bool) -> bool {
        if self
            .scene
            .layers
            .get(index)
            .is_none_or(|l| l.visible == visible)
        {
            return false;
        }
        self.seal_layer_history();
        self.scene.layers[index].visible = visible;
        let layer = self.scene.layers[index].clone();
        self.refresh_history_layer(&layer);
        if !layer.visible || layer.locked {
            self.prune_layer_selection(&layer.name);
        }
        self.scene.bump_view_version();
        true
    }
    pub fn rename_layer(&mut self, index: usize, name: String) -> bool {
        let Some(layer) = self.scene.layers.get(index) else {
            return false;
        };
        let old = layer.name.clone();
        if matches!(old.as_str(), "Layer0" | "Layer 0" | "Capa 0")
            || name.trim().is_empty()
            || old == name
            || self.scene.layers.iter().any(|l| l.name == name)
        {
            return false;
        }
        self.seal_layer_history();
        self.scene.layers[index].name = name.clone();
        for face in &mut self.scene.mesh.faces {
            if face
                .layer
                .as_deref()
                .or_else(|| face.attrs.get("layer").and_then(|v| v.as_str()))
                == Some(&old)
            {
                face.layer = Some(name.clone());
                face.attrs.insert("layer".into(), name.clone().into());
            }
        }
        for edge in &mut self.scene.mesh.edges {
            if edge.layer.as_deref() == Some(&old) {
                edge.layer = Some(name.clone());
            }
        }
        for group in &mut self.scene.groups {
            if group.layer.as_deref() == Some(&old) {
                group.layer = Some(name.clone());
            }
        }
        let layer = self.scene.layers[index].clone();
        self.refresh_history_layer(&layer);
        if !layer.visible || layer.locked {
            self.prune_layer_selection(&layer.name);
        }
        self.scene.bump_view_version();
        true
    }
    pub fn assign_selection_to_named_layer(&mut self, name: &str) -> usize {
        if name.trim().is_empty()
            || !self.scene.selection.iter().any(|s| {
                matches!(
                    s,
                    ingetrazo_core::scene::SelectionItem::Face(_)
                        | ingetrazo_core::scene::SelectionItem::Edge(_)
                        | ingetrazo_core::scene::SelectionItem::Group(_)
                )
            })
        {
            return 0;
        }
        let created = !self.scene.layers.iter().any(|l| l.name == name);
        self.save_undo_state();
        if created {
            self.scene.layers.push(Layer::new(name));
            let entities = self.scene.selection.iter().cloned().collect();
            self.undo_stack.last_mut().unwrap().history_layers.0 = Some(LayerDelta {
                assignment: Some((name.into(), false, entities)),
                ..Default::default()
            });
        }
        self.scene.assign_selection_to_layer(name)
    }

    pub fn delete_layer(&mut self, index: usize) -> bool {
        let Some(layer) = self.scene.layers.get(index) else {
            return false;
        };
        let name = layer.name.clone();
        if matches!(name.as_str(), "Layer0" | "Layer 0" | "Capa 0") {
            return false;
        }
        self.seal_layer_history();
        fn mesh(m: &mut ingetrazo_core::mesh::Mesh, name: &str) {
            for f in &mut m.faces {
                if f.layer
                    .as_deref()
                    .or_else(|| f.attrs.get("layer").and_then(|v| v.as_str()))
                    == Some(name)
                {
                    f.layer = None;
                    f.attrs.remove("layer");
                }
            }
            for e in &mut m.edges {
                if e.layer.as_deref() == Some(name) {
                    e.layer = None;
                }
            }
        }
        fn groups(gs: &mut [ingetrazo_core::groups::Group], name: &str) {
            for g in gs {
                if g.layer.as_deref() == Some(name) {
                    g.layer = None;
                }
                mesh(&mut g.mesh, name);
                groups(&mut g.children, name);
            }
        }
        mesh(&mut self.scene.mesh, &name);
        groups(&mut self.scene.groups, &name);
        for m in self.scene.mesh_shares.values_mut() {
            mesh(m, &name);
        }
        for d in self.scene.component_definitions.values_mut() {
            mesh(&mut d.mesh, &name);
            groups(&mut d.children, &name);
        }
        for d in &mut self.scene.dimensions {
            if d.layer.as_deref() == Some(&name) {
                d.layer = None;
            }
        }
        for t in &mut self.scene.text_labels {
            if t.layer.as_deref() == Some(&name) {
                t.layer = None;
            }
        }
        for p in &mut self.scene.section_planes {
            if p.native_document.get("layer").and_then(|v| v.as_str()) == Some(&name) {
                p.native_document.as_object_mut().unwrap().remove("layer");
            }
        }
        self.scene.layers.remove(index);
        self.active_layer_idx = self
            .active_layer_idx
            .min(self.scene.layers.len().saturating_sub(1));
        self.scene.bump_view_version();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::{
        groups::Group,
        mesh::Mesh,
        scene::{Scene, SelectionItem},
    };
    fn setup() -> IngeTrazoApp {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        a
    }
    #[test]
    fn registry_membership_is_not_ordinary_geometry_history() {
        let mut a = setup();
        a.save_undo_state();
        a.scene.mesh.add_vertex(DVec3::X);
        a.add_layer("Panel".into());
        a.set_layer_visibility(1, false);
        a.set_layer_locked(1, true);
        a.undo();
        assert!(a.scene.mesh.vertices.is_empty());
        assert_eq!(
            (a.scene.layers[1].visible, a.scene.layers[1].locked),
            (false, true)
        );
        a.redo();
        assert_eq!(a.scene.mesh.vertices.len(), 1);
        let raw = serde_json::to_value(&a.scene).unwrap();
        assert!(raw.get("history_layers").is_none());
        assert!(raw["layers"][0].get("history_id").is_none());
    }
    #[test]
    fn delete_selected_layer_reassigns_nested_mesh_and_keeps_other_layer() {
        let mut a = setup();
        a.add_layer("Tagged".into());
        a.add_layer("Keep".into());
        let mut mesh = Mesh::new();
        let x = mesh.add_vertex(DVec3::ZERO);
        let y = mesh.add_vertex(DVec3::X);
        let id = mesh.add_edge(x, y).unwrap();
        mesh.edges[id].layer = Some("Tagged".into());
        let mut child = Group::with_mesh("child", mesh);
        child.layer = Some("Tagged".into());
        let mut parent = Group::new("parent");
        parent.children.push(child);
        a.scene.groups.push(parent);
        assert!(a.delete_layer(1));
        assert_eq!(a.scene.layers[1].name, "Keep");
        assert!(a.scene.groups[0].children[0].layer.is_none());
        assert!(a.scene.groups[0].children[0].mesh.edges[0].layer.is_none());
        assert!(a.undo_stack.is_empty());
    }
    #[test]
    fn nested_ordinary_layer_panel_state_survives_lifted_model_undo() {
        let mut a = setup();
        let mut mesh = Mesh::new();
        mesh.add_vertex(DVec3::ZERO);
        a.scene.groups.push(Group::with_mesh("leaf", mesh));
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        a.save_undo_state();
        a.scene
            .geo_points
            .push(ingetrazo_core::georef::GeoPoint::new(DVec3::X, "Point", ""));
        a.add_layer("Global".into());
        a.finish_component_edit();
        a.undo();
        assert!(a.scene.geo_points.is_empty());
        assert!(a.scene.layers.iter().any(|l| l.name == "Global"));
        a.redo();
        assert_eq!(a.scene.geo_points.len(), 1);
    }
}
