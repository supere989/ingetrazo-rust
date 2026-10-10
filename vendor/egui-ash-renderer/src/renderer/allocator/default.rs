use crate::{RendererError, RendererResult};
use ash::{vk, Device};

use super::Allocate;

/// Abstraction over memory used by Vulkan resources.
pub type Memory = vk::DeviceMemory;

pub struct Allocator {
    uncertain:std::sync::atomic::AtomicBool,
    pub memory_properties: vk::PhysicalDeviceMemoryProperties,
    observer: Option<std::sync::Arc<dyn crate::MemoryObserver>>,
}

impl Allocator {
    pub fn new(memory_properties: vk::PhysicalDeviceMemoryProperties) -> Self {
        Self { memory_properties, observer:None,uncertain:std::sync::atomic::AtomicBool::new(false) }
    }

    pub fn with_observer(mut self,observer:std::sync::Arc<dyn crate::MemoryObserver>)->Self { self.observer=Some(observer);self }

    fn allocate(&self,device:&Device,requirements:vk::MemoryRequirements,mem_type:u32,bind:impl FnOnce(vk::DeviceMemory)->Result<(),vk::Result>)->RendererResult<Memory> {
        let heap=self.memory_properties.memory_types[mem_type as usize].heap_index;
        let ticket=match &self.observer {Some(observer)=>Some(observer.reserve(heap,requirements.size)?),None=>None};
        let info=vk::MemoryAllocateInfo::default().allocation_size(requirements.size).memory_type_index(mem_type);
        let memory=match unsafe {device.allocate_memory(&info,None)} {Ok(memory)=>memory,Err(error)=>{if let (Some(observer),Some(ticket))=(&self.observer,ticket){observer.cancel(ticket);}return Err(error.into());}};
        if let Err(error)=bind(memory) {unsafe {device.free_memory(memory,None)};if let (Some(observer),Some(ticket))=(&self.observer,ticket){observer.cancel(ticket);}return Err(error.into());}
        if let (Some(observer),Some(ticket))=(&self.observer,ticket){observer.commit(ticket,memory);}
        Ok(memory)
    }

    fn find_memory_type(
        &self,
        requirements: vk::MemoryRequirements,
        required_properties: vk::MemoryPropertyFlags,
    ) -> RendererResult<u32> {
        for i in 0..self.memory_properties.memory_type_count {
            if requirements.memory_type_bits & (1 << i) != 0
                && self.memory_properties.memory_types[i as usize]
                    .property_flags
                    .contains(required_properties)
            {
                return Ok(i);
            }
        }
        Err(RendererError::Allocator(
            "Failed to find suitable memory type.".into(),
        ))
    }
}

impl Allocate for Allocator {
    type Memory = Memory;
    fn can_destroy(&self)->bool{!self.uncertain.load(std::sync::atomic::Ordering::Acquire)&&self.observer.as_ref().is_none_or(|o|o.can_destroy())}
    fn gpu_wait_failed(&self){self.uncertain.store(true,std::sync::atomic::Ordering::Release);if let Some(o)=&self.observer{o.gpu_wait_failed();}}


    fn create_buffer(
        &mut self,
        device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Self::Memory)> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size as _)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };

        let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let allocation=(|| {
            let mem_type=self.find_memory_type(mem_requirements,vk::MemoryPropertyFlags::HOST_VISIBLE|vk::MemoryPropertyFlags::HOST_COHERENT)?;
            self.allocate(device,mem_requirements,mem_type,|memory| unsafe {device.bind_buffer_memory(buffer,memory,0)})
        })();
        let memory=match allocation {Ok(memory)=>memory,Err(error)=>{unsafe {device.destroy_buffer(buffer,None)};return Err(error);}};

        Ok((buffer, memory))
    }

    fn create_image(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
    ) -> RendererResult<(vk::Image, Self::Memory)> {
        let extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(extent)
            .mip_levels(1)
            .array_layers(1)
            .format(vk::Format::R8G8B8A8_UNORM)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::empty());

        let image = unsafe { device.create_image(&image_info, None)? };
        let mem_requirements = unsafe { device.get_image_memory_requirements(image) };
        let allocation=(|| {
            let mem_type=self.find_memory_type(mem_requirements,vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
            self.allocate(device,mem_requirements,mem_type,|memory| unsafe {device.bind_image_memory(image,memory,0)})
        })();
        let memory=match allocation {Ok(memory)=>memory,Err(error)=>{unsafe {device.destroy_image(image,None)};return Err(error);}};

        Ok((image, memory))
    }

    fn destroy_buffer(
        &mut self,
        device: &Device,
        buffer: vk::Buffer,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        unsafe {
            device.destroy_buffer(buffer, None);
            device.free_memory(memory, None);
            if let Some(observer)=&self.observer {observer.freed(memory);}
        }

        Ok(())
    }

    fn destroy_image(
        &mut self,
        device: &Device,
        image: vk::Image,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        unsafe {
            device.destroy_image(image, None);
            device.free_memory(memory, None);
            if let Some(observer)=&self.observer {observer.freed(memory);}
        }

        Ok(())
    }

    fn update_buffer<T: Copy>(
        &mut self,
        device: &Device,
        memory: &mut Self::Memory,
        data: &[T],
    ) -> RendererResult<()> {
        let size = std::mem::size_of_val(data) as _;
        unsafe {
            let data_ptr = device.map_memory(*memory, 0, size, vk::MemoryMapFlags::empty())?;
            let mut align = ash::util::Align::new(data_ptr, std::mem::align_of::<T>() as _, size);
            align.copy_from_slice(data);
            device.unmap_memory(*memory);
        }

        Ok(())
    }
}
