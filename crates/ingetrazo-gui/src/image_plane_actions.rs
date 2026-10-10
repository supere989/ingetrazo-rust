//! Original global reference-image object and command ownership.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    image_plane::{ImageDelta, ImagePlane},
    layers::Layer,
    scene::{Scene, SelectionItem},
};
pub(crate) fn restore(current: &Scene, target: &mut Scene) -> Option<ImageDelta> {
    let delta = target.history_images.0.take();
    current.image_objects.publish(&current.image_planes);
    target.image_objects = current.image_objects.clone();
    target.image_planes = delta.as_ref().map_or_else(
        || current.image_planes.clone(),
        |d| d.restore(&current.image_planes, &current.image_objects),
    );
    if let Some(d) = &delta {
        if let Some(created) = &d.add_layer_created {
            let mut made = created.lock().unwrap();
            if let Some((_, image)) = d.added.first() {
                // Undo removes by the command-held image's CURRENT name,
                // not the identity of the layer initially created.
                let image = current
                    .image_planes
                    .iter()
                    .find(|p| p.id == image.id)
                    .unwrap_or(image);
                if *made {
                    if let Some(name) = image.layer.as_deref().filter(|n| !n.is_empty()) {
                        if let Some(i) = target.layers.iter().position(|l| l.name == name) {
                            target.layers.remove(i);
                        }
                    }
                }
                *made = false;
            } else if let Some((_, image)) = d.removed.first() {
                let image = target
                    .image_planes
                    .iter()
                    .find(|p| p.id == image.id)
                    .unwrap_or(image);
                *made = false;
                if let Some(name) = image.layer.as_deref().filter(|n| !n.is_empty()) {
                    if !target.layers.iter().any(|l| l.name == name) {
                        target.layers.push(Layer::new(name));
                        *made = true;
                    }
                }
            }
        }
        if !d.added.is_empty() || !d.removed.is_empty() || !d.changed.is_empty() {
            fn remap(
                item: &SelectionItem,
                current: &Scene,
                target: &Scene,
            ) -> Option<SelectionItem> {
                match item {
                    SelectionItem::ImagePlane(i) => {
                        let id = current.image_planes.get(*i)?.id;
                        target
                            .image_planes
                            .iter()
                            .position(|p| p.id == id)
                            .map(SelectionItem::ImagePlane)
                    }
                    _ => Some(item.clone()),
                }
            }
            target.selection = current
                .selection
                .iter()
                .filter_map(|i| remap(i, current, target))
                .collect();
            target.selection_order = current
                .selection_order
                .iter()
                .filter_map(|i| remap(i, current, target))
                .collect();
            if d.add_command && !d.removed.is_empty() {
                target.selection.clear();
                target.selection_order.clear();
                for (_, p) in &d.removed {
                    if let Some(i) = target.image_planes.iter().position(|t| t.id == p.id) {
                        target.selection.insert(SelectionItem::ImagePlane(i));
                        target.selection_order.push(SelectionItem::ImagePlane(i));
                    }
                }
            }
        }
    }
    delta.map(|d| d.inverse())
}
impl IngeTrazoApp {
    pub(crate) fn seal_image_history(&mut self) {
        self.scene.image_objects.publish(&self.scene.image_planes);
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_context == self.scene.history_context
                && before.history_images.0.is_none()
            {
                before.history_images.0 = Some(ImageDelta::between(
                    &before.image_planes,
                    &self.scene.image_planes,
                ));
            }
        }
    }
    pub(crate) fn seal_image_history_before(&mut self, current: &Scene) {
        current.image_objects.publish(&current.image_planes);
        if let Some(before) = self.undo_stack.last_mut() {
            if before.history_images.0.is_none() {
                before.history_images.0 = Some(ImageDelta::between(
                    &before.image_planes,
                    &current.image_planes,
                ));
            }
        }
    }
    pub fn set_image_plane_locked(&mut self, id: uuid::Uuid, locked: bool) {
        let Some(i) = self.scene.image_planes.iter().position(|p| p.id == id) else {
            return;
        };
        if self.scene.image_planes[i].locked == locked {
            return;
        }
        self.seal_image_history();
        self.scene.image_planes[i].locked = locked;
        self.scene.image_objects.publish(&self.scene.image_planes);
        self.scene.bump_version();
    }
    pub fn add_image_plane(&mut self, mut image: ImagePlane) {
        self.save_undo_state();
        if !self.scene.embedded_textures.contains_key(&image.path) {
            if let Ok(bytes) = std::fs::read(&image.path) {
                let name = std::path::Path::new(&image.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let hash = blake3::hash(&bytes).to_hex();
                let key = format!("textures/{}-{name}", &hash[..16]);
                self.scene.embedded_textures.insert(key.clone(), bytes);
                image.path = key;
            }
        }
        let mut made_layer = false;
        if let Some(name) = &image.layer {
            if !self.scene.layers.iter().any(|l| &l.name == name) {
                self.scene.layers.push(Layer::new(name));
                made_layer = true;
            }
        }
        self.scene.image_planes.push(image);
        if let Some(before) = self.undo_stack.last_mut() {
            let mut delta = ImageDelta::between(&before.image_planes, &self.scene.image_planes);
            delta.add_layer_created = Some(std::sync::Arc::new(std::sync::Mutex::new(made_layer)));
            before.history_images.0 = Some(delta);
            // This command owns layer membership by name and do-time lookup.
            before.history_layers.0 = Some(Default::default());
        }
        self.scene.clear_selection();
        self.scene
            .select(SelectionItem::ImagePlane(self.scene.image_planes.len() - 1));
        self.scene.image_objects.publish(&self.scene.image_planes);
        self.scene.bump_version();
    }
    pub fn delete_image_planes(&mut self, ids: &[uuid::Uuid]) {
        if !self.scene.image_planes.iter().any(|p| ids.contains(&p.id)) {
            return;
        }
        self.save_undo_state();
        self.scene.image_objects.publish(&self.scene.image_planes);
        let selected: Vec<_> = self
            .scene
            .selection
            .iter()
            .filter_map(|item| {
                if let SelectionItem::ImagePlane(i) = item {
                    self.scene.image_planes.get(*i).map(|p| p.id)
                } else {
                    None
                }
            })
            .collect();
        self.scene.image_planes.retain(|p| !ids.contains(&p.id));
        self.scene
            .selection
            .retain(|i| !matches!(i, SelectionItem::ImagePlane(_)));
        self.scene
            .selection_order
            .retain(|i| !matches!(i, SelectionItem::ImagePlane(_)));
        for id in selected {
            if let Some(i) = self.scene.image_planes.iter().position(|p| p.id == id) {
                self.scene.select(SelectionItem::ImagePlane(i));
            }
        }
        self.scene.bump_version();
    }
    pub fn move_image_planes(
        &mut self,
        ids: &[uuid::Uuid],
        delta: glam::DVec3,
    ) -> Result<(), String> {
        if !delta.is_finite() {
            return Err("Image translation must be finite".into());
        }
        if !self.scene.image_planes.iter().any(|p| ids.contains(&p.id)) {
            return Ok(());
        }
        self.save_undo_state();
        for p in &mut self.scene.image_planes {
            if ids.contains(&p.id) {
                p.origin = (p.origin + delta).as_vec3().as_dvec3();
            }
        }
        self.scene.image_objects.publish(&self.scene.image_planes);
        self.scene.bump_version();
        Ok(())
    }
    pub fn set_image_plane_opacity(&mut self, id: uuid::Uuid, opacity: f64) -> Result<(), String> {
        if !opacity.is_finite() {
            return Err("Image opacity must be finite".into());
        }
        let Some(i) = self.scene.image_planes.iter().position(|p| p.id == id) else {
            return Ok(());
        };
        self.save_undo_state();
        self.scene.image_planes[i].opacity = opacity.clamp(0., 1.);
        self.scene.image_objects.publish(&self.scene.image_planes);
        self.scene.bump_version();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, DVec3};
    use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
    #[test]
    fn add_image_layer_redo_uses_current_name_and_rechecks_creation() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.add_image_plane(ImagePlane::new("scan", DVec3::ZERO, DVec3::X, DVec3::Y));
        let i = a
            .scene
            .layers
            .iter()
            .position(|l| l.name == "Images")
            .unwrap();
        a.rename_layer(i, "Renamed".into());
        a.undo();
        assert!(a.scene.layers.iter().any(|l| l.name == "Renamed"));
        a.scene.layers.push(Layer::new("Images"));
        a.redo();
        a.undo();
        assert!(a.scene.layers.iter().any(|l| l.name == "Images"));
        assert!(a.scene.image_planes.is_empty());
    }
    #[test]
    fn delete_image_layer_is_global_and_survives_owned_frame_undo() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let p = ImagePlane::new("scan", DVec3::ZERO, DVec3::X, DVec3::Y);
        let id = p.id;
        a.scene.layers.push(Layer::new("Images"));
        a.scene.image_planes.push(p);
        let mut g = Group::new("Leaf");
        g.mesh.add_vertex(DVec3::ZERO);
        a.scene.groups.push(g);
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        a.move_image_planes(&[id], DVec3::X).unwrap();
        let i = a
            .scene
            .layers
            .iter()
            .position(|l| l.name == "Images")
            .unwrap();
        a.delete_layer(i);
        a.finish_component_edit();
        a.undo();
        assert_eq!(a.scene.image_planes[0].origin, DVec3::ZERO);
        assert_eq!(a.scene.image_planes[0].layer, None);
        a.redo();
        assert_eq!(a.scene.image_planes[0].origin, DVec3::X);
        assert_eq!(a.scene.image_planes[0].layer, None);
    }
    #[test]
    fn reference_only_finish_keeps_precise_prototype_and_layer_owned_history() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let mut m = Mesh::new();
        m.add_vertex(DVec3::new(0.123456789, 0.234567891, 0.345678912));
        let mut g = Group::new("Leaf");
        g.mesh = m.clone();

        g.transform = DMat4::from_translation(DVec3::new(0.731, 0.932, 0.127));
        a.scene.groups.push(g);
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        let p = ImagePlane::new("scan", DVec3::ZERO, DVec3::X, DVec3::Y);
        let id = p.id;
        a.add_image_plane(p);
        a.finish_component_edit();
        assert_eq!(a.scene.groups[0].mesh, m);
        assert_eq!(a.undo_stack.len(), 1);
        a.undo();
        assert!(a.scene.image_planes.is_empty());
        assert!(!a.scene.layers.iter().any(|l| l.name == "Images"));
        a.redo();
        assert_eq!(a.scene.image_planes[0].id, id);
    }
    #[test]
    fn unrelated_geometry_history_does_not_own_plane_and_bad_input_is_atomic() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let p = ImagePlane::new("scan", DVec3::ZERO, DVec3::X, DVec3::Y);
        let id = p.id;
        a.scene.image_planes.push(p);
        a.save_undo_state();
        a.scene.mesh.add_vertex(DVec3::X);
        a.scene.bump_version();
        let before = a.scene.clone();
        assert!(a.move_image_planes(&[id], DVec3::splat(f64::NAN)).is_err());
        assert_eq!(a.scene, before);
        a.set_image_plane_locked(id, true);
        a.undo();
        assert!(a.scene.image_planes[0].locked);
        a.redo();
        assert!(a.scene.image_planes[0].locked);
    }
}
