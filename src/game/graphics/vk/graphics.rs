use ash::{
    Entry,
    extensions::{
        khr::Surface,
        ext::DebugUtils
    },
    Device,
    Instance,
    version::{
        DeviceV1_0,
        InstanceV1_0
    },
    vk::*
};
use crossbeam::sync::ShardedLock;
use glam::{Vec3A, Vec4, Mat4};
use image::GenericImageView;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::ffi::{c_void, CString};
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::sync::atomic::AtomicPtr;
use tokio::task::JoinHandle;
use vk_mem::*;

use crate::game::{Camera, ResourceManager};
use crate::game::enums::ShaderType;
use crate::game::graphics::vk::{UniformBuffers, DynamicBufferObject, DynamicModel, Initializer, ThreadPool};
use crate::game::shared::enums::ImageFormat;
use crate::game::shared::structs::{ViewProjection, Directional, PushConstant};
use crate::game::shared::traits::GraphicsBase;
use crate::game::shared::util::interpolate_alpha;
use crate::game::traits::Mappable;
use crate::game::util::{end_one_time_command_buffer, get_single_time_command_buffer};

#[allow(dead_code)]
pub struct Graphics {
    pub dynamic_objects: DynamicBufferObject,
    pub logical_device: Arc<Device>,
    pub pipeline: ManuallyDrop<super::Pipeline>,
    pub descriptor_sets: Vec<DescriptorSet>,
    pub push_constant: PushConstant,
    pub current_index: usize,
    pub sampler_descriptor_set_layout: DescriptorSetLayout,
    pub ssbo_descriptor_set_layout: DescriptorSetLayout,
    pub descriptor_pool: Arc<Mutex<DescriptorPool>>,
    pub command_pool: CommandPool,
    pub thread_pool: ThreadPool,
    pub allocator: Arc<ShardedLock<Allocator>>,
    pub graphics_queue: Arc<Mutex<Queue>>,
    pub present_queue: Arc<Mutex<Queue>>,
    pub compute_queue: Arc<Mutex<Queue>>,
    pub swapchain: ManuallyDrop<super::Swapchain>,
    pub frame_buffers: Vec<Framebuffer>,
    pub resource_manager: Weak<ShardedLock<ResourceManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>,
    entry: Entry,
    instance: Instance,
    surface_loader: Surface,
    debug_messenger: DebugUtilsMessengerEXT,
    surface: SurfaceKHR,
    physical_device: super::PhysicalDevice,
    descriptor_set_layout: DescriptorSetLayout,
    depth_image: ManuallyDrop<super::Image>,
    msaa_image: ManuallyDrop<super::Image>,
    uniform_buffers: ManuallyDrop<UniformBuffers>,
    camera: Arc<ShardedLock<Camera>>,
    command_buffers: Vec<CommandBuffer>,
    fences: Vec<Fence>,
    acquired_semaphores: Vec<Semaphore>,
    complete_semaphores: Vec<Semaphore>,
    sample_count: SampleCountFlags,
    depth_format: Format,
}

