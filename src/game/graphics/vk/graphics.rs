use ash::{
    extensions::{ext::DebugUtils, khr::Surface},
    version::{DeviceV1_0, InstanceV1_0},
    vk::*,
    Device, Entry, Instance,
};
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use std::cell::RefCell;
use std::convert::TryFrom;
use std::ffi::{c_void, CString};
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Weak};
use vk_mem::*;

use crate::game::enums::ShaderType;
use crate::game::graphics::vk::{
    DynamicBufferObject, DynamicModel, Initializer, ThreadPool, UniformBuffers,
};
use crate::game::shared::enums::ImageFormat;
use crate::game::shared::structs::{Directional, PushConstant, ViewProjection};
use crate::game::shared::traits::GraphicsBase;
use crate::game::shared::util::interpolate_alpha;
use crate::game::traits::Mappable;
use crate::game::util::{end_one_time_command_buffer, get_single_time_command_buffer};
use crate::game::{Camera, ResourceManager};
use ash::prelude::VkResult;

const SSBO_DATA_COUNT: usize = 50;

type ResourceManagerHandle =
    Weak<RwLock<ResourceManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>;

struct PrimarySSBOData {
    world_matrices: [Mat4; SSBO_DATA_COUNT],
    object_colors: [Vec4; SSBO_DATA_COUNT],
    reflectivities: [f32; SSBO_DATA_COUNT],
    shine_dampers: [f32; SSBO_DATA_COUNT],
}

pub struct Graphics {
    pub dynamic_objects: DynamicBufferObject,
    pub logical_device: Arc<Device>,
    pub pipeline: Arc<ShardedLock<ManuallyDrop<super::Pipeline>>>,
    pub descriptor_sets: Vec<DescriptorSet>,
    pub push_constant: PushConstant,
    pub descriptor_set_layout: DescriptorSetLayout,
    pub ssbo_descriptor_set_layout: DescriptorSetLayout,
    pub descriptor_pool: Arc<Mutex<DescriptorPool>>,
    pub command_pool: CommandPool,
    pub thread_pool: Arc<ThreadPool>,
    pub allocator: Arc<ShardedLock<Allocator>>,
    pub graphics_queue: Arc<Mutex<Queue>>,
    pub present_queue: Arc<Mutex<Queue>>,
    pub compute_queue: Arc<Mutex<Queue>>,
    pub swapchain: ManuallyDrop<super::Swapchain>,
    pub frame_buffers: Vec<Framebuffer>,
    pub resource_manager: ResourceManagerHandle,
    pub is_initialized: bool,
    entry: Entry,
    instance: Instance,
    surface_loader: Surface,
    debug_messenger: DebugUtilsMessengerEXT,
    surface: SurfaceKHR,
    physical_device: super::PhysicalDevice,
    depth_image: ManuallyDrop<super::Image>,
    msaa_image: ManuallyDrop<super::Image>,
    uniform_buffers: ManuallyDrop<UniformBuffers>,
    camera: Rc<RefCell<Camera>>,
    command_buffers: Vec<CommandBuffer>,
    fence: Fence,
    acquired_semaphore: Semaphore,
    completed_semaphore: Semaphore,
    sample_count: SampleCountFlags,
    depth_format: Format,
    sky_color: Vec4,
}

impl Graphics {
    pub fn new(
        window: &winit::window::Window,
        camera: Rc<RefCell<Camera>>,
        resource_manager: ResourceManagerHandle,
    ) -> anyhow::Result<Self> {
        let debug = dotenv::var("DEBUG")?.parse::<bool>()?;
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
        let (logical_device, graphics_queue, present_queue, compute_queue) =
            Initializer::create_logical_device(&instance, &physical_device, &enabled_layers, debug);
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
            &surface_loader,
            surface,
            &physical_device,
            window,
            &instance,
            Arc::downgrade(&device),
            Arc::downgrade(&allocator),
        );

        let command_pool_create_info = CommandPoolCreateInfo::builder()
            .queue_family_index(
                physical_device
                    .queue_indices
                    .graphics_family
                    .unwrap_or_default(),
            )
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool: CommandPool;
        unsafe {
            command_pool = device
                .create_command_pool(&command_pool_create_info, None)
                .expect("Failed to create command pool.");
        }
        let cpu_count = num_cpus::get();
        let thread_pool = Arc::new(ThreadPool::new(
            cpu_count,
            device.as_ref(),
            physical_device
                .queue_indices
                .graphics_family
                .unwrap_or_default(),
        ));
        let sample_count = Initializer::get_sample_count(&instance, &physical_device);
        let depth_format = Initializer::get_depth_format(&instance, &physical_device);
        let depth_image = Initializer::create_depth_image(
            Arc::downgrade(&device),
            depth_format,
            &swapchain,
            command_pool,
            graphics_queue,
            sample_count,
            Arc::downgrade(&allocator),
        );

