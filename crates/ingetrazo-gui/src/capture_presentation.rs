//! Borrowed presentation-only viewport collector. No input/tool dispatch or mesh copy.
use egui::{Color32, Pos2, Rect, Shape, Stroke};
use glam::DVec3;
use ingetrazo_core::scene::SelectionItem;

pub(crate) struct Presentation {
    pub shapes: Vec<egui::epaint::ClippedShape>,
    pub text: Vec<crate::annotation_overlay_geometry::PresentationText>,
}
pub(crate) struct Collector {
    text: Vec<crate::annotation_overlay_geometry::PresentationText>,
    rect: Rect,
    matrix: [[f32; 4]; 4],
    shapes: Vec<egui::epaint::ClippedShape>,
    text_bytes: usize,
}
impl Collector {
    fn add(&mut self, shape: Shape) -> Result<(), String> {
        if self.shapes.len() >= 200_000 {
            return Err("Capture presentation operation limit exceeded".into());
        }
        self.shapes
            .try_reserve(1)
            .map_err(|_| "Capture presentation allocation refused")?;
        self.shapes.push(egui::epaint::ClippedShape {
            clip_rect: self.rect,
            shape,
        });
        Ok(())
    }
    fn project(&self, p: DVec3) -> Option<Pos2> {
        let p = p.as_vec3();
        let q = std::array::from_fn::<_, 4, _>(|row| {
            self.matrix[0][row] * p.x
                + self.matrix[1][row] * p.y
                + self.matrix[2][row] * p.z
                + self.matrix[3][row]
        });
        if !q.iter().all(|v| v.is_finite()) || q[3] <= 0. {
            return None;
        }
        let nx = f64::from(q[0]) / f64::from(q[3]);
        let ny = f64::from(q[1]) / f64::from(q[3]);
        Some(
            self.rect.min
                + egui::vec2(
                    ((nx * 0.5 + 0.5) * f64::from(self.rect.width())) as f32,
                    ((1. - (ny * 0.5 + 0.5)) * f64::from(self.rect.height())) as f32,
                ),
        )
    }
    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32) -> Result<(), String> {
        self.add(Shape::line_segment([a, b], Stroke::new(width, color)))
    }
    fn dashed(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32) -> Result<(), String> {
        let delta = b - a;
        let length = delta.length();
        if !length.is_finite() {
            return Err("Capture dashed line overflow".into());
        }
        if length == 0. {
            return Ok(());
        }
        let direction = delta / length;
        let step = width * 6.;
        let dash = width * 4.;
        if step <= 0. || length / step > 200_000. {
            return Err("Capture dashed line admission refused".into());
        }
        let mut p = 0.;
        while p < length {
            self.line(
                a + direction * p,
                a + direction * (p + dash).min(length),
                width,
                color,
            )?;
            p += step;
        }
        Ok(())
    }
    fn text(
        &mut self,
        p: Pos2,
        text: &str,
        size: f32,
        color: Color32,
        center: bool,
    ) -> Result<(), String> {
        let total = self
            .text_bytes
            .checked_add(text.len())
            .ok_or("Capture text byte overflow")?;
        if total > 64 * 1024 {
            return Err("Capture presentation text limit exceeded".into());
        }
        self.text_bytes = total;
        self.text
            .try_reserve(1)
            .map_err(|_| "Capture text allocation refused")?;
        self.text
            .push(crate::annotation_overlay_geometry::PresentationText {
                position: [f64::from(p.x), f64::from(p.y)],
                text: text.into(),
                size_pt: f64::from(size),
                bold: center,
                center,
                color: color.to_array(),
            });
        Ok(())
    }
}

