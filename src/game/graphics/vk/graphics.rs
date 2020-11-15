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
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use vk_mem::*;

use crate::game::enums::ShaderType;
use crate::game::graphics::vk::{
    DescriptorAllocator, DescriptorBuilder, DescriptorLayoutCache, Initializer, RenderPassType,
    ThreadPool, UniformBuffers,
};
use crate::game::shared::enums::{ImageFormat, SceneType};
use crate::game::shared::structs::{Directional, PushConstant, ViewProjection};
use crate::game::shared::traits::{GraphicsBase, Renderable};
use crate::game::shared::util::interpolate_alpha;
use crate::game::traits::Mappable;
use crate::game::util::{end_one_time_command_buffer, get_single_time_command_buffer};
use crate::game::{Camera, ResourceManager, UIManager};
use ash::prelude::VkResult;

const SSBO_DATA_COUNT: usize = 50;
const REFLECTION_WIDTH: u32 = 320;
const REFLECTION_HEIGHT: u32 = 180;
const REFRACTION_WIDTH: u32 = 1280;
const REFRACTION_HEIGHT: u32 = 720;

type ResourceManagerHandle = Weak<
    RwLock<ManuallyDrop<ResourceManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>,
>;
type UIManagerHandle = std::rc::Weak<
    RefCell<ManuallyDrop<UIManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>,
>;

struct PrimarySSBOData {
    world_matrices: [Mat4; SSBO_DATA_COUNT],
    object_colors: [Vec4; SSBO_DATA_COUNT],
    reflectivities: [f32; SSBO_DATA_COUNT],
    shine_dampers: [f32; SSBO_DATA_COUNT],
}

struct FrameData {
    pub acquired_semaphore: Semaphore,
    pub completed_semaphore: Semaphore,
    pub fence: Fence,
    pub command_pool: CommandPool,
    pub main_command_buffer: CommandBuffer,
}

struct OffscreenFramebuffer {
    pub framebuffer: Vec<Framebuffer>,
    pub color_image: ManuallyDrop<super::Image>,
    pub depth_image: ManuallyDrop<super::Image>,
    pub msaa_image: ManuallyDrop<super::Image>,
    pub width: u32,
    pub height: u32,
}

impl Drop for OffscreenFramebuffer {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.color_image);
            ManuallyDrop::drop(&mut self.depth_image);
            ManuallyDrop::drop(&mut self.msaa_image);
        }
    }
}

struct OffscreenPass {
    pub framebuffers: [ManuallyDrop<OffscreenFramebuffer>; 2],
}

impl Drop for OffscreenPass {
    fn drop(&mut self) {
        unsafe {
            for buffer in self.framebuffers.iter_mut() {
                ManuallyDrop::drop(buffer);
            }
        }
    }
}

pub struct Graphics {
    pub logical_device: Arc<Device>,
    pub pipeline: Arc<ShardedLock<ManuallyDrop<super::Pipeline>>>,
    pub descriptor_set: DescriptorSet,
    pub push_constant: PushConstant,
    pub descriptor_set_layout: DescriptorSetLayout,
    pub ssbo_descriptor_set_layout: DescriptorSetLayout,
    pub descriptor_pool: Arc<Mutex<DescriptorPool>>,
    pub thread_pool: Arc<ThreadPool>,
    pub allocator: Arc<ShardedLock<Allocator>>,
    pub graphics_queue: Arc<Mutex<Queue>>,
    pub present_queue: Arc<Mutex<Queue>>,
    pub compute_queue: Arc<Mutex<Queue>>,
    pub swapchain: ManuallyDrop<super::Swapchain>,
    pub frame_buffers: Vec<Framebuffer>,
    pub resource_manager: ResourceManagerHandle,
    pub inflight_buffer_count: usize,
    pub instance: Arc<Instance>,
    pub physical_device: super::PhysicalDevice,
    pub ui_manager: Option<UIManagerHandle>,
    pub depth_format: Format,
    pub sample_count: SampleCountFlags,
    pub descriptor_allocator: Arc<Mutex<DescriptorAllocator>>,
    pub descriptor_layout_cache: Arc<Mutex<DescriptorLayoutCache>>,
    window: std::rc::Weak<RefCell<winit::window::Window>>,
    window_width: u32,
    window_height: u32,
    entry: Entry,
    surface_loader: Surface,
    debug_messenger: DebugUtilsMessengerEXT,
    surface: SurfaceKHR,
    depth_image: ManuallyDrop<super::Image>,
    msaa_image: ManuallyDrop<super::Image>,
    uniform_buffers: ManuallyDrop<UniformBuffers>,
    camera: Rc<RefCell<Camera>>,
    sky_color: Vec4,
    frame_data: Vec<FrameData>,
    current_frame: AtomicUsize,
    offscreen_pass: ManuallyDrop<OffscreenPass>,
    is_initialized: bool,
    //checkpoint_fn: NvDeviceDiagnosticCheckpointsFn,
}

