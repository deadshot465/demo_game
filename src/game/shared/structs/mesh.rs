use ash::vk::*;
use ash::version::DeviceV1_0;
use parking_lot::Mutex;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};

use crate::game::graphics;
use crate::game::shared::traits::disposable::Disposable;
use crate::game::structs::Vertex;

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
pub struct Mesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    pub primitives: Vec<Primitive>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Vec<ManuallyDrop<TextureType>>,
    pub sampler_resource: Option<SamplerResource>,
    pub is_disposed: bool,
    pub command_pool: Option<Arc<Mutex<ash::vk::CommandPool>>>,
    pub command_buffer: Option<CommandType>,
}

impl Mesh<graphics::vk::Buffer, ash::vk::CommandBuffer, graphics::vk::Image> {
    pub fn new(primitives: Vec<Primitive>) -> Self {
        let mesh = Mesh {
            primitives,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false,
            texture: vec![],
            sampler_resource: None,
            command_pool: None,
            command_buffer: None,
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

unsafe impl<BufferType, CommandType, TextureType> Send for Mesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          TextureType: 'static + Clone + Disposable { }
unsafe impl<BufferType, CommandType, TextureType> Sync for Mesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          TextureType: 'static + Clone + Disposable { }

impl<BufferType, CommandType, TextureType> Drop for Mesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          TextureType: 'static + Clone + Disposable {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped mesh.");
        }
        else {
            log::warn!("Mesh is already dropped.");
        }
    }
}

impl<BufferType, CommandType, TextureType> Disposable for Mesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          TextureType: 'static + Clone + Disposable {
    fn dispose(&mut self) {
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