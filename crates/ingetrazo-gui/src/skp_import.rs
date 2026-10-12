//! SketchUp App merge policy, distinct from replacement loader semantics.
use crate::IngeTrazoApp;
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
use ingetrazo_formats::skp::app_import::Plan;
use std::collections::HashMap;

pub(crate) fn persistent_registry(current: &Scene, target: &mut Scene) {
    target.skp_registry_persistent = true;
    target.materials.clone_from(&current.materials);
    target
        .embedded_textures
        .clone_from(&current.embedded_textures);
    if let Some(style) = current.native_document.get("back_face_color") {
        target.native_document["back_face_color"] = style.clone();
    } else if let Some(object) = target.native_document.as_object_mut() {
        object.remove("back_face_color");
    }
}
fn remap_mesh(mesh: &mut Mesh, names: &HashMap<String, String>) {
    for face in &mut mesh.faces {
        if let Some(name) = face.attrs.get_mut("mat") {
            if let Some(new) = name.as_str().and_then(|name| names.get(name)) {
                *name = new.clone().into();
            }
        }
    }
}
fn remap_groups(groups: &mut [Group], names: &HashMap<String, String>) {
    for group in groups {
        remap_mesh(&mut group.mesh, names);
        remap_groups(&mut group.children, names);
    }
}
impl IngeTrazoApp {
    pub(crate) fn publish_skp_import(&mut self, mut plan: Plan) -> Result<(), String> {
        let parked = !self.component_edit_stack.is_empty();
        let owner = if parked {
            &self.component_edit_stack[0].original_scene
        } else {
            &self.scene
        };
        let mut candidate = owner.clone();
        for (dst, src) in [(&mut candidate.layers, &self.scene.layers)] {
            if dst != src {
                dst.clone_from(src);
            }
        }
        if candidate.materials != self.scene.materials {
            candidate.materials.clone_from(&self.scene.materials);
        }
        if candidate.embedded_textures != self.scene.embedded_textures {
            candidate
                .embedded_textures
                .clone_from(&self.scene.embedded_textures);
        }
        // These are document-global in source even while editing a component.
        candidate.dimensions.clone_from(&self.scene.dimensions);
        candidate.text_labels.clone_from(&self.scene.text_labels);
        candidate.saved_views.clone_from(&self.scene.saved_views);
        for (old, added) in [
            (candidate.groups.len(), plan.scene.groups.len()),
            (candidate.materials.len(), plan.scene.materials.len()),
            (candidate.dimensions.len(), plan.scene.dimensions.len()),
            (candidate.text_labels.len(), plan.scene.text_labels.len()),
        ] {
            if old.checked_add(added).is_none_or(|n| n > 100000) {
                return Err("SketchUp document inventory admission exceeded".into());
            }
        }
        candidate
            .groups
            .try_reserve(plan.scene.groups.len())
            .map_err(|_| "SketchUp group allocation refused")?;
        candidate
            .layers
            .try_reserve(plan.scene.layers.len())
            .map_err(|_| "SketchUp layer allocation refused")?;
        candidate
            .materials
            .try_reserve(plan.scene.materials.len())
            .map_err(|_| "SketchUp material allocation refused")?;
        candidate
            .dimensions
            .try_reserve(plan.scene.dimensions.len())
            .map_err(|_| "SketchUp dimension allocation refused")?;
        candidate
            .text_labels
            .try_reserve(plan.scene.text_labels.len())
            .map_err(|_| "SketchUp text allocation refused")?;
        candidate
            .embedded_textures
            .try_reserve(plan.scene.embedded_textures.len())
            .map_err(|_| "SketchUp resource allocation refused")?;
        candidate
            .component_definitions
            .try_reserve(plan.scene.component_definitions.len())
            .map_err(|_| "SketchUp definition allocation refused")?;
        candidate
            .mesh_shares
            .try_reserve(plan.scene.mesh_shares.len())
            .map_err(|_| "SketchUp share allocation refused")?;
        let mut names = HashMap::new();
        names
            .try_reserve(plan.material_order.len())
            .map_err(|_| "SketchUp material join allocation refused")?;
        for base in plan.material_order {
            let Some(mut material) = plan.scene.materials.remove(&base) else {
                return Err("SketchUp material order lacks recipe".into());
            };
            let recipe = material.registry_recipe();
            let mut name = base.clone();
            let mut n = 2;
            while let Some(old) = candidate.materials.get(&name) {
                let old = old.registry_recipe();
                if ["color", "texture", "opacity"]
                    .into_iter()
                    .all(|key| old.get(key) == recipe.get(key))
                {
                    break;
                }
                name = format!("{base} ({n})");
                n += 1;
                if n > 100002 {
                    return Err("SketchUp material name admission exceeded".into());
                }
            }
            if !candidate.materials.contains_key(&name) {
                if name != base {
                    material.name = name.clone();
                    material.native_document["name"] = name.clone().into();
                    if let Some(object) = material.native_document.as_object_mut() {
                        object.remove("finish");
                    }
                    material.native_baseline = material.archive_model();
                }
                candidate.materials.insert(name.clone(), material);
            }
            if name != base {
                names.insert(base, name);
            }
        }
        remap_groups(&mut plan.scene.groups, &names);
        for mesh in plan.scene.mesh_shares.values_mut() {
            remap_mesh(mesh, &names);
        }
        for definition in plan.scene.component_definitions.values_mut() {
            remap_mesh(&mut definition.mesh, &names);
            remap_groups(&mut definition.children, &names);
        }
        for layer in plan.scene.layers {
            if !candidate.layers.iter().any(|old| old.name == layer.name) {
                candidate.layers.push(layer);
            }
        }
        candidate
            .saved_views
            .try_reserve(plan.scene.saved_views.len())
            .map_err(|_| "SketchUp view allocation refused")?;
        for view in plan.scene.saved_views {
            if !candidate
                .saved_views
                .iter()
                .any(|old| old.name == view.name)
            {
                candidate.saved_views.push(view);
            }
        }
        candidate.groups.extend(plan.scene.groups);
        candidate.dimensions.extend(plan.scene.dimensions);
        candidate.text_labels.extend(plan.scene.text_labels);
        candidate
            .embedded_textures
            .extend(plan.scene.embedded_textures);
        candidate
            .component_definitions
            .extend(plan.scene.component_definitions);
        candidate.mesh_shares.extend(plan.scene.mesh_shares);
        if candidate
            .native_document
            .get("back_face_color")
            .is_none_or(|v| v.is_null())
        {
            if let Some(color) = plan.scene.native_document.get("back_face_color") {
                candidate.native_document["back_face_color"] = color.clone();
            }
        }
        candidate.skp_registry_persistent = true;
        candidate.bind_dimensions();
        if parked {
            self.publish_root_import_policy(candidate, None, true)?;
            // Source SnapshotImport runs inside the active Qt edit lifecycle
            // even though SKP placements append at document root. Retain that
            // policy for subsequent share-back of pre-existing edits; the
            // existing finish guard still requires float32-authored frame and
            // points, preserving native f64 placement/edit semantics.
            if let Some(session) = self.component_edit_stack.last_mut() {
                session.note_source_mesh_import();
            }
        } else {
            self.undo_stack
                .try_reserve(1)
                .map_err(|_| "SketchUp history allocation refused")?;
            // Source registration/style are outside SnapshotImport; accepted empty
            // imports still append a history command.
            persistent_registry(&candidate, &mut self.scene);
            self.save_undo_state();
            if let Some(before) = self.undo_stack.last_mut() {
                before.history_skp_import = true;
                before.history_layers.0 = Some(ingetrazo_core::layers::LayerDelta::between(
                    &before.layers,
                    &candidate.layers,
                ));
                before.history_views.0 =
                    Some(ingetrazo_core::saved_views::ViewMembershipDelta::between(
                        &before.saved_views,
                        &candidate.saved_views,
                    ));
            }
            self.scene = candidate;
            self.bump_model_revision();
        }
        Ok(())
    }
}
