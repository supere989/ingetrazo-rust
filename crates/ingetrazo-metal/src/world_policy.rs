//! Immutable source opaque face-paint plan. No geometry projection or Scene copy.
use ingetrazo_render::buffer::MeshRenderBuffer;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Material,
    /// Source shaded mode: cached Qt-compatible texture mean, no image alpha.
    MaterialAverage,
    HiddenLine,
    Monochrome,
}
impl Mode {
    /// Operative viewport.py: only the opaque front `_tex_runs` take the
    /// shaded image-mean pass. Back overrides and glass still use textures.
    pub(crate) fn texture_mean(self,opacity:f64,back_side:bool)->bool {
        self==Self::MaterialAverage && opacity>=0.999 && !back_side
    }
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
    pub retained_payload_bytes: usize,
    pub default_back: Vec<(usize, usize)>,
    pub overrides: Vec<BackRun>,
}
const MAX_POLICY_PAYLOAD: usize = 8 * 1024 * 1024;
fn payload_bound(defaults: usize, paints: usize) -> Result<usize, String> {
    defaults.checked_mul(std::mem::size_of::<(usize,usize)>())
        .and_then(|n| paints.checked_mul(std::mem::size_of::<BackRun>() + std::mem::size_of::<(usize, &ingetrazo_render::buffer::SourceBackPaint)>()).and_then(|p|n.checked_add(p)))
        .and_then(|n|n.checked_add(std::mem::size_of::<Plan>() + std::mem::size_of::<Vec<(usize, &ingetrazo_render::buffer::SourceBackPaint)>>()))
        .filter(|n|*n<=MAX_POLICY_PAYLOAD)
        .ok_or_else(||"Metal world cold paint metadata exceeds8MiB payload admission".into())
}
impl Plan {
    pub(crate) fn from_packet(p: &MeshRenderBuffer) -> Result<Self, String> {
        if !p.source_front_cull.is_empty() {
            return Err(
                "Metal world translucent back/front-cull policy requires alpha stage".into(),
            );
        }
        payload_bound(p.source_default_back.len(),p.source_back_paints.len())?;
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
        let mut ordered = Vec::new();
        ordered.try_reserve_exact(p.source_back_paints.len()).map_err(|_|"Metal world paint ordering allocation refused")?;
        ordered.extend(p.source_back_paints.iter().enumerate());
        // Explicit original position tie-break preserves source order without
        // stable-sort scratch allocation. Only this bounded cold vector sorts.
        ordered.sort_unstable_by_key(|(index,p)| (p.order,*index));
        let mut default_back=Vec::new();
        default_back.try_reserve_exact(p.source_default_back.len()).map_err(|_|"Metal world default-back allocation refused")?;
        default_back.extend_from_slice(&p.source_default_back);
        let mut overrides=Vec::new();
        overrides.try_reserve_exact(ordered.len()).map_err(|_|"Metal world back override allocation refused")?;
        // Allocator metadata is excluded; account actual retained capacities and
        // simultaneous ordering payload before immutable plan publication.
        let retained_payload_bytes=default_back.capacity().checked_mul(std::mem::size_of::<(usize,usize)>())
            .and_then(|n|overrides.capacity().checked_mul(std::mem::size_of::<BackRun>()).and_then(|m|n.checked_add(m)))
            .and_then(|n|n.checked_add(std::mem::size_of::<Plan>())).ok_or("Metal world retained paint capacity overflow")?;
        let temporary=ordered.capacity().checked_mul(std::mem::size_of::<(usize,&ingetrazo_render::buffer::SourceBackPaint)>())
            .and_then(|n|n.checked_add(std::mem::size_of_val(&ordered))).ok_or("Metal world ordering capacity overflow")?;
        if retained_payload_bytes.checked_add(temporary).is_none_or(|n|n>MAX_POLICY_PAYLOAD) {
            return Err("Metal world actual paint capacities exceed8MiB payload admission".into());
        }
        overrides.extend(ordered.into_iter().map(|(_,p)|BackRun {first:p.first,count:p.count,
            color:[quantize(p.color[0]),quantize(p.color[1]),quantize(p.color[2]),1.]}));
        Ok(Self {retained_payload_bytes,default_back,overrides})
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
    fn metadata_admits_before_allocation_and_accounts_retained_capacity() {
        assert!(payload_bound(usize::MAX,0).is_err());
        assert!(payload_bound(0,MAX_POLICY_PAYLOAD).is_err());
        let mut p=MeshRenderBuffer::default();p.indices=vec![0;3];p.source_default_back=vec![(0,3)];
        let prior=Plan::from_packet(&p).unwrap();
        assert_eq!(prior.retained_payload_bytes,std::mem::size_of::<Plan>()+std::mem::size_of::<(usize,usize)>());
        p.source_back_paints=vec![ingetrazo_render::buffer::SourceBackPaint { first:0,count:3,order:0,color:[1.,0.,0.],opacity:1. }; MAX_POLICY_PAYLOAD / (std::mem::size_of::<BackRun>() + std::mem::size_of::<(usize,&ingetrazo_render::buffer::SourceBackPaint)>())+1];
        assert!(Plan::from_packet(&p).unwrap_err().contains("metadata exceeds"));
        p.source_back_paints.clear();p.source_default_back=vec![(0,4)];assert!(Plan::from_packet(&p).is_err());
        assert_eq!(prior.default_back,vec![(0,3)]);
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

#[cfg(test)]mod mean_policy_tests{
 use super::Mode;
 #[test]fn source_shaded_mean_does_not_replace_glass_or_back_texture_samples(){
  assert!(Mode::MaterialAverage.texture_mean(1.,false));
  assert!(Mode::MaterialAverage.texture_mean(0.999,false));
  assert!(!Mode::MaterialAverage.texture_mean(0.998999,false));
  assert!(!Mode::MaterialAverage.texture_mean(1.,true));
  assert!(!Mode::Material.texture_mean(1.,false));
 }
}
