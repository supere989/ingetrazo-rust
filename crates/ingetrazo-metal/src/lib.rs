//! Native Metal device and resource foundation, independent of Vulkan.
//! No portability translation, rendering wrapper or process is involved.
#[cfg(target_os = "macos")]
mod device;
#[cfg(target_os = "macos")]
pub use device::*;
