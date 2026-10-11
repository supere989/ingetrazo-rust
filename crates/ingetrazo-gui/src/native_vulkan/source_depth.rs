//! Native OpenGL clip-depth domain, negotiated once per device.
use ash::{vk,Instance};
use std::ffi::CStr;
pub(super) fn negotiate(instance:&Instance,physical:vk::PhysicalDevice,extensions:&[vk::ExtensionProperties])->(bool,vk::PhysicalDeviceDepthClipControlFeaturesEXT<'static>,vk::PhysicalDeviceExtendedDynamicState3FeaturesEXT<'static>){
 let has=|name:&CStr|extensions.iter().any(|e|unsafe{CStr::from_ptr(e.extension_name.as_ptr())}==name);
 let mut clip=vk::PhysicalDeviceDepthClipControlFeaturesEXT::default();let mut dynamic=vk::PhysicalDeviceExtendedDynamicState3FeaturesEXT::default();
 if has(ash::ext::depth_clip_control::NAME)&&has(ash::ext::extended_dynamic_state3::NAME){let mut query=vk::PhysicalDeviceFeatures2::default().push_next(&mut clip).push_next(&mut dynamic);unsafe{instance.get_physical_device_features2(physical,&mut query);}}
 let enabled=clip.depth_clip_control!=0&&dynamic.extended_dynamic_state3_depth_clip_negative_one_to_one!=0&&std::env::var_os("INGETRAZO_DIAGNOSTIC_PORTABLE_SOURCE_DEPTH").is_none();
 // Enable only the state actually used; querying other available feature bits
 // does not authorize the consumer to use them.
 clip=vk::PhysicalDeviceDepthClipControlFeaturesEXT::default().depth_clip_control(enabled);
 dynamic=vk::PhysicalDeviceExtendedDynamicState3FeaturesEXT::default().extended_dynamic_state3_depth_clip_negative_one_to_one(enabled);
 (enabled,clip,dynamic)
}

/// Portable homogeneous GL [-w,w] to Vulkan [0,w] clip-depth conversion.
/// This path is separately observable because it rounds the Z row before
/// vertex multiplication, unlike native viewport depth mapping.
pub(super) fn portable_matrix(mut matrix:[f32;16])->Result<[f32;16],String>{
 for c in 0..4{matrix[c*4+2]=(matrix[c*4+2]+matrix[c*4+3])*0.5;}
 if !matrix.iter().all(|v|v.is_finite()){return Err("Portable GL depth matrix arithmetic admission".into());}Ok(matrix)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn portable_depth_admits_full_gl_volume_and_refuses_overflow(){let m=portable_matrix(glam::Mat4::IDENTITY.to_cols_array()).unwrap();for(z,w,expected)in [(-1.,1.,0.),(0.,1.,0.5),(1.,1.,1.),(-2.,2.,0.),(2.,2.,2.)]{let p=glam::Mat4::from_cols_array(&m)*glam::Vec4::new(0.,0.,z,w);assert_eq!(p.z,expected);assert!((0.0..=p.w).contains(&p.z));}let mut bad=[0.;16];bad[2]=f32::MAX;bad[3]=f32::MAX;assert!(portable_matrix(bad).is_err());}
}
