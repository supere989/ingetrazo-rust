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
mod cad_abi;
mod glyph_admission;

#[cfg(target_os = "macos")]
pub mod native_input;

#[cfg(target_os = "macos")]
pub mod cad_callback_painter;

#[cfg(target_os = "macos")]
pub mod cad_compute_painter;

#[cfg(target_os = "macos")]
pub mod compute_canvas;

mod world_admission;
#[cfg(target_os = "macos")]
pub mod world;

pub mod world_policy;

pub mod world_placement;
pub mod world_scene;

pub mod world_surface;
mod world_face_guard;

#[cfg(target_os="macos")]
mod world_texture;

mod world_ghost;

mod world_preview;

mod world_background;
mod world_profile;
