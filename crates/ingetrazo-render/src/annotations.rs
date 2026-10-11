//! Ordered annotation overlay plan. Depth remains a GPU visibility dependency;
//! this producer never reads or rasterizes the model's depth on CPU.
use glam::Vec3;
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gate {
    Always,
    Point(u32),
    Both(u32, u32),
}
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Line {
        a: [f64; 2],
        b: [f64; 2],
        width: f64,
        color: [u8; 4],
    },
    Polygon {
        points: [[f64; 2]; 3],
        color: [u8; 4],
    },
    Ellipse {
        center: [f64; 2],
        radius: [f64; 2],
        width: f64,
        color: [u8; 4],
    },
    Text {
        position: [f64; 2],
        text: Arc<str>,
        size_pt: i32,
        color: [u8; 4],
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    pub primitive: Primitive,
    pub gate: Gate,
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub queries: Vec<[f32; 3]>,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub queries: usize,
    pub operations: usize,
    pub text_bytes: usize,
}
#[derive(Clone, Debug)]
pub struct Style {
    pub color: [u8; 4],
    pub font_size: i32,
    pub ends: String,
    pub units: String,
    pub decimals: i32,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            color: [45, 55, 75, 255],
            font_size: 9,
            ends: "arrow".into(),
            units: "m".into(),
            decimals: 2,
        }
    }
}
#[derive(Clone, Copy)]
pub struct DimensionInput<'a> {
    pub a: Vec3,
    pub b: Vec3,
    pub offset: Vec3,
    pub axis: Option<&'a str>,
    pub text: Option<&'a str>,
    pub highlighted: bool,
    pub hidden: bool,
}
#[derive(Clone, Copy)]
pub struct LeaderInput<'a> {
    pub anchor: Vec3,
    pub offset: Vec3,
    pub text: &'a str,
    pub highlighted: bool,
    pub hidden: bool,
}
pub struct Builder<P> {
    plan: Plan,
    project: P,
    limits: Limits,
    text_bytes: usize,
    failed: bool,
}
impl<P: FnMut(Vec3) -> Option<[f64; 2]>> Builder<P> {
    pub fn new(project: P, limits: Limits) -> Self {
        Self {
            plan: Plan {
                queries: Vec::new(),
                operations: Vec::new(),
            },
            project,
            limits,
            text_bytes: 0,
            failed: false,
        }
    }
    pub fn finish(self) -> Result<Plan, String> {
        if self.failed {
            Err("Annotation plan contains a refused operation".into())
        } else {
            Ok(self.plan)
        }
    }
    fn project(&mut self, p: Vec3) -> Result<Option<[f64; 2]>, String> {
        if !p.is_finite() {
            return Err("Nonfinite annotation point".into());
        }
        let result = (self.project)(p);
        if result.is_some_and(|a| !a.iter().all(|v| v.is_finite())) {
            return Err("Nonfinite annotation projection".into());
        }
        Ok(result)
    }
    fn query(&mut self, p: Vec3) -> Result<u32, String> {
        if self.plan.queries.len() >= self.limits.queries
            || self.plan.queries.len() >= u32::MAX as usize
        {
            return Err("Annotation query admission failed".into());
        }
        self.plan
            .queries
            .try_reserve(1)
            .map_err(|_| "Annotation query allocation failed")?;
        let id = self.plan.queries.len() as u32;
        self.plan.queries.push(p.to_array());
        Ok(id)
    }
    fn emit(&mut self, primitive: Primitive, gate: Gate) -> Result<(), String> {
        let point = |p: &[f64; 2]| p.iter().all(|v| v.is_finite());
        let valid = match &primitive {
            Primitive::Line { a, b, width, .. } => {
                point(a) && point(b) && width.is_finite() && *width >= 0.
            }
            Primitive::Polygon { points, .. } => points.iter().all(point),
            Primitive::Ellipse {
                center,
                radius,
                width,
                ..
            } => {
                point(center)
                    && point(radius)
                    && radius.iter().all(|v| *v >= 0.)
                    && width.is_finite()
                    && *width >= 0.
            }
            Primitive::Text {
                position, size_pt, ..
            } => point(position) && *size_pt > 0,
        };
        if !valid {
            return Err("Nonfinite or invalid final annotation primitive".into());
        }
        if self.plan.operations.len() >= self.limits.operations {
            return Err("Annotation operation admission failed".into());
        }
        self.plan
            .operations
            .try_reserve(1)
            .map_err(|_| "Annotation operation allocation failed")?;
        self.plan.operations.push(Operation { primitive, gate });
        Ok(())
    }
    fn segment(&mut self, a: Vec3, b: Vec3, width: f64, color: [u8; 4]) -> Result<(), String> {
        let mut previous = None;
        for i in 0..=16 {
            // Qt QVector3D subtraction/multiply/add narrows each operation to float.
            let world = a + (b - a) * (i as f32 / 16.);
            let current = if let Some(pixel) = self.project(world)? {
                Some((pixel, self.query(world)?))
            } else {
                None
            };
            if let (Some((pa, qa)), Some((pb, qb))) = (previous, current) {
                self.emit(
                    Primitive::Line {
                        a: pa,
                        b: pb,
                        width,
                        color,
                    },
                    Gate::Both(qa, qb),
                )?;
            }
            previous = current;
        }
        Ok(())
    }
    fn text(
        &mut self,
        position: [f64; 2],
        text: Arc<str>,
        size_pt: i32,
        color: [u8; 4],
        gate: Gate,
    ) -> Result<(), String> {
        self.emit(
            Primitive::Text {
                position,
                text,
                size_pt,
                color,
            },
            gate,
        )
    }
    fn authored_text(&mut self, text: &str) -> Result<Arc<str>, String> {
        self.text_bytes = self
            .text_bytes
            .checked_add(text.len())
            .filter(|&n| n <= self.limits.text_bytes)
            .ok_or("Annotation text admission failed")?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| "Annotation text allocation failed")?;
        owned.push_str(text);
        Ok(owned.into())
    }
    pub fn dimension(&mut self, input: DimensionInput<'_>, style: &Style) -> Result<(), String> {
        if self.failed {
            return Err("Annotation plan already refused".into());
        }
        let result = self.dimension_inner(input, style);
        self.failed |= result.is_err();
        result
    }
    fn dimension_inner(&mut self, input: DimensionInput<'_>, style: &Style) -> Result<(), String> {
        if input.hidden {
            return Ok(());
        }
        if !input.a.is_finite() || !input.b.is_finite() || !input.offset.is_finite() {
            return Err("Nonfinite annotation dimension".into());
        }
        let (ap, bp, value) = dimension_geometry(input);
        let (Some(pap), Some(pbp)) = (self.project(ap)?, self.project(bp)?) else {
            return Ok(());
        };
        let ink = if input.highlighted {
            [243, 115, 41, 255]
        } else {
            style.color
        };
        self.segment(input.a, ap, 1., ink)?;
        self.segment(input.b, bp, 1., ink)?;
        self.segment(ap, bp, 1.5, ink)?;
        let dx = pbp[0] - pap[0];
        let dy = pbp[1] - pap[1];
        let length = dx.hypot(dy);
        let ends = if style.ends.is_empty() {
            "arrow"
        } else {
            style.ends.as_str()
        };
        if length > 1e-6 && ends != "none" {
            let ux = dx / length;
            let uy = dy / length;
            for (pixel, world, sign) in [(pap, ap, 1.), (pbp, bp, -1.)] {
                let gate = Gate::Point(self.query(world)?);
                if ends == "tick" {
                    let ox = -uy * 4.;
                    let oy = ux * 4.;
                    self.emit(
                        Primitive::Line {
                            a: [pixel[0] - ox, pixel[1] - oy],
                            b: [pixel[0] + ox, pixel[1] + oy],
                            width: 1.5,
                            color: ink,
                        },
                        gate,
                    )?;
                } else {
                    let bx = pixel[0] + sign * ux * 10.;
                    let by = pixel[1] + sign * uy * 10.;
                    self.emit(
                        Primitive::Polygon {
                            points: [
                                pixel,
                                [bx - uy * 2.2, by + ux * 2.2],
                                [bx + uy * 2.2, by - ux * 2.2],
                            ],
                            color: ink,
                        },
                        gate,
                    )?;
                }
            }
        }
        let mid = (ap + bp) * 0.5;
        if let Some(pixel) = self.project(mid)? {
            let gate = Gate::Point(self.query(mid)?);
            let measured = format_length(value as f64, &style.units, style.decimals)?;
            let text = match input.text.filter(|s| !s.is_empty()) {
                Some(t) => replace_measurement(
                    t,
                    &measured,
                    self.limits.text_bytes.saturating_sub(self.text_bytes),
                )?,
                None => measured,
            };
            let text = self.authored_text(&text)?;
            self.text(
                [pixel[0] + 5., pixel[1] - 4.],
                text.clone(),
                style.font_size,
                [255, 255, 255, 230],
                gate,
            )?;
            self.text(
                [pixel[0] + 4., pixel[1] - 5.],
                text,
                style.font_size,
                ink,
                gate,
            )?;
        }
        Ok(())
    }
    /// Metrics must come from the actual native font provider. The controlled
    /// source comparator supplies captured metrics and does not assert font parity.
    pub fn leader(
        &mut self,
        input: LeaderInput<'_>,
        style: &Style,
        height: f64,
        advance: impl FnMut(&str) -> Result<f64, String>,
    ) -> Result<(), String> {
        if self.failed {
            return Err("Annotation plan already refused".into());
        }
        let result = self.leader_inner(input, style, height, advance);
        self.failed |= result.is_err();
        result
    }
    fn leader_inner(
        &mut self,
        input: LeaderInput<'_>,
        style: &Style,
        height: f64,
        mut advance: impl FnMut(&str) -> Result<f64, String>,
    ) -> Result<(), String> {
        if input.hidden {
            return Ok(());
        }
        if !height.is_finite() || height < 0. {
            return Err("Invalid annotation font metrics".into());
        }
        let pos = input.anchor + input.offset;
        let anchor = self.project(input.anchor)?;
        let Some(pixel) = self.project(pos)? else {
            return Ok(());
        };
        let ink = if input.highlighted {
            [243, 115, 41, 255]
        } else {
            style.color
        };
        self.segment(input.anchor, pos, 1.2, ink)?;
        if let Some(center) = anchor {
            let gate = Gate::Point(self.query(input.anchor)?);
            self.emit(
                Primitive::Ellipse {
                    center,
                    radius: [2.5, 2.5],
                    width: 1.2,
                    color: ink,
                },
                gate,
            )?;
        }
        if input.text.len() > self.limits.text_bytes.saturating_sub(self.text_bytes) {
            return Err("Annotation text admission failed".into());
        }
        let lines = splitlines(input.text, self.limits.operations / 2)?;
        let mut width = 0_f64;
        for line in &lines {
            let w = advance(line)?;
            if !w.is_finite() || w < 0. {
                return Err("Invalid annotation text advance".into());
            }
            width = width.max(w);
        }
        let x = if anchor.is_some_and(|a| pixel[0] < a[0]) {
            pixel[0] - 6. - width
        } else {
            pixel[0] + 6.
        };
        for (i, line) in lines.into_iter().enumerate() {
            let text = self.authored_text(line)?;
            let y = pixel[1] - 4. + i as f64 * height;
            self.text(
                [x + 1., y + 1.],
                text.clone(),
                style.font_size,
                [255, 255, 255, 230],
                Gate::Always,
            )?;
            self.text([x, y], text, style.font_size, ink, Gate::Always)?;
        }
        Ok(())
    }
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn length(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        0.
    } else {
        let s = v / m;
        m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
    }
}
fn linear_frame(a: Vec3, b: Vec3, axis: Option<&str>) -> Option<(Vec3, Vec3)> {
    let u = match axis {
        Some("x") => Vec3::X,
        Some("y") => Vec3::Y,
        Some("z") => Vec3::Z,
        _ => return None,
    };
    let ab = b - a;
    let n = length(ab);
    if n < 1e-9 {
        return None;
    }
    let d = ab / n;
    let e = d - u * dot(d, u);
    let n = length(e);
    if n < 1e-6 {
        return None;
    }
    Some((u, e / n))
}
/// Exact source float32 extension and dimension line endpoints for screen picking.
pub fn dimension_line_points(input: DimensionInput<'_>) -> (Vec3,Vec3) {
    let(a,b,_)=dimension_geometry(input);(a,b)
}
fn dimension_geometry(input: DimensionInput<'_>) -> (Vec3, Vec3, f32) {
    let a = input.a;
    let b = input.b;
    let ab = b - a;
    if let Some((u, e)) = linear_frame(a, b, input.axis) {
        let ta = dot(input.offset, e);
        let tb = ta - dot(ab, e);
        (a + e * ta, b + e * tb, dot(ab, u).abs())
    } else {
        let n = length(ab);
        let off = if n < 1e-9 {
            input.offset
        } else {
            let d = ab / n;
            input.offset - d * dot(input.offset, d)
        };
        (a + off, b + off, n)
    }
}
fn replace_measurement(template: &str, measured: &str, limit: usize) -> Result<String, String> {
    let count = template.matches("<>").count();
    let length = template
        .len()
        .checked_sub(count * 2)
        .and_then(|n| {
            count
                .checked_mul(measured.len())
                .and_then(|extra| n.checked_add(extra))
        })
        .filter(|&n| n <= limit)
        .ok_or("Annotation expanded text admission failed")?;
    let mut result = String::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| "Annotation expanded text allocation failed")?;
    let mut parts = template.split("<>");
    result.push_str(parts.next().unwrap_or(""));
    for part in parts {
        result.push_str(measured);
        result.push_str(part);
    }
    Ok(result)
}
pub fn splitlines(text: &str, limit: usize) -> Result<Vec<&str>, String> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\u{001c}'
                | '\u{001d}'
                | '\u{001e}'
                | '\u{0085}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            if result.len() >= limit {
                return Err("Annotation text line admission failed".into());
            }
            result
                .try_reserve(1)
                .map_err(|_| "Annotation line allocation failed")?;
            result.push(&text[start..i]);
            start = i + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|&(_, c)| c == '\n') {
                let (j, c) = chars.next().unwrap();
                start = j + c.len_utf8();
            }
        }
    }
    if start < text.len() {
        if result.len() >= limit {
            return Err("Annotation text line admission failed".into());
        }
        result
            .try_reserve(1)
            .map_err(|_| "Annotation line allocation failed")?;
        result.push(&text[start..]);
    }
    if result.is_empty() {
        if limit == 0 {
            return Err("Annotation text line admission failed".into());
        }
        result
            .try_reserve(1)
            .map_err(|_| "Annotation line allocation failed")?;
        result.push("");
    }
    Ok(result)
}
fn fraction_inches(inches: f64, n: i32) -> Result<String, String> {
    let denominator = match n.clamp(0, 4) {
        0 => 1.,
        1 => 4.,
        2 => 16.,
        3 => 32.,
        _ => 64.,
    };
    let scaled = inches * denominator;
    if !scaled.is_finite() {
        return Err("Annotation fractional conversion overflow".into());
    }
    let count = scaled.round_ties_even();
    let whole = (count / denominator).trunc();
    let numerator = count % denominator;
    if numerator == 0. {
        return Ok(format!("{whole:.0}"));
    }
    let mut a = numerator as u32;
    let mut b = denominator as u32;
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    let rest = format!("{}/{}", numerator as u32 / a, denominator as u32 / a);
    Ok(if whole == 0. {
        rest
    } else {
        format!("{whole:.0} {rest}")
    })
}
pub fn format_length(meters: f64, units: &str, decimals: i32) -> Result<String, String> {
    if !meters.is_finite() {
        return Err("Nonfinite annotation measured length".into());
    }
    let n = decimals.clamp(0, 6) as usize;
    let u = if units.is_empty() { "m" } else { units };
    Ok(match u {
        "in" => {
            let value = meters / 0.0254;
            if !value.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            format!("{value:.n$}\"")
        }
        "ft" => {
            let value = meters / 0.3048;
            if !value.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            format!("{value:.n$}'")
        }
        "ft-in" | "ft-in-frac" | "in-frac" => {
            let sign = if meters < 0. { "-" } else { "" };
            let total = meters.abs() / 0.0254;
            if !total.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            if u == "in-frac" {
                format!("{sign}{}\"", fraction_inches(total, n as i32)?)
            } else {
                let mut feet = (total / 12.).floor();
                let mut inches = total - feet * 12.;
                if u == "ft-in" {
                    if format!("{inches:.n$}").parse::<f64>().unwrap_or(inches) >= 12. {
                        feet += 1.;
                        inches = 0.;
                    }
                    format!("{sign}{feet:.0}'{inches:.n$}\"")
                } else {
                    let mut text = fraction_inches(inches, n as i32)?;
                    if text == "12" {
                        feet += 1.;
                        text = "0".into();
                    }
                    format!("{sign}{feet:.0}'{text}\"")
                }
            }
        }
        _ => {
            let factor = match u {
                "cm" => 100.,
                "mm" => 1000.,
                _ => 1.,
            };
            let suffix = if matches!(u, "m" | "cm" | "mm") {
                u
            } else {
                "m"
            };
            let converted = meters * factor;
            if !converted.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            format!("{converted:.n$} {suffix}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn vector(v: &Value) -> Vec3 {
        Vec3::new(
            v[0].as_f64().unwrap() as f32,
            v[1].as_f64().unwrap() as f32,
            v[2].as_f64().unwrap() as f32,
        )
    }
    fn operation(op: &Operation) -> Value {
        match &op.primitive {
            Primitive::Line { a, b, width, color } => {
                json!({"kind":"line","pen":color,"width":width,"pen_style":1,"a":a,"b":b})
            }
            Primitive::Polygon { points, color } => {
                json!({"kind":"polygon","pen":[0,0,0,255],"width":1.0,"pen_style":0,"points":points,"brush":color})
            }
            Primitive::Ellipse {
                center,
                radius,
                width,
                color,
            } => {
                json!({"kind":"ellipse","pen":color,"width":width,"pen_style":1,"center":center,"radius":radius,"brush":color})
            }
            Primitive::Text {
                position,
                text,
                size_pt,
                color,
            } => {
                json!({"kind":"text","pen":color,"width":1.0,"pen_style":1,"position":position,"text":text.as_ref(),"font":["Sans Serif",*size_pt as f64,true]})
            }
        }
    }
    #[test]
    fn ordered_geometry_matches_all_fifteen_actual_source_cases() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../../docs/audits/native-annotation-source/original.json"
        ))
        .unwrap();
        assert_eq!(
            reference["source_pin"],
            "6be29fe437a7cd992d4e079f71720821b25216b9"
        );
        let rows = reference["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 15);
        assert_eq!(
            rows.iter()
                .map(|r| r["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "arrow",
                "tick-selected",
                "none",
                "partial-occlusion",
                "hidden",
                "leader-right",
                "leader-left-selected",
                "leader-anchor-unprojectable",
                "leader-hidden",
                "leader-empty",
                "dim-hover",
                "dim-mm-axis",
                "dim-feet-fractional",
                "leader-hover",
                "leader-splitlines"
            ]
        );

        for row in rows {
            let input = &row["entity"];
            let style = &row["style"];
            let col = &style["color"];
            let style = Style {
                color: [
                    col[0].as_u64().unwrap() as u8,
                    col[1].as_u64().unwrap() as u8,
                    col[2].as_u64().unwrap() as u8,
                    255,
                ],
                font_size: style["font_size"].as_i64().unwrap() as i32,
                ends: style["ends"].as_str().unwrap().into(),
                units: style["units"].as_str().unwrap().into(),
                decimals: style["decimals"].as_i64().unwrap() as i32,
            };
            let reject = row["reject_negative_x_projection"].as_bool().unwrap();
            let mut builder = Builder::new(
                move |p: Vec3| {
                    if reject && p.x < 0. {
                        None
                    } else {
                        Some([160. + p.x as f64 * 30., 160. - p.y as f64 * 30.])
                    }
                },
                Limits {
                    queries: 1024,
                    operations: 1024,
                    text_bytes: 4096,
                },
            );
            let highlighted =
                row["selected"].as_bool().unwrap() || row["hovered"].as_bool().unwrap();
            let hidden = row["hidden"].as_bool().unwrap();
            if !input["a"].is_null() {
                builder
                    .dimension(
                        DimensionInput {
                            a: vector(&input["a"]),
                            b: vector(&input["b"]),
                            offset: vector(&input["offset"]),
                            axis: input["axis"].as_str(),
                            text: input["text"].as_str(),
                            highlighted,
                            hidden,
                        },
                        &style,
                    )
                    .unwrap();
            } else {
                let metrics = row["font_metrics"].as_array().unwrap().first();
                let height = metrics.map_or(0., |m| m["height"].as_f64().unwrap());
                builder
                    .leader(
                        LeaderInput {
                            anchor: vector(&input["anchor"]),
                            offset: vector(&input["offset"]),
                            text: input["text"].as_str().unwrap(),
                            highlighted,
                            hidden,
                        },
                        &style,
                        height,
                        |text| {
                            metrics
                                .and_then(|m| m["advances"].as_array())
                                .and_then(|list| list.iter().find(|a| a["text"] == text))
                                .and_then(|a| a["value"].as_f64())
                                .ok_or_else(|| "Missing actual source metric input".into())
                        },
                    )
                    .unwrap();
            }
            let plan = builder.finish().unwrap();
            let hidden = |id: u32| {
                row["controlled_occlusion"].as_bool().unwrap()
                    && (1.25..=2.75).contains(&plan.queries[id as usize][0])
            };
            let actual = plan
                .operations
                .iter()
                .filter(|op| match op.gate {
                    Gate::Always => true,
                    Gate::Point(q) => !hidden(q),
                    Gate::Both(a, b) => !hidden(a) && !hidden(b),
                })
                .map(operation)
                .collect::<Vec<_>>();
            assert_eq!(
                Value::Array(actual),
                row["operations"],
                "Source operation mismatch {}",
                row["name"]
            );
        }
    }
    #[test]
    fn source_empty_override_and_parallel_axis_fall_back_to_measured_length() {
        let input = DimensionInput {
            a: Vec3::ZERO,
            b: Vec3::new(0., 3., 4.),
            offset: Vec3::X,
            axis: Some("x"),
            text: Some(""),
            highlighted: false,
            hidden: false,
        };
        let (_, _, value) = dimension_geometry(input);
        assert_eq!(value, 0.);
        let parallel = DimensionInput {
            b: Vec3::new(5., 0., 0.),
            ..input
        };
        let (_, _, value) = dimension_geometry(parallel);
        assert_eq!(value, 5.);
        assert_eq!(
            splitlines("A\r\nB\u{2028}C\n", 16).unwrap(),
            vec!["A", "B", "C"]
        );
        assert_eq!(format_length(0.0381, "in-frac", 2).unwrap(), "1 1/2\"");
    }
    #[test]
    fn failed_partial_plan_cannot_be_published_or_resumed() {
        let mut builder = Builder::new(
            |p: Vec3| Some([p.x as f64, p.y as f64]),
            Limits {
                queries: 3,
                operations: 1,
                text_bytes: 32,
            },
        );
        let input = DimensionInput {
            a: Vec3::ZERO,
            b: Vec3::X,
            offset: Vec3::Y,
            axis: None,
            text: None,
            highlighted: false,
            hidden: false,
        };
        assert!(builder.dimension(input, &Style::default()).is_err());
        assert!(!builder.plan.operations.is_empty());
        assert!(builder.dimension(input, &Style::default()).is_err());
        assert!(builder.finish().is_err());
        assert!(format_length(f64::MAX, "mm", 2).is_err());
        assert!(format_length(f64::MAX / 100., "in-frac", 4).is_err());
    }
    #[test]
    fn derived_leader_position_overflow_refuses_entire_plan() {
        let mut builder = Builder::new(
            |p: Vec3| Some([if p.x == 0. { f64::MAX } else { -f64::MAX }, 0.]),
            Limits {
                queries: 1024,
                operations: 1024,
                text_bytes: 128,
            },
        );
        let input = LeaderInput {
            anchor: Vec3::ZERO,
            offset: Vec3::X,
            text: "A",
            highlighted: false,
            hidden: false,
        };
        assert!(builder
            .leader(input, &Style::default(), 18., |_| Ok(f64::MAX))
            .is_err());
        assert!(builder.finish().is_err());
    }
}
