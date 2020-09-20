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
use crate::game::shared::util::{get_single_time_command_buffer, end_one_time_command_buffer};
use crossbeam::sync::ShardedLock;
use parking_lot::Mutex;

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

    pub async fn create_buffer(graphics: Arc<ShardedLock<graphics::vk::Graphics>>,
                               vertices: Vec<Vertex>, indices: Vec<u32>,
                               command_pool: Arc<Mutex<ash::vk::CommandPool>>) -> (graphics::vk::Buffer, graphics::vk::Buffer) {
        let device: Arc<ash::Device>;
        let allocator: Arc<ShardedLock<vk_mem::Allocator>>;
        {
            let lock = graphics.read().unwrap();
            device = lock.logical_device.clone();
            allocator = lock.allocator.clone();
            drop(lock);
        }
        let vertex_buffer_size = DeviceSize::try_from(std::mem::size_of::<Vertex>() * vertices.len())
            .unwrap();
        let index_buffer_size = DeviceSize::try_from(std::mem::size_of::<u32>() * indices.len())
            .unwrap();
        let cmd_buffer = get_single_time_command_buffer(
            device.as_ref(), *command_pool.lock()
        );

        let device_handle1 = device.clone();
        let allocator_handle1 = allocator.clone();
        let vertices_handle = tokio::spawn(async move {
            let device_handle = device_handle1;
            let allocator_handle = allocator_handle1;
            let mut vertex_staging = graphics::vk::Buffer::new(
                Arc::downgrade(&device_handle), vertex_buffer_size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                Arc::downgrade(&allocator_handle)
            );
            let vertex_mapped = vertex_staging.map_memory(vertex_buffer_size, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(vertices.as_ptr() as *const c_void, vertex_mapped, vertex_buffer_size as usize);
            }
            let vertex_buffer = graphics::vk::Buffer::new(
                Arc::downgrade(&device_handle), vertex_buffer_size,
                BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL, Arc::downgrade(&allocator_handle)
            );
            (vertex_staging, vertex_buffer)
        });

        let device_handle2 = device.clone();
        let allocator_handle2 = allocator.clone();
        let indices_handle = tokio::spawn(async move {
            let device_handle = device_handle2;
            let allocator_handle = allocator_handle2;
            let mut index_staging = graphics::vk::Buffer::new(
                Arc::downgrade(&device_handle), index_buffer_size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                Arc::downgrade(&allocator_handle)
            );
            let index_mapped = index_staging.map_memory(index_buffer_size, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(indices.as_ptr() as *const c_void, index_mapped, index_buffer_size as usize);
            }
            let index_buffer = graphics::vk::Buffer::new(
                Arc::downgrade(&device_handle), index_buffer_size,
                BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL, Arc::downgrade(&allocator_handle)
            );
            (index_staging, index_buffer)
        });

        let (vertex_staging, vertex_buffer) = vertices_handle.await.unwrap();
        let (index_staging, index_buffer) = indices_handle.await.unwrap();
        let graphics_lock = graphics.read().unwrap();
        let pool_lock = command_pool.lock();
        vertex_buffer.copy_buffer(
            &vertex_staging, vertex_buffer_size, *pool_lock,
            *graphics_lock.graphics_queue.lock(), Some(cmd_buffer)
        );
        index_buffer.copy_buffer(
            &index_staging, index_buffer_size, *pool_lock,
            *graphics_lock.graphics_queue.lock(), Some(cmd_buffer)
        );
        end_one_time_command_buffer(cmd_buffer, device.as_ref(), *pool_lock, *graphics_lock.graphics_queue.lock());
        (vertex_buffer, index_buffer)
    }

    pub fn create_image(image_data: Vec<u8>, buffer_size: DeviceSize, width: u32, height: u32,
                              format: gltf::image::Format,
                              graphics: Arc<ShardedLock<graphics::vk::Graphics>>,
                              command_pool: Arc<Mutex<ash::vk::CommandPool>>) -> graphics::vk::Image {
        let lock = graphics.read().unwrap();
        let device = lock.logical_device.clone();
        let allocator = lock.allocator.clone();
        let _format = match format {
            gltf::image::Format::B8G8R8A8 => ash::vk::Format::B8G8R8A8_UNORM,
            gltf::image::Format::R8G8B8A8 => ash::vk::Format::R8G8B8A8_UNORM,
            _ => lock.swapchain.format.format
        };
        let cmd_buffer = get_single_time_command_buffer(
            device.as_ref(), *command_pool.lock()
        );

        let mut staging = graphics::vk::Buffer::new(
            Arc::downgrade(&device),
            buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
            Arc::downgrade(&allocator)
        );
        unsafe {
            let mapped = staging.map_memory(buffer_size, 0);
            std::ptr::copy_nonoverlapping(image_data.as_ptr() as *const c_void, mapped, buffer_size as usize);
        }
        let _width = width as f32;
        let _height = height as f32;
        let mip_levels = _width.max(_height).log2().floor() as u32;
        let mut image = graphics::vk::Image::new(
            Arc::downgrade(&device),
            ImageUsageFlags::TRANSFER_SRC | ImageUsageFlags::TRANSFER_DST | ImageUsageFlags::SAMPLED,
            MemoryPropertyFlags::DEVICE_LOCAL, _format,
            SampleCountFlags::TYPE_1,
            Extent2D::builder().width(width).height(height).build(),
            ImageType::TYPE_2D, mip_levels, ImageAspectFlags::COLOR,
            Arc::downgrade(&allocator)
        );
        let pool_lock = command_pool.lock();
        image.transition_layout(ImageLayout::UNDEFINED, ImageLayout::TRANSFER_DST_OPTIMAL,
                                *pool_lock, *lock.graphics_queue.lock(), ImageAspectFlags::COLOR, mip_levels, Some(cmd_buffer));
        image.copy_buffer_to_image(staging.buffer, width, height, *pool_lock, *lock.graphics_queue.lock(), Some(cmd_buffer));
        unsafe {
            image.generate_mipmap(ImageAspectFlags::COLOR, mip_levels, *pool_lock, *lock.graphics_queue.lock(), Some(cmd_buffer));
        }
        image.create_sampler(mip_levels);
        end_one_time_command_buffer(cmd_buffer, device.as_ref(), *pool_lock, *lock.graphics_queue.lock());
        image
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

impl<BufferType, CommandType, TextureType> Disposable for Mesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          TextureType: 'static + Clone + Disposable {
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