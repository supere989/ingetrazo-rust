//! Static backend selection at the composer boundary. Vulkan keeps its exact
//! existing production handle/painter. Metal producer payloads require a native
//! consuming painter; merely creating these callbacks proves no rendering.
use ingetrazo_render::cad2d::{Batch, Geometry, Pose};
use std::sync::Arc;
#[derive(Clone)]
pub enum Factory {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    Vulkan(crate::native_vulkan::cad2d::Factory),
    MetalProducer(ingetrazo_render::cad_callback::Factory),
}
#[derive(Clone)]
pub enum Handle {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    Vulkan(crate::native_vulkan::cad2d::Handle),
    MetalProducer(Arc<ingetrazo_render::cad_callback::Callback>),
}
#[cfg(any(target_os = "linux", target_os = "windows"))]
impl From<crate::native_vulkan::cad2d::Factory> for Factory {
    fn from(factory: crate::native_vulkan::cad2d::Factory) -> Self {
        Self::Vulkan(factory)
    }
}
impl Factory {
    pub fn handle(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<Handle, String> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Vulkan(factory) => factory.handle(geometry, batches, pose).map(Handle::Vulkan),
            Self::MetalProducer(factory) => factory
                .handle(geometry, batches, pose)
                .map(Handle::MetalProducer),
        }
    }
}
impl Handle {
    pub fn update(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<(), String> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Vulkan(handle) => handle.update(geometry, batches, pose),
            Self::MetalProducer(handle) => handle.update(geometry, batches, pose),
        }
    }
    pub fn shape(&self, rect: egui::Rect) -> egui::Shape {
        match self {
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Vulkan(handle) => handle.shape(rect),
            Self::MetalProducer(handle) => egui::PaintCallback {
                rect,
                callback: handle.clone(),
            }
            .into(),
        }
    }
}