        let msaa_image = Initializer::create_msaa_image(
            Arc::downgrade(&device),
            &swapchain,
            command_pool,
            graphics_queue,
            sample_count,
            Arc::downgrade(&allocator),
        );

        let view_projection = Initializer::create_view_projection(
            &*camera.borrow(),
            Arc::downgrade(&device),
            Arc::downgrade(&allocator),
        )?;
        let directional_light = Directional::new(
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Vec3A::new(-200.0, 200.0, 0.0),
            0.1,
            0.5,
        );
        let directional = Initializer::create_directional_light(
            &directional_light,
            Arc::downgrade(&device),
            Arc::downgrade(&allocator),
        )?;

        let ssbo_descriptor_set_layout =
            Initializer::create_ssbo_descriptor_set_layout(device.as_ref());
        let uniform_buffers = UniformBuffers::new(view_projection, directional);
        let min_alignment = physical_device
            .device_properties
            .limits
            .min_uniform_buffer_offset_alignment;
        let min_alignment = min_alignment as usize;
        let dynamic_alignment = if min_alignment > 0 {
            let mat4_size = std::mem::size_of::<Mat4>();
            (mat4_size + (min_alignment - 1)) & !(min_alignment - 1)
        } else {
            std::mem::size_of::<Mat4>()
        };
        let pipeline = super::Pipeline::new(device.clone());
        let command_buffers = Initializer::allocate_command_buffers(
            device.as_ref(),
            command_pool,
            swapchain.swapchain_images.len() as u32,
        );
        let (fence, acquired_semaphore, completed_semaphore) =
            Initializer::create_sync_object(device.as_ref());

