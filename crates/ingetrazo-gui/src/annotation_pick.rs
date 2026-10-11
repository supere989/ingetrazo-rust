//! Source Move screen acquisition: glyph blocks and dimension lines, never ray proximity.
use crate::annotation_overlay_geometry::{Cache, Limits};
use glam::{DVec2, DVec3};
use ingetrazo_tools::base::AnnotationHits;
#[derive(PartialEq)]
struct Key {
    revision: u64,
    version: u64,
    matrix: [u64; 16],
    extent: [u32; 2],
}
#[derive(Default)]
pub(crate) struct Picks {
    key: Option<Key>,
    font: Option<Cache>,
    labels: Vec<(u64, [f64; 4])>,
    dimensions: Vec<(uuid::Uuid, DVec2, DVec2)>,
    pub builds: u64,
}
impl Picks {
    pub(crate) fn update(
        &mut self,
        scene: &ingetrazo_core::scene::Scene,
        revision: u64,
        camera: &ingetrazo_core::camera::OrbitCamera,
        extent: [f32; 2],
    ) -> Result<(), String> {
        let columns = crate::navigation_ray::camera_snapshot(camera)
            .ok_or("Nonfinite annotation camera")?
            .matrix;
        let matrix: [f32; 16] = std::array::from_fn(|i| columns[i / 4][i % 4]);
        let key = Key {
            revision,
            version: scene.version,
            matrix: matrix.map(|x| u64::from(x.to_bits())),
            extent: extent.map(f32::to_bits),
        };
        if self.key.as_ref() == Some(&key) {
            return Ok(());
        }
        let project = |p: DVec3| {
            let p = p.as_vec3();
            let q: [f32; 4] = std::array::from_fn(|row| {
                matrix[row] * p.x + matrix[4 + row] * p.y + matrix[8 + row] * p.z + matrix[12 + row]
            });
            if !q.iter().all(|x| x.is_finite()) || q[3] <= 0. {
                return None;
            }
            Some(DVec2::new(
                (q[0] as f64 / q[3] as f64 + 1.) * 0.5 * f64::from(extent[0]),
                (1. - q[1] as f64 / q[3] as f64) * 0.5 * f64::from(extent[1]),
            ))
        };
        let mut labels = Vec::new();
        let mut dimensions = Vec::new();
        if self.font.is_none() {
            self.font = Some(Cache::new(
                ingetrazo_formats::annotation_font::Limits {
                    faces: 16,
                    retained_font_bytes: 32 * 1024 * 1024,
                },
                Limits {
                    cells: 1,
                    spans: 1,
                    geometry_bytes: 1,
                    glyphs: 1,
                    glyph_bytes: 1,
                },
            )?);
        }
        let font = self.font.as_mut().unwrap();
        let size = i32::try_from(
            scene.native_document["dimension_style"]["font_size"]
                .as_i64()
                .unwrap_or(9),
        )
        .map_err(|_| "Annotation font size overflow")?;
        for t in &scene.text_labels {
            if !scene.is_layer_selectable(t.layer.as_deref()) {
                continue;
            }
            let Some(p) = project(t.anchor + t.offset) else {
                continue;
            };
            let anchor = project(t.anchor);
            let mut lines = ingetrazo_render::annotations::splitlines(&t.text, 200_000)?;
            if lines.is_empty() {
                lines.push("");
            }
            let metrics = lines
                .iter()
                .map(|line| font.leader_pick_metrics(line, size, 96.))
                .collect::<Result<Vec<_>, _>>()?;
            let width = metrics.iter().map(|m| m[3]).fold(0., f64::max);
            let x = if anchor.is_some_and(|a| p.x < a.x) {
                p.x - 6. - width
            } else {
                p.x + 6.
            };
            for (i, m) in metrics.iter().enumerate() {
                let y = p.y - 4. + i as f64 * m[2];
                labels.push((
                    t.history_id,
                    [x - 3., y - m[0] - 3., x + m[3] + 3., y + m[1] + 3.],
                ));
            }
        }
        for d in &scene.dimensions {
            if !scene.is_layer_selectable(d.layer.as_deref())
                || d.native_document["hidden"].as_bool().unwrap_or(false)
            {
                continue;
            }
            let (a, b) = scene.dimension_points(d);
            let frame = scene.dimension_frame;
            let a = frame.transform_point3(a).as_vec3();
            let b = frame.transform_point3(b).as_vec3();
            let (ap, bp) = ingetrazo_render::annotations::dimension_line_points(
                ingetrazo_render::annotations::DimensionInput {
                    a,
                    b,
                    offset: frame.transform_vector3(d.offset).as_vec3(),
                    axis: d.axis.as_deref(),
                    text: None,
                    highlighted: false,
                    hidden: false,
                },
            );
            for (a, b) in [(a, ap), (b, bp), (ap, bp)] {
                if let (Some(a), Some(b)) = (project(a.as_dvec3()), project(b.as_dvec3())) {
                    dimensions.push((d.id, a, b));
                }
            }
        }
        self.labels = labels;
        self.dimensions = dimensions;
        self.key = Some(key);
        self.builds += 1;
        Ok(())
    }
    pub(crate) fn hit(&self, p: DVec2) -> AnnotationHits {
        let label = self
            .labels
            .iter()
            .find(|(_, r)| r[0] <= p.x && p.x <= r[2] && r[1] <= p.y && p.y <= r[3])
            .map(|(id, _)| *id);
        let mut best = 8.;
        let mut dimension = None;
        for (id, a, b) in &self.dimensions {
            let ab = *b - *a;
            let n = ab.length_squared();
            let t = if n == 0. {
                0.
            } else {
                ((p - *a).dot(ab) / n).clamp(0., 1.)
            };
            let d = (p - (*a + ab * t)).length();
            if d < best {
                best = d;
                dimension = Some(*id);
            }
        }
        AnnotationHits { label, dimension }
    }
}

