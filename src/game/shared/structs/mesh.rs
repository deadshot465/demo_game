use crate::game::shared::traits::disposable::Disposable;
use crate::game::structs::Vertex;
use std::mem::ManuallyDrop;
use crate::game::graphics;
use std::marker::PhantomData;
use ash::vk::{DescriptorSetLayoutCreateInfo, DescriptorSetLayoutBinding, ShaderStageFlags, DescriptorType, DescriptorSetLayout, DescriptorPoolSize, DescriptorPoolCreateInfo, DescriptorSetAllocateInfo, DescriptorImageInfo, ImageLayout, WriteDescriptorSet, DescriptorPool};
use ash::version::DeviceV1_0;
use std::sync::Weak;
use crossbeam::sync::ShardedLock;
use crate::game::graphics::vk::Graphics;
use crate::game::traits::GraphicsBase;

#[derive(Clone)]
pub enum SamplerResource {
    DescriptorSet(ash::vk::DescriptorSet)
}

#[derive(Clone, Debug)]
pub struct Primitive {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture_index: Option<usize>,
    pub is_disposed: bool,
}

#[derive(Clone)]
pub struct Mesh<BufferType: 'static + Clone + Disposable, TextureType: 'static + Clone + Disposable> {
    pub primitives: Vec<Primitive>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Vec<ManuallyDrop<TextureType>>,
    pub sampler_resource: Option<SamplerResource>,
    pub is_disposed: bool,
}

impl Mesh<graphics::vk::Buffer, graphics::vk::Image> {
    pub fn new(primitives: Vec<Primitive>) -> Self {
        let mesh = Mesh {
            primitives,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false,
            texture: vec![],
            sampler_resource: None,
        };
        mesh
    }

    pub fn get_vertex_buffer(&self) -> ash::vk::Buffer {
        if let Some(buffer) = self.vertex_buffer.as_ref() {
            buffer.buffer
        }
        else {
            panic!("Vertex buffer is not yet created.");
        }
    }

    pub fn get_index_buffer(&self) -> ash::vk::Buffer {
        if let Some(buffer) = self.index_buffer.as_ref() {
            buffer.buffer
        }
        else {
            panic!("Index buffer is not yet created.");
        }
    }

    pub fn create_sampler_resource(&mut self, logical_device: Weak<ash::Device>,
                                   sampler_descriptor_set_layout: DescriptorSetLayout,
                                   descriptor_pool: DescriptorPool) {
        let device = logical_device.upgrade();
        if device.is_none() {
            log::error!("Cannot upgrade weak reference to strong reference.");
            return;
        }
        let device = device.unwrap();
        unsafe {
            let descriptor_set_info = DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&[sampler_descriptor_set_layout])
                .build();
            let descriptor_set = device
                .allocate_descriptor_sets(&descriptor_set_info)
                .expect("Failed to allocate descriptor set for texture.");
            self.sampler_resource = Some(SamplerResource::DescriptorSet(descriptor_set[0]));
            log::info!("Successfully allocate descriptor set for texture.");

            let image_info = DescriptorImageInfo::builder()
                .sampler(self.texture[0].sampler)
                .image_view(self.texture[0].image_view)
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
        }
    }
}

impl<BufferType: 'static + Clone + Disposable, TextureType: 'static + Clone + Disposable> Drop for Mesh<BufferType, TextureType> {
    fn drop(&mut self) {
        log::info!("Dropping mesh...");
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped mesh.");
        }
        else {
            log::warn!("Mesh is already dropped.");
        }
    }
}

impl<BufferType: 'static + Clone + Disposable, TextureType: 'static + Clone + Disposable> Disposable for Mesh<BufferType, TextureType> {
    fn dispose(&mut self) {
        log::info!("Disposing mesh...");
        unsafe {
            let has_texture = !self.texture.is_empty();
            if has_texture {
                for texture in self.texture.iter_mut() {
                    ManuallyDrop::drop(texture);
                }
            }
            if let Some(buffer) = self.index_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.vertex_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
        }
        self.is_disposed = true;
        log::info!("Successfully disposed mesh.");
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