        let sky_color: Vec4 = Vec4::new(0.5, 0.5, 0.5, 1.0);
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
            descriptor_set_layout: DescriptorSetLayout::null(),
            uniform_buffers: ManuallyDrop::new(uniform_buffers),
            push_constant: PushConstant::new(0, 0, sky_color),
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
            pipeline: Arc::new(ShardedLock::new(ManuallyDrop::new(pipeline))),
            command_buffers,
            frame_buffers: vec![],
            fence,
            acquired_semaphore,
            completed_semaphore,
            sample_count,
            depth_format,
            allocator,
            thread_pool,
            ssbo_descriptor_set_layout,
            sky_color,
            is_initialized: false,
        })
    }

    pub fn begin_draw(&self, frame_buffer: Framebuffer) -> anyhow::Result<()> {
        let clear_color = ClearColorValue {
            float32: self.sky_color.into(),
        };
        let clear_depth = ClearDepthStencilValue::builder().depth(1.0).stencil(0);
        let clear_values = vec![
            ClearValue { color: clear_color },
            ClearValue {
                depth_stencil: *clear_depth,
            },
        ];
        let extent = self.swapchain.extent;
        let render_area = Rect2D::builder().extent(extent).offset(Offset2D::default());
        let mut renderpass_ptr = AtomicPtr::new(std::ptr::null_mut());
        {
            let renderpass_begin_info = Box::new(
                RenderPassBeginInfo::builder()
                    .render_pass(
                        self.pipeline
                            .read()
                            .expect("Failed to lock pipeline for beginning the renderpass.")
                            .render_pass,
                    )
                    .clear_values(clear_values.as_slice())
                    .render_area(*render_area)
                    .framebuffer(frame_buffer),
            );
            renderpass_ptr = AtomicPtr::new(Box::into_raw(renderpass_begin_info));
        }
        let renderpass_ptr = Arc::new(renderpass_ptr);
        let viewports = vec![Viewport::builder()
            .width(extent.width as f32)
            .height(extent.height as f32)
            .x(0.0)
            .y(0.0)
            .min_depth(0.0)
            .max_depth(1.0)
            .build()];
        let scissors = vec![Rect2D::builder()
            .extent(extent)
            .offset(Offset2D::default())
            .build()];
        let mut inheritance_ptr = AtomicPtr::new(std::ptr::null_mut());
        {
            let inheritance_info = Box::new(
                CommandBufferInheritanceInfo::builder()
                    .framebuffer(frame_buffer)
                    .render_pass(
                        self.pipeline
                            .read()
                            .expect("Failed to lock pipeline for creating the inheritance info.")
                            .render_pass,
                    )
                    .build(),
            );
            inheritance_ptr = AtomicPtr::new(Box::into_raw(inheritance_info));
        }
        let inheritance_ptr = Arc::new(inheritance_ptr);
        unsafe {
            let result = self
                .logical_device
                .begin_command_buffer(self.command_buffers[0], &CommandBufferBeginInfo::builder());
            if let Err(e) = result {
                log::error!("Error beginning command buffer: {}", e.to_string());
            }
            self.logical_device.cmd_begin_render_pass(
                self.command_buffers[0],
                &*renderpass_ptr.load(Ordering::SeqCst),
                SubpassContents::SECONDARY_COMMAND_BUFFERS,
            );
            let command_buffers =
                self.update_secondary_command_buffers(inheritance_ptr, viewports[0], scissors[0])?;
            self.logical_device
                .cmd_execute_commands(self.command_buffers[0], command_buffers.as_slice());
            self.logical_device
                .cmd_end_render_pass(self.command_buffers[0]);
            let result = self
                .logical_device
                .end_command_buffer(self.command_buffers[0]);
            if let Err(e) = result {
                log::error!("Error ending command buffer: {}", e.to_string());
            }
        }
        Ok(())
    }

    pub fn create_buffer<VertexType: 'static + Send>(
        graphics: Arc<RwLock<Self>>,
        vertices: Vec<VertexType>,
        indices: Vec<u32>,
        command_pool: Arc<Mutex<ash::vk::CommandPool>>,
    ) -> anyhow::Result<(super::Buffer, super::Buffer)> {
        use crossbeam::channel::*;

        let device: Arc<ash::Device>;
        let allocator: Arc<ShardedLock<vk_mem::Allocator>>;
        {
            let lock = graphics.read();
            device = lock.logical_device.clone();
            allocator = lock.allocator.clone();
            drop(lock);
        }
        let vertex_buffer_size =
            DeviceSize::try_from(std::mem::size_of::<VertexType>() * vertices.len())?;
        let index_buffer_size = DeviceSize::try_from(std::mem::size_of::<u32>() * indices.len())?;
        let cmd_buffer = get_single_time_command_buffer(device.as_ref(), *command_pool.lock());

        let device_handle1 = device.clone();
        let allocator_handle1 = allocator.clone();
        let (vertices_send, vertices_recv) = bounded(5);
        rayon::spawn(move || {
            let device_handle = device_handle1;
            let allocator_handle = allocator_handle1;
            let mut vertex_staging = super::Buffer::new(
                Arc::downgrade(&device_handle),
                vertex_buffer_size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                Arc::downgrade(&allocator_handle),
            );
            let vertex_mapped = vertex_staging.map_memory(vertex_buffer_size, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    vertices.as_ptr() as *const c_void,
                    vertex_mapped,
                    vertex_buffer_size as usize,
                );
            }
            let vertex_buffer = super::Buffer::new(
                Arc::downgrade(&device_handle),
                vertex_buffer_size,
                BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
                Arc::downgrade(&allocator_handle),
            );
            vertices_send
                .send((vertex_staging, vertex_buffer))
                .expect("Failed to create vertex buffer.");
        });

        let device_handle2 = device.clone();
        let (indices_send, indices_recv) = bounded(5);
        rayon::spawn(move || {
            let device_handle = device_handle2;
            let allocator_handle = allocator;
            let mut index_staging = super::Buffer::new(
                Arc::downgrade(&device_handle),
                index_buffer_size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                Arc::downgrade(&allocator_handle),
            );
            let index_mapped = index_staging.map_memory(index_buffer_size, 0);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    indices.as_ptr() as *const c_void,
                    index_mapped,
                    index_buffer_size as usize,
                );
            }
            let index_buffer = super::Buffer::new(
                Arc::downgrade(&device_handle),
                index_buffer_size,
                BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
                Arc::downgrade(&allocator_handle),
            );
            indices_send
                .send((index_staging, index_buffer))
                .expect("Failed to create index buffer.");
        });

        let (vertex_staging, vertex_buffer) = vertices_recv.recv()?;
        let (index_staging, index_buffer) = indices_recv.recv()?;
        let graphics_lock = graphics.read();
        let pool_lock = command_pool.lock();
        vertex_buffer.copy_buffer(
            &vertex_staging,
            vertex_buffer_size,
            *pool_lock,
            *graphics_lock.graphics_queue.lock(),
            Some(cmd_buffer),
        );
        index_buffer.copy_buffer(
            &index_staging,
            index_buffer_size,
            *pool_lock,
            *graphics_lock.graphics_queue.lock(),
            Some(cmd_buffer),
        );
        end_one_time_command_buffer(
            cmd_buffer,
            device.as_ref(),
            *pool_lock,
            *graphics_lock.graphics_queue.lock(),
        );
        Ok((vertex_buffer, index_buffer))
    }

    pub fn create_gltf_textures(
        images: Vec<gltf::image::Data>,
        graphics: Arc<RwLock<Self>>,
        command_pool: Arc<Mutex<CommandPool>>,
    ) -> anyhow::Result<(Vec<Arc<ShardedLock<super::Image>>>, usize)> {
        let mut textures = vec![];
        let mut texture_handles = vec![];
        use gltf::image::Format;
        for image in images.iter() {
            let buffer_size = image.width * image.height * 4;
            let pool = command_pool.clone();
            let g = graphics.clone();
            let width = image.width;
            let height = image.height;
            let format = image.format;
            let pixels = match image.format {
                Format::R8G8B8 | Format::B8G8R8 => {
                    interpolate_alpha(image.pixels.to_vec(), width, height, buffer_size as usize)
                }
                _ => image.pixels.to_vec(),
            };

            use crossbeam::channel::*;

            let (texture_send, texture_recv) = bounded(5);
            rayon::spawn(move || {
                let result = Initializer::create_image_from_raw(
                    pixels,
                    buffer_size as u64,
                    width,
                    height,
                    match format {
                        Format::B8G8R8 => ImageFormat::GltfFormat(Format::B8G8R8A8),
                        Format::R8G8B8 => ImageFormat::GltfFormat(Format::R8G8B8A8),
                        _ => ImageFormat::GltfFormat(format),
                    },
                    g,
                    pool,
                    SamplerAddressMode::REPEAT,
                );
                texture_send
                    .send(result)
                    .expect("Failed to send texture result.");
            });
            texture_handles.push(texture_recv);
        }
        for handle in texture_handles.into_iter() {
            textures.push(handle.recv()??);
        }
        let graphics_lock = graphics.read();
        let resource_manager = graphics_lock.resource_manager.clone();
        drop(graphics_lock);
        let texture_index_offset: usize;
        match resource_manager.upgrade() {
            Some(rm) => {
                texture_index_offset = rm.read().get_texture_count();
                let mut rm_lock = rm.write();
                let textures_ptrs = textures
                    .into_iter()
                    .map(|img| rm_lock.add_texture(img))
                    .collect::<Vec<_>>();
                log::info!("Model texture count: {}", textures_ptrs.len());
                Ok((textures_ptrs, texture_index_offset))
            }
            None => {
                panic!("Failed to upgrade resource manager.");
            }
        }
    }

    pub fn create_image_from_file(
        file_name: &str,
        graphics: Arc<RwLock<Self>>,
        command_pool: Arc<Mutex<CommandPool>>,
        sampler_address_mode: SamplerAddressMode,
    ) -> anyhow::Result<(Arc<ShardedLock<super::Image>>, usize)> {
        Initializer::create_image_from_file(
            file_name,
            graphics.clone(),
            command_pool,
            sampler_address_mode,
        )
    }

    pub fn create_secondary_command_buffer(
        device: &ash::Device,
        command_pool: CommandPool,
    ) -> CommandBuffer {
        let allocate_info = CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .level(CommandBufferLevel::SECONDARY)
            .command_pool(command_pool);
        unsafe {
            let buffer = device
                .allocate_command_buffers(&allocate_info)
                .expect("Failed to allocate secondary command buffer.");
            buffer[0]
        }
    }

    pub fn get_command_pool(graphics: &Self, model_index: usize) -> Arc<Mutex<CommandPool>> {
        let thread_count = graphics.thread_pool.thread_count;
        graphics.thread_pool.threads[model_index % thread_count]
            .command_pool
            .clone()
    }

    pub fn get_command_pool_and_secondary_command_buffer(
        graphics: &Self,
        model_index: usize,
    ) -> (Arc<Mutex<CommandPool>>, CommandBuffer) {
        let thread_count = graphics.thread_pool.thread_count;
        let pool_handle = graphics.thread_pool.threads[model_index % thread_count]
            .command_pool
            .clone();
        let command_pool = *pool_handle.lock();
        let device = graphics.logical_device.as_ref();
        let command_buffer = Self::create_secondary_command_buffer(device, command_pool);
        (pool_handle, command_buffer)
    }

    pub fn get_idle_command_pool(&self) -> Arc<Mutex<CommandPool>> {
        self.thread_pool.get_idle_command_pool()
    }

    pub fn initialize(&mut self) -> anyhow::Result<()> {
        self.create_descriptor_set_layout()?;
        //self.create_dynamic_model_buffers()?;
        self.create_primary_ssbo()?;
        self.allocate_descriptor_set()?;
        let color_format: Format = self.swapchain.format.format;
        let depth_format = self.depth_format;
        let sample_count = self.sample_count;
        self.pipeline
            .write()
            .expect("Failed to lock pipeline for creating the renderpass.")
            .create_renderpass(color_format, depth_format, sample_count);
        self.create_graphics_pipeline(ShaderType::BasicShader)?;
        self.create_graphics_pipeline(ShaderType::BasicShaderWithoutTexture)?;
        self.create_graphics_pipeline(ShaderType::AnimatedModel)?;
        self.create_graphics_pipeline(ShaderType::Terrain)?;
        self.create_graphics_pipeline(ShaderType::Water)?;
        let width = self.swapchain.extent.width;
        let height = self.swapchain.extent.height;
        self.frame_buffers = Self::create_frame_buffers(
            width,
            height,
            self.pipeline
                .read()
                .expect("Failed to lock pipeline for creating frame buffers.")
                .render_pass,
            &self.swapchain,
            &self.depth_image,
            &self.msaa_image,
            self.logical_device.as_ref(),
        );
        self.is_initialized = true;
        Ok(())
    }

    pub fn render(&self) -> anyhow::Result<()> {
        if !self.is_initialized {
            return Ok(());
        }
        unsafe {
            let fences = vec![self.fence];
            self.logical_device
                .wait_for_fences(fences.as_slice(), true, 1_000_000_000)
                .expect("Failed to wait for fences.");
            self.logical_device
                .reset_fences(fences.as_slice())
                .expect("Failed to reset fences.");
            let result: VkResult<(u32, bool)>;
            {
                let swapchain_loader = &self.swapchain.swapchain_loader;
                result = swapchain_loader.acquire_next_image(
                    self.swapchain.swapchain,
                    u64::MAX,
                    self.acquired_semaphore,
                    Fence::null(),
                );
            }
            let mut image_index = 0_u32;
            let mut is_suboptimal = false;
            match result {
                Ok(index) => {
                    image_index = index.0;
                    is_suboptimal = index.1;
                }
                Err(e) => match e {
                    ash::vk::Result::ERROR_OUT_OF_DATE_KHR | ash::vk::Result::SUBOPTIMAL_KHR => {
                        return Err(anyhow::anyhow!("Swapchain is out of date or suboptimal."));
                    }
                    _ => (),
                },
            }
            if is_suboptimal {
                return Err(anyhow::anyhow!("Swapchain is suboptimal."));
            }
            self.begin_draw(self.frame_buffers[image_index as usize])?;

            let wait_stages = vec![PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let command_buffers = vec![self.command_buffers[0]];
            let complete_semaphores = vec![self.completed_semaphore];
            let acquired_semaphores = vec![self.acquired_semaphore];
            let submit_info = vec![SubmitInfo::builder()
                .command_buffers(command_buffers.as_slice())
                .signal_semaphores(complete_semaphores.as_slice())
                .wait_dst_stage_mask(wait_stages.as_slice())
                .wait_semaphores(acquired_semaphores.as_slice())
                .build()];

            self.logical_device
                .queue_submit(
                    *self.graphics_queue.lock(),
                    submit_info.as_slice(),
                    fences[0],
                )
                .expect("Failed to submit the queue.");

            let image_indices = vec![image_index];
            let swapchain = vec![self.swapchain.swapchain];
            let present_info = PresentInfoKHR::builder()
                .wait_semaphores(complete_semaphores.as_slice())
                .image_indices(image_indices.as_slice())
                .swapchains(swapchain.as_slice());
            {
                let swapchain_loader = &self.swapchain.swapchain_loader;
                swapchain_loader
                    .queue_present(*self.present_queue.lock(), &present_info)
                    .expect("Failed to present with the swapchain.");
            }
            Ok(())
        }
    }

    pub fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        if !self.is_initialized {
            return Ok(());
        }
        let resource_arc = self.resource_manager.upgrade().unwrap();
        let resource_lock = resource_arc.write();
        for model in resource_lock.model_queue.iter() {
            let mut model_lock = model.lock();
            model_lock.update(delta_time);
        }
        drop(resource_lock);
        //self.update_dynamic_buffer(delta_time)?;

        let vp_size = std::mem::size_of::<ViewProjection>();
        let camera = self.camera.borrow();
        let view_projection =
            ViewProjection::new(camera.get_view_matrix(), camera.get_projection_matrix());
        let mapped = self.uniform_buffers.view_projection.mapped_memory;
        unsafe {
            std::ptr::copy_nonoverlapping(
                &view_projection as *const _ as *const c_void,
                mapped,
                vp_size,
            );
        }
        Ok(())
    }

    pub fn update_secondary_command_buffers(
        &self,
        inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
        viewport: Viewport,
        scissor: Rect2D,
    ) -> anyhow::Result<Vec<CommandBuffer>> {
        let resource_manager = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade resource manager.");
        {
            let push_constant = self.push_constant;
            let ptr = inheritance_info;
            let resource_lock = resource_manager.read();
            for model in resource_lock.model_queue.iter() {
                let ptr_clone = ptr.clone();
                let device_clone = self.logical_device.clone();
                let pipeline_clone = self.pipeline.clone();
                let descriptor_set = self.descriptor_sets[0];
                model.lock().render(
                    ptr_clone,
                    push_constant,
                    viewport,
                    scissor,
                    device_clone,
                    pipeline_clone,
                    descriptor_set,
                    self.thread_pool.clone(),
                );
            }
        }
        self.thread_pool.wait()?;
        let resource_lock = resource_manager.read();
        let command_buffers = resource_lock.command_buffers.to_vec();
        Ok(command_buffers)
    }

    pub fn recreate_swapchain(&mut self) {}

    unsafe fn dispose(&mut self) {
        for buffer in self.frame_buffers.iter() {
            self.logical_device.destroy_framebuffer(*buffer, None);
        }
        self.logical_device
            .free_command_buffers(self.command_pool, self.command_buffers.as_slice());
        let pipeline = &mut *self
            .pipeline
            .write()
            .expect("Failed to lock pipeline for disposal.");
        ManuallyDrop::drop(pipeline);
        self.logical_device
            .destroy_descriptor_pool(*self.descriptor_pool.lock(), None);
        ManuallyDrop::drop(&mut self.uniform_buffers);
        ManuallyDrop::drop(&mut self.msaa_image);
        ManuallyDrop::drop(&mut self.depth_image);
        ManuallyDrop::drop(&mut self.swapchain);
    }

    fn create_descriptor_set_layout(&mut self) -> anyhow::Result<()> {
        let resource_manager = self.resource_manager.upgrade().expect(
            "Failed to upgrade Weak of resource manager for creating descriptor set layout.",
        );
        let resource_lock = resource_manager.read();
        let total_texture_count = resource_lock.get_texture_count();
        drop(resource_lock);
        drop(resource_manager);
        let mut descriptor_set_layout_binding = vec![];
        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::VERTEX)
                .build(),
        );

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .build(),
        );

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(2)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .stage_flags(ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT)
                .build(),
        );

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(3)
                .descriptor_count(total_texture_count as u32)
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .build(),
        );

        let create_info = DescriptorSetLayoutCreateInfo::builder()
            .bindings(descriptor_set_layout_binding.as_slice());
        unsafe {
            let descriptor_set_layout = self
                .logical_device
                .create_descriptor_set_layout(&create_info, None)?;
            log::info!("Descriptor set layout successfully created.");
            self.descriptor_set_layout = descriptor_set_layout;
            Ok(())
        }
    }

    /*fn create_dynamic_model_buffers(&mut self) -> anyhow::Result<()> {
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
    }*/

    fn create_primary_ssbo(&mut self) -> anyhow::Result<()> {
        let resource_manager = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade Weak of resource manager for creating primary SSBO.");
        let resource_lock = resource_manager.read();
        let is_models_empty = resource_lock.model_queue.is_empty();
        if is_models_empty {
            return Err(anyhow::anyhow!("There are no models in resource manager."));
        }
        let mut model_metadata = PrimarySSBOData {
            world_matrices: [Mat4::identity(); SSBO_DATA_COUNT],
            object_colors: [Vec4::zero(); SSBO_DATA_COUNT],
            reflectivities: [0.0; SSBO_DATA_COUNT],
            shine_dampers: [0.0; SSBO_DATA_COUNT],
        };
        for model in resource_lock.model_queue.iter() {
            let model_lock = model.lock();
            let metadata = model_lock.get_model_metadata();
            let ssbo_index = model_lock.get_ssbo_index();
            model_metadata.world_matrices[ssbo_index] = metadata.world_matrix;
            model_metadata.object_colors[ssbo_index] = metadata.object_color;
            model_metadata.reflectivities[ssbo_index] = metadata.reflectivity;
            model_metadata.shine_dampers[ssbo_index] = metadata.shine_damper;
        }
        let buffer_size = std::mem::size_of::<PrimarySSBOData>();
        drop(resource_lock);
        drop(resource_manager);
        let mut buffer = super::Buffer::new(
            Arc::downgrade(&self.logical_device),
            buffer_size as u64,
            BufferUsageFlags::STORAGE_BUFFER,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
            Arc::downgrade(&self.allocator),
        );
        unsafe {
            let mapped = buffer.map_memory(buffer_size as u64, 0);
            std::ptr::copy_nonoverlapping(
                &model_metadata as *const _ as *const std::ffi::c_void,
                mapped,
                buffer_size,
            );
            self.uniform_buffers.primary_ssbo = Some(ManuallyDrop::new(buffer));
            log::info!("Primary SSBO successfully created.");
        }
        Ok(())
    }

    fn allocate_descriptor_set(&mut self) -> anyhow::Result<()> {
        let resource_manager = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade Weak of resource manager for allocating descriptor set.");
        let resource_lock = resource_manager.read();
        let texture_count = resource_lock.get_texture_count();
        let mut ssbo_count = 1;
        ssbo_count += resource_lock.get_model_count();
        let mut pool_sizes = vec![];
        pool_sizes.push(
            DescriptorPoolSize::builder()
                .descriptor_count(1)
                .ty(DescriptorType::UNIFORM_BUFFER)
                .build(),
        );
        pool_sizes.push(
            DescriptorPoolSize::builder()
                .descriptor_count(1)
                .ty(DescriptorType::UNIFORM_BUFFER)
                .build(),
        );
        pool_sizes.push(
            DescriptorPoolSize::builder()
                .descriptor_count(texture_count as u32)
                .ty(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .build(),
        );
        pool_sizes.push(
            DescriptorPoolSize::builder()
                .descriptor_count(ssbo_count as u32)
                .ty(DescriptorType::STORAGE_BUFFER)
                .build(),
        );

        let image_count = self.swapchain.swapchain_images.len();
        let pool_info = DescriptorPoolCreateInfo::builder()
            .max_sets(u32::try_from(image_count + texture_count + ssbo_count)?)
            .pool_sizes(pool_sizes.as_slice());

        unsafe {
            self.descriptor_pool = Arc::new(Mutex::new(
                self.logical_device
                    .create_descriptor_pool(&pool_info, None)?,
            ));
            log::info!("Descriptor pool successfully created.");
            let set_layout = vec![self.descriptor_set_layout];
            let allocate_info = DescriptorSetAllocateInfo::builder()
                .descriptor_pool(*self.descriptor_pool.lock())
                .set_layouts(set_layout.as_slice());
            self.descriptor_sets = self
                .logical_device
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
            write_descriptors.push(
                WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(vp_buffer_info.as_slice())
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .dst_binding(0)
                    .dst_set(self.descriptor_sets[0])
                    .build(),
            );
            write_descriptors.push(
                WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(dl_buffer_info.as_slice())
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .dst_binding(1)
                    .dst_set(self.descriptor_sets[0])
                    .build(),
            );

            let ssbo_buffer = self
                .uniform_buffers
                .primary_ssbo
                .as_ref()
                .expect("Primary SSBO buffer doesn't exist.");
            let ssbo_buffer_info = vec![DescriptorBufferInfo::builder()
                .range(ssbo_buffer.buffer_size)
                .offset(0)
                .buffer(ssbo_buffer.buffer)
                .build()];
            write_descriptors.push(
                WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(ssbo_buffer_info.as_slice())
                    .descriptor_type(DescriptorType::STORAGE_BUFFER)
                    .dst_binding(2)
                    .dst_set(self.descriptor_sets[0])
                    .build(),
            );

            let mut texture_info = vec![];
            for texture in resource_lock.textures.iter() {
                let texture_lock = texture
                    .read()
                    .expect("Failed to lock texture for creating the descriptor set.");
                let image_info = DescriptorImageInfo::builder()
                    .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(texture_lock.image_view)
                    .sampler(texture_lock.sampler)
                    .build();
                texture_info.push(image_info);
            }
            write_descriptors.push(
                WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .image_info(texture_info.as_slice())
                    .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .dst_binding(3)
                    .dst_set(self.descriptor_sets[0])
                    .build(),
            );

            self.logical_device
                .update_descriptor_sets(write_descriptors.as_slice(), &[]);
            log::info!("Descriptor successfully updated.");
            Ok(())
        }
    }

    fn create_graphics_pipeline(&mut self, shader_type: ShaderType) -> anyhow::Result<()> {
        let shaders = vec![
            super::Shader::new(
                self.logical_device.clone(),
                match shader_type {
                    ShaderType::AnimatedModel => "./shaders/basicShader_animated.spv",
                    ShaderType::Terrain => "./shaders/terrain_vert.spv",
                    _ => "./shaders/vert.spv",
                },
                ShaderStageFlags::VERTEX,
            ),
            super::Shader::new(
                self.logical_device.clone(),
                match shader_type {
                    ShaderType::BasicShader => "./shaders/frag.spv",
                    ShaderType::BasicShaderWithoutTexture => "./shaders/basicShader_noTexture.spv",
                    ShaderType::Terrain => "./shaders/terrain_frag.spv",
                    ShaderType::Water => "./shaders/water_frag.spv",
                    _ => "./shaders/frag.spv",
                },
                ShaderStageFlags::FRAGMENT,
            ),
        ];

        let mut descriptor_set_layout = vec![self.descriptor_set_layout];
        match shader_type {
            ShaderType::AnimatedModel => {
                descriptor_set_layout.push(self.ssbo_descriptor_set_layout);
            }
            _ => (),
        }
        self.pipeline
            .write()
            .expect("Failed to lock pipeline when creating the pipeline.")
            .create_graphic_pipelines(
                descriptor_set_layout.as_slice(),
                self.sample_count,
                shaders,
                shader_type,
            )?;
        Ok(())
    }

    fn create_frame_buffers(
        frame_width: u32,
        frame_height: u32,
        renderpass: RenderPass,
        swapchain: &super::Swapchain,
        depth_image: &super::Image,
        msaa_image: &super::Image,
        device: &Device,
    ) -> Vec<Framebuffer> {
        let mut frame_buffers = vec![];
        let image_count = swapchain.swapchain_images.len();
        for i in 0..image_count {
            let image_views = vec![
                msaa_image.image_view,
                depth_image.image_view,
                swapchain.swapchain_images[i].image_view,
            ];
            let frame_buffer_info = FramebufferCreateInfo::builder()
                .height(frame_height)
                .width(frame_width)
                .layers(1)
                .attachments(image_views.as_slice())
                .render_pass(renderpass);
            unsafe {
                frame_buffers.push(
                    device
                        .create_framebuffer(&frame_buffer_info, None)
                        .expect("Failed to create framebuffer."),
                );
            }
        }
        frame_buffers
    }

    /*fn update_dynamic_buffer(&mut self, _delta_time: f64) -> anyhow::Result<()> {
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
                *ptr = *model;
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
    }*/
}

