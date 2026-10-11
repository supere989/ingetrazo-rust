// SPDX-License-Identifier: GPL-3.0-or-later
//! Vulkan admission of the shared source camera-facing packet.
use ingetrazo_core::{scene::Scene,camera::OrbitCamera};
use ingetrazo_render::buffer::TextureBuffers;
pub(super) struct Packet(ingetrazo_render::billboards::Packet);
impl std::ops::Deref for Packet { type Target=ingetrazo_render::billboards::Packet;fn deref(&self)->&Self::Target{&self.0} }
impl Packet {
 pub(super) fn build(scene:&Scene)->Result<Self,String>{ingetrazo_render::billboards::Packet::build(scene).map(Self)}
 pub(super) fn textures(&self,camera:&OrbitCamera)->Result<TextureBuffers,String>{
  let snapshot=crate::navigation_ray::camera_snapshot(camera).ok_or("Billboard camera admission")?;
  self.0.textures(camera,glam::Vec3::from_array(snapshot.eye))
 }
}
pub(super) fn key(scene:&Scene,camera:&OrbitCamera,epoch:u64)->Result<([u64;3],[u32;7]),String>{
 let snapshot=crate::navigation_ray::camera_snapshot(camera).ok_or("Billboard camera admission")?;let target=camera.target.as_vec3().to_array();Ok(([scene.version,scene.view_version,epoch],[u32::from(camera.perspective),snapshot.eye[0].to_bits(),snapshot.eye[1].to_bits(),snapshot.eye[2].to_bits(),target[0].to_bits(),target[1].to_bits(),target[2].to_bits()]))
}