impl Graphics {
    pub fn new(window: &winit::window::Window, camera: Arc<ShardedLock<Camera>>, resource_manager: Weak<ShardedLock<ResourceManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>) -> anyhow::Result<Self> {
        let debug = dotenv::var("DEBUG")?
            .parse::<bool>()?;
        let entry = Entry::new()?;
        let enabled_layers = vec![CString::new("VK_LAYER_KHRONOS_validation")?];
        let instance = Initializer::create_instance(debug, &enabled_layers, &entry, window)?;
        let surface_loader = Surface::new(&entry, &instance);
        let debug_messenger = if debug {
            Initializer::create_debug_messenger(&instance, &entry)
        } else {
            DebugUtilsMessengerEXT::null()
        };
        let surface = Initializer::create_surface(window, &entry, &instance)?;
        let physical_device = super::PhysicalDevice::new(&instance, &surface_loader, surface);
        let (logical_device, graphics_queue, present_queue, compute_queue) = Initializer::create_logical_device(
            &instance, &physical_device, &enabled_layers, debug
        );
        let allocator_info = vk_mem::AllocatorCreateInfo {
            physical_device: physical_device.physical_device,
            device: logical_device.clone(),
            instance: instance.clone(),
            flags: AllocatorCreateFlags::NONE,
            preferred_large_heap_block_size: 0,
            frame_in_use_count: 0,
            heap_size_limits: None,
        };
        let allocator = vk_mem::Allocator::new(&allocator_info)
            .expect("Failed to create VMA memory allocator.");
        let device = Arc::new(logical_device);
        let allocator = Arc::new(ShardedLock::new(allocator));
        let swapchain = Initializer::create_swapchain(
            &surface_loader, surface, &physical_device, window, &instance, Arc::downgrade(&device),
            Arc::downgrade(&allocator)
        );

        let command_pool_create_info = CommandPoolCreateInfo::builder()
            .queue_family_index(physical_device.queue_indices.graphics_family.unwrap_or_default())
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool: CommandPool;
        unsafe {
            command_pool = device
                .create_command_pool(&command_pool_create_info, None)
                .expect("Failed to create command pool.");
        }
        let cpu_count = num_cpus::get();
        let thread_pool = ThreadPool::new(cpu_count, device.as_ref(),
                                                  physical_device.queue_indices.graphics_family
                                                      .unwrap_or_default());
        let sample_count = Initializer::get_sample_count(&instance, &physical_device);
        let depth_format = Initializer::get_depth_format(&instance, &physical_device);
        let depth_image = Initializer::create_depth_image(
            Arc::downgrade(&device), depth_format, &swapchain, command_pool, graphics_queue, sample_count, Arc::downgrade(&allocator)
        );

        let msaa_image = Initializer::create_msaa_image(
            Arc::downgrade(&device), &swapchain, command_pool, graphics_queue, sample_count, Arc::downgrade(&allocator)
        );

        let descriptor_set_layout = Self::create_descriptor_set_layout(device.as_ref());
        let sampler_descriptor_set_layout = Self::create_sampler_descriptor_set_layout(device.as_ref());
        let ssbo_descriptor_set_layout = Self::create_ssbo_descriptor_set_layout(device.as_ref());
        let view_projection = Self::create_view_projection(
            camera.read().unwrap().deref(), Arc::downgrade(&device), Arc::downgrade(&allocator))?;
        let directional_light = Directional::new(
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Vec3A::new(0.0, -5.0, 0.0),
            0.1,
            0.5);
        let directional = Self::create_directional_light(
            &directional_light, Arc::downgrade(&device),
            Arc::downgrade(&allocator)
        )?;

        let uniform_buffers = UniformBuffers::new(view_projection, directional);
        let min_alignment = physical_device
            .device_properties
            .limits.min_uniform_buffer_offset_alignment;
        let min_alignment = min_alignment as usize;
        let dynamic_alignment = if min_alignment > 0 {
            let mat4_size = std::mem::size_of::<Mat4>();
            (mat4_size + (min_alignment - 1)) & !(min_alignment - 1)
        } else {
            std::mem::size_of::<Mat4>()
        };
        let pipeline = super::Pipeline::new(device.clone());
        let command_buffers = Self::allocate_command_buffers(device.as_ref(), command_pool, swapchain.swapchain_images.len() as u32);
        let (fences, acquired_semaphores, complete_semaphores) = Self::create_sync_object(device.as_ref(), swapchain.swapchain_images.len() as u32);

        Ok(Graphics {
            entry,
            instance,
            surface_loader,
            debug_messenger,
            surface,
            physical_device,
            logical_device: device,
            graphics_queue: Arc::new(Mutex::new(graphics_queue)),
            present_queue: Arc::new(Mutex::new(present_queue)),
            compute_queue: Arc::new(Mutex::new(compute_queue)),
            swapchain: ManuallyDrop::new(swapchain),
            command_pool,
            depth_image: ManuallyDrop::new(depth_image),
            msaa_image: ManuallyDrop::new(msaa_image),
            descriptor_set_layout,
            uniform_buffers: ManuallyDrop::new(uniform_buffers),
            push_constant: PushConstant::new(0, Vec4::new(0.0, 1.0, 1.0, 1.0)),
            camera,
            resource_manager,
            dynamic_objects: DynamicBufferObject {
                models: DynamicModel::new(),
                meshes: DynamicModel::new(),
                min_alignment: min_alignment as DeviceSize,
                dynamic_alignment: dynamic_alignment as DeviceSize,
            },
            descriptor_pool: Arc::new(Mutex::new(DescriptorPool::null())),
            descriptor_sets: vec![],
            pipeline: ManuallyDrop::new(pipeline),
            command_buffers,
            frame_buffers: vec![],
            fences,
            acquired_semaphores,
            complete_semaphores,
            current_index: 0,
            sample_count,
            sampler_descriptor_set_layout,
            depth_format,
            allocator,
            thread_pool,
            ssbo_descriptor_set_layout,
        })
    }

    pub fn begin_draw(&self, frame_buffer: Framebuffer) -> anyhow::Result<()> {
        let clear_color = ClearColorValue {
            float32: [1.0, 1.0, 0.0, 1.0]
        };
        let clear_depth = ClearDepthStencilValue::builder()
            .depth(1.0).stencil(0);
        let clear_values = vec![ClearValue {
            color: clear_color
        }, ClearValue {
            depth_stencil: *clear_depth
        }];
        let cmd_buffer_begin_info = CommandBufferBeginInfo::builder();
        let extent = self.swapchain.extent;
        let render_area = Rect2D::builder()
            .extent(extent)
            .offset(Offset2D::default());
        let renderpass_begin_info = RenderPassBeginInfo::builder()
            .render_pass(self.pipeline.render_pass)
            .clear_values(clear_values.as_slice())
            .render_area(*render_area)
            .framebuffer(frame_buffer);
        let viewports = vec![Viewport::builder()
            .width(extent.width as f32)
            .height(extent.height as f32)
            .x(0.0).y(0.0).min_depth(0.0).max_depth(1.0).build()];
        let scissors = vec![
            Rect2D::builder()
                .extent(extent)
                .offset(Offset2D::default())
                .build()];
        let mut inheritance_info = CommandBufferInheritanceInfo::builder()
            .framebuffer(frame_buffer)
            .render_pass(self.pipeline.render_pass)
            .build();
        let inheritance_ptr = &mut inheritance_info as *mut CommandBufferInheritanceInfo;
        let ptr = Arc::new(AtomicPtr::from(inheritance_ptr));
        unsafe {
            let result = self.logical_device.begin_command_buffer(self.command_buffers[0], &cmd_buffer_begin_info);
            if let Err(e) = result {
                log::error!("Error beginning command buffer: {}", e.to_string());
            }
            self.logical_device.cmd_begin_render_pass(self.command_buffers[0],
                                                      &renderpass_begin_info,
                                                      SubpassContents::SECONDARY_COMMAND_BUFFERS);

            let command_buffers = self
                .update_secondary_command_buffers(ptr, viewports[0], scissors[0])?;
            self.logical_device.cmd_execute_commands(self.command_buffers[0], command_buffers.as_slice());
            self.logical_device.cmd_end_render_pass(self.command_buffers[0]);
            let result = self.logical_device.end_command_buffer(self.command_buffers[0]);
            if let Err(e) = result {
                log::error!("Error ending command buffer: {}", e.to_string());
            }
        }
        Ok(())
    }

