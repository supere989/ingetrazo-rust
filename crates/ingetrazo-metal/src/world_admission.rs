//! Cold immutable packet admission and constant-size per-frame pose admission.
use ingetrazo_render::buffer::MeshRenderBuffer;
pub(crate) fn pose(mvp: [[f32; 4]; 4], extent: (u32, u32), clear: [f64; 4]) -> Result<(), String> {
    if mvp.iter().flatten().any(|v| !v.is_finite())
        || clear
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        || extent.0 > 16384
        || extent.1 > 16384
    {
        return Err("Metal world pose/extent/clear invalid".into());
    }
    Ok(())
}
pub(crate) fn opaque(data: &MeshRenderBuffer) -> Result<super::world_policy::Plan, String> {
    let bytes = data
        .vertices
        .len()
        .checked_mul(std::mem::size_of::<ingetrazo_render::buffer::Vertex>())
        .and_then(|n| {
            data.indices
                .len()
                .checked_mul(4)
                .and_then(|i| n.checked_add(i))
        })
        .ok_or("Metal world packet size overflow")?;
    if bytes > 256 * 1024 * 1024 {
        return Err("Metal world immutable packet exceeds256MiB".into());
    }
    if data.indices.len() % 3 != 0
        || data
            .indices
            .iter()
            .any(|i| *i as usize >= data.vertices.len())
    {
        return Err("Metal world triangle/index invalid".into());
    }
    if data.vertices.iter().any(|v| {
        v.position
            .iter()
            .chain(v.normal.iter())
            .chain(v.color.iter())
            .chain(v.uv.iter())
            .any(|x| !x.is_finite())
            || v.color.iter().any(|x| !(0. ..=1.).contains(x))
            || v.color[3] != 1.
    }) {
        return Err("Metal world opaque vertex invalid".into());
    }
    if !data.source_opacity_runs.is_empty() || !data.source_texture_runs.is_empty() {
        return Err("Metal world source paint policies require subsequent viewport stage".into());
    }
    super::world_policy::Plan::from_packet(data)
}
#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_render::buffer::Vertex;
    fn packet() -> MeshRenderBuffer {
        let mut p = MeshRenderBuffer::default();
        p.vertices = vec![
            Vertex {
                position: [0.; 3],
                normal: [0., 0., 1.],
                color: [1.; 4],
                uv: [0.; 2]
            };
            3
        ];
        p.indices = vec![0, 1, 2];
        p
    }
    #[test]
    fn vertex_abi_matches_packed_msl() {
        assert_eq!(std::mem::size_of::<Vertex>(), 48);
        assert_eq!(std::mem::offset_of!(Vertex, color), 24);
        assert_eq!(std::mem::offset_of!(Vertex, uv), 40);
    }
    #[test]
    fn cold_packet_rejects_invalid_and_unsupported_without_repair() {
        let p = packet();
        assert!(opaque(&p).is_ok());
        let mut p = packet();
        p.indices[2] = 3;
        assert!(opaque(&p).is_err());
        let mut p = packet();
        p.vertices[0].position[0] = f32::NAN;
        assert!(opaque(&p).is_err());
        let mut p = packet();
        p.vertices[0].color[3] = 0.5;
        assert!(opaque(&p).is_err());
        let mut p = packet();
        p.source_default_back.push((0, 4));
        assert!(opaque(&p).is_err());
    }
    #[test]
    fn warm_pose_still_rejects_bad_camera_and_extent() {
        let mut m = [[0.; 4]; 4];
        assert!(pose(m, (800, 600), [1.; 4]).is_ok());
        m[2][1] = f32::INFINITY;
        assert!(pose(m, (800, 600), [1.; 4]).is_err());
        assert!(pose([[0.; 4]; 4], (16385, 600), [1.; 4]).is_err());
    }
}

/// Cold standalone allocation admission; heap layout is not a standalone size bound.
/// Reserves all available logical capacity, never guesses a texture byte multiplier.
pub(crate) fn standalone_capacity(b: &ingetrazo_core::gpu_memory::MemoryBudget) -> Result<u64, String> {
    let h = b.heaps.first().ok_or("Metal memory heap missing")?;
    let own = h.committed.checked_add(h.reserved).ok_or("Metal heap accounting overflow")?;
    let mut available = h.limit.saturating_sub(own);
    if let Some(live) = h.live_budget {
        available = available.min(live.saturating_sub(h.live_usage.saturating_sub(h.committed)).saturating_sub(own));
    }
    if let Some(limit) = b.total_limit {
        let total = b.heaps.iter().try_fold(0u64, |n,h| n.checked_add(h.committed)?.checked_add(h.reserved)).ok_or("Metal total accounting overflow")?;
        available = available.min(limit.saturating_sub(total));
    }
    Ok(available)
}
#[cfg(test)]
mod standalone_tests {
    use super::*;
    #[test]
    fn respects_reservations_global_and_external_usage() {
        let mut b=ingetrazo_core::gpu_memory::MemoryBudget::new([(1000,900)]);
        let t=b.reserve(0,100).unwrap();b.commit(t,1).unwrap();
        let pending=b.reserve(0,200).unwrap();
        assert_eq!(standalone_capacity(&b).unwrap(),600);
        b.total_limit=Some(500);assert_eq!(standalone_capacity(&b).unwrap(),200);
        b.update_live(0,450,200).unwrap();assert_eq!(standalone_capacity(&b).unwrap(),50);
        b.cancel(pending).unwrap();assert_eq!(standalone_capacity(&b).unwrap(),250);
        let all=standalone_capacity(&b).unwrap();
        let ticket=b.reserve(0,all).unwrap();
        assert!(b.reserve(0,1).is_err());
        b.cancel(ticket).unwrap();
        assert_eq!(standalone_capacity(&b).unwrap(),all);
        b.total_limit=Some(100);assert_eq!(standalone_capacity(&b).unwrap(),0);
    }
}