pub(crate) fn collect(
    app: &crate::IngeTrazoApp,
    _ctx: &egui::Context,
    logical_size: [f32; 2],
) -> Result<Presentation, String> {
    if !logical_size.iter().all(|v| v.is_finite() && *v > 0.) {
        return Err("Capture logical extent refused".into());
    }
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(logical_size[0], logical_size[1]));
    let mut out = Collector {
        text: Vec::new(),
        rect,
        matrix: crate::navigation_ray::camera_snapshot(&app.camera)
            .ok_or("Capture source camera outside admission")?
            .matrix,
        shapes: Vec::new(),
        text_bytes: 0,
    };
    let scene = &app.scene;
    // Line guides belong to the world/depth pass; only point crosses are overlays.
    for (index, guide) in scene.guides.iter().enumerate() {
        let ingetrazo_core::guides::GuideType::Point(p) = guide.kind else {
            continue;
        };
        if let Some(p) = out.project(p) {
            let selected = scene.is_selected(&SelectionItem::Guide(index));
            let color = if selected {
                Color32::from_rgb(243, 115, 41)
            } else {
                Color32::from_rgb(70, 90, 120)
            };
            let width = if selected { 2. } else { 1. };
            out.line(p - egui::vec2(5., 0.), p + egui::vec2(5., 0.), width, color)?;
            out.line(p - egui::vec2(0., 5.), p + egui::vec2(0., 5.), width, color)?;
        }
    }
    if scene.show_section_planes {
        for (index, plane) in scene.section_planes.iter().enumerate() {
            let Some(corners) = scene.section_frame(index) else {
                continue;
            };
            let projected = corners.map(|p| out.project(p));
            let [Some(a), Some(b), Some(c), Some(d)] = projected else {
                continue;
            };
            let points = [a, b, c, d];
            let selected = scene.is_selected(&SelectionItem::SectionPlane(index));
            let color = if plane.active {
                Color32::from_rgb(243, 115, 41)
            } else {
                Color32::from_rgb(178, 182, 188)
            };
            let width = if plane.active { 2. } else { 1.4 } + if selected { 1.2 } else { 0. };
            for i in 0..4 {
                if selected {
                    out.line(points[i], points[(i + 1) % 4], width, color)?;
                } else {
                    out.dashed(points[i], points[(i + 1) % 4], width, color)?;
                }
                for next in [(i + 1) % 4, (i + 3) % 4] {
                    let v = points[next] - points[i];
                    let length = v.length();
                    if length > 0. {
                        out.line(
                            points[i],
                            points[i] + v * (14.0f32.min(length * 0.18) / length),
                            width,
                            color,
                        )?;
                    }
                }
            }
            let symbol = plane.symbol.trim();
            if !symbol.is_empty() {
                let symbol: String = symbol.chars().take(3).collect();
                for point in points {
                    out.add(Shape::circle_filled(
                        point,
                        9.,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                    ))?;
                    out.add(Shape::circle_stroke(point, 9., Stroke::new(width, color)))?;
                    out.text(point, &symbol, 8.5, color, true)?;
                }
            }
        }
    }
    let eye = app.camera.eye();
    let light = DVec3::new(0.3, 0.4, 0.85).normalize();
    for path in &scene.geo_paths {
        if let Some(triangles) = &path.surface_tris {
            if triangles.len() > 200_000 {
                return Err("Capture geographic surface limit exceeded".into());
            }
            let mut order = Vec::new();
            order
                .try_reserve_exact(triangles.len())
                .map_err(|_| "Capture surface ordering refused")?;
            order.extend(0..triangles.len());
            order.sort_by(|&a, &b| {
                let (a0, a1, a2) = triangles[a];
                let (b0, b1, b2) = triangles[b];
                (b0 + b1 + b2 - eye * 3.)
                    .length_squared()
                    .total_cmp(&(a0 + a1 + a2 - eye * 3.).length_squared())
            });
            for i in order {
                let (a, b, c) = triangles[i];
                let (Some(a2), Some(b2), Some(c2)) =
                    (out.project(a), out.project(b), out.project(c))
                else {
                    continue;
                };
                let normal = (b - a).cross(c - a).normalize_or_zero();
                let shade = 0.55 + 0.45 * normal.dot(light).abs().max(0.);
                let rgb = if path.surface == Some(ingetrazo_core::georef::SurfaceFillMode::Flat) {
                    [90., 120., 200.]
                } else {
                    [120., 160., 90.]
                };
                out.add(Shape::convex_polygon(
                    vec![a2, b2, c2],
                    Color32::from_rgba_unmultiplied(
                        (rgb[0] * shade) as u8,
                        (rgb[1] * shade) as u8,
                        (rgb[2] * shade) as u8,
                        150,
                    ),
                    Stroke::NONE,
                ))?;
            }
        }
    }
    for (index, path) in scene.geo_paths.iter().enumerate() {
        if path.points.len() > 200_000 {
            return Err("Capture geographic path limit exceeded".into());
        }
        let selected = scene.is_selected(&SelectionItem::GeoPath(index));
        let color = if selected {
            Color32::from_rgb(243, 115, 41)
        } else {
            Color32::from_rgb(0, 190, 210)
        };
        let mut projected = Vec::new();
        projected
            .try_reserve_exact(path.points.len())
            .map_err(|_| "Capture path projection refused")?;
        for &p in &path.points {
            let p = if let Some(terrain) = scene.terrain.as_ref().filter(|t| t.visible) {
                DVec3::new(p.x, p.y, terrain.height_at(p.x, p.y).unwrap_or(p.z))
            } else {
                p
            };
            projected.push(out.project(p));
        }
        for pair in projected.windows(2) {
            if let [Some(a), Some(b)] = pair {
                out.line(*a, *b, 2.5, color)?;
            }
        }
        if path.closed && projected.len() > 2 {
            if let (Some(Some(a)), Some(Some(b))) = (projected.last(), projected.first()) {
                out.line(*a, *b, 2.5, color)?;
            }
        }
        for p in projected.into_iter().flatten() {
            out.add(Shape::circle_filled(p, 3.5, color))?;
            out.add(Shape::circle_stroke(
                p,
                3.5,
                Stroke::new(1., Color32::WHITE),
            ))?;
        }
        if selected && path.closed && path.points.len() >= 3 {
            let centroid = path.points.iter().copied().sum::<DVec3>() / path.points.len() as f64;
            if let Some(p) = out.project(centroid) {
                out.text(
                    p,
                    &format!(
                        "Area: {:.1} m² ({:.3} ha)\nPerimeter: {:.2} m",
                        path.area(),
                        path.area() / 10000.,
                        path.perimeter()
                    ),
                    12.,
                    Color32::WHITE,
                    true,
                )?;
            }
        }
    }
    for point in &scene.geo_points {
        if let Some(p) = out.project(point.position) {
            let color = Color32::from_rgb(206, 66, 87);
            out.line(p - egui::vec2(5., 0.), p + egui::vec2(5., 0.), 1.6, color)?;
            out.line(p - egui::vec2(0., 5.), p + egui::vec2(0., 5.), 1.6, color)?;
            if !point.name.is_empty() {
                out.text(
                    p + egui::vec2(7., -4.),
                    &point.name,
                    8.,
                    Color32::from_rgb(120, 34, 48),
                    false,
                )?;
            }
        }
    }
    Ok(Presentation {
        shapes: out.shapes,
        text: out.text,
    })
}
