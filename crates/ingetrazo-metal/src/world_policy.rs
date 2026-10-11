//! Immutable source opaque face-paint plan. No geometry projection or Scene copy.
use ingetrazo_render::buffer::MeshRenderBuffer;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Material,
    HiddenLine,
    Monochrome,
}
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub mode: Mode,
    pub front: [f32; 4],
    pub back: [f32; 4],
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            mode: Mode::Material,
            front: [1.; 4],
            back: [0.62, 0.70, 0.78, 1.],
        }
    }
}
impl Policy {
    pub(crate) fn validate(self) -> Result<(), String> {
        if self
            .front
            .iter()
            .chain(self.back.iter())
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self.front[3] != 1.
            || self.back[3] != 1.
        {
            return Err("Metal world opaque style color invalid".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub(crate) struct BackRun {
    pub first: usize,
    pub count: usize,
    pub color: [f32; 4],
}
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub default_back: Vec<(usize, usize)>,
    pub overrides: Vec<BackRun>,
}
impl Plan {
    pub(crate) fn from_packet(p: &MeshRenderBuffer) -> Result<Self, String> {
        if !p.source_front_cull.is_empty() {
            return Err(
                "Metal world translucent back/front-cull policy requires alpha stage".into(),
            );
        }
        let mut end = 0;
        for &(first, count) in &p.source_default_back {
            range(first, count, p.indices.len())?;
            if first < end {
                return Err("Metal world default-back spans overlap or are unordered".into());
            }
            end = first + count;
        }
        for paint in &p.source_back_paints {
            range(paint.first, paint.count, p.indices.len())?;
            if !paint.opacity.is_finite()
                || paint.opacity < 0.999
                || paint.opacity > 1.
                || paint
                    .color
                    .iter()
                    .any(|c| !c.is_finite() || !(0. ..=1.).contains(c))
            {
                return Err("Metal world back paint invalid or requires alpha stage".into());
            }
        }
        let mut ordered: Vec<_> = p.source_back_paints.iter().collect();
        ordered.sort_by_key(|p| p.order);
        Ok(Self {
            default_back: p.source_default_back.clone(),
            overrides: ordered
                .into_iter()
                .map(|p| BackRun {
                    first: p.first,
                    count: p.count,
                    color: [
                        quantize(p.color[0]),
                        quantize(p.color[1]),
                        quantize(p.color[2]),
                        1.,
                    ],
                })
                .collect(),
        })
    }
}
fn quantize(c: f32) -> f32 {
    (c * 64.).round_ties_even() / 64.
}
fn range(first: usize, count: usize, len: usize) -> Result<(), String> {
    if first % 3 != 0
        || count == 0
        || count % 3 != 0
        || first.checked_add(count).is_none_or(|end| end > len)
    {
        return Err("Metal world paint triangle span invalid".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_render::buffer::SourceBackPaint;
    #[test]
    fn opaque_back_plan_preserves_default_ranges_and_source_order() {
        let mut p = MeshRenderBuffer::default();
        p.indices = vec![0; 9];
        p.source_default_back = vec![(0, 3)];
        p.source_back_paints = vec![
            SourceBackPaint {
                first: 3,
                count: 3,
                order: 9,
                color: [0.2, 0.4, 0.6],
                opacity: 1.,
            },
            SourceBackPaint {
                first: 6,
                count: 3,
                order: 2,
                color: [1., 0., 0.],
                opacity: 1.,
            },
        ];
        let plan = Plan::from_packet(&p).unwrap();
        assert_eq!(plan.default_back, vec![(0, 3)]);
        assert_eq!(
            plan.overrides.iter().map(|r| r.first).collect::<Vec<_>>(),
            vec![6, 3]
        );
        assert_eq!(
            plan.overrides[1].color,
            [13. / 64., 26. / 64., 38. / 64., 1.]
        );
    }
    #[test]
    fn malformed_and_alpha_back_policies_refuse() {
        let mut p = MeshRenderBuffer::default();
        p.indices = vec![0; 3];
        p.source_default_back = vec![(0, 4)];
        assert!(Plan::from_packet(&p).is_err());
        p.source_default_back = vec![(0, 3), (0, 3)];
        assert!(Plan::from_packet(&p).is_err());
        p.source_default_back.clear();
        p.source_back_paints.push(SourceBackPaint {
            first: 0,
            count: 3,
            order: 0,
            color: [1., 0., 0.],
            opacity: 0.5,
        });
        assert!(Plan::from_packet(&p).is_err());
        p.source_back_paints[0].opacity = f64::NAN;
        assert!(Plan::from_packet(&p).is_err());
    }
    #[test]
    fn source_round_ties_even_keeps_adjacent_half_grid_colors() {
        assert_eq!(quantize(0.5 / 64.), 0.);
        assert_eq!(quantize(1.5 / 64.), 2. / 64.);
        assert_eq!(quantize(62.5 / 64.), 62. / 64.);
        assert_eq!(quantize(63.5 / 64.), 1.);
    }
    #[test]
    fn style_changes_validate_without_geometry_work() {
        let p = Policy {
            mode: Mode::Monochrome,
            front: [0.1, 0.2, 0.3, 1.],
            back: [0.4, 0.5, 0.6, 1.],
        };
        assert!(p.validate().is_ok());
        assert!(Policy {
            back: [0., 0., 0., 0.5],
            ..p
        }
        .validate()
        .is_err());
    }
}
