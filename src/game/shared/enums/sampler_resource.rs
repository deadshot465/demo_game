use ash::version::DeviceV1_0;
use ash::vk::{DescriptorSetLayout, DescriptorPool, DescriptorSetAllocateInfo, DescriptorImageInfo, ImageLayout, WriteDescriptorSet, DescriptorType};
use std::sync::Weak;

use crate::game::graphics::vk::Image;

#[derive(Clone, Debug)]
pub enum SamplerResource {
    DescriptorSet(ash::vk::DescriptorSet)
}

pub fn create_sampler_resource(logical_device: Weak<ash::Device>,
                               sampler_descriptor_set_layout: DescriptorSetLayout,
                               descriptor_pool: DescriptorPool, texture: &Image) -> SamplerResource {
    let device = logical_device.upgrade();
    if device.is_none() {
        panic!("Cannot upgrade weak reference to strong reference.");
    }
    let device = device.unwrap();
    unsafe {
        let layouts = vec![sampler_descriptor_set_layout];
        let descriptor_set_info = DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(layouts.as_slice());
        let descriptor_set = device
            .allocate_descriptor_sets(&descriptor_set_info)
            .expect("Failed to allocate descriptor set for texture.");
        let sampler_resource = SamplerResource::DescriptorSet(descriptor_set[0]);
        log::info!("Successfully allocate descriptor set for texture.");

        let image_info = DescriptorImageInfo::builder()
            .sampler(texture.sampler)
            .image_view(texture.image_view)
            .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .build();
        let write_descriptor = WriteDescriptorSet::builder()
            .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
            .dst_set(descriptor_set[0])
            .dst_binding(0)
            .dst_array_element(0)
            .image_info(&[image_info])
            .build();
        device.update_descriptor_sets(&[write_descriptor], &[]);
        log::info!("Descriptor set for texture sampler successfully updated.");
        sampler_resource
    }
}