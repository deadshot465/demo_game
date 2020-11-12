use crate::game::graphics::vk::{DescriptorAllocator, DescriptorLayoutCache};
use ash::version::DeviceV1_0;
use ash::vk::{
    DescriptorBufferInfo, DescriptorImageInfo, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType, ShaderStageFlags,
    WriteDescriptorSet,
};

pub struct DescriptorBuilder<'a> {
    layout_cache: &'a mut DescriptorLayoutCache,
    allocator: &'a mut DescriptorAllocator,
    writes: Vec<WriteDescriptorSet>,
    bindings: Vec<DescriptorSetLayoutBinding>,
}

impl<'a> DescriptorBuilder<'a> {
    pub fn builder(
        layout_cache: &'a mut DescriptorLayoutCache,
        allocator: &'a mut DescriptorAllocator,
    ) -> Self {
        DescriptorBuilder {
            layout_cache,
            allocator,
            writes: vec![],
            bindings: vec![],
        }
    }

    pub fn bind_buffer(
        mut self,
        binding: u32,
        buffer_info: &'a [DescriptorBufferInfo],
        descriptor_type: DescriptorType,
        stage_flags: ShaderStageFlags,
    ) -> Self {
        let new_binding = DescriptorSetLayoutBinding::builder()
            .descriptor_count(1)
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .binding(binding)
            .build();
        self.bindings.push(new_binding);

        let new_write = WriteDescriptorSet::builder()
            .descriptor_type(descriptor_type)
            .buffer_info(buffer_info)
            .dst_array_element(0)
            .dst_binding(binding)
            .build();
        self.writes.push(new_write);
        self
    }

    pub fn bind_image(
        mut self,
        binding: u32,
        image_info: &'a [DescriptorImageInfo],
        descriptor_type: DescriptorType,
        stage_flags: ShaderStageFlags,
    ) -> Self {
        let new_binding = DescriptorSetLayoutBinding::builder()
            .descriptor_count(1)
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .binding(binding)
            .build();
        self.bindings.push(new_binding);

        let new_write = WriteDescriptorSet::builder()
            .descriptor_type(descriptor_type)
            .image_info(image_info)
            .dst_array_element(0)
            .dst_binding(binding)
            .build();
        self.writes.push(new_write);
        self
    }

    pub fn build(
        mut self,
        descriptor_set: &'a mut DescriptorSet,
        descriptor_set_layout: &'a mut DescriptorSetLayout,
    ) -> bool {
        let layout_info =
            DescriptorSetLayoutCreateInfo::builder().bindings(self.bindings.as_slice());
        *descriptor_set_layout = self.layout_cache.create_descriptor_layout(&layout_info);

        let success = self
            .allocator
            .allocate(descriptor_set, *descriptor_set_layout);
        if !success {
            false
        } else {
            for write in self.writes.iter_mut() {
                write.dst_set = *descriptor_set;
            }
            unsafe {
                let device = self
                    .allocator
                    .logical_device
                    .upgrade()
                    .expect("Failed to upgrade device handle.");
                device.update_descriptor_sets(self.writes.as_slice(), &[]);
            }
            true
        }
    }
}
