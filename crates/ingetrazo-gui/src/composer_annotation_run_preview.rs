//! Retained literal run rubber-band path; no authored entities or history ownership.
use super::super::{annotation_canvas, text_canvas};
use crate::{composer_annotations::Draws, composer_backend::Factory};
use glam::DVec2;
use ingetrazo_formats::sheet_pdf::{Draw, PathOp, StrokeOptions};
use std::sync::Arc;
#[derive(Clone, Default)]
pub(super) struct Preview {
    pub visible: bool,
    elements: Vec<(bool, [f64; 2])>,
    draws: Option<Arc<Draws>>,
    canvas: annotation_canvas::Cache,
    preparations: u64,
}
impl Preview {
    pub(super) fn retire(&mut self) {
        self.visible = false;
    }
    pub(super) fn update(&mut self, elements: &[(bool, [f64; 2])]) {
        self.visible = true;
        if self.draws.is_some()
            && self.elements.len() == elements.len()
            && self
                .elements
                .iter()
                .zip(elements)
                .all(|(a, b)| a.0 == b.0 && a.1.map(f64::to_bits) == b.1.map(f64::to_bits))
        {
            return;
        }
        self.elements.clear();
        self.elements.extend_from_slice(elements);
        let path = elements
            .iter()
            .map(|(line, p)| {
                if *line {
                    PathOp::Line(*p)
                } else {
                    PathOp::Move(*p)
                }
            })
            .collect();
        self.draws = Some(Arc::new(Draws {
            draws: vec![Draw {
                picture: None,
                rotation_deg: 0.,
                origin_mm: [0., 0.],
                text: None,
                even_odd: false,
                path,
                stroke: Some(([58. / 255., 110. / 255., 165. / 255., 1.], 0.3)),
                stroke_options: Some(StrokeOptions {
                    cap: 2,
                    join: 2,
                    dash: vec![0.3 * 4., 0.3 * 2.],
                }),
                fill: None,
                clip: None,
                clip_intersection: None,
                flatten_transform: false,
            }],
            rectangles: vec![],
            source_lines: vec![],
            source_points: vec![],
            rectangle_fills:vec![],
        }));
        self.preparations = self.preparations.wrapping_add(1);
    }
    pub(super) fn paint(
        &mut self,
        painter: &egui::Painter,
        factory: Option<&Factory>,
        fonts: &mut text_canvas::CanvasPainter,
        zoom: f32,
        paper: egui::Pos2,
    ) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }
        let Some(draws) = self.draws.as_ref() else {
            return Ok(());
        };
        if self.elements.is_empty() {
            return Ok(());
        }
        let factory = factory.ok_or("Native run preview provider unavailable")?;
        self.canvas
            .paint(painter, factory, fonts, (0, 0), draws, zoom, paper)?;
        if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
            painter.ctx().data_mut(|d| {
                d.insert_temp(
                    egui::Id::new("run-preview-submitted-geometries"),
                    self.canvas.geometry_ids(),
                )
            });
        }
        Ok(())
    }
    pub(super) fn observation(&self) -> serde_json::Value {
        let mut value = serde_json::json!({"exists":self.visible,"elements":if self.visible {self.elements.iter().map(|(line,p)|serde_json::json!({"type":usize::from(*line),"x":p[0],"y":p[1]})).collect::<Vec<_>>()}else{vec![]},"preparations":self.preparations,"canvas":self.canvas.stats()});
        if self.visible {
            if let Some(draw) = self.draws.as_ref().and_then(|d| d.draws.first()) {
                let options = draw.stroke_options.as_ref().unwrap();
                let (ink, width) = draw.stroke.unwrap();
                value["visible"] = true.into();
                value["opacity"] = 1.0.into();
                value["z"] = 100000.0.into();
                value["pen"] = serde_json::json!({"rgba":ink.map(|v|(v*255.).round()as u8),"width":width,"style":if options.dash.is_empty(){1}else{2},"cap":match options.cap{0=>0,1=>32,_=>16},"join":match options.join{0=>0,1=>128,_=>64},"cosmetic":false,"dash_pattern":options.dash.iter().map(|v|v/width).collect::<Vec<_>>(),"dash_offset":0.});
                value["brush_style"] = 0.into();
                value["scene_transform"] = serde_json::json!([1., 0., 0., 0., 1., 0., 0., 0., 1.]);
            }
        }
        value
    }
}
/// Qt QPainterPath drops consecutive identical LineTo points, preserving moves.
struct Elements {
    values: [(bool, [f64; 2]); 6],
    len: usize,
}
impl Elements {
    fn new() -> Self {
        Self {
            values: [(false, [0., 0.]); 6],
            len: 0,
        }
    }
    fn push(&mut self, v: (bool, [f64; 2])) {
        self.values[self.len] = v;
        self.len += 1;
    }
    fn last(&self) -> Option<&(bool, [f64; 2])> {
        self.values[..self.len].last()
    }
}
fn line(out: &mut Elements, p: DVec2) {
    if out.last().is_none_or(|(_, q)| *q != p.to_array()) {
        out.push((true, p.to_array()));
    }
}
pub(super) fn update(app: &mut super::IngeTrazoApp, pos: DVec2) {
    let run = &app.composer_state.annotation_authoring.runs;
    if run.points.is_empty() || (run.sep.is_none() && run.points.len() > 2) {
        return;
    }
    let mut elements = Elements::new();
    let a = run.points[0].0;
    if run.sep.is_none() {
        elements.push((false, a.to_array()));
        for (p, _) in &run.points[1..] {
            line(&mut elements, *p);
        }
        if run.points.len() < 2 {
            line(&mut elements, pos);
        } else {
            let b = run.points[1].0;
            let sep = super::runs::separation(a, b, pos, run.axis);
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let length = (dx * dx + dy * dy).sqrt().max(1e-9);
            let n = DVec2::new(-dy / length, dx / length);
            elements.push((false, (a + n * sep).to_array()));
            line(&mut elements, b + n * sep);
        }
    } else {
        let last = if run.baseline {
            a
        } else {
            run.points.last().unwrap().0
        };
        let sep = if run.baseline {
            let step = app
                .scene
                .dimension_style()
                .get("base_step_mm")
                .and_then(serde_json::Value::as_f64)
                .filter(|v| *v > 0.1)
                .unwrap_or(8.)
                * run.segments.len() as f64;
            let sep = run.sep.unwrap();
            sep + if sep >= 0. { step } else { -step }
        } else {
            super::runs::segment_sep(run, last, pos)
        };
        let dx = pos.x - last.x;
        let dy = pos.y - last.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length > 1e-9 {
            let n = match run.axis {
                "h" => DVec2::new(0., 1.),
                "v" => DVec2::new(1., 0.),
                _ => DVec2::new(-dy / length, dx / length),
            };
            let aa = last + n * sep;
            let b = DVec2::new(
                if run.axis == "v" { aa.x } else { pos.x },
                if run.axis == "h" { aa.y } else { pos.y },
            );
            elements.push((false, aa.to_array()));
            line(&mut elements, b);
            elements.push((false, pos.to_array()));
            line(&mut elements, b);
        }
    }
    app.composer_state
        .annotation_authoring
        .run_preview
        .update(&elements.values[..elements.len]);
}
