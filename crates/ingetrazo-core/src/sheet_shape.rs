//! Original composer shapes in physical millimetres. Backend-independent paint commands.
use glam::DVec2;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetShape {
    pub kind: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub w_mm: f64,
    pub h_mm: f64,
    pub stroke_mm: f64,
    pub fill: bool,
    pub invert: bool,
    pub radius_mm: f64,
    pub sides: i32,
    pub color: String,
    pub fill_color: String,
    pub ground: String,
    pub tick_mm: f64,
    pub tick_step_mm: f64,
    pub band_mm: f64,
    pub z: f64,
    pub locked: bool,
    pub hidden: bool,
    pub group_id: String,
    pub list_name: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
impl Default for SheetShape {
    fn default() -> Self {
        Self {
            kind: "rect".into(),
            x_mm: 20.,
            y_mm: 20.,
            w_mm: 40.,
            h_mm: 25.,
            stroke_mm: 0.35,
            fill: false,
            invert: false,
            radius_mm: 0.,
            sides: 6,
            color: "#1e242c".into(),
            fill_color: "#e2e8ee".into(),
            ground: "ticks".into(),
            tick_mm: 2.5,
            tick_step_mm: 3.,
            band_mm: 6.,
            z: 0.,
            locked: false,
            hidden: false,
            group_id: String::new(),
            list_name: String::new(),
            extra: Default::default(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum ShapePath {
    RoundedRect { size: DVec2, radius: f64 },
    Ellipse { size: DVec2 },
    Polygon(Vec<DVec2>),
    Line(DVec2, DVec2),
}
#[derive(Debug, Clone, PartialEq)]
pub struct ShapePaint {
    pub path: ShapePath,
    pub stroke: Option<(String, f64)>,
    pub fill: Option<(String, f64)>,
    pub clip: Option<std::sync::Arc<[DVec2]>>,
}
impl SheetShape {
    /// Local item coordinates; consumers translate once by x_mm/y_mm.
    /// All strokes require round caps, matching the original painter.
    pub fn paint_commands(&self, command_budget: usize) -> Result<Vec<ShapePaint>, String> {
        if ![
            self.x_mm,
            self.y_mm,
            self.w_mm,
            self.h_mm,
            self.stroke_mm,
            self.radius_mm,
            self.tick_step_mm,
            self.tick_mm,
            self.band_mm,
        ]
        .iter()
        .all(|x| x.is_finite())
        {
            return Err("Non-finite sheet shape geometry".into());
        }
        let extent = self.w_mm.hypot(self.h_mm);
        let estimated = if self.kind == "terreno" && self.ground != "band" {
            (extent + 2. * self.band_mm.max(0.5)) / self.tick_step_mm.max(0.5) + 3.
        } else {
            3.
        };
        if !estimated.is_finite() || estimated > command_budget as f64 {
            return Err("Sheet shape exceeds paint-command budget".into());
        }
        if self.hidden {
            return Ok(Vec::new());
        }
        let size = DVec2::new(self.w_mm, self.h_mm);
        let stroke = Some((self.color.clone(), self.stroke_mm));
        let fill = if self.fill {
            Some((self.fill_color.clone(), 1.))
        } else {
            None
        };
        let command = |path, fill| ShapePaint {
            path,
            stroke: stroke.clone(),
            fill,
            clip: None,
        };
        match self.kind.as_str() {
            "rect" => {
                let radius = self.radius_mm.min(self.w_mm / 2.).min(self.h_mm / 2.);
                return Ok(vec![command(
                    ShapePath::RoundedRect {
                        size,
                        radius: if radius > 0.01 { radius } else { 0. },
                    },
                    fill,
                )]);
            }
            "elipse" => return Ok(vec![command(ShapePath::Ellipse { size }, fill)]),
            "poligono" => {
                let n = self.sides.clamp(3, 24);
                let center = size / 2.;
                return Ok(vec![command(
                    ShapePath::Polygon(
                        (0..n)
                            .map(|i| {
                                let angle = std::f64::consts::TAU * f64::from(i) / f64::from(n);
                                center + DVec2::new(center.x * angle.sin(), -center.y * angle.cos())
                            })
                            .collect(),
                    ),
                    fill,
                )]);
            }
            _ => {}
        }
        let a = DVec2::new(0., if self.invert { self.h_mm } else { 0. });
        let b = DVec2::new(self.w_mm, if self.invert { 0. } else { self.h_mm });
        let mut commands = Vec::new();
        if self.kind == "terreno" {
            let delta = b - a;
            let length = delta.length();
            if length >= 1e-6 {
                let u = delta / length;
                let mut n = DVec2::new(-u.y, u.x);
                if n.y < 0. {
                    n = -n;
                }
                let step = self.tick_step_mm.max(0.5);
                let tick = self.tick_mm.max(0.2);
                let depth = self.band_mm.max(0.5);
                let band: std::sync::Arc<[DVec2]> = [a, b, b + n * depth, a + n * depth].into();
                let ground_stroke = Some((self.color.clone(), (self.stroke_mm * 0.6).max(0.1)));
                match self.ground.as_str() {
                    "band" => commands.push(ShapePaint {
                        path: ShapePath::Polygon(band.to_vec()),
                        stroke: None,
                        fill: Some((self.fill_color.clone(), f64::from(0.6_f32))),
                        clip: None,
                    }),
                    "hatch" => {
                        let mut t = -depth;
                        while t < length + depth {
                            let start = a + u * t;
                            commands.push(ShapePaint {
                                path: ShapePath::Line(start, start + (n - u) * depth),
                                stroke: ground_stroke.clone(),
                                fill: None,
                                clip: Some(band.clone()),
                            });
                            t += step;
                        }
                        commands.push(ShapePaint {
                            path: ShapePath::Line(a + n * depth, b + n * depth),
                            stroke: ground_stroke,
                            fill: None,
                            clip: None,
                        });
                    }
                    _ => {
                        let mut t = step / 2.;
                        while t <= length {
                            let start = a + u * t;
                            commands.push(ShapePaint {
                                path: ShapePath::Line(
                                    start,
                                    start + (n - u) * (tick / std::f64::consts::SQRT_2),
                                ),
                                stroke: ground_stroke.clone(),
                                fill: None,
                                clip: None,
                            });
                            t += step;
                        }
                    }
                }
            }
        }
        commands.push(command(ShapePath::Line(a, b), None));
        if self.kind == "flecha" {
            let angle = (b.y - a.y).atan2(b.x - a.x);
            let length = (self.stroke_mm * 7.).max(2.5);
            for offset in [153_f64.to_radians(), -153_f64.to_radians()] {
                commands.push(command(
                    ShapePath::Line(
                        b,
                        b + DVec2::new((angle + offset).cos(), (angle + offset).sin()) * length,
                    ),
                    None,
                ));
            }
        }
        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terrain_hangs_down_for_both_diagonals_and_keeps_clip() {
        for invert in [false, true] {
            let shape = SheetShape {
                kind: "terreno".into(),
                ground: "hatch".into(),
                invert,
                ..Default::default()
            };
            let commands = shape.paint_commands(1000).unwrap();
            let clip = commands[0].clip.as_ref().unwrap();
            assert!(clip[2].y >= clip[1].y && clip[3].y >= clip[0].y);
            assert!(commands[..commands.len() - 2]
                .iter()
                .all(|c| c.clip.as_ref() == Some(clip)));
            assert!(commands[..commands.len() - 2]
                .iter()
                .all(|c| std::sync::Arc::ptr_eq(c.clip.as_ref().unwrap(), clip)));
            assert!(commands.last().unwrap().clip.is_none());
        }
    }
    #[test]
    fn hidden_and_invalid_shapes_do_not_create_unbounded_work() {
        assert!(SheetShape {
            hidden: true,
            ..Default::default()
        }
        .paint_commands(100)
        .unwrap()
        .is_empty());
        assert!(SheetShape {
            w_mm: f64::INFINITY,
            ..Default::default()
        }
        .paint_commands(100)
        .is_err());
        assert!(SheetShape {
            kind: "terreno".into(),
            w_mm: 1e20,
            ..Default::default()
        }
        .paint_commands(100)
        .is_err());
    }
    #[test]
    fn original_schema_preserves_unknown_fields_and_polygon_limits() {
        let shape: SheetShape = serde_json::from_value(
            serde_json::json!({"kind":"poligono","sides":100,"fill":true,"vendor":{"uid":71}}),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&shape).unwrap()["vendor"]["uid"], 71);
        let commands = shape.paint_commands(100).unwrap();
        let ShapePath::Polygon(points) = &commands[0].path else {
            panic!()
        };
        assert_eq!(points.len(), 24);
        assert_eq!(points[0], DVec2::new(20., 0.));
        assert!(commands[0].fill.is_some());
    }
}
