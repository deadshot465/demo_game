use ash::vk::*;

pub trait Mappable {
    //fn allocate_memory(&mut self, instance: &ash::Instance, physical_device: PhysicalDevice, memory_properties: MemoryPropertyFlags) -> DeviceMemory;
    fn map_memory(&mut self, device_size: DeviceSize, offset: DeviceSize) -> *mut std::ffi::c_void;
    fn unmap_memory(&mut self);

    //fn get_memory_type_index(&self, instance: &ash::Instance, physical_device: PhysicalDevice, memory_type: u32, memory_properties: MemoryPropertyFlags) -> u32;
    //fn map_device_memory(&mut self, instance: &ash::Instance, memory_requirements: &MemoryRequirements, physical_device: PhysicalDevice, memory_properties: MemoryPropertyFlags) -> DeviceMemory;
}
