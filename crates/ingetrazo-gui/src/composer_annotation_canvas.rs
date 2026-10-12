//! Retained annotation coverage on the existing native CAD callback path.
//! Layout ownership and source hit testing belong to the authoring controller.
use super::text_canvas::CanvasPainter;
use crate::{
    composer_annotations::Draws,
    composer_backend::{Factory, Handle},
};
use egui::{Pos2, Rect, Vec2};
use ingetrazo_formats::sheet_pdf::{Draw, PathOp};
use ingetrazo_render::cad2d::{Batch, Geometry, Pose};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};
const ENTRY_BYTES: usize = 32 * 1024 * 1024;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PARTS: usize = 10000;
const MAX_ENTRIES: usize = 10000;
type Key = (usize, usize);
#[derive(Clone)]
struct Part {
    geometry: Arc<Geometry>,
    batches: Arc<[Batch]>,
    shift: [f64; 2],
    handle: Handle,
}
#[derive(Clone)]
struct Entry {
    draws: Weak<Draws>,
    density: u64,
    phase: [u64; 2],
    factory: (u8, usize),
    parts: Vec<Part>,
    bounds: Rect,
    bytes: usize,
}
#[derive(Clone, Default)]
pub(super) struct Cache {
    items: HashMap<Key, Entry>,
    bytes: usize,
    builds: u64,
    paints: u64,
}
impl Cache {
    pub(super) fn clear(&mut self) {
        self.items.clear();
        self.bytes = 0;
    }
    pub(super) fn geometry_ids(&self)->Vec<u64>{self.items.values().flat_map(|e|e.parts.iter().map(|p|p.geometry.id())).collect()}
    pub(super) fn stats(&self) -> (u64, u64, usize) {
        (self.builds, self.paints, self.bytes)
    }
    /// `base` already contains the authored position. The shared Draws must be
    /// local to that position. Integer device motion only changes callback poses.
    pub(super) fn paint(
        &mut self,
        painter: &egui::Painter,
        factory: &Factory,
        fonts: &mut CanvasPainter,
        key: Key,
        draws: &Arc<Draws>,
        zoom: f32,
        base: Pos2,
    ) -> Result<Rect, String> {
        let ppp = painter.ctx().pixels_per_point();
        if !zoom.is_finite() || zoom <= 0. || !ppp.is_finite() || ppp <= 0. || !base.is_finite() {
            return Err("Invalid annotation canvas projection".into());
        }
        let density = f64::from(zoom) * f64::from(ppp);
        let physical = [
            f64::from(base.x) * f64::from(ppp),
            f64::from(base.y) * f64::from(ppp),
        ];
        if physical
            .iter()
            .any(|p| !p.is_finite() || p.abs() > 1_000_000.)
            || !density.is_finite()
        {
            return Err("Annotation canvas projection exceeds domain".into());
        }
        let anchor = physical.map(f64::floor);
        let phase = [physical[0] - anchor[0], physical[1] - anchor[1]];
        let phase_bits = phase.map(f64::to_bits);
        let identity = factory.cache_identity();
        let same = self.items.get(&key).is_some_and(|e| {
            e.density == density.to_bits()
                && e.phase == phase_bits
                && e.factory == identity
                && e.draws
                    .upgrade()
                    .is_some_and(|old| Arc::ptr_eq(&old, draws))
        });
        if !same {
            // Publish only a fully admitted entry. Failure retains previous resources.
            let entry = prepare(draws, fonts, factory, density, phase, anchor, ppp)?;
            let prior = self.items.get(&key).map_or(0, |e| e.bytes);
            while self.items.len() - usize::from(self.items.contains_key(&key)) >= MAX_ENTRIES
                || self.bytes - prior + entry.bytes > CACHE_BYTES
            {
                let Some(victim) = self.items.keys().copied().find(|k| *k != key) else {
                    return Err("Annotation canvas cache admission exceeded".into());
                };
                self.bytes -= self.items.remove(&victim).unwrap().bytes;
            }
            self.items
                .try_reserve(1)
                .map_err(|_| "Annotation canvas cache allocation refused")?;
            self.bytes = self.bytes - prior + entry.bytes;
            self.items.insert(key, entry);
            self.builds += 1;
        }
        let entry = &self.items[&key];
        // Validate all poses before adding any callback to this frame.
        for part in &entry.parts {
            part.geometry
                .validate_ordered_pose(pose(anchor, part.shift, ppp), ppp)?;
        }
        for part in &entry.parts {
            part.handle.update(
                part.geometry.clone(),
                part.batches.clone(),
                pose(anchor, part.shift, ppp),
            )?;
        }
        for part in &entry.parts {
            painter.add(part.handle.shape(painter.clip_rect()));
        }
        self.paints += 1;
        if entry.bounds == Rect::NOTHING {
            return Ok(Rect::from_min_size(base, Vec2::ZERO));
        }
        Ok(Rect::from_min_max(
            Pos2::new(
                (f64::from(entry.bounds.min.x) + anchor[0]) as f32 / ppp,
                (f64::from(entry.bounds.min.y) + anchor[1]) as f32 / ppp,
            ),
            Pos2::new(
                (f64::from(entry.bounds.max.x) + anchor[0]) as f32 / ppp,
                (f64::from(entry.bounds.max.y) + anchor[1]) as f32 / ppp,
            ),
        ))
    }
}
fn pose(anchor: [f64; 2], shift: [f64; 2], ppp: f32) -> Pose {
    Pose {
        scale: [1. / ppp; 2],
        translation: [
            ((anchor[0] - shift[0]) / f64::from(ppp)) as f32,
            ((anchor[1] - shift[1]) / f64::from(ppp)) as f32,
        ],
    }
}
fn geometry_bytes(g: &Geometry, b: &[Batch], factory: &Factory) -> Result<usize, String> {
    let mut bytes =
        usize::try_from(g.logical_bytes()).map_err(|_| "Annotation geometry size overflow")?;
    for n in [
        g.source_spans().map_or(0, |s| s.logical_bytes() as usize),
        g.ordered_draws()
            .map_or(0, |r| r.len() * std::mem::size_of::<std::ops::Range<u32>>()),
        g.premultiplied_palette().map_or(0, |p| p.len() * 4),
        std::mem::size_of_val(b),
        factory.retained_cpu_bytes_bound(b.len())?,
    ] {
        bytes = bytes
            .checked_add(n)
            .ok_or("Annotation geometry size overflow")?;
    }
    Ok(bytes)
}
fn prepare(
    draws: &Arc<Draws>,
    fonts: &mut CanvasPainter,
    factory: &Factory,
    density: f64,
    phase: [f64; 2],
    anchor: [f64; 2],
    ppp: f32,
) -> Result<Entry, String> {
    if draws.draws.len() > MAX_PARTS {
        return Err("Annotation canvas draw count exceeded".into());
    }
    let mut records = Vec::new();
    let mut bytes = std::mem::size_of::<Entry>();
    let mut bounds = Rect::NOTHING;
    for (index, source) in draws.draws.iter().enumerate() {
        let mut draw = source.clone();
        // Text records support rotated glyphs and physical decorations directly.
        // Path records use a finite integer translation, preserving device phase
        // while allowing negative local leaders and radius endpoints.
        let (shift, device) = if draw.text.is_some() {
            ([0.; 2], [0; 2])
        } else {
            path_extent(&draw, density, phase)?
        };
        for axis in 0..2 {
            draw.origin_mm[axis] += (phase[axis] + shift[axis]) / density;
        }
        let parts = if draw.text.is_some() {
            fonts.physical_records(std::slice::from_ref(&draw), density)?
        } else if draws.rectangles.binary_search(&index).is_ok() {
            fonts.physical_rectangle(&draw, density)?
        } else if draws.source_points.binary_search(&index).is_ok() {
            crate::sheet_image_paths::source_point_records(&draw, density, device)?
        } else if draws.source_lines.binary_search(&index).is_ok() {
            crate::sheet_image_paths::source_line_records(&draw, density, device)?
        } else {
            crate::sheet_image_paths::records(std::slice::from_ref(&draw), density, device)?
        };
        for (geometry, batches) in parts {
            bytes = bytes
                .checked_add(
                    geometry_bytes(&geometry, &batches, factory)?
                        .checked_add(std::mem::size_of::<Part>())
                        .ok_or("Annotation part size overflow")?,
                )
                .ok_or("Annotation canvas budget overflow")?;
            if bytes > ENTRY_BYTES || records.len() >= MAX_PARTS {
                return Err("Annotation canvas resource budget exceeded".into());
            }
            for point in geometry.bounds() {
                bounds.extend_with(Pos2::new(
                    (f64::from(point[0]) - shift[0]) as f32,
                    (f64::from(point[1]) - shift[1]) as f32,
                ));
            }
            geometry.validate_ordered_pose(pose(anchor, shift, ppp), ppp)?;
            records.push((geometry, batches, shift));
        }
    }
    let mut parts = Vec::new();
    parts
        .try_reserve_exact(records.len())
        .map_err(|_| "Annotation callback allocation refused")?;
    for (geometry, batches, shift) in records {
        let handle = factory.handle(geometry.clone(), batches.clone(), pose(anchor, shift, ppp))?;
        parts.push(Part {
            geometry,
            batches,
            shift,
            handle,
        });
    }
    Ok(Entry {
        draws: Arc::downgrade(draws),
        density: density.to_bits(),
        phase: phase.map(f64::to_bits),
        factory: factory.cache_identity(),
        parts,
        bounds,
        bytes,
    })
}
/// Control hull plus the admitted annotation pen bounds is conservative for
/// every line/cubic. Integer shifts cannot change raster phase or dash length.
fn path_extent(draw: &Draw, density: f64, phase: [f64; 2]) -> Result<([f64; 2], [u32; 2]), String> {
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    let mut include = |point: [f64; 2]| -> Result<(), String> {
        for axis in 0..2 {
            let value = draw.origin_mm[axis] * density + point[axis] * density + phase[axis];
            if !value.is_finite() || value.abs() > 1_000_000. {
                return Err("Annotation path exceeds finite canvas domain".into());
            }
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
        Ok(())
    };
    for op in &draw.path {
        match op {
            PathOp::Move(p) | PathOp::Line(p) => include(*p)?,
            PathOp::Cubic(p) => {
                include([p[0], p[1]])?;
                include([p[2], p[3]])?;
                include([p[4], p[5]])?;
            }
            PathOp::Close => {}
        }
    }
    if !low[0].is_finite() {
        return Err("Annotation path has no finite points".into());
    }
    let margin = match draw.stroke {
        Some((_, w)) if w.is_finite() && w >= 0. => w * density + 2.,
        Some(_) => return Err("Invalid annotation canvas stroke".into()),
        None => 2.,
    };
    if !margin.is_finite() {
        return Err("Annotation canvas stroke overflow".into());
    }
    if draw
        .stroke_options
        .as_ref()
        .is_some_and(|o| o.join != 2 && o.join != 1)
    {
        return Err("Unsupported annotation canvas miter extent".into());
    }
    let mut shift = [0.; 2];
    let mut device = [0; 2];
    for axis in 0..2 {
        // Also translate far-positive local coordinates, avoiding a giant device box.
        shift[axis] = -(low[axis] - margin).floor();
        let extent = (high[axis] + margin + shift[axis]).ceil();
        if !extent.is_finite() || extent < 1. || extent > 1_000_000. {
            return Err("Annotation canvas extent exceeds budget".into());
        }
        device[axis] = extent as u32;
    }
    Ok((shift, device))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_formats::sheet_pdf::StrokeOptions;
    fn line(end: f64) -> Arc<Draws> {
        Arc::new(Draws {
            draws: vec![Draw {
                origin_mm: [0.; 2],
                rotation_deg: 0.,
                text: None,
                picture: None,
                path: vec![PathOp::Move([-12., -7.]), PathOp::Line([end, 9.])],
                stroke: Some(([0.1, 0.2, 0.3, 1.], 0.25)),
                stroke_options: Some(StrokeOptions {
                    cap: 2,
                    join: 2,
                    dash: vec![],
                }),
                fill: None,
                even_odd: false,
                clip: None,
                clip_intersection: None,
                flatten_transform: false,
            }],
            rectangles: vec![],
            source_lines: vec![0],
            source_points: vec![],
        })
    }
    #[test]
    fn integer_moves_reuse_geometry_and_invalid_edits_preserve_cache() {
        let ctx = egui::Context::default();
        let backend = ingetrazo_render::cad_callback::Factory::default();
        let factory = Factory::MetalProducer(backend.clone());
        let mut cache = Cache::default();
        let mut fonts = CanvasPainter::default();
        let draws = line(20.);
        let mut previous = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            for x in [50., 50., 75.] {
                let r = cache
                    .paint(
                        &p,
                        &factory,
                        &mut fonts,
                        (0, 0),
                        &draws,
                        2.,
                        Pos2::new(x, 50.),
                    )
                    .unwrap();
                assert!(r.min.x < x && r.min.y < 50., "negative leader was cropped");
                if x == 75. {
                    let before: Rect = previous.unwrap();
                    assert_eq!(r.min.x - before.min.x, 25.);
                }
                previous = Some(r);
            }
            assert_eq!(cache.stats().0, 1);
            let metrics = backend.metrics();
            assert!(metrics.handles > 0);
            assert_eq!(metrics.updates, metrics.handles);
            let owned = cache.items[&(0, 0)].parts[0].geometry.clone();
            let bytes = cache.stats().2;
            assert!(cache
                .paint(
                    &p,
                    &factory,
                    &mut fonts,
                    (0, 0),
                    &line(f64::NAN),
                    2.,
                    Pos2::new(75., 50.)
                )
                .is_err());
            assert!(Arc::ptr_eq(&owned, &cache.items[&(0, 0)].parts[0].geometry));
            assert_eq!(cache.stats().2, bytes);
            cache
                .paint(
                    &p,
                    &factory,
                    &mut fonts,
                    (0, 0),
                    &draws,
                    2.,
                    Pos2::new(75.25, 50.),
                )
                .unwrap();
            assert_eq!(
                cache.stats().0,
                2,
                "fractional pixel movement needs new coverage"
            );
        });
    }
    #[test]
    fn dense_page_keeps_idle_geometry_within_byte_budget() {
        let ctx = egui::Context::default();
        let backend = ingetrazo_render::cad_callback::Factory::default();
        let factory = Factory::MetalProducer(backend.clone());
        let mut cache = Cache::default();
        let mut fonts = CanvasPainter::default();
        let draws = line(20.);
        // More ordinary annotations than the former 128-entry ceiling must fit
        // without cold raster work on every traversal of a visible page.
        let mut cold = None;
        for pass in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                for i in 0..256 {
                    cache
                        .paint(
                            &painter,
                            &factory,
                            &mut fonts,
                            (0, i),
                            &draws,
                            2.,
                            Pos2::new(50. + (i % 16) as f32 * 30., 50. + (i / 16) as f32 * 30.),
                        )
                        .unwrap();
                }
            });
            let metrics = backend.metrics();
            let observed = (
                cache.stats().0,
                cache.stats().2,
                metrics.handles,
                metrics.updates,
            );
            if pass == 0 {
                cold = Some(observed);
            } else {
                assert_eq!(Some(observed), cold, "warm dense page rebuilt geometry");
            }
            assert!(cache.stats().2 <= CACHE_BYTES);
        }
        assert_eq!(cache.stats().0, 256);
        assert_eq!(cache.stats().1, 768);
    }
}
