use ash::vk::{
    BufferUsageFlags, DescriptorBufferInfo, DescriptorSet, DescriptorSetAllocateInfo,
    DescriptorType, MemoryPropertyFlags, WriteDescriptorSet,
};
use glam::Mat4;
use parking_lot::RwLock;
use std::mem::ManuallyDrop;
use std::sync::Arc;

use crate::game::graphics::vk::{Buffer, Graphics};
use crate::game::shared::traits::Disposable;
use crate::game::traits::Mappable;
use ash::version::DeviceV1_0;

#[derive(Clone)]
pub struct SSBO {
    pub buffer: Buffer,
    pub descriptor_set: DescriptorSet,
    pub is_disposed: bool,
}

impl SSBO {
    pub fn new(
        graphics: Arc<RwLock<ManuallyDrop<Graphics>>>,
        data: &[Mat4; 500],
    ) -> anyhow::Result<Self> {
        let graphics_lock = graphics.read();
        let device = graphics_lock.logical_device.clone();
        let allocator = graphics_lock.allocator.clone();
        let buffer_size = std::mem::size_of::<Mat4>() * 500;
        let descriptor_set_layout = graphics_lock.ssbo_descriptor_set_layout;
        let mut buffer = Buffer::new(
            Arc::downgrade(&device),
            buffer_size as u64,
            BufferUsageFlags::STORAGE_BUFFER,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
            Arc::downgrade(&allocator),
        );
        let mapped = buffer.map_memory(buffer_size as u64, 0);
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const std::ffi::c_void,
                mapped,
                buffer_size,
            );
            let layouts = vec![descriptor_set_layout];
            let allocate_info = DescriptorSetAllocateInfo::builder()
                .descriptor_pool(*graphics_lock.descriptor_pool.lock())
                .set_layouts(layouts.as_slice());
            let descriptor_set = device
                .allocate_descriptor_sets(&allocate_info)
                .expect("Failed to allocate descriptor set for SSBO.");
            log::info!("Successfully allocated descriptor set for SSBO.");
            let buffer_info = vec![DescriptorBufferInfo::builder()
                .buffer(buffer.buffer)
                .offset(0)
                .range(buffer_size as u64)
                .build()];
            let write_descriptor = vec![WriteDescriptorSet::builder()
                .buffer_info(buffer_info.as_slice())
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .dst_array_element(0)
                .dst_binding(0)
                .dst_set(descriptor_set[0])
                .build()];
            device.update_descriptor_sets(write_descriptor.as_slice(), &[]);
            log::info!("Descriptor set for SSBO successfully updated.");
            Ok(SSBO {
                buffer,
                descriptor_set: descriptor_set[0],
                is_disposed: false,
            })
        }
    }
}

impl Drop for SSBO {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl Disposable for SSBO {
    fn dispose(&mut self) {
        self.buffer.dispose();
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        unimplemented!()
    }

    fn set_name(&mut self, _name: String) -> &str {
        unimplemented!()
    }
}
