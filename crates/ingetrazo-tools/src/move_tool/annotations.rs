//! Source-owned annotation poses: labels move their offset, dimensions their
//! unanchored ends or perpendicular dimension line. Guides are not Move targets.
use glam::DVec3;
use ingetrazo_core::{
    dimensions::Dimension,
    scene::{Scene, SelectionItem},
    textlabel::TextLabel,
};
#[derive(Debug, Default, Clone)]
pub(super) struct Annotations {
    labels: Vec<(usize, TextLabel)>,
    dimensions: Vec<(usize, Dimension, bool)>,
}
fn add(a: DVec3, b: DVec3) -> DVec3 {
    (a.as_vec3() + b.as_vec3()).as_dvec3()
}
fn shift(d: &mut Dimension, line: bool, step: DVec3) {
    if line {
        let ab = d.end.as_vec3() - d.start.as_vec3();
        let length = ab.length();
        let mut step = step.as_vec3();
        if length > 1e-9 {
            let direction = ab / length;
            step -= direction * step.dot(direction);
        }
        d.offset = add(d.offset, step.as_dvec3());
    } else {
        if d.anchor_a.is_none() {
            d.start = add(d.start, step);
        }
        if d.anchor_b.is_none() {
            d.end = add(d.end, step);
        }
    }
}
impl Annotations {
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty() && self.dimensions.is_empty()
    }
    pub fn gather(
        &mut self,
        scene: &Scene,
        vertices: &[usize],
        groups: &[usize],
        hits: crate::base::AnnotationHits,
    ) -> Result<(), String> {
        *self = Self::default();
        let mut picked = Vec::new();
        if scene.selection.is_empty() {
            if let Some(token)=hits.label {
                let id=scene.text_labels.iter().position(|t|t.history_id==token).ok_or("Stale text hit")?;
                picked.push(SelectionItem::TextLabel(id));
            }
            if let Some(token)=hits.dimension {
                let id=scene.dimensions.iter().position(|d|d.id==token).ok_or("Stale dimension hit")?;
                picked.push(SelectionItem::Dimension(id));
            }
        }
        let items:Vec<_>=if scene.selection.is_empty(){picked}else{scene.selection.iter().cloned().collect()};
        for item in &items {
            match item {
                SelectionItem::TextLabel(i) => {
                    let t = scene.text_labels.get(*i).ok_or("Stale text selection")?;
                    if !scene.is_layer_selectable(t.layer.as_deref()) {
                        return Err("Text label is hidden or locked".into());
                    }
                    self.labels.push((*i, t.clone()));
                }
                SelectionItem::Dimension(i) => {
                    let d = scene
                        .dimensions
                        .get(*i)
                        .ok_or("Stale dimension selection")?;
                    if !scene.is_layer_selectable(d.layer.as_deref()) {
                        return Err("Dimension is hidden or locked".into());
                    }
                    let riding = |a: &ingetrazo_core::dimension_anchors::VertexAnchor| {
                        vertices.iter().any(|&id| {
                            scene
                                .mesh
                                .vertices
                                .get(id)
                                .is_some_and(|v| v.uid == a.vertex)
                        }) || groups
                            .iter()
                            .any(|&id| scene.groups.get(id).is_some_and(|g| a.path.contains(&g.id)))
                    };
                    if d.anchor_a.as_ref().is_some_and(riding)
                        && d.anchor_b.as_ref().is_some_and(riding)
                    {
                        continue;
                    }
                    self.dimensions.push((
                        *i,
                        d.clone(),
                        d.anchor_a.is_some() && d.anchor_b.is_some(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn label_index(scene: &Scene, i: usize, t: &TextLabel) -> Option<usize> {
        if scene
            .text_labels
            .get(i)
            .is_some_and(|x| x.history_id == t.history_id)
        {
            Some(i)
        } else {
            scene
                .text_labels
                .iter()
                .position(|x| x.history_id == t.history_id)
        }
    }
    fn dimension_index(scene: &Scene, i: usize, d: &Dimension) -> Option<usize> {
        if scene.dimensions.get(i).is_some_and(|x| x.id == d.id) {
            Some(i)
        } else {
            scene.dimensions.iter().position(|x| x.id == d.id)
        }
    }
    pub fn step(&mut self, scene: &mut Scene, step: DVec3) {
        for (i, t) in &mut self.labels {
            let index = Self::label_index(scene, *i, t);
            let current = index
                .map(|j| scene.text_labels[j].offset)
                .unwrap_or(t.offset);
            t.offset = add(current, step);
            if let Some(j) = index {
                *i = j;
                scene.text_labels[j].offset = t.offset;
            }
        }
        for (i, d, line) in &mut self.dimensions {
            let index = Self::dimension_index(scene, *i, d);
            if let Some(j) = index {
                *i = j;
                (d.start,d.end)=scene.dimension_points(&scene.dimensions[j]);
                d.offset = scene.dimensions[j].offset;
            }
            shift(d, *line, step);
            if let Some(j) = index {
                let live = &mut scene.dimensions[j];
                live.start = d.start;
                live.end = d.end;
                live.offset = d.offset;
            }
        }
    }
    pub fn restore_current(&self, scene: &mut Scene) {
        for (i, t) in &self.labels {
            if let Some(j) = Self::label_index(scene, *i, t) {
                scene.text_labels[j].offset = t.offset;
            }
        }
        for (i, d, _) in &self.dimensions {
            if let Some(j) = Self::dimension_index(scene, *i, d) {
                scene.dimensions[j].start = d.start;
                scene.dimensions[j].end = d.end;
                scene.dimensions[j].offset = d.offset;
            }
        }
    }
    pub fn restore_baseline(&self, scene: &mut Scene, delta: DVec3) {
        let mut copy = self.clone();
        copy.step(scene, -delta);
    }
}
