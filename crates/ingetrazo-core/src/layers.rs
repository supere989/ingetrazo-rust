// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Layers / Tags system.

use serde::{Deserialize, Serialize};

pub const DEFAULT_LAYER: &str = "Layer 0";

/// Original entity_layer uses `label or DEFAULT_LAYER`; registry names remain literal.
pub fn entity_layer_name(label: Option<&str>) -> &str {
    label.filter(|name| !name.is_empty()).unwrap_or(DEFAULT_LAYER)
}

#[cfg(test)]
mod entity_label_tests {
    use super::*;
    #[test]
    fn empty_entity_labels_use_default_without_renaming_authored_registry() {
        let mut scene = crate::scene::Scene::new();
        scene.layers[0].visible = false;
        scene.layers.insert(0, Layer::new(""));
        scene.layers.push(Layer::new("Layer0"));
        scene.layers.push(Layer::new(" "));
        assert!(!scene.is_layer_visible(None));
        assert!(!scene.is_layer_visible(Some("")));
        assert!(scene.is_layer_visible(Some("Layer0")));
        assert!(scene.is_layer_visible(Some(" ")));
        assert_eq!(scene.layers[0].name, "");
        assert_eq!(entity_layer_name(Some(" ")), " ");
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    #[serde(skip, default = "uuid::Uuid::new_v4")]
    pub history_id: uuid::Uuid,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub color: Option<[f32; 3]>,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            history_id: uuid::Uuid::new_v4(),
            name: name.into(),
            visible: true,
            locked: false,
            color: None,
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new(DEFAULT_LAYER)
    }
}

impl PartialEq for Layer {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name
            && self.visible == o.visible
            && self.locked == o.locked
            && self.color == o.color
    }
}
/// Runtime command-owned registry membership. Panel edits are not model commands.
#[derive(Debug, Clone, Default)]
pub struct LayerHistory(pub Option<LayerDelta>);
impl PartialEq for LayerHistory {
    fn eq(&self, o: &Self) -> bool {
        match (&self.0, &o.0) {
            (Some(a), Some(b)) => a == b,
            (Some(a), None) | (None, Some(a)) => {
                a.added.is_empty() && a.removed.is_empty() && a.assignment.is_none()
            }
            (None, None) => true,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerDelta {
    pub added: Vec<Layer>,
    pub removed: Vec<(usize, Layer)>,
    pub assignment: Option<(String, bool, Vec<crate::scene::SelectionItem>)>,
}
impl LayerDelta {
    pub fn between(before: &[Layer], after: &[Layer]) -> Self {
        let b: std::collections::HashSet<_> = before.iter().map(|l| l.history_id).collect();
        let a: std::collections::HashSet<_> = after.iter().map(|l| l.history_id).collect();
        Self {
            assignment: None,
            added: after
                .iter()
                .filter(|l| !b.contains(&l.history_id))
                .cloned()
                .collect(),
            removed: before
                .iter()
                .enumerate()
                .filter(|(_, l)| !a.contains(&l.history_id))
                .map(|(i, l)| (i, l.clone()))
                .collect(),
        }
    }
    pub fn restore(&self, current: &[Layer]) -> Vec<Layer> {
        if let Some((name, ensure, _)) = &self.assignment {
            let mut out = current.to_vec();
            if *ensure {
                if !out.iter().any(|l| l.name == *name) {
                    out.push(Layer::new(name));
                }
            } else {
                out.retain(|l| l.name != *name);
            }
            return out;
        }
        let ids: std::collections::HashSet<_> = self.added.iter().map(|l| l.history_id).collect();
        let mut out: Vec<_> = current
            .iter()
            .filter(|l| !ids.contains(&l.history_id))
            .cloned()
            .collect();
        for (i, l) in &self.removed {
            if !out.iter().any(|x| x.history_id == l.history_id) {
                out.insert((*i).min(out.len()), l.clone());
            }
        }
        out
    }
    pub fn inverse(&self) -> Self {
        Self {
            assignment: self
                .assignment
                .as_ref()
                .map(|(name, ensure, entities)| (name.clone(), !*ensure, entities.clone())),
            added: self.removed.iter().map(|(_, l)| l.clone()).collect(),
            removed: self
                .added
                .iter()
                .enumerate()
                .map(|(i, l)| (usize::MAX - i, l.clone()))
                .collect(),
        }
    }
    pub fn refresh(&mut self, layer: &Layer) {
        for l in self
            .added
            .iter_mut()
            .chain(self.removed.iter_mut().map(|(_, l)| l))
        {
            if l.history_id == layer.history_id {
                *l = layer.clone();
            }
        }
    }
}
