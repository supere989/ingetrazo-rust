//! Renderer-neutral immutable physical picture pixel ownership and admission.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
#[derive(Debug)]
pub struct Pixels {
    id: u64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl Pixels {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Arc<Self>, String> {
        let bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|v| v.checked_mul(4))
            .filter(|v| *v <= 256 * 1024 * 1024)
            .ok_or("Paper picture pixel admission exceeded")?;
        if width == 0
            || height == 0
            || width > 65535
            || height > 65535
            || bytes != rgba.len() as u64
            || rgba.capacity() > 256 * 1024 * 1024
        {
            return Err("Paper picture dimensions or pixel inventory invalid".into());
        }
        let id = NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "Paper picture identity exhausted")?;
        Ok(Arc::new(Self {
            id,
            width,
            height,
            rgba,
        }))
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn bytes(&self) -> usize {
        self.rgba.capacity() + std::mem::size_of::<Self>()
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
#[derive(Clone, Debug)]
pub struct Picture {
    pub image: Arc<Pixels>,
    pub rect_mm: [f64; 4],
}
impl Picture {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .rect_mm
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            || self.rect_mm[2] <= 0.
            || self.rect_mm[3] <= 0.
        {
            return Err("Invalid physical picture box".into());
        }
        Ok(())
    }
}
