use crate::game::shared::traits::disposable::Disposable;
use crate::game::structs::Vertex;
use std::mem::ManuallyDrop;
use crate::game::graphics;
use ash::vk::*;
use ash::version::DeviceV1_0;
use std::sync::{Arc, Weak};
use std::convert::TryFrom;
use crate::game::traits::Mappable;
use std::ffi::c_void;

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
pub struct Mesh<BufferType: 'static + Clone + Disposable, CommandType: 'static, TextureType: 'static + Clone + Disposable> {
    pub primitives: Vec<Primitive>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Vec<ManuallyDrop<TextureType>>,
    pub sampler_resource: Option<SamplerResource>,
    pub is_disposed: bool,
    pub command_pool: Option<ash::vk::CommandPool>,
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

    pub fn create_vertex_buffer(&mut self, graphics: &graphics::vk::Graphics) {
        let vertices = self.primitives.iter()
            .map(|p| &p.vertices)
            .flatten()
            .map(|v| *v)
            .collect::<Vec<_>>();
        let buffer_size = DeviceSize::try_from(std::mem::size_of::<Vertex>() * vertices.len())
            .unwrap();
        let buffer = Self::create_buffer(vertices, buffer_size, graphics,
                            BufferUsageFlags::VERTEX_BUFFER,
                            *self.command_pool.as_ref().unwrap(),
                            *self.command_buffer.as_ref().unwrap());
        self.vertex_buffer = Some(ManuallyDrop::new(buffer));
    }

    pub fn create_index_buffer(&mut self, graphics: &graphics::vk::Graphics) {
        let indices = self.primitives.iter()
            .map(|p| &p.indices)
            .flatten()
            .map(|i| *i)
            .collect::<Vec<_>>();
        let buffer_size = DeviceSize::try_from(std::mem::size_of::<u32>() * indices.len())
            .unwrap();
        let buffer = Self::create_buffer(indices, buffer_size, graphics,
                                         BufferUsageFlags::INDEX_BUFFER,
                                         *self.command_pool.as_ref().unwrap(),
                                         *self.command_buffer.as_ref().unwrap());
        self.index_buffer = Some(ManuallyDrop::new(buffer));
    }

    pub fn begin_command_buffer(&self, device: &ash::Device) {
        let begin_info = CommandBufferBeginInfo::builder()
            .build();
        unsafe {
            device.begin_command_buffer(self.command_buffer.unwrap(), &begin_info)
                .expect("Failed to begin command buffer for mesh.");
        }
    }

    pub fn end_command_buffer(&self, device: &ash::Device) {
        unsafe {
            device.end_command_buffer(self.command_buffer.unwrap())
                .expect("Failed to end command buffer for mesh.");
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

    fn create_buffer<T>(data: Vec<T>, buffer_size: DeviceSize,
                        graphics: &graphics::vk::Graphics, buffer_usage: BufferUsageFlags,
                        command_pool: CommandPool, command_buffer: CommandBuffer) -> graphics::vk::Buffer {
        let mut staging = graphics::vk::Buffer::new(
            Arc::downgrade(&graphics.logical_device), buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            Arc::downgrade(&graphics.allocator)
        );
        let mapped = staging.map_memory(buffer_size, 0);
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr() as *const c_void, mapped, buffer_size as usize);
        }
        let buffer = graphics::vk::Buffer::new(
            Arc::downgrade(&graphics.logical_device), buffer_size,
            BufferUsageFlags::TRANSFER_DST | buffer_usage,
            MemoryPropertyFlags::DEVICE_LOCAL,
            Arc::downgrade(&graphics.allocator)
        );
        buffer.copy_buffer(
            &staging, buffer_size, command_pool,
            graphics.graphics_queue, Some(command_buffer)
        );
        buffer
    }
}

unsafe impl<BufferType: 'static + Clone + Disposable, CommandType, TextureType: 'static + Clone + Disposable> Send for Mesh<BufferType, CommandType, TextureType> { }
unsafe impl<BufferType: 'static + Clone + Disposable, CommandType, TextureType: 'static + Clone + Disposable> Sync for Mesh<BufferType, CommandType, TextureType> { }

impl<BufferType: 'static + Clone + Disposable, CommandType, TextureType: 'static + Clone + Disposable> Drop for Mesh<BufferType, CommandType, TextureType> {
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

impl<BufferType: 'static + Clone + Disposable, CommandType, TextureType: 'static + Clone + Disposable> Disposable for Mesh<BufferType, CommandType, TextureType> {
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