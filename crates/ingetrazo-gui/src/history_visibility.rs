//! Hide actions own object identities, including objects detached from the document.
use crate::IngeTrazoApp;
use ingetrazo_core::{
    groups::Group,
    saved_views::{GroupVisibilityCommand, VisibilityTarget as Target},
    scene::Scene,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn group_flags(groups: &[Group], out: &mut HashMap<Target, bool>) {
    for group in groups {
        out.insert(Target::Group(group.history_id), group.hidden);
        group_flags(&group.children, out);
    }
}
fn flags(scene: &Scene) -> HashMap<Target, bool> {
    let mut out = HashMap::new();
    group_flags(&scene.groups, &mut out);
    out.extend(
        scene
            .mesh
            .edges
            .iter()
            .map(|e| (Target::Edge(e.history_id), e.hidden)),
    );
    out.extend(
        scene
            .mesh
            .faces
            .iter()
            .map(|f| (Target::Face(f.history_id), hidden_face(f))),
    );
    out
}
pub(crate) fn mesh_snapshot_flags(scene: &Scene) -> Vec<(Target, Option<serde_json::Value>)> {
    scene
        .mesh
        .edges
        .iter()
        .map(|e| (Target::Edge(e.history_id), Some(e.hidden.into())))
        .chain(
            scene
                .mesh
                .faces
                .iter()
                .map(|f| (Target::Face(f.history_id), f.attrs.get("hidden").cloned())),
        )
        .collect()
}
fn apply_snapshot(scene: &mut Scene, fields: &[(Target, Option<serde_json::Value>)]) {
    let fields: HashMap<_, _> = fields.iter().map(|(id, value)| (*id, value)).collect();
    for e in &mut scene.mesh.edges {
        if let Some(Some(value)) = fields.get(&Target::Edge(e.history_id)) {
            e.hidden = value.as_bool().expect("captured edge bool");
        }
    }
    for f in &mut scene.mesh.faces {
        if let Some(value) = fields.get(&Target::Face(f.history_id)) {
            if let Some(value) = value {
                f.attrs.insert("hidden".into(), value.clone());
            } else {
                f.attrs.remove("hidden");
            }
        }
    }
}
fn hidden_face(face: &ingetrazo_core::mesh::MeshFace) -> bool {
    face.attrs.get("hidden").is_some_and(|v| match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(v) => *v,
        serde_json::Value::Number(v) => v.as_f64() != Some(0.0),
        serde_json::Value::String(v) => !v.is_empty(),
        serde_json::Value::Array(v) => !v.is_empty(),
        serde_json::Value::Object(v) => !v.is_empty(),
    })
}
fn apply_groups(groups: &mut [Group], values: &HashMap<Target, bool>) {
    for group in groups {
        if let Some(&hidden) = values.get(&Target::Group(group.history_id)) {
            group.hidden = hidden;
        }
        apply_groups(&mut group.children, values);
    }
}
fn apply(scene: &mut Scene, values: &HashMap<Target, bool>, mesh: bool) {
    apply_groups(&mut scene.groups, values);
    if !mesh {
        return;
    }
    for edge in &mut scene.mesh.edges {
        if let Some(&hidden) = values.get(&Target::Edge(edge.history_id)) {
            edge.hidden = hidden;
        }
    }
    for face in &mut scene.mesh.faces {
        if let Some(&hidden) = values.get(&Target::Face(face.history_id)) {
            if hidden {
                face.attrs.insert("hidden".into(), true.into());
            } else {
                face.attrs.remove("hidden");
            }
        }
    }
}

