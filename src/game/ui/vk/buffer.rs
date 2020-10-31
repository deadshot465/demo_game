use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk::{
    BufferCreateInfo, BufferUsageFlags, DeviceMemory, MemoryAllocateInfo, MemoryMapFlags,
    MemoryPropertyFlags, PhysicalDevice, SharingMode,
};

#[derive(Copy, Clone, Debug)]
pub struct Buffer {
    pub buffer: ash::vk::Buffer,
    pub device_memory: DeviceMemory,
    pub buffer_size: u64,
    mapped_memory: *mut std::ffi::c_void,
}

impl Buffer {
    pub fn new(
        device: &ash::Device,
        buffer_size: u64,
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        usage_flag: BufferUsageFlags,
        memory_properties: MemoryPropertyFlags,
    ) -> Self {
        let buffer_info = BufferCreateInfo::builder()
            .usage(usage_flag)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .size(buffer_size);

        unsafe {
            let buffer = device
                .create_buffer(&buffer_info, None)
                .expect("Failed to create buffer for Nuklear.");
            let memory_requirements = device.get_buffer_memory_requirements(buffer);
            let allocation_info = MemoryAllocateInfo::builder()
                .allocation_size(memory_requirements.size)
                .memory_type_index(Self::find_memory_type_index(
                    instance,
                    physical_device,
                    memory_requirements.memory_type_bits,
                    memory_properties,
                ));
            let device_memory = device
                .allocate_memory(&allocation_info, None)
                .expect("Failed to allocate memory for staging buffer.");
            device
                .bind_buffer_memory(buffer, device_memory, 0)
                .expect("Failed to bind buffer memory.");
            Buffer {
                buffer,
                device_memory,
                mapped_memory: std::ptr::null_mut(),
                buffer_size,
            }
        }
    }

    pub fn get_mapped_memory(&mut self, device: &ash::Device) -> *mut std::ffi::c_void {
        if self.mapped_memory.is_null() {
            self.mapped_memory = unsafe {
                device
                    .map_memory(
                        self.device_memory,
                        0,
                        self.buffer_size,
                        MemoryMapFlags::empty(),
                    )
                    .expect("Failed to map memory for buffer.")
            };
        }
        self.mapped_memory
    }

    fn find_memory_type_index(
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        memory_type: u32,
        memory_properties: MemoryPropertyFlags,
    ) -> u32 {
        unsafe {
            let properties = instance.get_physical_device_memory_properties(physical_device);
            for i in 0..properties.memory_type_count {
                if ((memory_type & (1 << i)) != 0)
                    && ((properties.memory_types[i as usize].property_flags & memory_properties)
                        == memory_properties)
                {
                    return i;
                }
            }
        }
        0
    }
}
