use ash::vk;
#[derive(Debug,PartialEq,Eq)] pub struct Queues {pub graphics:u32,pub present:u32,pub compute:Option<u32>}
pub fn select(families:&[vk::QueueFamilyProperties],present:&[bool])->Option<Queues> {
 if families.len()!=present.len(){return None;}
 let usable=|i:usize|families[i].queue_count>0;
 let graphics=(0..families.len()).find(|&i|usable(i)&&families[i].queue_flags.contains(vk::QueueFlags::GRAPHICS)&&present[i])
  .or_else(||(0..families.len()).find(|&i|usable(i)&&families[i].queue_flags.contains(vk::QueueFlags::GRAPHICS)))?;
 let p=if present[graphics]{graphics}else{(0..families.len()).find(|&i|usable(i)&&present[i])?};
 let compute=(0..families.len()).find(|&i|usable(i)&&families[i].queue_flags.contains(vk::QueueFlags::COMPUTE)&&!families[i].queue_flags.contains(vk::QueueFlags::GRAPHICS))
  .or_else(||(0..families.len()).find(|&i|usable(i)&&families[i].queue_flags.contains(vk::QueueFlags::COMPUTE)));
 Some(Queues {graphics:graphics as u32,present:p as u32,compute:compute.map(|i|i as u32)})
}
pub fn rank(kind:vk::PhysicalDeviceType)->u8 {match kind {vk::PhysicalDeviceType::DISCRETE_GPU=>5,vk::PhysicalDeviceType::INTEGRATED_GPU=>4,vk::PhysicalDeviceType::VIRTUAL_GPU=>3,vk::PhysicalDeviceType::CPU=>1,_=>2}}
#[cfg(test)] mod tests {use super::*;fn f(n:u32,flags:vk::QueueFlags)->vk::QueueFamilyProperties {vk::QueueFamilyProperties {queue_count:n,queue_flags:flags,..Default::default()}}
 #[test]fn sparse_split_graphics_present_preserve_real_indices(){assert_eq!(select(&[f(0,vk::QueueFlags::GRAPHICS),f(1,vk::QueueFlags::GRAPHICS),f(1,vk::QueueFlags::TRANSFER),f(1,vk::QueueFlags::COMPUTE)],&[false,false,true,false]),Some(Queues{graphics:1,present:2,compute:Some(3)}));}
 #[test]fn combined_and_dedicated_compute_preferred(){assert_eq!(select(&[f(1,vk::QueueFlags::GRAPHICS),f(1,vk::QueueFlags::GRAPHICS|vk::QueueFlags::COMPUTE),f(1,vk::QueueFlags::COMPUTE)],&[false,true,false]),Some(Queues{graphics:1,present:1,compute:Some(2)}));assert!(rank(vk::PhysicalDeviceType::DISCRETE_GPU)>rank(vk::PhysicalDeviceType::CPU));}
 #[test]fn no_present_or_graphics_is_rejected(){assert!(select(&[f(1,vk::QueueFlags::COMPUTE)],&[true]).is_none());assert!(select(&[f(1,vk::QueueFlags::GRAPHICS)],&[false]).is_none());assert!(select(&[],&[true]).is_none());}
}
