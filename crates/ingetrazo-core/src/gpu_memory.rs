//! Backend-neutral memory admission. Counts real allocation requirements, not payload sizes.
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct HeapBudget {
    pub capacity: u64,
    pub limit: u64,
    pub live_budget: Option<u64>,
    pub live_usage: u64,
    pub reserved: u64,
    pub committed: u64,
}
#[derive(Clone, Debug)]
struct Allocation {
    heap: usize,
    bytes: u64,
    retire_after: Option<u64>,
}
#[derive(Debug)]
pub struct MemoryBudget {
    pub heaps: Vec<HeapBudget>,
    pub total_limit: Option<u64>,
    pub peak_committed: u64,
    pub peak_admitted: u64,
    next: u64,
    reservations: HashMap<u64, Allocation>,
    allocations: HashMap<u64, Allocation>,
}
impl MemoryBudget {
    pub fn new(heaps: impl IntoIterator<Item = (u64, u64)>) -> Self {
        Self {
            heaps: heaps
                .into_iter()
                .map(|(capacity, limit)| HeapBudget {
                    capacity,
                    limit: limit.min(capacity),
                    live_budget: None,
                    live_usage: 0,
                    reserved: 0,
                    committed: 0,
                })
                .collect(),
            total_limit: None,
            peak_committed: 0,
            peak_admitted: 0,
            next: 1,
            reservations: HashMap::new(),
            allocations: HashMap::new(),
        }
    }
    pub fn update_live(&mut self, heap: usize, budget: u64, usage: u64) -> Result<(), String> {
        let h = self.heaps.get_mut(heap).ok_or("Unknown memory heap")?;
        h.live_budget = Some(budget.min(h.capacity));
        h.live_usage = usage;
        Ok(())
    }
    pub fn reserve(&mut self, heap: usize, bytes: u64) -> Result<u64, String> {
        if bytes == 0 {
            return Err("Zero-byte allocation".into());
        }
        let total = self
            .heaps
            .iter()
            .try_fold(bytes, |n, h| {
                n.checked_add(h.committed)?.checked_add(h.reserved)
            })
            .ok_or("Global allocation size overflow")?;
        if let Some(limit) = self.total_limit {
            if total > limit {
                return Err(format!(
                    "GPU allocation {bytes} exceeds global application budget {limit}"
                ));
            }
        }
        let h = self.heaps.get_mut(heap).ok_or("Unknown memory heap")?;
        let own = h
            .committed
            .checked_add(h.reserved)
            .and_then(|n| n.checked_add(bytes))
            .ok_or("Allocation size overflow")?;
        if own > h.limit {
            return Err(format!("GPU heap {heap}: allocation {bytes} exceeds application budget {}; reserved {}; committed {}",h.limit,h.reserved,h.committed));
        }
        if let Some(budget) = h.live_budget {
            let external = h.live_usage.saturating_sub(h.committed);
            if external.checked_add(own).ok_or("Heap usage overflow")? > budget {
                return Err(format!("GPU heap {heap}: allocation {bytes} exceeds live budget {budget}; driver usage {}",h.live_usage));
            }
        }
        let ticket = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or("Reservation identifier overflow")?;
        h.reserved += bytes;
        self.reservations.insert(
            ticket,
            Allocation {
                heap,
                bytes,
                retire_after: None,
            },
        );
        self.peak_admitted = self.peak_admitted.max(total);
        Ok(ticket)
    }
    pub fn cancel(&mut self, ticket: u64) -> Result<(), String> {
        let a = self
            .reservations
            .remove(&ticket)
            .ok_or("Unknown reservation")?;
        self.heaps[a.heap].reserved -= a.bytes;
        Ok(())
    }
    pub fn commit(&mut self, ticket: u64, handle: u64) -> Result<(), String> {
        if self.allocations.contains_key(&handle) {
            return Err("Allocation handle already tracked".into());
        }
        let a = self
            .reservations
            .remove(&ticket)
            .ok_or("Unknown reservation")?;
        let h = &mut self.heaps[a.heap];
        h.reserved -= a.bytes;
        h.committed += a.bytes;
        self.allocations.insert(handle, a);
        self.peak_committed = self.peak_committed.max(self.total_committed());
        Ok(())
    }
    /// Marks last use; bytes stay charged until caller destroys memory after that fence.
    pub fn retire(&mut self, handle: u64, fence_serial: u64) -> Result<(), String> {
        let a = self
            .allocations
            .get_mut(&handle)
            .ok_or("Unknown allocation")?;
        a.retire_after = Some(a.retire_after.unwrap_or(0).max(fence_serial));
        Ok(())
    }
    pub fn releasable(&self, completed_serial: u64) -> Vec<u64> {
        self.allocations
            .iter()
            .filter_map(|(&id, a)| {
                a.retire_after
                    .filter(|s| *s <= completed_serial)
                    .map(|_| id)
            })
            .collect()
    }
    /// Call only after the actual API free. Retirement alone does not reduce accounting.
    pub fn freed(&mut self, handle: u64) -> Result<(), String> {
        let a = self
            .allocations
            .remove(&handle)
            .ok_or("Unknown allocation")?;
        self.heaps[a.heap].committed -= a.bytes;
        Ok(())
    }
    pub fn total_committed(&self) -> u64 {
        self.heaps.iter().map(|h| h.committed).sum()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TextureStorage {
    Rgba8,
    Bc7,
    Bc5,
}
/// Logical transfer bytes for mip levels. Vulkan allocation admission still uses memory requirements.
pub fn texture_mip_bytes(
    width: u32,
    height: u32,
    depth: u32,
    layers: u32,
    mips: u32,
    format: TextureStorage,
) -> Result<u64, String> {
    if width == 0 || height == 0 || depth == 0 || layers == 0 || mips == 0 {
        return Err("Texture dimensions and mip count must be positive".into());
    }
    let max_mips = 32 - width.max(height).max(depth).leading_zeros();
    if mips > max_mips {
        return Err("Too many mip levels".into());
    }
    let mut total = 0u64;
    for mip in 0..mips {
        let w = (width >> mip).max(1) as u64;
        let h = (height >> mip).max(1) as u64;
        let d = (depth >> mip).max(1) as u64;
        let (x, y, b) = match format {
            TextureStorage::Rgba8 => (w, h, 4),
            TextureStorage::Bc7 | TextureStorage::Bc5 => (w.div_ceil(4), h.div_ceil(4), 16),
        };
        let bytes = x
            .checked_mul(y)
            .and_then(|n| n.checked_mul(d))
            .and_then(|n| n.checked_mul(layers as u64))
            .and_then(|n| n.checked_mul(b))
            .ok_or("Texture byte size overflow")?;
        total = total
            .checked_add(bytes)
            .ok_or("Texture mip chain overflow")?;
    }
    Ok(total)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_aligned_requirements_rollback_and_fence_delayed_free() {
        let mut b = MemoryBudget::new([(1024, 768)]);
        let ticket = b.reserve(0, 512).unwrap();
        assert!(b.reserve(0, 257).is_err());
        b.cancel(ticket).unwrap();
        assert_eq!(b.heaps[0].reserved, 0);
        let ticket = b.reserve(0, 512).unwrap();
        b.commit(ticket, 41).unwrap();
        b.retire(41, 9).unwrap();
        assert!(b.releasable(8).is_empty());
        assert_eq!(b.releasable(9), vec![41]);
        assert!(b.reserve(0, 257).is_err());
        assert_eq!(b.total_committed(), 512);
        b.freed(41).unwrap();
        assert_eq!(b.total_committed(), 0);
        assert!(b.reserve(0, 768).is_ok());
    }
    #[test]
    fn live_driver_usage_not_double_counted_and_external_pressure_blocks() {
        let mut b = MemoryBudget::new([(4096, 4096)]);
        let t = b.reserve(0, 1024).unwrap();
        b.commit(t, 1).unwrap();
        b.update_live(0, 2048, 1024).unwrap();
        assert!(b.reserve(0, 1024).is_ok());
        b.update_live(0, 2048, 1800).unwrap();
        assert!(b.reserve(0, 1).is_err());
    }
    #[test]
    fn invalid_requests_do_not_change_accounting() {
        let mut b = MemoryBudget::new([(1024, 1024)]);
        assert!(b.reserve(0, 0).is_err());
        assert!(b.reserve(9, 1).is_err());
        assert!(b.reserve(0, u64::MAX).is_err());
        assert_eq!(b.heaps[0].reserved, 0);
        assert_eq!(b.total_committed(), 0);
    }
    #[test]
    fn compressed_odd_dimensions_layers_and_tiny_mips_use_complete_blocks() {
        assert_eq!(
            texture_mip_bytes(7, 5, 1, 2, 3, TextureStorage::Bc7).unwrap(),
            192
        );
        assert_eq!(
            texture_mip_bytes(7, 5, 1, 2, 3, TextureStorage::Bc5).unwrap(),
            192
        );
        assert_eq!(
            texture_mip_bytes(7, 5, 1, 1, 3, TextureStorage::Rgba8).unwrap(),
            168
        );
        assert!(texture_mip_bytes(1, 1, 1, 1, 2, TextureStorage::Bc7).is_err());
        assert!(texture_mip_bytes(
            u32::MAX,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            1,
            TextureStorage::Rgba8
        )
        .is_err());
    }
}

#[cfg(test)]
mod global_limit_regression {
    use super::*;
    #[test]
    fn global_limit_includes_other_heaps_reservations_and_retired_allocations() {
        let mut b = MemoryBudget::new([(1000, 1000), (1000, 1000)]);
        b.total_limit = Some(512);
        let t = b.reserve(0, 256).unwrap();
        let u = b.reserve(1, 256).unwrap();
        assert!(b.reserve(1, 1).is_err());
        b.commit(t, 11).unwrap();
        b.retire(11, 4).unwrap();
        b.cancel(u).unwrap();
        assert!(b.reserve(1, 257).is_err());
        b.freed(11).unwrap();
        assert!(b.reserve(1, 512).is_ok());
    }
}