pub(crate) fn restore(
    current: &Scene,
    target: &mut Scene,
    owners: &mut HashMap<Target, bool>,
) -> Option<GroupVisibilityCommand> {
    let live = flags(current);
    for (&id, value) in owners.iter_mut() {
        if let Some(&hidden) = live.get(&id) {
            *value = hidden;
        }
    }
    // Groups are live placement objects. Mesh snapshots explicitly capture flags;
    // only delta commands retain live mesh flags instead of snapshot values.
    apply(
        target,
        &live,
        target
            .history_group_visibility
            .as_ref()
            .is_some_and(|c| c.retain_mesh_flags),
    );
    apply(
        target,
        owners,
        target
            .history_group_visibility
            .as_ref()
            .is_some_and(|c| c.retain_mesh_flags),
    );
    let Some(command) = target.history_group_visibility.take() else {
        // A true mesh snapshot overwrites the state of the captured objects.
        let restored = flags(target);
        for (&id, value) in owners.iter_mut() {
            if !matches!(id, Target::Group(_)) {
                if let Some(&hidden) = restored.get(&id) {
                    *value = hidden;
                }
            }
        }
        return None;
    };
    if let Some(after) = &command.snapshot_after {
        if !command.undo {
            apply_snapshot(target, after);
        }
        let restored = flags(target);
        for (&id, value) in owners.iter_mut() {
            if !matches!(id, Target::Group(_)) {
                if let Some(&hidden) = restored.get(&id) {
                    *value = hidden;
                }
            }
        }
        return Some(GroupVisibilityCommand {
            undo: !command.undo,
            ..command
        });
    }
    if !command.targets.is_empty() {
        target.selection = current.selection.clone();
        target.selection_order = current.selection_order.clone();
    }
    let mut values = HashMap::with_capacity(command.targets.len());
    let mut previous = Vec::with_capacity(command.targets.len());
    for &(id, before) in &command.targets {
        previous.push((
            id,
            owners
                .get(&id)
                .or_else(|| live.get(&id))
                .copied()
                .unwrap_or(before),
        ));
        let hidden = if command.undo { before } else { command.hidden };
        values.insert(id, hidden);
        owners.insert(id, hidden);
    }
    apply(target, &values, true);
    Some(GroupVisibilityCommand {
        targets: if command.undo {
            command.targets
        } else {
            previous
        },
        retain_mesh_flags: command.retain_mesh_flags,
        snapshot_after: command.snapshot_after,
        hidden: command.hidden,
        undo: !command.undo,
    })
}
impl IngeTrazoApp {
    /// Inspect command-owned visibility, including a detached history target.
    pub fn visibility_target_hidden(&self, target: Target) -> Option<bool> {
        self.all_object_flags()
            .get(&target)
            .or_else(|| self.visibility_owners.get(&target))
            .copied()
    }
    fn visibility_flags(&self) -> HashMap<Target, bool> {
        let mut result = HashMap::new();
        for session in &self.component_edit_stack {
            group_flags(&session.original_scene.groups, &mut result);
        }
        result.extend(flags(&self.scene));
        result
    }
    fn all_object_flags(&self) -> HashMap<Target, bool> {
        let mut result = HashMap::new();
        fn meshes(groups: &[Group], out: &mut HashMap<Target, bool>) {
            for g in groups {
                out.extend(
                    g.mesh
                        .edges
                        .iter()
                        .map(|e| (Target::Edge(e.history_id), e.hidden)),
                );
                out.extend(
                    g.mesh
                        .faces
                        .iter()
                        .map(|f| (Target::Face(f.history_id), hidden_face(f))),
                );
                meshes(&g.children, out);
            }
        }
        for session in &self.component_edit_stack {
            meshes(&session.original_scene.groups, &mut result);
            result.extend(flags(&session.original_scene));
        }
        meshes(&self.scene.groups, &mut result);
        result.extend(flags(&self.scene));
        result
    }
    pub(crate) fn apply_context_visibility(&mut self) {
        fn meshes(groups: &mut [Group], values: &HashMap<Target, bool>) {
            for g in groups {
                for e in &mut g.mesh.edges {
                    if let Some(&hidden) = values.get(&Target::Edge(e.history_id)) {
                        e.hidden = hidden;
                    }
                }
                for f in &mut g.mesh.faces {
                    if let Some(&hidden) = values.get(&Target::Face(f.history_id)) {
                        if hidden {
                            f.attrs.insert("hidden".into(), true.into());
                        } else {
                            f.attrs.remove("hidden");
                        }
                    }
                }
                meshes(&mut g.children, values);
            }
        }
        meshes(&mut self.scene.groups, &self.visibility_owners);
        for session in &mut self.component_edit_stack {
            apply(&mut session.original_scene, &self.visibility_owners, true);
            meshes(&mut session.original_scene.groups, &self.visibility_owners);
            apply(&mut session.cancel_scene, &self.visibility_owners, true);
            meshes(&mut session.cancel_scene.groups, &self.visibility_owners);
        }
    }