    pub async fn create_buffer<VertexType: 'static + Send>(graphics: Arc<ShardedLock<Self>>,
                                                           vertices: Vec<VertexType>, indices: Vec<u32>,
                                                           command_pool: Arc<Mutex<ash::vk::CommandPool>>) -> anyhow::Result<(super::Buffer, super::Buffer)> {
        let device: Arc<ash::Device>;
        let allocator: Arc<ShardedLock<vk_mem::Allocator>>;
        {
            let lock = graphics.read().unwrap();
            device = lock.logical_device.clone();
            allocator = lock.allocator.clone();
            drop(lock);
        }
        let vertex_buffer_size = DeviceSize::try_from(std::mem::size_of::<VertexType>() * vertices.len())?;
        let index_buffer_size = DeviceSize::try_from(std::mem::size_of::<u32>() * indices.len())?;
        let cmd_buffer = get_single_time_command_buffer(
            device.as_ref(), *command_pool.lock()
        );

        let device_handle1 = device.clone();
        let allocator_handle1 = allocator.clone();
        let vertices_handle = tokio::spawn(async move {
            let device_handle = device_handle1;
            let allocator_handle = allocator_handle1;
            let mut vertex_staging = super::Buffer::new(
                Arc::downgrade(&device_handle), vertex_buffer_size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                Arc::downgrade(&allocator_handle)
            );
            let vertex_mapped = vertex_staging.map_memory(vertex_buffer_size, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(vertices.as_ptr() as *const c_void, vertex_mapped, vertex_buffer_size as usize);
            }
            let vertex_buffer = super::Buffer::new(
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
            let mut index_staging = super::Buffer::new(
                Arc::downgrade(&device_handle), index_buffer_size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                Arc::downgrade(&allocator_handle)
            );
            let index_mapped = index_staging.map_memory(index_buffer_size, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(indices.as_ptr() as *const c_void, index_mapped, index_buffer_size as usize);
            }
            let index_buffer = super::Buffer::new(
                Arc::downgrade(&device_handle), index_buffer_size,
                BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL, Arc::downgrade(&allocator_handle)
            );
            (index_staging, index_buffer)
        });

        let (vertex_staging, vertex_buffer) = vertices_handle.await?;
        let (index_staging, index_buffer) = indices_handle.await?;
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
        Ok((vertex_buffer, index_buffer))
    }

    pub async fn create_gltf_textures(images: Vec<gltf::image::Data>,
                                      graphics: Arc<ShardedLock<Graphics>>,
                                      command_pool: Arc<Mutex<CommandPool>>) -> anyhow::Result<Vec<Arc<ShardedLock<super::Image>>>> {
        let mut textures = vec![];
        let mut texture_handles = vec![];
        use gltf::image::Format;
        for image in images.iter() {
            let buffer_size = image.width * image.height * 4;
            let texture: JoinHandle<anyhow::Result<super::Image>>;
            let pool = command_pool.clone();
            let g = graphics.clone();
            let width = image.width;
            let height = image.height;
            let format = image.format;
            let pixels = match image.format {
                Format::R8G8B8 | Format::B8G8R8 => {
                    interpolate_alpha(image.pixels.to_vec(), width, height, buffer_size as usize)
                },
                _ => image.pixels.to_vec()
            };
            texture = tokio::spawn(async move {
                Self::create_image_from_raw(
                    pixels, buffer_size as u64,
                    width, height, match format {
                        Format::B8G8R8 => ImageFormat::GltfFormat(Format::B8G8R8A8),
                        Format::R8G8B8 => ImageFormat::GltfFormat(Format::R8G8B8A8),
                        _ => ImageFormat::GltfFormat(format)
                    }, g, pool, SamplerAddressMode::REPEAT
                )
            });
            texture_handles.push(texture);
        }
        for handle in texture_handles.into_iter() {
            textures.push(handle.await??);
        }
        let graphics_lock = graphics.read().unwrap();
        let resource_manager = graphics_lock.resource_manager.clone();
        drop(graphics_lock);
        match resource_manager.upgrade() {
            Some(rm) => {
                let mut rm_lock = rm.write().unwrap();
                let textures_ptrs = textures.into_iter()
                    .map(|img| rm_lock.add_texture(img))
                    .collect::<Vec<_>>();
                log::info!("Model texture count: {}", textures_ptrs.len());
                Ok(textures_ptrs)
            },
            None => {
                panic!("Failed to upgrade resource manager.");
            }
        }
    }

    pub async fn create_image_from_file(file_name: &str, graphics: Arc<ShardedLock<Graphics>>,
                                        command_pool: Arc<Mutex<CommandPool>>,
                                        sampler_address_mode: SamplerAddressMode) -> anyhow::Result<Arc<ShardedLock<super::Image>>> {
        let image = image::open(file_name)?;
        let buffer_size = image.width() * image.height() * 4;
        let bytes = match image.color() {
            image::ColorType::Bgr8 | image::ColorType::Rgb8 => interpolate_alpha(image.to_bytes(), image.width(), image.height(), buffer_size as usize),
            _ => image.to_bytes()
        };
        let width = image.width();
        let height = image.height();
        let color_type = image.color();
        let graphics_clone = graphics.clone();
        let command_pool_clone = command_pool.clone();
        use image::ColorType;
        let texture = tokio::spawn(async move {
            Self::create_image_from_raw(
                bytes, buffer_size as u64, width, height,
                match color_type {
                    ColorType::Rgb8 => ImageFormat::ColorType(ColorType::Rgba8),
                    ColorType::Bgr8 => ImageFormat::ColorType(ColorType::Bgra8),
                    _ => ImageFormat::ColorType(color_type)
                }, graphics_clone, command_pool_clone,
                sampler_address_mode)
        }).await??;
        let resource_manager = graphics
            .read()
            .unwrap()
            .resource_manager
            .clone();
        match resource_manager.upgrade() {
            Some(rm) => {
                let mut rm_lock = rm.write().unwrap();
                let image = rm_lock.add_texture(texture);
                Ok(image)
            },
            None => {
                panic!("Failed to upgrade resource manager.");
            }
        }
    }

    pub fn create_secondary_command_buffer(&self, command_pool: Arc<Mutex<CommandPool>>) -> CommandBuffer {
        let allocate_info = CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .level(CommandBufferLevel::SECONDARY)
            .command_pool(*command_pool.lock());
        unsafe {
            let buffer = self.logical_device
                .allocate_command_buffers(&allocate_info)
                .expect("Failed to allocate secondary command buffer.");
            buffer[0]
        }
    }

    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        self.create_dynamic_model_buffers()?;
        self.allocate_descriptor_set()?;
        let color_format: Format = self.swapchain.format.format;
        let depth_format = self.depth_format;
        let sample_count = self.sample_count;
        self.pipeline.create_renderpass(
            color_format,
            depth_format,
            sample_count);
        self.create_graphics_pipeline(ShaderType::BasicShader).await;
        self.create_graphics_pipeline(ShaderType::BasicShaderWithoutTexture).await;
        self.create_graphics_pipeline(ShaderType::AnimatedModel).await;
        let width = self.swapchain.extent.width;
        let height = self.swapchain.extent.height;
        self.frame_buffers = Self::create_frame_buffers(
            width, height, self.pipeline.render_pass,
            &self.swapchain, &self.depth_image, &self.msaa_image, self.logical_device.as_ref()
        );
        Ok(())
    }

    pub fn render(&self) -> anyhow::Result<u32> {
        unsafe {
            let swapchain_loader = self.swapchain.swapchain_loader.clone();
            let result = swapchain_loader
                .acquire_next_image(self.swapchain.swapchain, u64::MAX,
                                    self.acquired_semaphores[self.current_index], Fence::null());
            if let Err(e) = result {
                return Err(anyhow::anyhow!("Error acquiring swapchain image: {}", e.to_string()));
            }
            let (image_index, _is_suboptimal) = result.unwrap();
            self.begin_draw(self.frame_buffers[image_index as usize])?;

            let wait_stages = vec![PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let submit_info = vec![SubmitInfo::builder()
                .command_buffers(&[self.command_buffers[0]])
                .signal_semaphores(&[self.complete_semaphores[self.current_index]])
                .wait_dst_stage_mask(wait_stages.as_slice())
                .wait_semaphores(&[self.acquired_semaphores[self.current_index]])
                .build()];

            self.logical_device.reset_fences(&[self.fences[self.current_index]])
                .expect("Failed to reset fences.");
            self.logical_device.queue_submit(*self.graphics_queue.lock(), submit_info.as_slice(), self.fences[self.current_index])
                .expect("Failed to submit the queue.");

            let present_info = PresentInfoKHR::builder()
                .wait_semaphores(&[self.complete_semaphores[self.current_index]])
                .image_indices(&[image_index])
                .swapchains(&[self.swapchain.swapchain])
                .build();

            swapchain_loader
                .queue_present(*self.present_queue.lock(), &present_info)
                .expect("Failed to present with the swapchain.");
            self.logical_device.wait_for_fences(&[self.fences[self.current_index]], true, u64::MAX)
                .expect("Failed to wait for fences.");
            Ok(image_index)
        }
    }

    pub fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let resource_arc = self.resource_manager.upgrade().unwrap();
        let mut resource_lock = resource_arc.write().unwrap();
        for model in resource_lock.models.iter_mut() {
            model.lock().update(delta_time);
        }
        for model in resource_lock.skinned_models.iter_mut() {
            model.lock().update(delta_time);
        }
        drop(resource_lock);
        self.update_dynamic_buffer(delta_time)?;
        Ok(())
    }

    pub fn update_secondary_command_buffers(&self,
                                            inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
                                            viewport: Viewport, scissor: Rect2D) -> anyhow::Result<Vec<CommandBuffer>> {
        let resource_manager = self.resource_manager.upgrade().unwrap();
        let resource_lock = resource_manager.read().unwrap();
        let thread_count = self.thread_pool.thread_count;
        let dynamic_alignment = self.dynamic_objects.dynamic_alignment;
        let push_constant = self.push_constant;
        let ptr = inheritance_info;
        for model in resource_lock.models.iter() {
            let model_clone = model.clone();
            let model_index = model_clone.lock().model_index;
            let ptr_clone = ptr.clone();
            self.thread_pool.threads[model_index % thread_count]
                .add_job(move || {
                    let model_lock = model_clone.lock();
                    let ptr = ptr_clone;
                    model_lock
                        .render(ptr, dynamic_alignment, push_constant, viewport, scissor);
                });
        }
        for model in resource_lock.skinned_models.iter() {
            let model_clone = model.clone();
            let model_index = model_clone.lock().model_index;
            let ptr_clone = ptr.clone();
            self.thread_pool.threads[model_index % thread_count]
                .add_job(move || {
                    let model_lock = model_clone.lock();
                    let ptr = ptr_clone;
                    model_lock
                        .render(ptr, dynamic_alignment, push_constant, viewport, scissor);
                });
        }
        self.thread_pool.wait();
        let mut model_command_buffers = resource_lock.models.iter()
            .map(|model| {
                let mesh_command_buffers = model.lock().meshes.iter()
                    .map(|mesh| mesh.command_buffer.unwrap())
                    .collect::<Vec<_>>();
                mesh_command_buffers
            })
            .flatten()
            .collect::<Vec<_>>();
        let mut skinned_model_command_buffers = resource_lock.skinned_models.iter()
            .map(|model| {
                let primitive_command_buffers = model.lock().skinned_meshes.iter()
                    .map(|mesh| mesh.primitives.iter()
                        .map(|primitive| primitive.command_buffer.unwrap())
                        .collect::<Vec<_>>())
                    .flatten()
                    .collect::<Vec<_>>();
                primitive_command_buffers
            })
            .flatten()
            .collect::<Vec<_>>();
        model_command_buffers.append(&mut skinned_model_command_buffers);
        Ok(model_command_buffers)
    }

    unsafe fn dispose(&mut self) {
        for buffer in self.frame_buffers.iter() {
            self.logical_device.destroy_framebuffer(*buffer, None);
        }
        self.logical_device.free_command_buffers(self.command_pool, self.command_buffers.as_slice());
        ManuallyDrop::drop(&mut self.pipeline);
        self.logical_device.destroy_descriptor_pool(*self.descriptor_pool.lock(), None);
        ManuallyDrop::drop(&mut self.uniform_buffers);
        ManuallyDrop::drop(&mut self.msaa_image);
        ManuallyDrop::drop(&mut self.depth_image);
        ManuallyDrop::drop(&mut self.swapchain);
    }

    fn create_descriptor_set_layout(device: &Device) -> DescriptorSetLayout {
        let mut descriptor_set_layout_binding = vec![];
        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::VERTEX | ShaderStageFlags::TESSELLATION_CONTROL)
                .build());

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .build());

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(2)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .stage_flags(ShaderStageFlags::VERTEX | ShaderStageFlags::TESSELLATION_CONTROL)
                .build());

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(3)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .stage_flags(ShaderStageFlags::VERTEX)
                .build());

        let create_info = DescriptorSetLayoutCreateInfo::builder()
            .bindings(descriptor_set_layout_binding.as_slice());
        unsafe {
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&create_info, None)
                .expect("Failed to create descriptor set layout.");
            log::info!("Descriptor set layout successfully created.");
            descriptor_set_layout
        }
    }

    fn create_sampler_descriptor_set_layout(device: &Device) -> DescriptorSetLayout {
        let layout_bindings = vec![DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .build()];
        let create_info = DescriptorSetLayoutCreateInfo::builder()
            .bindings(layout_bindings.as_slice());
        unsafe {
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&create_info, None)
                .expect("Failed to create descriptor set layout for sampler.");
            log::info!("Descriptor set layout for sampler successfully created.");
            descriptor_set_layout
        }
    }

    fn create_ssbo_descriptor_set_layout(device: &Device) -> DescriptorSetLayout {
        let layout_bindings = vec![DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::STORAGE_BUFFER)
            .stage_flags(ShaderStageFlags::VERTEX)
            .build()];
        let create_info = DescriptorSetLayoutCreateInfo::builder()
            .bindings(layout_bindings.as_slice());
        unsafe {
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&create_info, None)
                .expect("Failed to create descriptor set layout for ssbo.");
            log::info!("Descriptor set layout for ssbo successfully created.");
            descriptor_set_layout
        }
    }

    fn create_view_projection(camera: &Camera,
                              device: Weak<Device>, allocator: Weak<ShardedLock<Allocator>>) -> anyhow::Result<super::Buffer> {
        let vp_size = std::mem::size_of::<ViewProjection>();
        let view_projection = ViewProjection::new(
            camera.get_view_matrix(),
            camera.get_projection_matrix()
        );
        unsafe {
            let mut vp_buffer = super::buffer::Buffer::new(
                device,
                DeviceSize::try_from(vp_size)?,
                BufferUsageFlags::UNIFORM_BUFFER,
                MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT, allocator);
            let mapped = vp_buffer.map_memory(u64::try_from(vp_size)?, 0);
            std::ptr::copy_nonoverlapping(&view_projection as *const _ as *const c_void, mapped, vp_size);
            Ok(vp_buffer)
        }
    }

    fn create_directional_light(directional: &Directional,
                              device: Weak<Device>, allocator: Weak<ShardedLock<Allocator>>) -> anyhow::Result<super::Buffer> {
        let dl_size = std::mem::size_of::<Directional>();
        unsafe {
            let mut dl_buffer = super::buffer::Buffer::new(
                device,
                DeviceSize::try_from(dl_size)?,
                BufferUsageFlags::UNIFORM_BUFFER,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE, allocator);
            let mapped = dl_buffer.map_memory(u64::try_from(dl_size)?, 0);
            std::ptr::copy(directional as *const _ as *const c_void, mapped, dl_size);
            Ok(dl_buffer)
        }
    }

    fn create_dynamic_model_buffers(&mut self) -> anyhow::Result<()> {
        let arc = self.resource_manager.upgrade();
        if arc.is_none() {
            panic!("Resource manager has been destroyed.");
        }
        let resource_manager = arc.unwrap();
        let resource_lock = resource_manager.read().unwrap();
        if resource_lock.models.is_empty() && resource_lock.skinned_models.is_empty() {
            return Err(anyhow::anyhow!("There are no models in resource manager."));
        }
        let model_count = resource_lock.get_model_count() + resource_lock.get_skinned_model_count();
        let mut matrices = vec![Mat4::identity(); model_count];
        for model in resource_lock.models.iter() {
            let model_lock = model.lock();
            matrices[model_lock.model_index] = model_lock.get_world_matrix();
        }
        for model in resource_lock.skinned_models.iter() {
            let model_lock = model.lock();
            matrices[model_lock.model_index] = model_lock.get_world_matrix();
        }
        drop(resource_lock);
        drop(resource_manager);
        let dynamic_alignment = self.dynamic_objects.dynamic_alignment;
        let buffer_size = dynamic_alignment * DeviceSize::try_from(matrices.len())?;
        let mut dynamic_model = DynamicModel {
            model_matrices: matrices,
            buffer: std::ptr::null_mut()
        };
        dynamic_model.buffer = aligned_alloc::aligned_alloc(buffer_size as usize, dynamic_alignment as usize) as *mut Mat4;
        assert_ne!(dynamic_model.buffer, std::ptr::null_mut());

        let mut buffer = super::Buffer::new(
            Arc::downgrade(&self.logical_device),
            buffer_size,
            BufferUsageFlags::UNIFORM_BUFFER,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT, Arc::downgrade(&self.allocator));
        unsafe {
            for (i, model) in dynamic_model.model_matrices.iter().enumerate() {
                let ptr = std::mem::transmute::<usize, *mut Mat4>(
                    std::mem::transmute::<*mut Mat4, usize>(dynamic_model.buffer) +
                        (i * (dynamic_alignment as usize))
                );
                *ptr = model.clone();
            }
            let mapped = buffer.map_memory(WHOLE_SIZE, 0);
            std::ptr::copy_nonoverlapping(dynamic_model.buffer as *mut c_void, mapped, buffer_size as usize);
        }
        self.dynamic_objects.models = dynamic_model;
        self.uniform_buffers.model_buffer = Some(ManuallyDrop::new(buffer));
        Ok(())
    }

    fn allocate_descriptor_set(&mut self) -> anyhow::Result<()> {
        let mut texture_count = 0_usize;
        let mut ssbo_count = 0;
        if let Some(r) = self.resource_manager.upgrade() {
            let resource_lock = r.read().unwrap();
            for model in resource_lock.models.iter() {
                for mesh in model.lock().meshes.iter() {
                    texture_count += if mesh.texture.is_empty() {
                        0
                    } else {
                        mesh.texture.len()
                    };
                }
            }
            for model in resource_lock.skinned_models.iter() {
                for mesh in model.lock().skinned_meshes.iter() {
                    for primitive in mesh.primitives.iter() {
                        texture_count += if primitive.texture.is_some() {
                            1
                        } else {
                            0
                        };
                    }
                    ssbo_count += 1;
                }
            }
            drop(resource_lock);
        }
        let mut pool_sizes = vec![];
        pool_sizes.push(DescriptorPoolSize::builder()
            .descriptor_count(1)
            .ty(DescriptorType::UNIFORM_BUFFER)
            .build());
        pool_sizes.push(DescriptorPoolSize::builder()
            .descriptor_count(1)
            .ty(DescriptorType::UNIFORM_BUFFER)
            .build());
        pool_sizes.push(DescriptorPoolSize::builder()
            .descriptor_count(1)
            .ty(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .build());
        pool_sizes.push(DescriptorPoolSize::builder()
            .descriptor_count(1)
            .ty(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .build());
        pool_sizes.push(DescriptorPoolSize::builder()
            .descriptor_count(texture_count as u32)
            .ty(DescriptorType::COMBINED_IMAGE_SAMPLER)
            .build());
        pool_sizes.push(DescriptorPoolSize::builder()
            .descriptor_count(ssbo_count as u32)
            .ty(DescriptorType::STORAGE_BUFFER)
            .build());

        let image_count = self.swapchain.swapchain_images.len();
        let pool_info = DescriptorPoolCreateInfo::builder()
            .max_sets(u32::try_from(image_count + texture_count + ssbo_count)?)
            .pool_sizes(pool_sizes.as_slice());

        unsafe {
            self.descriptor_pool = Arc::new(Mutex::new(self.logical_device
                .create_descriptor_pool(&pool_info, None)
                .expect("Failed to create descriptor pool.")));
            log::info!("Descriptor pool successfully created.");
            let set_layout = vec![self.descriptor_set_layout];
            let allocate_info = DescriptorSetAllocateInfo::builder()
                .descriptor_pool(*self.descriptor_pool.lock())
                .set_layouts(set_layout.as_slice());
            self.descriptor_sets = self.logical_device
                .allocate_descriptor_sets(&allocate_info)
                .expect("Failed to allocate descriptor sets.");

            log::info!("Descriptor sets successfully allocated.");

            let vp_buffer = &self.uniform_buffers.view_projection;
            let vp_buffer_info = vec![DescriptorBufferInfo::builder()
                .buffer(vp_buffer.buffer)
                .offset(0)
                .range(vp_buffer.buffer_size)
                .build()];

            let dl_buffer = &self.uniform_buffers.directional_light;
            let dl_buffer_info = vec![DescriptorBufferInfo::builder()
                .buffer(dl_buffer.buffer)
                .offset(0)
                .range(dl_buffer.buffer_size)
                .build()];

            let mut write_descriptors = vec![];
            write_descriptors.push(WriteDescriptorSet::builder()
                .dst_array_element(0)
                .buffer_info(vp_buffer_info.as_slice())
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .dst_binding(0)
                .dst_set(self.descriptor_sets[0])
                .build());
            write_descriptors.push(WriteDescriptorSet::builder()
                .dst_array_element(0)
                .buffer_info(dl_buffer_info.as_slice())
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .dst_binding(1)
                .dst_set(self.descriptor_sets[0])
                .build());

            let model_buffer_info = vec![DescriptorBufferInfo::builder()
                .range(WHOLE_SIZE)
                .offset(0)
                .buffer(self.uniform_buffers.model_buffer
                    .as_ref()
                    .unwrap()
                    .buffer)
                .build()];

            let mesh_buffer_info = vec![DescriptorBufferInfo::builder()
                .range(WHOLE_SIZE)
                .offset(0)
                .buffer(ash::vk::Buffer::null())
                .build()];

            if self.uniform_buffers.model_buffer.is_some() {
                write_descriptors.push(WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(model_buffer_info.as_slice())
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                    .dst_binding(2)
                    .dst_set(self.descriptor_sets[0])
                    .build());
            }

            if self.uniform_buffers.mesh_buffer.is_some() {
                write_descriptors.push(WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(mesh_buffer_info.as_slice())
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                    .dst_binding(3)
                    .dst_set(self.descriptor_sets[0])
                    .build());
            }

            self.logical_device.update_descriptor_sets(write_descriptors.as_slice(), &[]);
            log::info!("Descriptor successfully updated.");
            Ok(())
        }
    }

    async fn create_graphics_pipeline(&mut self, shader_type: ShaderType) {
        unsafe {
            let shaders = vec![
                super::Shader::new(
                    self.logical_device.clone(),
                    match shader_type {
                        ShaderType::AnimatedModel => "./shaders/basicShader_animated.spv",
                        _ => "./shaders/vert.spv"
                    },
                    ShaderStageFlags::VERTEX
                ),
                super::Shader::new(
                    self.logical_device.clone(),
                    match shader_type {
                        ShaderType::BasicShader => "./shaders/frag.spv",
                        ShaderType::BasicShaderWithoutTexture => "./shaders/basicShader_noTexture.spv",
                        _ => "./shaders/frag.spv"
                    },
                    ShaderStageFlags::FRAGMENT
                )
            ];

            let mut descriptor_set_layout = vec![self.descriptor_set_layout];
            match shader_type {
                ShaderType::BasicShader => {
                    descriptor_set_layout.push(self.sampler_descriptor_set_layout);
                },
                ShaderType::AnimatedModel => {
                    descriptor_set_layout.push(self.sampler_descriptor_set_layout);
                    descriptor_set_layout.push(self.ssbo_descriptor_set_layout);
                },
                _ => ()
            }
            self.pipeline.create_graphic_pipelines(
                descriptor_set_layout.as_slice(),
                self.sample_count, shaders, shader_type)
                .await;
        }
    }

    fn allocate_command_buffers(device: &Device, command_pool: CommandPool, image_count: u32) -> Vec<CommandBuffer> {
        let command_buffer_info = CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(image_count)
            .level(CommandBufferLevel::PRIMARY);
        unsafe {
            let cmd_buffers = device.allocate_command_buffers(&command_buffer_info)
                .expect("Failed to allocate command buffers.");
            cmd_buffers
        }
    }

    fn create_frame_buffers(frame_width: u32, frame_height: u32,
                            renderpass: RenderPass, swapchain: &super::Swapchain,
                            depth_image: &super::Image, msaa_image: &super::Image, device: &Device) -> Vec<Framebuffer> {
        let mut frame_buffers = vec![];
        let image_count = swapchain.swapchain_images.len();
        for i in 0..image_count {
            let image_views = vec![
                msaa_image.image_view,
                depth_image.image_view,
                swapchain.swapchain_images[i].image_view
            ];
            let frame_buffer_info = FramebufferCreateInfo::builder()
                .height(frame_height)
                .width(frame_width)
                .layers(1)
                .attachments(image_views.as_slice())
                .render_pass(renderpass);
            unsafe {
                frame_buffers.push(
                    device.create_framebuffer(&frame_buffer_info, None)
                        .expect("Failed to create framebuffer.")
                );
            }
        }
        frame_buffers
    }

    fn create_sync_object(device: &Device, image_count: u32) -> (Vec<Fence>, Vec<Semaphore>, Vec<Semaphore>) {
        let fence_info = FenceCreateInfo::builder();
        let semaphore_info = SemaphoreCreateInfo::builder();
        let mut fences = vec![];
        let mut acquired_semaphores = vec![];
        let mut complete_semaphores = vec![];
        unsafe {
            for _ in 0..image_count {
                fences.push(
                    device.create_fence(&fence_info, None).expect("Failed to create fence.")
                );
                acquired_semaphores.push(
                    device.create_semaphore(&semaphore_info, None).expect("Failed to create semaphore.")
                );
                complete_semaphores.push(
                    device.create_semaphore(&semaphore_info, None).expect("Failed to create semaphore.")
                );
            }
        }
        (fences, acquired_semaphores, complete_semaphores)
    }

    fn create_image_from_raw(image_data: Vec<u8>, buffer_size: DeviceSize, width: u32, height: u32,
                                 format: ImageFormat,
                                 graphics: Arc<ShardedLock<Self>>,
                                 command_pool: Arc<Mutex<ash::vk::CommandPool>>,
                             sampler_address_mode: SamplerAddressMode) -> anyhow::Result<super::Image> {
        let lock = graphics.read().unwrap();
        let device = lock.logical_device.clone();
        let allocator = lock.allocator.clone();
        let image_format = match format {
            ImageFormat::GltfFormat(gltf_format) => match gltf_format {
                gltf::image::Format::B8G8R8A8 => ash::vk::Format::B8G8R8A8_UNORM,
                gltf::image::Format::R8G8B8A8 => ash::vk::Format::R8G8B8A8_UNORM,
                _ => lock.swapchain.format.format
            },
            ImageFormat::VkFormat(vk_format) => vk_format,
            ImageFormat::ColorType(color_type) => match color_type {
                image::ColorType::Bgra8 => ash::vk::Format::B8G8R8A8_UNORM,
                image::ColorType::Rgba8 => ash::vk::Format::R8G8B8A8_UNORM,
                image::ColorType::L16 => ash::vk::Format::R16_UNORM,
                _ => lock.swapchain.format.format
            }
        };
        let cmd_buffer = get_single_time_command_buffer(
            device.as_ref(), *command_pool.lock()
        );

        let mut staging = super::Buffer::new(
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
        let mut image = super::Image::new(
            Arc::downgrade(&device),
            ImageUsageFlags::TRANSFER_SRC | ImageUsageFlags::TRANSFER_DST | ImageUsageFlags::SAMPLED,
            MemoryPropertyFlags::DEVICE_LOCAL, image_format,
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
        image.create_sampler(mip_levels, sampler_address_mode);
        end_one_time_command_buffer(cmd_buffer, device.as_ref(), *pool_lock, *lock.graphics_queue.lock());
        Ok(image)
    }

    fn update_dynamic_buffer(&mut self, _delta_time: f64) -> anyhow::Result<()> {
        let resource_manager = self.resource_manager.upgrade().unwrap();
        let resource_lock = resource_manager.read().unwrap();
        let model_count = resource_lock.get_model_count() + resource_lock.get_skinned_model_count();
        let mut matrices = vec![Mat4::identity(); model_count];
        for model in resource_lock.models.iter() {
            let model_lock = model.lock();
            matrices[model_lock.model_index] = model_lock.get_world_matrix();
        }
        for model in resource_lock.skinned_models.iter() {
            let model_lock = model.lock();
            matrices[model_lock.model_index] = model_lock.get_world_matrix();
        }
        drop(resource_lock);
        drop(resource_manager);
        self.dynamic_objects.models.model_matrices = matrices;
        let dynamic_model = &self.dynamic_objects.models;
        let dynamic_alignment = self.dynamic_objects.dynamic_alignment;
        unsafe {
            for (i, model) in dynamic_model.model_matrices.iter().enumerate() {
                let ptr = std::mem::transmute::<usize, *mut Mat4>(
                    std::mem::transmute::<*mut Mat4, usize>(dynamic_model.buffer) +
                        (i * (dynamic_alignment as usize))
                );
                *ptr = model.clone();
            }
            let mapped = self.uniform_buffers.model_buffer
                .as_ref()
                .unwrap()
                .mapped_memory;
            let buffer_size = self.uniform_buffers.model_buffer
                .as_ref()
                .unwrap()
                .buffer_size;
            std::ptr::copy_nonoverlapping(dynamic_model.buffer as *mut c_void, mapped, buffer_size as usize);
            Ok(())
        }
    }
}

impl GraphicsBase<super::Buffer, CommandBuffer, super::Image> for Graphics {
    fn get_commands(&self) -> &Vec<CommandBuffer> {
        &self.command_buffers
    }
}

unsafe impl Send for Graphics {}
unsafe impl Sync for Graphics {}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            log::info!("Dropping graphics...");
            self.logical_device.device_wait_idle()
                .expect("Failed to wait for device to idle.");
            self.dispose();
            for semaphore in self.complete_semaphores.iter() {
                self.logical_device.destroy_semaphore(*semaphore, None);
            }
            for semaphore in self.acquired_semaphores.iter() {
                self.logical_device.destroy_semaphore(*semaphore, None);
            }
            for fence in self.fences.iter() {
                self.logical_device.destroy_fence(*fence, None);
            }
            for thread in self.thread_pool.threads.iter() {
                self.logical_device.destroy_command_pool(*thread.command_pool.lock(), None);
            }
            self.logical_device.destroy_command_pool(self.command_pool, None);
            self.logical_device.destroy_descriptor_set_layout(self.ssbo_descriptor_set_layout, None);
            self.logical_device.destroy_descriptor_set_layout(self.sampler_descriptor_set_layout, None);
            self.logical_device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.allocator.write().unwrap().destroy();
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            if self.debug_messenger != DebugUtilsMessengerEXT::null() {
                let debug_loader = DebugUtils::new(&self.entry, &self.instance);
                debug_loader.destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            self.instance.destroy_instance(None);
            log::info!("Successfully dropped graphics.");
        }
    }
}