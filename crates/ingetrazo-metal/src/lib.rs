//! Native Metal device and resource foundation, independent of Vulkan.
//! No portability translation, rendering wrapper or process is involved.
#[cfg(target_os = "macos")]
mod device;
#[cfg(target_os = "macos")]
pub use device::*;

#[cfg(target_os = "macos")]
mod submission;
#[cfg(target_os = "macos")]
pub use submission::*;

#[cfg(target_os = "macos")]
pub mod presentation;
#[cfg(target_os = "macos")]
pub mod egui_painter;
#[cfg(target_os = "macos")]
mod cad2d;
#[cfg(target_os = "macos")]
pub use cad2d::*;
mod span_plan;

#[cfg(target_os = "macos")]
pub mod native_input;

pub mod cad_callback_painter;

#[cfg(target_os = "macos")]
pub mod cad_compute_painter;