    pub(crate) fn sync_visibility_owners(&mut self) {
        if self.visibility_owners.is_empty() {
            return;
        }
        let live = self.all_object_flags();
        let retained: HashSet<_> = self
            .undo_stack
            .iter()
            .chain(&self.redo_stack)
            .chain(
                self.component_edit_stack
                    .iter()
                    .flat_map(|s| s.original_undo.iter().chain(&s.original_redo)),
            )
            .filter_map(|s| s.history_group_visibility.as_ref())
            .flat_map(|c| c.targets.iter().map(|&(id, _)| id))
            .collect();
        self.visibility_owners.retain(|id, value| {
            if let Some(&hidden) = live.get(id) {
                *value = hidden;
            }
            retained.contains(id)
        });
    }
    /// Complete an identity-preserving mesh snapshot command's flag ownership.
    /// Called at publication, before later out-of-history view changes.
    pub fn seal_snapshot_visibility(&mut self) {
        let after = mesh_snapshot_flags(&self.scene);
        if let Some(before) = self.undo_stack.last_mut() {
            before.history_group_visibility = Some(GroupVisibilityCommand {
                targets: Vec::new(),
                hidden: false,
                undo: true,
                retain_mesh_flags: false,
                snapshot_after: Some(after),
            });
        }
    }
    pub(crate) fn mark_visibility_delta(&mut self) {
        if let Some(before) = self.undo_stack.last_mut() {
            before.history_group_visibility = Some(GroupVisibilityCommand {
                targets: Vec::new(),
                hidden: false,
                undo: true,
                retain_mesh_flags: true,
                snapshot_after: None,
            });
        }
    }
    pub(crate) fn own_group_visibility(&mut self, ids: Vec<Uuid>, hidden: bool) {
        self.own_visibility(ids.into_iter().map(Target::Group).collect(), hidden);
    }
    fn own_visibility(&mut self, ids: Vec<Target>, hidden: bool) {
        let live = self.all_object_flags();
        let targets = ids
            .into_iter()
            .filter_map(|id| {
                live.get(&id)
                    .or_else(|| self.visibility_owners.get(&id))
                    .map(|&before| (id, before))
            })
            .collect::<Vec<_>>();
        for &(id, _) in &targets {
            self.visibility_owners.insert(id, hidden);
        }
        if let Some(before) = self.undo_stack.last_mut() {
            before.history_group_visibility = Some(GroupVisibilityCommand {
                targets,
                hidden,
                undo: true,
                retain_mesh_flags: true,
                snapshot_after: None,
            });
        }
    }
    fn group_visibility_target(&self, id: Uuid) -> Option<Target> {
        fn find(groups: &[Group], id: Uuid) -> Option<Target> {
            groups.iter().find_map(|g| {
                if g.id == id {
                    Some(Target::Group(g.history_id))
                } else {
                    find(&g.children, id)
                }
            })
        }
        find(&self.scene.groups, id).or_else(|| {
            self.component_edit_stack
                .iter()
                .find_map(|s| find(&s.original_scene.groups, id))
        })
    }
    fn selection_visibility(&self, item: &ingetrazo_core::scene::SelectionItem) -> Option<Target> {
        use ingetrazo_core::scene::SelectionItem;
        match item {
            SelectionItem::Group(i) => self
                .scene
                .groups
                .get(*i)
                .map(|g| Target::Group(g.history_id)),
            SelectionItem::Edge(i) => self
                .scene
                .mesh
                .edges
                .get(*i)
                .map(|e| Target::Edge(e.history_id)),
            SelectionItem::Face(i) => self
                .scene
                .mesh
                .faces
                .get(*i)
                .map(|f| Target::Face(f.history_id)),
            SelectionItem::ContextGroup { group, .. } => self.group_visibility_target(*group),
            SelectionItem::ContextFace { context, face, .. } => self
                .component_edit_stack
                .iter()
                .find(|s| s.original_scene.history_context == *context)
                .and_then(|s| s.original_scene.mesh.faces.get(*face))
                .map(|f| Target::Face(f.history_id)),
            _ => None,
        }
    }
    fn selected_visibility(&self) -> Vec<Target> {
        self.scene
            .selection
            .iter()
            .filter_map(|s| self.selection_visibility(s))
            .collect()
    }
    fn visibility_command(&mut self, targets: Vec<Target>, hidden: bool) {
        self.save_undo_state();
        self.own_visibility(targets.clone(), hidden);
        let values = targets.iter().map(|&id| (id, hidden)).collect();
        apply(&mut self.scene, &values, true);
        self.apply_context_visibility();
        if hidden {
            let targets: HashSet<_> = targets.into_iter().collect();
            let remove = self
                .scene
                .selection
                .iter()
                .filter(|item| {
                    self.selection_visibility(item)
                        .is_some_and(|id| targets.contains(&id))
                })
                .cloned()
                .collect::<Vec<_>>();
            for item in &remove {
                self.scene.selection.remove(item);
            }
            self.scene
                .selection_order
                .retain(|item| !remove.contains(item));
        }
        if let Some(before) = self.undo_stack.last_mut() {
            before.selection = self.scene.selection.clone();
            before.selection_order = self.scene.selection_order.clone();
        }
        self.scene.bump_version();
    }
    /// Raw HideCommand spelling, including already-hidden targets.
    pub fn hide_selection_command(&mut self) {
        self.visibility_command(self.selected_visibility(), true);
    }
    pub fn hide_selection(&mut self) {
        let live = self.all_object_flags();
        let targets = self
            .selected_visibility()
            .into_iter()
            .filter(|id| live.get(id) == Some(&false))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            self.status_message = ingetrazo_core::i18n::tr("Nothing visible selected");
        } else {
            let count = targets.len();
            self.visibility_command(targets, true);
            self.status_message = ingetrazo_core::i18n::tr("Hidden {n} entity/entities")
                .replace("{n}", &count.to_string());
        }
    }
    pub fn unhide_selected(&mut self) {
        let live = self.all_object_flags();
        let targets = self
            .selected_visibility()
            .into_iter()
            .filter(|id| live.get(id) == Some(&true))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            self.status_message = ingetrazo_core::i18n::tr("No hidden entity selected");
        } else {
            let count = targets.len();
            self.visibility_command(targets, false);
            self.status_message = ingetrazo_core::i18n::tr("Unhid {n} entity/entities")
                .replace("{n}", &count.to_string());
        }
    }
    pub fn unhide_last(&mut self) {
        self.sync_visibility_owners();
        let live = self.all_object_flags();
        let targets = self
            .undo_stack
            .iter()
            .rev()
            .filter_map(|s| s.history_group_visibility.as_ref())
            .filter(|c| c.hidden)
            .find_map(|c| {
                let targets = c
                    .targets
                    .iter()
                    .map(|&(id, _)| id)
                    .filter(|id| {
                        !matches!(id, Target::Face(_))
                            && live.get(id).or_else(|| self.visibility_owners.get(id))
                                == Some(&true)
                    })
                    .collect::<Vec<_>>();
                (!targets.is_empty()).then_some(targets)
            });
        if let Some(targets) = targets {
            let count = targets.len();
            self.visibility_command(targets, false);
            self.status_message = ingetrazo_core::i18n::tr("Unhid {n} entity/entities")
                .replace("{n}", &count.to_string());
        } else {
            self.status_message = ingetrazo_core::i18n::tr("Nothing to unhide");
        }
    }
    pub fn unhide_all(&mut self) {
        let targets = self
            .visibility_flags()
            .into_iter()
            .filter_map(|(id, hidden)| hidden.then_some(id))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            self.status_message = ingetrazo_core::i18n::tr("Nothing hidden");
        } else {
            self.visibility_command(targets, false);
            self.status_message = ingetrazo_core::i18n::tr("Unhid all objects");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::scene::SelectionItem;
    #[test]
    fn hide_filters_noops_and_removes_ordered_targets() {
        let mut a = IngeTrazoApp::new();
        a.scene.groups.push(Group::new("visible"));
        a.scene.groups.push(Group::new("hidden"));
        a.scene.groups[1].hidden = true;
        a.scene.select(SelectionItem::Group(0));
        a.scene.select(SelectionItem::Group(1));
        a.hide_selection();
        assert_eq!(a.undo_stack.len(), 1);
        assert_eq!(a.scene.selection_order, vec![SelectionItem::Group(1)]);
        a.hide_selection();
        assert_eq!(a.undo_stack.len(), 1);
        a.unhide_selected();
        assert!(!a.scene.groups[1].hidden);
        assert!(a.scene.groups[0].hidden);
    }
    #[test]
    fn actual_delete_compaction_can_recreate_a_distinct_loose_edge() {
        let mut a = IngeTrazoApp::new();
        a.add_edge_command(glam::DVec3::ZERO, glam::DVec3::X, None, None)
            .unwrap();
        a.add_edge_command(glam::DVec3::ZERO, glam::DVec3::Y, None, None)
            .unwrap();
        let old = a.scene.mesh.edges[0].history_id;
        let kept = a.scene.mesh.edges[1].history_id;
        a.scene.select(SelectionItem::Edge(0));
        a.delete_selection();
        a.add_edge_command(glam::DVec3::ZERO, glam::DVec3::X, None, None)
            .unwrap();
        assert_eq!(a.scene.mesh.edges[0].history_id, kept);
        assert_ne!(a.scene.mesh.edges[1].history_id, old);
    }
    #[test]
    fn snapshot_redo_preserves_literal_hidden_payload() {
        let mut a = IngeTrazoApp::new();
        let ids =
            [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y].map(|p| a.scene.mesh.add_vertex(p));
        a.scene.mesh.add_face(ids.to_vec(), vec![]);
        a.save_undo_state();
        let raw = serde_json::json!({"vendor":[1,2]});
        a.scene.mesh.faces[0]
            .attrs
            .insert("hidden".into(), raw.clone());
        a.seal_snapshot_visibility();
        a.scene.mesh.faces[0].attrs.remove("hidden");
        a.undo();
        assert!(!a.scene.mesh.faces[0].attrs.contains_key("hidden"));
        a.redo();
        assert_eq!(a.scene.mesh.faces[0].attrs.get("hidden"), Some(&raw));
    }
    #[test]
    fn detached_owners_survive_cancel_checkpoints_and_release_with_history() {
        let mut a = IngeTrazoApp::new();
        a.add_edge_command(glam::DVec3::ZERO, glam::DVec3::X, None, None)
            .unwrap();
        let target = Target::Edge(a.scene.mesh.edges[0].history_id);
        a.scene.select(SelectionItem::Edge(0));
        a.hide_selection();
        a.scene.select(SelectionItem::Edge(0));
        a.delete_selection();
        a.scene.groups.push(Group::new("context"));
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        assert_eq!(a.component_edit_stack.len(), 1);
        // A nested branch can remove current stacks without removing the
        // cancellation checkpoint's ownership of the earlier Hide command.
        a.undo_stack.clear();
        a.redo_stack.clear();
        a.sync_visibility_owners();
        assert_eq!(a.visibility_owners.get(&target), Some(&true));
        a.component_edit_stack.clear();
        a.sync_visibility_owners();
        assert!(a.visibility_owners.is_empty());
    }
}
