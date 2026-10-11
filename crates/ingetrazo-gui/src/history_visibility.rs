//! Source placement flags are owned by Hide commands, not geometry snapshots.
use crate::IngeTrazoApp;
use ingetrazo_core::{groups::Group, saved_views::GroupVisibilityCommand, scene::Scene};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
fn flags(groups: &[Group], out: &mut HashMap<Uuid, bool>) {
    for group in groups {
        out.insert(group.history_id, group.hidden);
        flags(&group.children, out);
    }
}
fn apply(groups: &mut [Group], values: &HashMap<Uuid, bool>) {
    for group in groups {
        if let Some(&hidden) = values.get(&group.history_id) {
            group.hidden = hidden;
        }
        apply(&mut group.children, values);
    }
}
pub(crate) fn restore(current: &Scene, target: &mut Scene) -> Option<GroupVisibilityCommand> {
    let mut live = HashMap::new();
    flags(&current.groups, &mut live);
    apply(&mut target.groups, &live);
    let command = target.history_group_visibility.take()?;
    let mut values = HashMap::with_capacity(command.targets.len());
    let mut previous = Vec::with_capacity(command.targets.len());
    for &(id, before) in &command.targets {
        values.insert(id, if command.undo { before } else { command.hidden });
        previous.push((id, live.get(&id).copied().unwrap_or(before)));
    }
    apply(&mut target.groups, &values);
    Some(GroupVisibilityCommand {
        targets: if command.undo {
            command.targets
        } else {
            previous
        },
        hidden: command.hidden,
        undo: !command.undo,
    })
}
impl IngeTrazoApp {
    pub(crate) fn own_group_visibility(&mut self, ids: Vec<Uuid>, hidden: bool) {
        let selected = ids.into_iter().collect::<HashSet<_>>();
        let mut live = HashMap::new();
        flags(&self.scene.groups, &mut live);
        let targets = live
            .into_iter()
            .filter(|(id, _)| selected.contains(id))
            .collect();
        if let Some(before) = self.undo_stack.last_mut() {
            before.history_group_visibility = Some(GroupVisibilityCommand {
                targets,
                hidden,
                undo: true,
            });
        }
    }
}