impl GraphicsBase<super::Buffer, CommandBuffer, super::Image> for Graphics {
    fn get_commands(&self) -> &Vec<CommandBuffer> {
        &self.command_buffers
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    fn set_disposing(&mut self) {
        self.is_initialized = false;
    }

    unsafe fn wait_idle(&self) {
        self.logical_device
            .device_wait_idle()
            .expect("Failed to wait until device is idling.");
    }
}

unsafe impl Send for Graphics {}
unsafe impl Sync for Graphics {}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            log::info!("Dropping graphics...");
            self.logical_device
                .device_wait_idle()
                .expect("Failed to wait for device to idle.");
            self.dispose();
            self.logical_device
                .destroy_semaphore(self.completed_semaphore, None);
            self.logical_device
                .destroy_semaphore(self.acquired_semaphore, None);
            self.logical_device.destroy_fence(self.fence, None);
            for thread in self.thread_pool.threads.iter() {
                self.logical_device
                    .destroy_command_pool(*thread.command_pool.lock(), None);
            }
            self.logical_device
                .destroy_command_pool(self.command_pool, None);
            self.logical_device
                .destroy_descriptor_set_layout(self.ssbo_descriptor_set_layout, None);
            self.logical_device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.allocator
                .write()
                .expect("Failed to lock the memory allocator.")
                .destroy();
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
