//! Bounded persistent RGBA canvas owners and one direct BGRA presentation draw.
//! No submission/wait/readback; a caller frame must retain its Lease until completion.
use crate::NativeMetal;
use ingetrazo_core::gpu_memory::MemoryBudget;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::{
    mem::ManuallyDrop,
    sync::{Arc, Mutex},
};
struct Image {
    raw: ManuallyDrop<Retained<ProtocolObject<dyn MTLTexture>>>,
    budget: Arc<Mutex<MemoryBudget>>,
    handle: u64,
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.raw) };
        if let Ok(mut b) = self.budget.lock() {
            let _ = b.freed(self.handle);
        }
    }
}
pub struct Lease {
    image: Arc<Image>,
    pipeline: Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
}
#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub allocations: u64,
    pub allocation_bytes: u64,
    pub leases: u64,
    pub reused: u64,
}
#[derive(Default)]
pub struct Pool {
    images: Vec<Arc<Image>>,
    pipeline: Option<Arc<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>>,
    pub stats: Stats,
}
impl Pool {
    pub fn acquire(
        &mut self,
        metal: &NativeMetal,
        extent: (u32, u32),
    ) -> Result<Option<Arc<Lease>>, String> {
        if metal.poisoned.load(std::sync::atomic::Ordering::Acquire){return Err("Metal canvas admission poisoned".into());}
        if !crate::cad_compute_painter::Painter::supported(metal) {
            return Err("Exact compute canvas requires native read/write tier2".into());
        }
        if extent.0 == 0 || extent.1 == 0 {
            return Ok(None);
        }
        if extent.0 > 16384 || extent.1 > 16384 {
            return Err("Metal compute canvas extent exceeds16384".into());
        }
        // Drop only idle mismatched owners. Prepared/in-flight leases preserve images.
        self.images.retain(|image| {
            Arc::strong_count(image) > 1
                || (image.raw.width() == extent.0 as usize
                    && image.raw.height() == extent.1 as usize)
        });
        let cached = self
            .images
            .iter()
            .find(|image| {
                Arc::strong_count(image) == 1
                    && image.raw.width() == extent.0 as usize
                    && image.raw.height() == extent.1 as usize
            })
            .cloned();
        if cached.is_none() && self.images.len() >= 3 {
            return Ok(None);
        }
        let pipeline = if let Some(p) = &self.pipeline {
            p.clone()
        } else {
            let library = metal
                .device
                .newLibraryWithSource_options_error(
                    &NSString::from_str(include_str!("shaders/compute_canvas.metal")),
                    None,
                )
                .map_err(|e| format!("Metal canvas presentation shader:{e:?}"))?;
            let descriptor = MTLRenderPipelineDescriptor::new();
            let vertex=library.newFunctionWithName(&NSString::from_str("canvas_vertex")).ok_or("Canvas vertex missing")?;
            let fragment=library.newFunctionWithName(&NSString::from_str("canvas_fragment")).ok_or("Canvas fragment missing")?;
            descriptor.setVertexFunction(Some(&vertex));descriptor.setFragmentFunction(Some(&fragment));
            let color = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
            color.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            color.setBlendingEnabled(false);
            Arc::new(
                metal
                    .device
                    .newRenderPipelineStateWithDescriptor_error(&descriptor)
                    .map_err(|e| format!("Metal canvas presentation pipeline:{e:?}"))?,
            )
        };
        let mut allocated = 0;
        let image = if let Some(image) = cached {
            image
        } else {
            let request = u64::from(extent.0) * u64::from(extent.1) * 4;
            let ticket = metal
                .budget
                .lock()
                .map_err(|_| "Metal budget poisoned")?
                .reserve(0, request)?;
            let desc = unsafe {
                MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                    MTLPixelFormat::RGBA8Unorm,
                    extent.0 as usize,
                    extent.1 as usize,
                    false,
                )
            };
            desc.setStorageMode(MTLStorageMode::Private);
            desc.setHazardTrackingMode(MTLHazardTrackingMode::Tracked);
            desc.setUsage(
                MTLTextureUsage::RenderTarget
                    | MTLTextureUsage::ShaderRead
                    | MTLTextureUsage::ShaderWrite,
            );
            let Some(raw) = metal.device.newTextureWithDescriptor(&desc) else {
                metal.budget.lock().unwrap().cancel(ticket)?;
                return Err("Metal compute canvas allocation failed".into());
            };
            let handle =
                (&*raw as *const ProtocolObject<dyn MTLTexture>).cast::<()>() as usize as u64;
            allocated = MTLResource::allocatedSize(&*raw) as u64;
            let mut budget = metal.budget.lock().map_err(|_| "Metal budget poisoned")?;
            if allocated != request {
                budget.cancel(ticket)?;
                let actual = budget.reserve(0, allocated)?;
                budget.commit(actual, handle)?;
            } else {
                budget.commit(ticket, handle)?;
            }
            drop(budget);
            Arc::new(Image {
                raw: ManuallyDrop::new(raw),
                budget: metal.budget.clone(),
                handle,
            })
        };
        if allocated != 0 {
            self.images.push(image.clone());
            self.stats.allocations += 1;
            self.stats.allocation_bytes += allocated;
        } else {
            self.stats.reused += 1;
        }
        self.pipeline = Some(pipeline.clone());
        self.stats.leases += 1;
        Ok(Some(Arc::new(Lease { image, pipeline })))
    }
    pub fn clear_idle(&mut self) {
        self.images.retain(|image| Arc::strong_count(image) > 1);
    }
}
impl Lease {
    pub fn texture(&self) -> &ProtocolObject<dyn MTLTexture> {
        &self.image.raw
    }
    pub fn allocated_bytes(&self) -> u64 {
        MTLResource::allocatedSize(&**self.image.raw) as u64
    }
    pub fn present(
        &self,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
        drawable: &ProtocolObject<dyn MTLTexture>,
    ) -> Result<(), String> {
        if drawable.pixelFormat() != MTLPixelFormat::BGRA8Unorm
            || drawable.width() != self.image.raw.width()
            || drawable.height() != self.image.raw.height()
        {
            return Err("Metal canvas drawable format/extent mismatch".into());
        }
        let pass = MTLRenderPassDescriptor::new();
        let color = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        color.setTexture(Some(drawable));
        color.setLoadAction(MTLLoadAction::DontCare);
        color.setStoreAction(MTLStoreAction::Store);
        let encoder = command
            .renderCommandEncoderWithDescriptor(&pass)
            .ok_or("Metal canvas presentation encoder missing")?;
        encoder.setRenderPipelineState(&self.pipeline);
        unsafe {
            encoder.setFragmentTexture_atIndex(Some(&self.image.raw), 0);
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        encoder.endEncoding();
        Ok(())
    }
}