impl Graphics {
    pub fn new(
        window: std::rc::Weak<RefCell<winit::window::Window>>,
        camera: Rc<RefCell<Camera>>,
        resource_manager: ResourceManagerHandle,
    ) -> anyhow::Result<Self> {
        let window_ptr = window.upgrade().expect("Failed to upgrade window handle.");
        let window_handle = window_ptr.borrow();
        let debug = dotenv::var("DEBUG")?.parse::<bool>()?;
        let entry = Entry::new()?;
        let enabled_layers = vec![CString::new("VK_LAYER_KHRONOS_validation")?];
        let instance =
            Initializer::create_instance(debug, &enabled_layers, &entry, &*window_handle)?;
        let surface_loader = Surface::new(&entry, &instance);
        let debug_messenger = if debug {
            Initializer::create_debug_messenger(&instance, &entry)
        } else {
            DebugUtilsMessengerEXT::null()
        };
        let surface = Initializer::create_surface(&*window_handle, &entry, &instance)?;
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
            &*window_handle,
            &instance,
            Arc::downgrade(&device),
            Arc::downgrade(&allocator),
        );

        let inflight_buffer_count = std::env::var("INFLIGHT_BUFFER_COUNT")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let mut frame_data = vec![];
        for _ in 0..inflight_buffer_count {
            let command_pool_create_info = CommandPoolCreateInfo::builder().queue_family_index(
                physical_device
                    .queue_indices
                    .graphics_family
                    .unwrap_or_default(),
            );
            unsafe {
                let command_pool = device
                    .create_command_pool(&command_pool_create_info, None)
                    .expect("Failed to create command pool.");
                let (fence, acquired_semaphore, completed_semaphore) =
                    Initializer::create_sync_object(device.as_ref());
                let command_buffers =
                    Initializer::allocate_command_buffers(device.as_ref(), command_pool, 1);
                frame_data.push(FrameData {
                    acquired_semaphore,
                    completed_semaphore,
                    fence,
                    command_pool,
                    main_command_buffer: command_buffers[0],
                });
            }
        }