impl crate::IngeTrazoApp {
    /// Same source-screen acquisition route used by the production viewport.
    pub fn update_move_annotation_hits(
        &mut self,
        extent: [f32; 2],
        cursor: DVec2,
    ) -> Result<AnnotationHits, String> {
        if self.active_tool_type != ingetrazo_tools::base::ToolType::Move
            || !self.scene.selection.is_empty()
            || !self.active_tool.wants_annotation_hits()
        {
            self.active_tool.note_annotation_hits(Default::default());
            return Ok(Default::default());
        }
        if let Err(error)=self.annotation_pick_cache.update(&self.scene,self.model_revision,&self.camera,extent){self.active_tool.note_annotation_pick_error(error.clone());return Err(error);}
        let hits = self.annotation_pick_cache.hit(cursor);
        self.active_tool.note_annotation_hits(hits);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_strict_threshold_and_first_label_are_source_ordered() {
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        let p = Picks {
            labels: vec![(7, [0., 0., 10., 10.]), (8, [0., 0., 10., 10.])],
            dimensions: vec![
                (a, DVec2::ZERO, DVec2::X * 10.),
                (b, DVec2::ZERO, DVec2::X * 10.),
            ],
            ..Default::default()
        };
        assert_eq!(
            p.hit(DVec2::new(5., 5.)),
            AnnotationHits {
                label: Some(7),
                dimension: Some(a)
            }
        );
        assert_eq!(p.hit(DVec2::new(5., 8.)).dimension, None);
    }
    #[test]
    fn warm_pick_reuses_metrics_and_hidden_locked_dimensions_do_not_acquire() {
        let mut app = crate::IngeTrazoApp::new();
        app.system_clipboard = None;
        let mut dim = ingetrazo_core::dimensions::Dimension::new(DVec3::ZERO, DVec3::X, DVec3::Y);
        dim.layer = Some("named".into());
        app.scene.dimensions.push(dim);
        app.scene
            .layers
            .push(ingetrazo_core::layers::Layer::new("named"));
        let mut picks = Picks::default();
        picks
            .update(&app.scene, 0, &app.camera, [800., 600.])
            .unwrap();
        assert_eq!(picks.dimensions.len(), 3);
        let builds = picks.builds;
        for _ in 0..100 {
            picks
                .update(&app.scene, 0, &app.camera, [800., 600.])
                .unwrap();
        }
        assert_eq!(picks.builds, builds);
        app.scene.layers.last_mut().unwrap().locked = true;
        picks
            .update(&app.scene, 1, &app.camera, [800., 600.])
            .unwrap();
        assert!(picks.dimensions.is_empty());
        app.scene.layers.last_mut().unwrap().locked = false;
        app.scene.layers.last_mut().unwrap().visible = false;
        picks
            .update(&app.scene, 2, &app.camera, [800., 600.])
            .unwrap();
        assert!(picks.dimensions.is_empty());
    }
    #[test]
    fn empty_annotation_hit_owns_geometry_without_selection_or_history_until_commit() {
        use ingetrazo_core::math::Ray;
        use ingetrazo_tools::base::{ToolContext, ToolType};
        let mut app = crate::IngeTrazoApp::new();
        app.system_clipboard = None;
        let v = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| app.scene.mesh.add_vertex(p));
        app.scene.mesh.add_face(v.to_vec(), vec![]);
        let label = ingetrazo_core::textlabel::TextLabel::new(DVec3::ZERO, DVec3::X, "Text");
        let token = label.history_id;
        app.scene.text_labels.push(label);
        app.set_tool(ToolType::Move);
        app.active_tool.note_annotation_hits(AnnotationHits {
            label: Some(token),
            dimension: None,
        });
        let before = app.scene.mesh.clone();
        let ctx = |p| ToolContext::new(p, Ray::new(DVec3::new(0.2, 0.2, 10.), -DVec3::Z));
        app.dispatch_tool_click(&ctx(DVec3::ZERO));
        assert!(app.scene.selection.is_empty());
        assert!(app.undo_stack.is_empty());
        app.dispatch_tool_move(&ctx(DVec3::Y));
        assert_eq!(app.scene.mesh, before);
        assert_eq!(app.scene.text_labels[0].offset, DVec3::X + DVec3::Y);
        app.dispatch_tool_click(&ctx(DVec3::Y));
        assert_eq!(app.undo_stack.len(), 1);
        app.undo();
        assert_eq!(app.scene.text_labels[0].offset, DVec3::X);
        app.redo();
        assert_eq!(app.scene.text_labels[0].offset, DVec3::X + DVec3::Y);
    }
}
