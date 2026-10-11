//! Shared original float camera arithmetic; the public GUI API is retained.
pub use ingetrazo_core::source_camera::{CameraSnapshot, camera_snapshot, cursor_ray, plane_hit};
pub(crate) use ingetrazo_core::source_camera::affine_inverse;