        let cpu_count = num_cpus::get();
        let thread_pool = Arc::new(ThreadPool::new(
            cpu_count,
            inflight_buffer_count,
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
            swapchain.extent,
            frame_data[0].command_pool,
            graphics_queue,
            sample_count,
            Arc::downgrade(&allocator),
        );

        let msaa_image = Initializer::create_msaa_image(
            Arc::downgrade(&device),
            swapchain.format.format,
            swapchain.extent,
            frame_data[0].command_pool,
            graphics_queue,
            sample_count,
            Arc::downgrade(&allocator),
        );

        let view_projection = Initializer::create_view_projection(
            &*camera.borrow(),
            Arc::downgrade(&device),
            Arc::downgrade(&allocator),
        )?;
        let light_x = std::env::var("LIGHT_X").unwrap().parse::<f32>().unwrap();
        let light_z = std::env::var("LIGHT_Z").unwrap().parse::<f32>().unwrap();
        let directional_light = Directional::new(
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Vec3A::new(light_x, 20000.0, light_z),
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
        let mut pipeline = super::Pipeline::new(device.clone());
        let color_format = swapchain.format.format;
        pipeline.create_normal_renderpass(color_format, depth_format, sample_count);
        pipeline.create_offscreen_renderpass(color_format, depth_format, sample_count)?;
        let offscreen_renderpass = pipeline
            .render_pass
            .get(&RenderPassType::Offscreen)
            .cloned()
            .expect("Failed to get offscreen renderpass.");
        let offscreen_pass = Self::create_offscreen_pass(
            Arc::downgrade(&device),
            color_format,
            depth_format,
            sample_count,
            swapchain.swapchain_images.len(),
            Arc::downgrade(&allocator),
            frame_data[0].command_pool,
            graphics_queue,
            offscreen_renderpass,
        )?;

        let descriptor_layout_cache = DescriptorLayoutCache::new(Arc::downgrade(&device));
        let descriptor_allocator = DescriptorAllocator::new(Arc::downgrade(&device));

        let sky_color: Vec4 = Vec4::new(0.5, 0.5, 0.5, 1.0);
        /*let checkpoint_fn = NvDeviceDiagnosticCheckpointsFn::load(|name| unsafe {
            std::mem::transmute(instance.get_device_proc_addr(device.handle(), name.as_ptr()))
        });*/
        let winit::dpi::PhysicalSize {
            width: window_width,
            height: window_height,
        } = window_handle.inner_size();
        drop(window_handle);
        drop(window_ptr);
        Ok(Graphics {
            entry,
            instance: Arc::new(instance),
            surface_loader,
            debug_messenger,
            surface,
            physical_device,
            ui_manager: None,
            logical_device: device,
            graphics_queue: Arc::new(Mutex::new(graphics_queue)),
            present_queue: Arc::new(Mutex::new(present_queue)),
            compute_queue: Arc::new(Mutex::new(compute_queue)),
            swapchain: ManuallyDrop::new(swapchain),
            depth_image: ManuallyDrop::new(depth_image),
            msaa_image: ManuallyDrop::new(msaa_image),
            descriptor_set_layout: DescriptorSetLayout::null(),
            uniform_buffers: ManuallyDrop::new(uniform_buffers),
            push_constant: PushConstant::new(0, 0, sky_color),
            camera,
            resource_manager,
            descriptor_pool: Arc::new(Mutex::new(DescriptorPool::null())),
            descriptor_set: DescriptorSet::null(),
            pipeline: Arc::new(ShardedLock::new(ManuallyDrop::new(pipeline))),
            frame_buffers: vec![],
            sample_count,
            depth_format,
            allocator,
            thread_pool,
            ssbo_descriptor_set_layout,
            sky_color,
            is_initialized: false,
            frame_data,
            current_frame: AtomicUsize::new(0),
            inflight_buffer_count,
            offscreen_pass: ManuallyDrop::new(offscreen_pass),
            window,
            window_width,
            window_height,
            //checkpoint_fn,
            descriptor_allocator: Arc::new(Mutex::new(descriptor_allocator)),
            descriptor_layout_cache: Arc::new(Mutex::new(descriptor_layout_cache)),
        })
    }

    pub fn create_vertex_and_index_buffer<VertexType: 'static + Send + Sync>(
        graphics: Arc<RwLock<ManuallyDrop<Self>>>,
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
        graphics: Arc<RwLock<ManuallyDrop<Self>>>,
        command_pool: Arc<Mutex<CommandPool>>,
    ) -> anyhow::Result<(Vec<Arc<ShardedLock<super::Image>>>, usize)> {
        let mut textures = vec![];
        let mut texture_handles = vec![];
        use gltf::image::Format;
        for image in images.iter() {
            let buffer_size = image.width * image.height * 4;
            let pool = command_pool.clone();
            let graphics_clone = graphics.clone();
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
                    graphics_clone,
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
        graphics: Arc<RwLock<ManuallyDrop<Self>>>,
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

    pub fn get_command_pool(
        graphics: &Self,
        model_index: usize,
        frame_index: usize,
    ) -> Arc<Mutex<CommandPool>> {
        let thread_count = graphics.thread_pool.thread_count;
        graphics.thread_pool.threads[model_index % thread_count].command_pools[frame_index].clone()
    }

    pub fn get_command_pool_and_secondary_command_buffer(
        graphics: &Self,
        model_index: usize,
        frame_index: usize,
    ) -> (Arc<Mutex<CommandPool>>, CommandBuffer) {
        let pool_handle = Self::get_command_pool(graphics, model_index, frame_index);
        let command_pool = *pool_handle.lock();
        let device = graphics.logical_device.as_ref();
        let command_buffer = Self::create_secondary_command_buffer(device, command_pool);
        (pool_handle, command_buffer)
    }

    pub fn get_idle_command_pool(&self) -> Arc<Mutex<CommandPool>> {
        self.thread_pool.get_idle_command_pool()
    }

    pub fn initialize(&mut self) -> anyhow::Result<()> {
        //self.create_descriptor_set_layout()?;
        self.create_primary_ssbo()?;
        //self.allocate_descriptor_set()?;
        self.allocate_descriptors()?;
        self.create_graphics_pipeline(ShaderType::BasicShader)?;
        self.create_graphics_pipeline(ShaderType::BasicShaderWithoutTexture)?;
        self.create_graphics_pipeline(ShaderType::AnimatedModel)?;
        self.create_graphics_pipeline(ShaderType::Terrain)?;
        self.create_graphics_pipeline(ShaderType::Water)?;
        self.create_graphics_pipeline(ShaderType::InstanceDraw)?;
        let width = self.swapchain.extent.width;
        let height = self.swapchain.extent.height;
        self.frame_buffers = Self::create_frame_buffers(
            width,
            height,
            self.pipeline
                .read()
                .expect("Failed to lock pipeline for creating frame buffers.")
                .render_pass
                .get(&RenderPassType::Primary)
                .cloned()
                .unwrap(),
            &self.swapchain,
            &self.depth_image,
            &self.msaa_image,
            self.logical_device.as_ref(),
        );
        self.is_initialized = true;
        Ok(())
    }

    pub fn recreate_swapchain(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        if self.is_initialized {
            unsafe {
                self.wait_idle();
                self.set_disposing();
                if let Some(ui) = self.ui_manager.as_ref() {
                    let ui_manager = ui.upgrade().expect("Failed to upgrade UI handle.");
                    let mut borrowed = ui_manager.borrow_mut();
                    borrowed.wait_idle();
                    borrowed.set_disposing();
                }
                self.dispose()?;
            }
        }

        if width == 0 || height == 0 {
            return Ok(());
        }
        self.camera
            .borrow_mut()
            .update_window(width as f64, height as f64);
        let window = self
            .window
            .upgrade()
            .expect("Failed to upgrade window handle.");
        let handle = window.borrow();
        self.swapchain = ManuallyDrop::new(Initializer::create_swapchain(
            &self.surface_loader,
            self.surface,
            &self.physical_device,
            &*handle,
            &*self.instance,
            Arc::downgrade(&self.logical_device),
            Arc::downgrade(&self.allocator),
        ));
        self.depth_image = ManuallyDrop::new(Initializer::create_depth_image(
            Arc::downgrade(&self.logical_device),
            self.depth_format,
            self.swapchain.extent,
            self.frame_data[0].command_pool,
            *self.graphics_queue.lock(),
            self.sample_count,
            Arc::downgrade(&self.allocator),
        ));
        self.msaa_image = ManuallyDrop::new(Initializer::create_msaa_image(
            Arc::downgrade(&self.logical_device),
            self.swapchain.format.format,
            self.swapchain.extent,
            self.frame_data[0].command_pool,
            *self.graphics_queue.lock(),
            self.sample_count,
            Arc::downgrade(&self.allocator),
        ));
        let view_projection = Initializer::create_view_projection(
            &*self.camera.borrow(),
            Arc::downgrade(&self.logical_device),
            Arc::downgrade(&self.allocator),
        )?;
        let light_x = std::env::var("LIGHT_X").unwrap().parse::<f32>().unwrap();
        let light_z = std::env::var("LIGHT_Z").unwrap().parse::<f32>().unwrap();
        let directional_light = Directional::new(
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Vec3A::new(light_x, 20000.0, light_z),
            0.1,
            0.5,
        );
        let directional_light = Initializer::create_directional_light(
            &directional_light,
            Arc::downgrade(&self.logical_device),
            Arc::downgrade(&self.allocator),
        )?;
        self.uniform_buffers =
            ManuallyDrop::new(UniformBuffers::new(view_projection, directional_light));
        self.pipeline = Arc::new(ShardedLock::new(ManuallyDrop::new(super::Pipeline::new(
            self.logical_device.clone(),
        ))));
        {
            let mut pipeline_handle = self
                .pipeline
                .write()
                .expect("Failed to get pipeline handle.");
            pipeline_handle.create_normal_renderpass(
                self.swapchain.format.format,
                self.depth_format,
                self.sample_count,
            );
            pipeline_handle.create_offscreen_renderpass(
                self.swapchain.format.format,
                self.depth_format,
                self.sample_count,
            )?;
        }
        let offscreen_renderpass = self
            .pipeline
            .read()
            .expect("Failed to get read handle from Pipeline.")
            .render_pass
            .get(&RenderPassType::Offscreen)
            .cloned()
            .expect("Failed to get offscreen renderpass.");
        self.offscreen_pass = ManuallyDrop::new(Self::create_offscreen_pass(
            Arc::downgrade(&self.logical_device),
            self.swapchain.format.format,
            self.depth_format,
            self.sample_count,
            self.swapchain.swapchain_images.len(),
            Arc::downgrade(&self.allocator),
            self.frame_data[0].command_pool,
            *self.graphics_queue.lock(),
            offscreen_renderpass,
        )?);
        self.initialize()?;
        if let Some(ui) = self.ui_manager.as_ref() {
            let ui_manager = ui.upgrade().expect("Failed to upgrade UI handle.");
            let mut borrowed = ui_manager.borrow_mut();
            borrowed.set_initialized();
        }
        Ok(())
    }

    pub fn render(
        &self,
        renderables: &[Arc<
            Mutex<Box<dyn Renderable<Graphics, super::Buffer, CommandBuffer, super::Image> + Send>>,
        >],
    ) -> anyhow::Result<()> {
        if !self.is_initialized {
            return Ok(());
        }
        unsafe {
            let (current_frame, frame_index) = self.get_current_frame();
            let fences = vec![current_frame.fence];
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
                    current_frame.acquired_semaphore,
                    Fence::null(),
                );
            }
            let mut image_index = 0_u32;
            match result {
                Ok(index) => {
                    image_index = index.0;
                }
                Err(e) => match e {
                    ash::vk::Result::ERROR_OUT_OF_DATE_KHR => {
                        println!("Device out of date. (Acquiring image.)");
                        return Err(anyhow::anyhow!("Swapchain is out of date or suboptimal."));
                    }
                    _ => (),
                },
            }
            self.logical_device
                .reset_command_pool(current_frame.command_pool, CommandPoolResetFlags::empty())?;

            let extent = self.swapchain.extent;
            let viewports = vec![Viewport::builder()
                .width(extent.width as f32)
                .height(extent.height as f32)
                .x(0.0)
                .y(0.0)
                .min_depth(0.0)
                .max_depth(1.0)
                .build()];

            self.begin_draw(
                self.frame_buffers[image_index as usize],
                current_frame,
                frame_index,
                viewports.as_slice(),
                renderables,
            )?;

            let wait_stages = vec![PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let command_buffers = vec![current_frame.main_command_buffer];
            let mut complete_semaphores = vec![current_frame.completed_semaphore];
            let acquired_semaphores = vec![current_frame.acquired_semaphore];
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

            let ui_overlay_finished = if let Some(ui) = self.ui_manager.as_ref() {
                let ui_manager = ui.upgrade().expect("Failed to upgrade UI handle.");
                let mut borrowed = ui_manager.borrow_mut();
                Some(borrowed.render(
                    self.frame_buffers[image_index as usize],
                    viewports[0],
                    nuklear::Vec2 {
                        x: (self.window_width / extent.width) as f32,
                        y: (self.window_height / extent.height) as f32,
                    },
                    complete_semaphores[0],
                ))
            } else {
                None
            };

            if let Some(semaphore) = ui_overlay_finished {
                complete_semaphores = vec![semaphore];
            }
            let image_indices = vec![image_index];
            let swapchain = vec![self.swapchain.swapchain];
            let present_info = PresentInfoKHR::builder()
                .wait_semaphores(complete_semaphores.as_slice())
                .image_indices(image_indices.as_slice())
                .swapchains(swapchain.as_slice());
            {
                let swapchain_loader = &self.swapchain.swapchain_loader;
                let result =
                    swapchain_loader.queue_present(*self.present_queue.lock(), &present_info);
                match result {
                    Ok(suboptimal) => {
                        if suboptimal {
                            let handle = self
                                .window
                                .upgrade()
                                .expect("Failed to upgrade window handle.");
                            handle.borrow().request_redraw();
                            return Err(anyhow::anyhow!("Swapchain is suboptimal."));
                        }
                    }
                    Err(e) => match e {
                        ash::vk::Result::ERROR_OUT_OF_DATE_KHR
                        | ash::vk::Result::SUBOPTIMAL_KHR => {
                            println!("Device out of date. (Presenting.)");
                            return Err(anyhow::anyhow!("Swapchain is out of date or suboptimal."));
                        }
                        _ => panic!("Error when submitting the queue:"),
                    },
                }
            }
            self.current_frame.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    pub fn update(&mut self, delta_time: f64, scene_type: SceneType) -> anyhow::Result<()> {
        if !self.is_initialized {
            return Ok(());
        }
        let resource_arc = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade resource manager handle.");
        let resource_lock = resource_arc.read();
        let current_model_queue = resource_lock
            .model_queue
            .get(&scene_type)
            .expect("Failed to get model queue of the current scene.");
        for model in current_model_queue.iter() {
            let mut model_lock = model.lock();
            model_lock.update(delta_time);
        }
        drop(resource_lock);

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
            let sets = self
                .logical_device
                .allocate_descriptor_sets(&allocate_info)
                .expect("Failed to allocate descriptor sets.");
            self.descriptor_set = sets[0];

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
                    .dst_set(self.descriptor_set)
                    .build(),
            );
            write_descriptors.push(
                WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(dl_buffer_info.as_slice())
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .dst_binding(1)
                    .dst_set(self.descriptor_set)
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
                    .dst_set(self.descriptor_set)
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
                    .dst_set(self.descriptor_set)
                    .build(),
            );

            self.logical_device
                .update_descriptor_sets(write_descriptors.as_slice(), &[]);
            log::info!("Descriptor successfully updated.");
            Ok(())
        }
    }

    fn allocate_descriptors(&mut self) -> anyhow::Result<()> {
        let mut cache = self.descriptor_layout_cache.lock();
        let mut allocator = self.descriptor_allocator.lock();

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

        let mut texture_info = vec![];
        {
            let resource = self
                .resource_manager
                .upgrade()
                .expect("Failed to upgrade resource manager handle.");
            let resource_lock = resource.read();
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
        }

        if let Some((descriptor_set, descriptor_set_layout)) =
            DescriptorBuilder::builder(&mut *cache, &mut *allocator)
                .bind_buffer(
                    0,
                    None,
                    &vp_buffer_info,
                    DescriptorType::UNIFORM_BUFFER,
                    ShaderStageFlags::VERTEX,
                )
                .bind_buffer(
                    1,
                    None,
                    &dl_buffer_info,
                    DescriptorType::UNIFORM_BUFFER,
                    ShaderStageFlags::FRAGMENT,
                )
                .bind_buffer(
                    2,
                    None,
                    &ssbo_buffer_info,
                    DescriptorType::STORAGE_BUFFER,
                    ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                )
                .bind_image(
                    3,
                    Some(texture_info.len() as u32),
                    &texture_info,
                    DescriptorType::COMBINED_IMAGE_SAMPLER,
                    ShaderStageFlags::FRAGMENT,
                )
                .build()
        {
            self.descriptor_set = descriptor_set;
            self.descriptor_set_layout = descriptor_set_layout;
        } else {
            panic!("Failed to allocate descriptor set and descriptor set layout.");
        }

        Ok(())
    }

    fn begin_draw(
        &self,
        frame_buffer: Framebuffer,
        current_frame: &FrameData,
        frame_index: usize,
        viewports: &[Viewport],
        renderables: &[Arc<
            Mutex<Box<dyn Renderable<Graphics, super::Buffer, CommandBuffer, super::Image> + Send>>,
        >],
    ) -> anyhow::Result<()> {
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
        let mut render_area = Rect2D::builder()
            .extent(Extent2D {
                width: REFLECTION_WIDTH,
                height: REFLECTION_HEIGHT,
            })
            .offset(Offset2D::default());
        let scissors = vec![Rect2D::builder()
            .extent(extent)
            .offset(Offset2D::default())
            .build()];
        let offscreen_renderpass = self
            .pipeline
            .read()
            .expect("Failed to lock pipeline for beginning the renderpass.")
            .render_pass
            .get(&RenderPassType::Offscreen)
            .copied()
            .expect("Failed to get offscreen renderpass.");
        let primary_renderpass = self
            .pipeline
            .read()
            .expect("Failed to lock pipeline for beginning the renderpass.")
            .render_pass
            .get(&RenderPassType::Primary)
            .copied()
            .expect("Failed to get primary renderpass.");

        // Begin command buffer
        unsafe {
            let result = self.logical_device.begin_command_buffer(
                current_frame.main_command_buffer,
                &CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            );
            if let Err(e) = result {
                log::error!("Error beginning command buffer: {}", e.to_string());
            }
        }

        let mut all_command_buffers = vec![];
        // First renderpass
        let mut renderpass_begin_info = RenderPassBeginInfo::builder()
            .render_pass(offscreen_renderpass)
            .clear_values(clear_values.as_slice())
            .render_area(*render_area)
            .framebuffer(self.offscreen_pass.framebuffers[0].framebuffer[frame_index]);

        /*let inheritance_ptr = {
            let inheritance_info = Box::new(
                CommandBufferInheritanceInfo::builder()
                    .framebuffer(self.offscreen_pass.framebuffers[0].framebuffer[frame_index])
                    .render_pass(offscreen_renderpass)
                    .build(),
            );
            AtomicPtr::new(Box::into_raw(inheritance_info))
        };
        let inheritance_handle = Arc::new(inheritance_ptr);
        unsafe {
            self.logical_device.cmd_begin_render_pass(
                current_frame.main_command_buffer,
                &renderpass_begin_info,
                SubpassContents::SECONDARY_COMMAND_BUFFERS,
            );
            self.update_secondary_command_buffers(
                inheritance_handle,
                viewports[0],
                scissors[0],
                frame_index,
                scene_type,
            )?;
            self.logical_device
                .cmd_end_render_pass(current_frame.main_command_buffer);
            //all_command_buffers.append(&mut command_buffers);
        }*/

        // Second renderpass
        /*renderpass_begin_info = renderpass_begin_info
            .framebuffer(self.offscreen_pass.framebuffers[1].framebuffer[frame_index]);
        render_area = render_area.extent(Extent2D {
            width: REFRACTION_WIDTH,
            height: REFRACTION_HEIGHT,
        });
        let inheritance_ptr = {
            let inheritance_info = Box::new(
                CommandBufferInheritanceInfo::builder()
                    .framebuffer(self.offscreen_pass.framebuffers[1].framebuffer[frame_index])
                    .render_pass(offscreen_renderpass)
                    .build(),
            );
            AtomicPtr::new(Box::into_raw(inheritance_info))
        };
        let inheritance_handle = Arc::new(inheritance_ptr);
        unsafe {
            self.logical_device.cmd_begin_render_pass(
                current_frame.main_command_buffer,
                &renderpass_begin_info,
                SubpassContents::SECONDARY_COMMAND_BUFFERS,
            );
            self.update_secondary_command_buffers(
                inheritance_handle,
                viewports[0],
                scissors[0],
                frame_index,
                scene_type,
            )?;
            self.logical_device
                .cmd_end_render_pass(current_frame.main_command_buffer);
            //all_command_buffers.append(&mut command_buffers);
        }*/

        // Primary renderpass
        render_area = render_area.extent(extent);
        renderpass_begin_info = renderpass_begin_info
            .render_area(*render_area)
            .framebuffer(frame_buffer)
            .render_pass(primary_renderpass);

        let inheritance_ptr = {
            let inheritance_info = Box::new(
                CommandBufferInheritanceInfo::builder()
                    .framebuffer(frame_buffer)
                    .render_pass(primary_renderpass)
                    .build(),
            );
            AtomicPtr::new(Box::into_raw(inheritance_info))
        };
        let inheritance_handle = Arc::new(inheritance_ptr);
        unsafe {
            self.logical_device.cmd_begin_render_pass(
                current_frame.main_command_buffer,
                &renderpass_begin_info,
                SubpassContents::SECONDARY_COMMAND_BUFFERS,
            );
            let mut command_buffers = self.update_secondary_command_buffers(
                inheritance_handle,
                viewports[0],
                scissors[0],
                frame_index,
                renderables,
            )?;
            all_command_buffers.append(&mut command_buffers);
            self.logical_device.cmd_execute_commands(
                current_frame.main_command_buffer,
                all_command_buffers.as_slice(),
            );
            self.logical_device
                .cmd_end_render_pass(current_frame.main_command_buffer);
            let result = self
                .logical_device
                .end_command_buffer(current_frame.main_command_buffer);
            if let Err(e) = result {
                log::error!("Error ending command buffer: {}", e.to_string());
            }
        }
        Ok(())
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

    fn create_graphics_pipeline(&mut self, shader_type: ShaderType) -> anyhow::Result<()> {
        let shaders = vec![
            super::Shader::new(
                self.logical_device.clone(),
                match shader_type {
                    ShaderType::AnimatedModel => "./shaders/basicShader_animated.spv",
                    ShaderType::Terrain => "./shaders/terrain_vert.spv",
                    ShaderType::InstanceDraw => "./shaders/instance_vert.spv",
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
                    ShaderType::InstanceDraw => "./shaders/instance_frag.spv",
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

    fn create_offscreen_pass(
        device: Weak<Device>,
        color_format: Format,
        depth_format: Format,
        sample_count: SampleCountFlags,
        image_count: usize,
        allocator: Weak<ShardedLock<Allocator>>,
        command_pool: CommandPool,
        graphics_queue: Queue,
        offscreen_renderpass: RenderPass,
    ) -> anyhow::Result<OffscreenPass> {
        let reflection_image = super::Image::new(
            device.clone(),
            ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::SAMPLED,
            MemoryPropertyFlags::DEVICE_LOCAL,
            color_format,
            SampleCountFlags::TYPE_1,
            Extent2D::builder()
                .height(REFLECTION_HEIGHT)
                .width(REFLECTION_WIDTH)
                .build(),
            ImageType::TYPE_2D,
            1,
            ImageAspectFlags::COLOR,
            allocator.clone(),
        );

        let reflection_depth_image = Initializer::create_depth_image(
            device.clone(),
            depth_format,
            Extent2D {
                width: REFLECTION_WIDTH,
                height: REFLECTION_HEIGHT,
            },
            command_pool,
            graphics_queue,
            sample_count,
            allocator.clone(),
        );

        let reflection_msaa_image = Initializer::create_msaa_image(
            device.clone(),
            color_format,
            Extent2D {
                width: REFLECTION_WIDTH,
                height: REFLECTION_HEIGHT,
            },
            command_pool,
            graphics_queue,
            sample_count,
            allocator.clone(),
        );

        let image_views = vec![
            reflection_msaa_image.image_view,
            reflection_depth_image.image_view,
            reflection_image.image_view,
        ];

        let framebuffer_info = FramebufferCreateInfo::builder()
            .width(REFLECTION_WIDTH)
            .height(REFLECTION_HEIGHT)
            .render_pass(offscreen_renderpass)
            .attachments(image_views.as_slice())
            .layers(1);
        let device_arc = device
            .upgrade()
            .expect("Failed to upgrade logical device to create offscreen framebuffer.");

        unsafe {
            let mut framebuffers = vec![];
            for _ in 0..image_count {
                framebuffers.push(device_arc.create_framebuffer(&framebuffer_info, None)?);
            }
            let first_framebuffer = OffscreenFramebuffer {
                framebuffer: framebuffers,
                color_image: ManuallyDrop::new(reflection_image),
                depth_image: ManuallyDrop::new(reflection_depth_image),
                msaa_image: ManuallyDrop::new(reflection_msaa_image),
                width: REFLECTION_WIDTH,
                height: REFLECTION_HEIGHT,
            };

            let refraction_image = super::Image::new(
                device.clone(),
                ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::SAMPLED,
                MemoryPropertyFlags::DEVICE_LOCAL,
                color_format,
                SampleCountFlags::TYPE_1,
                Extent2D::builder()
                    .height(REFRACTION_HEIGHT)
                    .width(REFRACTION_WIDTH)
                    .build(),
                ImageType::TYPE_2D,
                1,
                ImageAspectFlags::COLOR,
                allocator.clone(),
            );

            let refraction_depth_image = Initializer::create_depth_image(
                device.clone(),
                depth_format,
                Extent2D {
                    width: REFRACTION_WIDTH,
                    height: REFRACTION_HEIGHT,
                },
                command_pool,
                graphics_queue,
                sample_count,
                allocator.clone(),
            );

            let refraction_msaa_image = Initializer::create_msaa_image(
                device.clone(),
                color_format,
                Extent2D {
                    width: REFRACTION_WIDTH,
                    height: REFRACTION_HEIGHT,
                },
                command_pool,
                graphics_queue,
                sample_count,
                allocator.clone(),
            );

            let image_views = vec![
                refraction_msaa_image.image_view,
                refraction_depth_image.image_view,
                refraction_image.image_view,
            ];

            let framebuffer_info = FramebufferCreateInfo::builder()
                .width(REFRACTION_WIDTH)
                .height(REFRACTION_HEIGHT)
                .render_pass(offscreen_renderpass)
                .attachments(image_views.as_slice())
                .layers(1);
            let mut framebuffers = vec![];
            for _ in 0..image_count {
                framebuffers.push(device_arc.create_framebuffer(&framebuffer_info, None)?);
            }

            let second_framebuffer = OffscreenFramebuffer {
                framebuffer: framebuffers,
                color_image: ManuallyDrop::new(refraction_image),
                depth_image: ManuallyDrop::new(refraction_depth_image),
                msaa_image: ManuallyDrop::new(refraction_msaa_image),
                width: REFRACTION_WIDTH,
                height: REFRACTION_HEIGHT,
            };

            let framebuffers = [
                ManuallyDrop::new(first_framebuffer),
                ManuallyDrop::new(second_framebuffer),
            ];
            Ok(OffscreenPass { framebuffers })
        }
    }

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
        for (_, model_queue) in resource_lock.model_queue.iter() {
            for model in model_queue.iter() {
                let model_lock = model.lock();
                let metadata = model_lock.get_model_metadata();
                let ssbo_index = model_lock.get_ssbo_index();
                model_metadata.world_matrices[ssbo_index] = metadata.world_matrix;
                model_metadata.object_colors[ssbo_index] = metadata.object_color;
                model_metadata.reflectivities[ssbo_index] = metadata.reflectivity;
                model_metadata.shine_dampers[ssbo_index] = metadata.shine_damper;
            }
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

    fn destroy_scene_resource(&mut self) {
        unsafe {
            self.logical_device
                .destroy_descriptor_pool(*self.descriptor_pool.lock(), None);
            self.logical_device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            ManuallyDrop::drop(&mut self.uniform_buffers);
        }
    }

    unsafe fn dispose(&mut self) -> anyhow::Result<()> {
        for buffer in self.frame_buffers.iter() {
            self.logical_device.destroy_framebuffer(*buffer, None);
        }
        for buffer in self.offscreen_pass.framebuffers.iter_mut() {
            for framebuffer in buffer.framebuffer.iter() {
                self.logical_device.destroy_framebuffer(*framebuffer, None);
            }
            ManuallyDrop::drop(buffer);
        }
        ManuallyDrop::drop(&mut self.offscreen_pass);

        {
            let pipeline = &mut *self
                .pipeline
                .write()
                .expect("Failed to lock pipeline for disposal.");
            ManuallyDrop::drop(pipeline);
        }
        self.destroy_scene_resource();
        ManuallyDrop::drop(&mut self.msaa_image);
        ManuallyDrop::drop(&mut self.depth_image);
        ManuallyDrop::drop(&mut self.swapchain);
        Ok(())
    }

    fn get_current_frame(&self) -> (&FrameData, usize) {
        let current_frame = self.current_frame.load(Ordering::SeqCst);
        let inflight_buffer_count = self.inflight_buffer_count;
        (
            &self.frame_data[current_frame % inflight_buffer_count],
            current_frame % inflight_buffer_count,
        )
    }

    fn update_secondary_command_buffers(
        &self,
        inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
        viewport: Viewport,
        scissor: Rect2D,
        frame_index: usize,
        renderables: &[Arc<
            Mutex<Box<dyn Renderable<Graphics, super::Buffer, CommandBuffer, super::Image> + Send>>,
        >],
    ) -> anyhow::Result<Vec<CommandBuffer>> {
        {
            let push_constant = self.push_constant;
            let ptr = inheritance_info;
            for model in renderables.iter() {
                let ptr_clone = ptr.clone();
                let device_clone = self.logical_device.clone();
                let pipeline_clone = self.pipeline.clone();
                let descriptor_set = self.descriptor_set;
                model.lock().render(
                    ptr_clone,
                    push_constant,
                    viewport,
                    scissor,
                    device_clone,
                    pipeline_clone,
                    descriptor_set,
                    self.thread_pool.clone(),
                    frame_index,
                );
            }
        }
        self.thread_pool.wait()?;
        let command_buffers = renderables
            .iter()
            .map(|r| r.lock().get_command_buffers(frame_index))
            .flatten()
            .collect::<Vec<_>>();
        Ok(command_buffers)
    }
}

impl GraphicsBase<super::Buffer, CommandBuffer, super::Image> for Graphics {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    fn set_disposing(&mut self) {
        self.is_initialized = false;
    }

    unsafe fn wait_idle(&self) {
        for frame in self.frame_data.iter() {
            let fence = [frame.fence];
            self.logical_device
                .wait_for_fences(&fence[0..], true, u64::MAX)
                .expect("Failed to wait for fences to complete.");
        }
    }
}

unsafe impl Send for Graphics {}
unsafe impl Sync for Graphics {}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            self.logical_device
                .device_wait_idle()
                .expect("Failed to wait for device to idle.");
            self.dispose().expect("Failed to dispose graphics.");
            for frame in self.frame_data.iter() {
                let fences = [frame.fence];
                self.logical_device
                    .wait_for_fences(&fences[0..], true, u64::MAX)
                    .expect("Failed to wait for fences of graphics.");
                self.logical_device
                    .destroy_semaphore(frame.completed_semaphore, None);
                self.logical_device
                    .destroy_semaphore(frame.acquired_semaphore, None);
                self.logical_device.destroy_fence(frame.fence, None);
                let command_buffers = vec![frame.main_command_buffer];
                self.logical_device
                    .free_command_buffers(frame.command_pool, command_buffers.as_slice());
                self.logical_device
                    .destroy_command_pool(frame.command_pool, None);
            }
            for thread in self.thread_pool.threads.iter() {
                for pool in thread.command_pools.iter() {
                    self.logical_device.destroy_command_pool(*pool.lock(), None);
                }
            }
            self.logical_device
                .destroy_descriptor_set_layout(self.ssbo_descriptor_set_layout, None);
            /*self.logical_device
            .destroy_descriptor_set_layout(self.descriptor_set_layout, None);*/
            self.allocator
                .write()
                .expect("Failed to lock the memory allocator.")
                .destroy();
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            if self.debug_messenger != DebugUtilsMessengerEXT::null() {
                let debug_loader = DebugUtils::new(&self.entry, &*self.instance);
                debug_loader.destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            self.instance.destroy_instance(None);
            log::info!("Successfully dropped graphics.");
        }
    }
}
