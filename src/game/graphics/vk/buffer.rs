use ash::{
    Device,
    Instance,
    vk::{
        BufferCopy,
        BufferCreateInfo,
        BufferUsageFlags,
        CommandPool,
        DeviceMemory,
        DeviceSize,
        MemoryAllocateInfo,
        MemoryMapFlags,
        MemoryPropertyFlags,
        MemoryRequirements,
        PhysicalDevice,
        Queue,
        SharingMode,
    },
};
use std::ffi::c_void;
use std::sync::Weak;
use ash::version::{DeviceV1_0, InstanceV1_0};
use crate::game::shared::traits::mappable::Mappable;
use crate::game::shared::traits::disposable::Disposable;
use crate::game::util::{get_single_time_command_buffer, end_one_time_command_buffer};

#[derive(Clone)]
pub struct Buffer {
    pub buffer: ash::vk::Buffer,
    pub device_memory: DeviceMemory,
    pub mapped_memory: *mut c_void,
    pub is_disposed: bool,
    pub buffer_size: DeviceSize,
    logical_device: Weak<Device>,
}

impl Buffer {
    pub fn new(instance: &Instance, device: Weak<Device>,
               physical_device: PhysicalDevice, buffer_size: DeviceSize,
               usage_flag: BufferUsageFlags,
               memory_properties: MemoryPropertyFlags) -> Self {
        let create_info = BufferCreateInfo::builder()
            .sharing_mode(SharingMode::EXCLUSIVE)
            .size(buffer_size)
            .usage(usage_flag)
            .build();
        unsafe {
            let buffer = device
                .upgrade()
                .unwrap()
                .create_buffer(&create_info, None)
                .expect("Failed to create buffer.");

            let mut _instance = Buffer {
                logical_device: device,
                buffer,
                device_memory: DeviceMemory::null(),
                mapped_memory: std::ptr::null_mut(),
                is_disposed: false,
                buffer_size,
            };

            let device_memory = _instance.allocate_memory(instance, physical_device, memory_properties);
            _instance.device_memory = device_memory;
            _instance
        }
    }

    pub fn copy_buffer(&self, src_buffer: &Buffer, buffer_size: DeviceSize, command_pool: CommandPool, graphics_queue: Queue) {
        unsafe {
            let device = self.logical_device.upgrade();
            if let Some(d) = device {
                let copy_info = BufferCopy::builder()
                    .src_offset(0)
                    .size(buffer_size)
                    .dst_offset(0);
                let cmd_buffer = get_single_time_command_buffer(d.as_ref(), command_pool);
                d.cmd_copy_buffer(cmd_buffer, src_buffer.buffer, self.buffer, &[copy_info.build()]);
                end_one_time_command_buffer(cmd_buffer, d.as_ref(), command_pool, graphics_queue);
            }
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl Disposable for Buffer {
    fn dispose(&mut self) {
        if self.is_disposed {
            return;
        }
        unsafe {
            if !self.mapped_memory.is_null() {
                self.unmap_memory();
            }
            let device = self.logical_device.upgrade().unwrap();
            if self.device_memory != DeviceMemory::null() {
                device
                    .free_memory(self.device_memory, None);
            }
            device.destroy_buffer(self.buffer, None);
            self.is_disposed = true;
        }
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

impl Mappable for Buffer {
    fn allocate_memory(&mut self, instance: &Instance, physical_device: PhysicalDevice, memory_properties: MemoryPropertyFlags) -> DeviceMemory {
        unsafe {
            let device = self.logical_device.upgrade().unwrap();
            let requirements = device
                .get_buffer_memory_requirements(self.buffer);
            let device_memory = self.map_device_memory(instance,
                                                       &requirements, physical_device, memory_properties);
            device
                .bind_buffer_memory(self.buffer, device_memory, 0)
                .expect("Failed to bind buffer memory.");
            self.device_memory = device_memory;
            self.device_memory
        }
    }

    fn map_memory(&mut self, device_size: u64, offset: u64) -> *mut c_void {
        unsafe {
            self.mapped_memory = self.logical_device
                .upgrade()
                .unwrap()
                .map_memory(self.device_memory, offset, device_size, MemoryMapFlags::empty())
                .expect("Failed to map device memory.");
            self.mapped_memory
        }
    }

    fn unmap_memory(&mut self) {
        unsafe {
            self.logical_device
                .upgrade()
                .unwrap()
                .unmap_memory(self.device_memory);
        }
    }

    fn get_memory_type_index(&self, instance: &Instance, physical_device: PhysicalDevice, memory_type: u32, memory_properties: MemoryPropertyFlags) -> u32 {
        unsafe {
            let properties = instance.get_physical_device_memory_properties(physical_device);
            for i in 0..properties.memory_type_count {
                if (memory_type & (1 << i)) != 0 &&
                    ((properties.memory_types[i as usize].property_flags & memory_properties) == memory_properties) {
                    return i as u32;
                }
            }
        }
        0
    }

    fn map_device_memory(&mut self, instance: &Instance, memory_requirements: &MemoryRequirements, physical_device: PhysicalDevice, memory_properties: MemoryPropertyFlags) -> DeviceMemory {
        let memory_type_index = self.get_memory_type_index(instance, physical_device,
                                                           memory_requirements.memory_type_bits, memory_properties);
        let allocate_info = MemoryAllocateInfo::builder()
            .memory_type_index(memory_type_index)
            .allocation_size(memory_requirements.size)
            .build();
        unsafe {
            let device_memory = self.logical_device
                .upgrade()
                .unwrap()
                .allocate_memory(&allocate_info, None)
                .expect("Failed to allocate device memory.");
            device_memory
        }
    }
}