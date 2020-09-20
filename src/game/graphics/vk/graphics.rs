use ash::{
    Entry,
    extensions::{
        khr::{
            Surface,
            Swapchain,
            Win32Surface
        },
        ext::DebugUtils
    },
    Device,
    Instance,
    version::{
        EntryV1_0,
        DeviceV1_0,
        InstanceV1_0
    },
    vk::*
};
use std::collections::HashSet;
use std::ffi::{
    c_void, CStr, CString
};
use std::mem::ManuallyDrop;
use std::sync::{Arc, RwLock, Weak};
use std::sync::atomic::AtomicPtr;
use crate::game::{Camera, ResourceManager};
use crate::game::shared::structs::{ViewProjection, Directional, Vertex, PushConstant};
use std::convert::TryFrom;
use crate::game::traits::Mappable;
use std::ops::Deref;
use crate::game::graphics::vk::{UniformBuffers, DynamicBufferObject, DynamicModel, ThreadPool};
use glam::{Vec3A, Vec4, Mat4};
use crate::game::enums::ShaderType;
use crate::game::shared::traits::GraphicsBase;
use vk_mem::*;
use crossbeam::sync::ShardedLock;
use dashmap::DashMap;
use parking_lot::Mutex;

#[allow(dead_code)]
pub struct Graphics {
    pub dynamic_objects: DynamicBufferObject,
    pub logical_device: Arc<Device>,
    pub pipeline: ManuallyDrop<super::Pipeline>,
    pub descriptor_sets: Vec<DescriptorSet>,
    pub push_constant: PushConstant,
    pub current_index: usize,
    pub sampler_descriptor_set_layout: DescriptorSetLayout,
    pub descriptor_pool: DescriptorPool,
    pub command_pool: CommandPool,
    pub thread_pool: ThreadPool,
    pub allocator: Arc<ShardedLock<Allocator>>,
    pub graphics_queue: Arc<Mutex<Queue>>,
    pub present_queue: Arc<Mutex<Queue>>,
    pub compute_queue: Arc<Mutex<Queue>>,
    pub command_buffer_list: DashMap<CommandPool, Vec<CommandBuffer>>,
    pub swapchain: ManuallyDrop<super::Swapchain>,
    pub frame_buffers: Vec<Framebuffer>,
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
    camera: Arc<RwLock<Camera>>,
    resource_manager: Weak<RwLock<ResourceManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>,
    command_buffers: Vec<CommandBuffer>,
    fences: Vec<Fence>,
    acquired_semaphores: Vec<Semaphore>,
    complete_semaphores: Vec<Semaphore>,
    sample_count: SampleCountFlags,
    depth_format: Format,
}

impl Graphics {
    pub fn new(window: &winit::window::Window, camera: Arc<RwLock<Camera>>, resource_manager: Weak<RwLock<ResourceManager<Graphics, super::Buffer, CommandBuffer, super::Image>>>) -> Self {
        let debug = true;
        let entry = Entry::new().unwrap();
        let enabled_layers = vec![CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
        let instance = Self::create_instance(debug, &enabled_layers, &entry);
        let surface_loader = Surface::new(&entry, &instance);
        let debug_messenger = if debug {
            Self::create_debug_messenger(&instance, &entry)
        } else {
            DebugUtilsMessengerEXT::null()
        };
        let surface = Self::create_surface(window, &entry, &instance);
        let physical_device = super::PhysicalDevice::new(&instance, &surface_loader, surface);
        let (logical_device, graphics_queue, present_queue, compute_queue) = Self::create_logical_device(
            &instance, &physical_device, &enabled_layers, true
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
        let swapchain = Self::create_swapchain(
            &surface_loader, surface, &physical_device, window, &instance, Arc::downgrade(&device),
            Arc::downgrade(&allocator)
        );

        let command_pool_create_info = CommandPoolCreateInfo::builder()
            .queue_family_index(physical_device.queue_indices.graphics_family.unwrap())
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .build();

        let command_pool: CommandPool;
        unsafe {
            command_pool = device
                .create_command_pool(&command_pool_create_info, None)
                .expect("Failed to create command pool.");
        }
        let cpu_count = num_cpus::get();
        let thread_pool = ThreadPool::new(cpu_count, device.as_ref(),
                                                  physical_device.queue_indices.graphics_family.unwrap());
        let sample_count = Self::get_sample_count(&instance, &physical_device);
        let depth_format = Self::get_depth_format(&instance, &physical_device);
        let depth_image = Self::create_depth_image(
            Arc::downgrade(&device), depth_format, &swapchain, command_pool, graphics_queue, sample_count, Arc::downgrade(&allocator)
        );

        let msaa_image = Self::create_msaa_image(
            Arc::downgrade(&device), &swapchain, command_pool, graphics_queue, sample_count, Arc::downgrade(&allocator)
        );

        let descriptor_set_layout = Self::create_descriptor_set_layout(device.as_ref());
        let sampler_descriptor_set_layout = Self::create_sampler_descriptor_set_layout(device.as_ref());
        let view_projection = Self::create_view_projection(
            camera.read().unwrap().deref(), Arc::downgrade(&device), Arc::downgrade(&allocator));
        let directional_light = Directional::new(
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Vec3A::new(0.0, -5.0, 0.0),
            0.1,
            0.5);
        let directional = Self::create_directional_light(
            &directional_light, Arc::downgrade(&device),
            Arc::downgrade(&allocator)
        );

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

        Graphics {
            entry: Entry::new().unwrap(),
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
            descriptor_pool: DescriptorPool::null(),
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
            command_buffer_list: DashMap::new(),
        }
    }

    unsafe extern "system" fn debug_callback(_severity: DebugUtilsMessageSeverityFlagsEXT,
                                             _message_type: DebugUtilsMessageTypeFlagsEXT,
                                             p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
                                             _p_user_data: *mut c_void) -> Bool32 {
        let message = CStr::from_ptr((*p_callback_data).p_message);
        if let Ok(msg) = message.to_str() {
            log::info!("{}", msg);
        }
        FALSE
    }

    fn get_required_extensions(debug: bool) -> Vec<*const i8> {
        let mut extensions = vec![
            Surface::name().as_ptr(),
            Win32Surface::name().as_ptr()
        ];
        if debug {
            extensions.push(DebugUtils::name().as_ptr());
        }
        extensions
    }

    fn create_instance(debug: bool, enabled_layers: &Vec<CString>, entry: &Entry) -> Instance {
        let app_name = CString::new("Demo Engine Rust").unwrap();
        let engine_name = CString::new("Demo Engine").unwrap();
        let app_info = ApplicationInfo::builder()
            .api_version(make_version(1, 2, 0))
            .application_name(&*app_name)
            .application_version(make_version(0, 0, 1))
            .engine_name(&*engine_name)
            .engine_version(make_version(0, 0, 1));

        let extensions = Self::get_required_extensions(debug);
        let layers = enabled_layers.iter().map(|s| {
            s.as_ptr()
        }).collect::<Vec<_>>();

        let mut instance_info = InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(extensions.as_slice())
            .enabled_layer_names(layers.as_slice());

        if debug {
            instance_info = instance_info.enabled_layer_names(layers.as_slice());
        }

        let instance_info = instance_info.build();
        unsafe {
            let instance = entry.create_instance(&instance_info, None)
                .expect("Failed to create Vulkan instance.");
            log::info!("Vulkan instance successfully created.");
            instance
        }
    }

    fn create_debug_messenger(instance: &Instance, entry: &Entry) -> DebugUtilsMessengerEXT {
        let create_info = DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(DebugUtilsMessageSeverityFlagsEXT::ERROR |
                DebugUtilsMessageSeverityFlagsEXT::WARNING |
                DebugUtilsMessageSeverityFlagsEXT::VERBOSE)
            .message_type(DebugUtilsMessageTypeFlagsEXT::all())
            .pfn_user_callback(Some(Self::debug_callback))
            .build();
        let debug_utils_loader = DebugUtils::new(entry, instance);
        unsafe {
            let messenger = debug_utils_loader
                .create_debug_utils_messenger(&create_info, None)
                .expect("Failed to create debug messenger.");
            log::info!("Debug messenger successfully created.");
            messenger
        }
    }

    #[cfg(target_os = "windows")]
    fn create_surface(window: &winit::window::Window, entry: &Entry, instance: &Instance) -> SurfaceKHR {
        use winit::platform::windows::WindowExtWindows;
        use winapi::um::libloaderapi::GetModuleHandleW;

        let hwnd = window.hwnd() as HWND;
        unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null()) as *const c_void;
            let win32_create_info = Win32SurfaceCreateInfoKHR::builder()
                .hwnd(hwnd)
                .hinstance(hinstance)
                .build();
            let win32_surface_loader = Win32Surface::new(entry, instance);
            let surface = win32_surface_loader
                .create_win32_surface(&win32_create_info, None)
                .expect("Failed to create Win32 surface.");
            log::info!("Win32 surface successfully created.");
            surface
        }
    }

    fn create_logical_device(instance: &Instance, physical_device: &super::PhysicalDevice,
                             enabled_layers: &Vec<CString>, debug: bool) -> (Device, Queue, Queue, Queue) {
        let layers = enabled_layers.iter().map(|s| {
            s.as_ptr()
        }).collect::<Vec<_>>();
        let extensions = vec![Swapchain::name()];
        let extensions = extensions.into_iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();
        let features = PhysicalDeviceFeatures::builder()
            .tessellation_shader(true)
            .shader_sampled_image_array_dynamic_indexing(true)
            .sampler_anisotropy(true)
            .sample_rate_shading(true)
            .geometry_shader(true)
            .build();
        let mut indexing_features = PhysicalDeviceDescriptorIndexingFeatures::builder()
            .runtime_descriptor_array(true)
            .descriptor_binding_partially_bound(true)
            .build();
        let mut queue_create_infos = vec![];
        let mut unique_indices = HashSet::new();
        unique_indices.insert(physical_device.queue_indices.graphics_family.unwrap());
        unique_indices.insert(physical_device.queue_indices.present_family.unwrap());
        unique_indices.insert(physical_device.queue_indices.compute_family.unwrap());
        let priority = [1.0_f32];
        for index in unique_indices.iter() {
            let queue_create_info = DeviceQueueCreateInfo::builder()
                .queue_family_index(*index)
                .queue_priorities(&priority)
                .build();
            queue_create_infos.push(queue_create_info);
        }
        let mut create_info = DeviceCreateInfo::builder()
            .enabled_extension_names(extensions.as_slice())
            .enabled_features(&features)
            .push_next(&mut indexing_features)
            .queue_create_infos(queue_create_infos.as_ref());
        if debug {
            create_info = create_info.enabled_layer_names(layers.as_slice());
        }
        unsafe {
            let device = instance
                .create_device(physical_device.physical_device.clone(), &create_info.build(), None)
                .expect("Failed to create logical device.");
            let graphics_queue = device.get_device_queue(physical_device
                                                              .queue_indices
                                                              .graphics_family.unwrap(), 0);
            let present_queue = device.get_device_queue(physical_device
                                                             .queue_indices
                                                             .present_family.unwrap(), 0);
            let compute_queue = device.get_device_queue(physical_device
                                                             .queue_indices
                                                             .compute_family.unwrap(), 0);
            log::info!("Device queue successfully acquired.");
            log::info!("Logical device successfully created.");
            (device, graphics_queue, present_queue, compute_queue)
        }
    }

    fn create_swapchain(surface_loader: &Surface, surface: SurfaceKHR,
                        physical_device: &super::PhysicalDevice, window: &winit::window::Window,
                        instance: &Instance, device: Weak<Device>, allocator: Weak<ShardedLock<Allocator>>) -> super::Swapchain {
        super::Swapchain::new(
            surface_loader, surface, physical_device.physical_device, window,
            physical_device.queue_indices, instance, device, allocator
        )
    }

    fn choose_depth_format(depth_formats: Vec<Format>, tiling: ImageTiling, feature_flags: FormatFeatureFlags, instance: &Instance, physical_device: &super::PhysicalDevice) -> Format {
        for format in depth_formats.iter() {
            unsafe {
                let format_properties = instance
                    .get_physical_device_format_properties(
                        physical_device.physical_device,
                        *format);
                if tiling == ImageTiling::LINEAR &&
                    ((format_properties.linear_tiling_features & feature_flags) == feature_flags) {
                    return *format;
                }
                if tiling == ImageTiling::OPTIMAL &&
                    ((format_properties.optimal_tiling_features & feature_flags) == feature_flags) {
                    return *format;
                }
            }
        }
        depth_formats[0]
    }

    fn get_depth_format(instance: &Instance, physical_device: &super::PhysicalDevice) -> Format {
        Self::choose_depth_format(vec![Format::D32_SFLOAT, Format::D24_UNORM_S8_UINT, Format::D16_UNORM],
                            ImageTiling::OPTIMAL, FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT, instance, physical_device)
    }

    fn create_depth_image(device: Weak<ash::Device>,
                          format: Format,
                          swapchain: &super::Swapchain, command_pool: CommandPool,
                          graphics_queue: Queue, sample_count: SampleCountFlags, allocator: Weak<ShardedLock<Allocator>>) -> super::Image {
        let extent = swapchain.extent;
        let mut image = super::Image
        ::new(device,
              ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
              MemoryPropertyFlags::DEVICE_LOCAL, format,
              sample_count, extent, ImageType::TYPE_2D, 1, ImageAspectFlags::DEPTH,
              allocator);
        image.transition_layout(ImageLayout::UNDEFINED,
                                ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                command_pool, graphics_queue,
                                ImageAspectFlags::DEPTH, 1, None);
        log::info!("Depth image successfully created.");
        image
    }

    fn get_sample_count(instance: &Instance, physical_device: &super::PhysicalDevice) -> SampleCountFlags {
        unsafe {
            let properties = instance
                .get_physical_device_properties(physical_device.physical_device);
            let sample_count: SampleCountFlags = properties.limits.sampled_image_color_sample_counts;
            let supported_samples: SampleCountFlags;
            supported_samples = if (sample_count & SampleCountFlags::TYPE_64) == SampleCountFlags::TYPE_64 {
                SampleCountFlags::TYPE_64
            } else if (sample_count & SampleCountFlags::TYPE_32) == SampleCountFlags::TYPE_32 {
                SampleCountFlags::TYPE_32
            } else if (sample_count & SampleCountFlags::TYPE_16) == SampleCountFlags::TYPE_16 {
                SampleCountFlags::TYPE_16
            } else if (sample_count & SampleCountFlags::TYPE_8) == SampleCountFlags::TYPE_8 {
                SampleCountFlags::TYPE_8
            } else if (sample_count & SampleCountFlags::TYPE_4) == SampleCountFlags::TYPE_4 {
                SampleCountFlags::TYPE_4
            } else if (sample_count & SampleCountFlags::TYPE_32) == SampleCountFlags::TYPE_2 {
                SampleCountFlags::TYPE_2
            } else {
                SampleCountFlags::TYPE_1
            };
            log::info!("Sample count: {:?}", supported_samples);
            supported_samples
        }
    }

    fn create_msaa_image(device: Weak<ash::Device>,
                         swapchain: &super::Swapchain, command_pool: CommandPool,
                         graphics_queue: Queue, sample_count: SampleCountFlags, allocator: Weak<ShardedLock<Allocator>>) -> super::Image {
        let mut image = super::image::Image::new(
            device,
            ImageUsageFlags::TRANSIENT_ATTACHMENT | ImageUsageFlags::COLOR_ATTACHMENT,
            MemoryPropertyFlags::DEVICE_LOCAL,
            swapchain.format.format,
            sample_count, swapchain.extent, ImageType::TYPE_2D, 1,
            ImageAspectFlags::COLOR, allocator
        );
        image.transition_layout(ImageLayout::UNDEFINED, ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                                command_pool, graphics_queue, ImageAspectFlags::COLOR, 1, None);
        log::info!("Msaa image successfully created.");
        image
    }

    fn create_descriptor_set_layout(device: &Device) -> DescriptorSetLayout {
        let mut descriptor_set_layout_binding = vec![];
        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::VERTEX)
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
                .stage_flags(ShaderStageFlags::VERTEX)
                .build());

        descriptor_set_layout_binding.push(
            DescriptorSetLayoutBinding::builder()
                .binding(3)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .stage_flags(ShaderStageFlags::VERTEX)
                .build());

        let create_info = DescriptorSetLayoutCreateInfo::builder()
            .bindings(descriptor_set_layout_binding.as_slice())
            .build();
        unsafe {
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&create_info, None)
                .expect("Failed to create descriptor set layout.");
            log::info!("Descriptor set layout successfully created.");
            descriptor_set_layout
        }
    }

    fn create_sampler_descriptor_set_layout(device: &Device) -> DescriptorSetLayout {
        let layout_binding = DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .build();
        let create_info = DescriptorSetLayoutCreateInfo::builder()
            .bindings(&[layout_binding])
            .build();
        unsafe {
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&create_info, None)
                .expect("Failed to create descriptor set layout for sampler.");
            log::info!("Descriptor set layout for sampler successfully created.");
            descriptor_set_layout
        }
    }

    fn create_view_projection(camera: &Camera,
                              device: Weak<Device>, allocator: Weak<ShardedLock<Allocator>>) -> super::Buffer {
        let vp_size = std::mem::size_of::<ViewProjection>();
        let view_projection = ViewProjection::new(
            camera.get_view_matrix(),
            camera.get_projection_matrix()
        );
        unsafe {
            let mut vp_buffer = super::buffer::Buffer::new(
                device,
                DeviceSize::try_from(vp_size).unwrap(),
                BufferUsageFlags::UNIFORM_BUFFER,
                MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT, allocator);
            let mapped = vp_buffer.map_memory(u64::try_from(vp_size).unwrap(), 0);
            std::ptr::copy_nonoverlapping(&view_projection as *const _ as *const c_void, mapped, vp_size);
            vp_buffer
        }
    }

    fn create_directional_light(directional: &Directional,
                              device: Weak<Device>, allocator: Weak<ShardedLock<Allocator>>) -> super::Buffer {
        let dl_size = std::mem::size_of::<Directional>();
        unsafe {
            let mut dl_buffer = super::buffer::Buffer::new(
                device,
                DeviceSize::try_from(dl_size).unwrap(),
                BufferUsageFlags::UNIFORM_BUFFER,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE, allocator);
            let mapped = dl_buffer.map_memory(u64::try_from(dl_size).unwrap(), 0);
            std::ptr::copy(directional as *const _ as *const c_void, mapped, dl_size);
            dl_buffer
        }
    }

    fn create_dynamic_model_buffers(&mut self) {
        let arc = self.resource_manager.upgrade();
        if arc.is_none() {
            panic!("Resource manager has been destroyed.");
        }
        let resource_manager = arc.unwrap();
        let resource_lock = resource_manager.read().unwrap();
        let mut matrices = vec![];
        let mut indices = vec![];
        if resource_lock.models.is_empty() {
            return;
        }
        for model in resource_lock.models.iter() {
            indices.push(indices.len());
            matrices.push(model.lock().get_world_matrix());
        }
        drop(resource_lock);
        drop(resource_manager);
        let dynamic_alignment = self.dynamic_objects.dynamic_alignment;
        let buffer_size = dynamic_alignment * DeviceSize::try_from(matrices.len()).unwrap();
        let mut dynamic_model = DynamicModel {
            model_indices: indices,
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
            std::ptr::copy(dynamic_model.buffer as *mut c_void, mapped, buffer_size as usize);
        }
        self.dynamic_objects.models = dynamic_model;
        self.uniform_buffers.model_buffer = Some(ManuallyDrop::new(buffer));
    }

    fn allocate_descriptor_set(&mut self) {
        let mut texture_count = 0_usize;
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

        let image_count = self.swapchain.swapchain_images.len();
        let pool_info = DescriptorPoolCreateInfo::builder()
            .max_sets(u32::try_from(image_count + texture_count).unwrap())
            .pool_sizes(pool_sizes.as_slice())
            .build();

        unsafe {
            self.descriptor_pool = self.logical_device
                .create_descriptor_pool(&pool_info, None)
                .expect("Failed to create descriptor pool.");
            log::info!("Descriptor pool successfully created.");
            let set_layout = vec![self.descriptor_set_layout];
            let allocate_info = DescriptorSetAllocateInfo::builder()
                .descriptor_pool(self.descriptor_pool)
                .set_layouts(set_layout.as_slice())
                .build();
            self.descriptor_sets = self.logical_device
                .allocate_descriptor_sets(&allocate_info)
                .expect("Failed to allocate descriptor sets.");

            println!("Descriptor set count: {}", self.descriptor_sets.len());
            log::info!("Descriptor sets successfully allocated.");

            let vp_buffer = &self.uniform_buffers.view_projection;
            let vp_buffer_info = DescriptorBufferInfo::builder()
                .buffer(vp_buffer.buffer)
                .offset(0)
                .range(vp_buffer.buffer_size)
                .build();

            let dl_buffer = &self.uniform_buffers.directional_light;
            let dl_buffer_info = DescriptorBufferInfo::builder()
                .buffer(dl_buffer.buffer)
                .offset(0)
                .range(dl_buffer.buffer_size)
                .build();

            let mut write_descriptors = vec![];
            write_descriptors.push(WriteDescriptorSet::builder()
                .dst_array_element(0)
                .buffer_info(&[vp_buffer_info])
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .dst_binding(0)
                .dst_set(self.descriptor_sets[0])
                .build());
            write_descriptors.push(WriteDescriptorSet::builder()
                .dst_array_element(0)
                .buffer_info(&[dl_buffer_info])
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .dst_binding(1)
                .dst_set(self.descriptor_sets[0])
                .build());

            if let Some(buffer) = self.uniform_buffers.model_buffer.as_ref() {
                let model_buffer_info = DescriptorBufferInfo::builder()
                    .range(WHOLE_SIZE)
                    .offset(0)
                    .buffer(buffer.buffer)
                    .build();

                write_descriptors.push(WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(&[model_buffer_info])
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                    .dst_binding(2)
                    .dst_set(self.descriptor_sets[0])
                    .build());
            }

            if let Some(buffer) = self.uniform_buffers.mesh_buffer.as_ref() {
                let mesh_buffer_info = DescriptorBufferInfo::builder()
                    .range(WHOLE_SIZE)
                    .offset(0)
                    .buffer(buffer.buffer)
                    .build();

                write_descriptors.push(WriteDescriptorSet::builder()
                    .dst_array_element(0)
                    .buffer_info(&[mesh_buffer_info])
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                    .dst_binding(3)
                    .dst_set(self.descriptor_sets[0])
                    .build());
            }

            self.logical_device.update_descriptor_sets(write_descriptors.as_slice(), &[]);
            log::info!("Descriptor successfully updated.");
        }
    }

    async fn create_graphics_pipeline(&mut self, shader_type: ShaderType) {
        unsafe {
            let shaders = vec![
                super::Shader::new(
                    self.logical_device.clone(),
                    "./shaders/vert.spv",
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
            if shader_type == ShaderType::BasicShader {
                descriptor_set_layout.push(self.sampler_descriptor_set_layout);
            }
            self.pipeline.create_graphic_pipelines(
                descriptor_set_layout.as_slice(),
                self.sample_count, shaders, None, shader_type)
                .await;
        }
    }

    fn allocate_command_buffers(device: &Device, command_pool: CommandPool, image_count: u32) -> Vec<CommandBuffer> {
        let command_buffer_info = CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(image_count)
            .level(CommandBufferLevel::PRIMARY)
            .build();
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
                .render_pass(renderpass)
                .build();
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
        let fence_info = FenceCreateInfo::builder().build();
        let semaphore_info = SemaphoreCreateInfo::builder().build();
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

    pub fn create_secondary_command_buffer(&self, command_pool: Arc<Mutex<CommandPool>>) -> CommandBuffer {
        let allocate_info = CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .level(CommandBufferLevel::SECONDARY)
            .command_pool(*command_pool.lock())
            .build();
        unsafe {
            let buffer = self.logical_device
                .allocate_command_buffers(&allocate_info)
                .expect("Failed to allocate secondary command buffer.");
            buffer[0]
        }
    }

    pub async fn initialize(&mut self) {
        self.create_dynamic_model_buffers();
        self.allocate_descriptor_set();
        let color_format: Format = self.swapchain.format.format;
        let depth_format = self.depth_format;
        let sample_count = self.sample_count;
        self.pipeline.create_renderpass(
            color_format,
            depth_format,
            sample_count);
        self.create_graphics_pipeline(ShaderType::BasicShader).await;
        self.create_graphics_pipeline(ShaderType::BasicShaderWithoutTexture).await;
        let width = self.swapchain.extent.width;
        let height = self.swapchain.extent.height;
        self.frame_buffers = Self::create_frame_buffers(
            width, height, self.pipeline.render_pass,
            &self.swapchain, &self.depth_image, &self.msaa_image, self.logical_device.as_ref()
        );
    }

    pub fn begin_draw(&self, frame_buffer: Framebuffer) {
        let clear_color = ClearColorValue {
            float32: [1.0, 1.0, 0.0, 1.0]
        };
        let clear_depth = ClearDepthStencilValue::builder()
            .depth(1.0).stencil(0).build();
        let clear_values = vec![ClearValue {
            color: clear_color
        }, ClearValue {
            depth_stencil: clear_depth
        }];
        let cmd_buffer_begin_info = CommandBufferBeginInfo::builder()
            .build();
        let extent = self.swapchain.extent;
        let render_area = Rect2D::builder()
            .extent(extent)
            .offset(Offset2D::default())
            .build();
        let renderpass_begin_info = RenderPassBeginInfo::builder()
            .render_pass(self.pipeline.render_pass)
            .clear_values(clear_values.as_slice())
            .render_area(render_area)
            .framebuffer(frame_buffer)
            .build();
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
                .update_secondary_command_buffers(ptr, viewports[0], scissors[0]);
            self.logical_device.cmd_execute_commands(self.command_buffers[0], command_buffers.as_slice());
            self.logical_device.cmd_end_render_pass(self.command_buffers[0]);
            let result = self.logical_device.end_command_buffer(self.command_buffers[0]);
            if let Err(e) = result {
                log::error!("Error ending command buffer: {}", e.to_string());
            }
        }
    }

    pub fn update_secondary_command_buffers(&self,
                                            inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
                                            viewport: Viewport, scissor: Rect2D) -> Vec<CommandBuffer> {
        let resource_manager = self.resource_manager.upgrade().unwrap();
        let resource_lock = resource_manager.read().unwrap();
        let model_count = resource_lock.get_model_count();
        let dynamic_alignment = self.dynamic_objects.dynamic_alignment;
        let push_constant = self.push_constant;
        let ptr = inheritance_info;
        for model in resource_lock.models.iter() {
            let model_clone = model.clone();
            let model_index = model_clone.lock().model_index;
            let ptr_clone = ptr.clone();
            self.thread_pool.threads[model_index % model_count]
                .add_job(move || {
                    let model_lock = model_clone.lock();
                    let ptr = ptr_clone;
                    model_lock
                        .render(ptr, dynamic_alignment, push_constant, viewport, scissor);
                });
        }
        self.thread_pool.wait();
        let command_buffers = resource_lock.models.iter()
            .map(|model| {
                let mesh_command_buffers = model.lock().meshes.iter()
                    .map(|mesh| mesh.command_buffer.unwrap())
                    .collect::<Vec<_>>();
                mesh_command_buffers
            })
            .flatten()
            .collect::<Vec<_>>();
        command_buffers
    }

    pub fn update(&mut self) {

    }

    pub fn render(&self) -> u32 {
        unsafe {
            let swapchain_loader = self.swapchain.swapchain_loader.clone();
            let result = swapchain_loader
                .acquire_next_image(self.swapchain.swapchain, u64::MAX,
                                    self.acquired_semaphores[self.current_index], Fence::null());
            if let Err(e) = result {
                log::error!("Error acquiring swapchain image: {}", e.to_string());
                return 0;
            }
            let (image_index, _is_suboptimal) = result.unwrap();
            self.begin_draw(self.frame_buffers[image_index as usize]);

            let wait_stages = vec![PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let submit_info = SubmitInfo::builder()
                .command_buffers(&[self.command_buffers[0]])
                .signal_semaphores(&[self.complete_semaphores[self.current_index]])
                .wait_dst_stage_mask(wait_stages.as_slice())
                .wait_semaphores(&[self.acquired_semaphores[self.current_index]])
                .build();

            self.logical_device.reset_fences(&[self.fences[self.current_index]])
                .expect("Failed to reset fences.");
            self.logical_device.queue_submit(*self.graphics_queue.lock(), &[submit_info], self.fences[self.current_index])
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
            image_index
        }
    }

    unsafe fn dispose(&mut self) {
        for buffer in self.frame_buffers.iter() {
            self.logical_device.destroy_framebuffer(*buffer, None);
        }
        self.logical_device.free_command_buffers(self.command_pool, self.command_buffers.as_slice());
        ManuallyDrop::drop(&mut self.pipeline);
        self.logical_device.destroy_descriptor_pool(self.descriptor_pool, None);
        ManuallyDrop::drop(&mut self.uniform_buffers);
        ManuallyDrop::drop(&mut self.msaa_image);
        ManuallyDrop::drop(&mut self.depth_image);
        ManuallyDrop::drop(&mut self.swapchain);
    }
}

impl GraphicsBase<super::Buffer, CommandBuffer, super::Image> for Graphics {
    fn create_vertex_buffer(&self, vertices: &[Vertex], command_buffer: Option<CommandBuffer>) -> super::Buffer {
        let buffer_size = DeviceSize::try_from(std::mem::size_of::<Vertex>() * vertices.len())
            .unwrap();
        let mut staging = super::Buffer::new(
            Arc::downgrade(&self.logical_device), buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            Arc::downgrade(&self.allocator)
        );
        let mapped = staging.map_memory(buffer_size, 0);
        unsafe {
            std::ptr::copy_nonoverlapping(vertices.as_ptr() as *const c_void, mapped, buffer_size as usize);
        }
        let buffer = super::Buffer::new(
            Arc::downgrade(&self.logical_device), buffer_size,
            BufferUsageFlags::TRANSFER_DST | BufferUsageFlags::VERTEX_BUFFER,
            MemoryPropertyFlags::DEVICE_LOCAL,
            Arc::downgrade(&self.allocator)
        );
        buffer.copy_buffer(&staging, buffer_size, self.command_pool, *self.graphics_queue.lock(), command_buffer);
        drop(staging);
        buffer
    }

    fn create_index_buffer(&self, indices: &[u32], command_buffer: Option<CommandBuffer>) -> super::Buffer {
        let buffer_size = DeviceSize::try_from(std::mem::size_of::<u32>() * indices.len())
            .unwrap();
        let mut staging = super::Buffer::new(
            Arc::downgrade(&self.logical_device), buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            Arc::downgrade(&self.allocator)
        );
        let mapped = staging.map_memory(buffer_size, 0);
        unsafe {
            std::ptr::copy_nonoverlapping(indices.as_ptr() as *const c_void, mapped, buffer_size as usize);
        }
        let buffer = super::Buffer::new(
            Arc::downgrade(&self.logical_device), buffer_size,
            BufferUsageFlags::TRANSFER_DST | BufferUsageFlags::INDEX_BUFFER,
            MemoryPropertyFlags::DEVICE_LOCAL,
            Arc::downgrade(&self.allocator)
        );
        buffer.copy_buffer(&staging, buffer_size, self.command_pool, *self.graphics_queue.lock(), command_buffer);
        drop(staging);
        buffer
    }

    fn get_commands(&self) -> &Vec<CommandBuffer> {
        &self.command_buffers
    }

    fn create_image(&self, image_data: &[u8], buffer_size: u64, width: u32, height: u32, format: gltf::image::Format) -> super::Image {
        log::info!("Loading texture...");
        log::info!("Image data length: {}, Buffer size: {}, Width: {}, Height: {}", image_data.len(), buffer_size, width, height);
        let mut staging = super::Buffer::new(
            Arc::downgrade(&self.logical_device),
            buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
            Arc::downgrade(&self.allocator)
        );
        unsafe {
            let mapped = staging.map_memory(buffer_size, 0);
            std::ptr::copy_nonoverlapping(image_data.as_ptr() as *const c_void, mapped, buffer_size as usize);

            let _format = match format {
                gltf::image::Format::B8G8R8A8 => ash::vk::Format::B8G8R8A8_UNORM,
                gltf::image::Format::R8G8B8A8 => ash::vk::Format::R8G8B8A8_UNORM,
                _ => self.swapchain.format.format
            };
            let _width = width as f32;
            let _height = height as f32;
            let mip_levels = _width.max(_height).log2().floor() as u32;
            let mut image = super::Image::new(
                Arc::downgrade(&self.logical_device),
                ImageUsageFlags::TRANSFER_SRC | ImageUsageFlags::TRANSFER_DST | ImageUsageFlags::SAMPLED,
                MemoryPropertyFlags::DEVICE_LOCAL, _format,
                SampleCountFlags::TYPE_1,
                Extent2D::builder().width(width).height(height).build(),
                ImageType::TYPE_2D, mip_levels, ImageAspectFlags::COLOR, Arc::downgrade(&self.allocator)
            );
            image.transition_layout(ImageLayout::UNDEFINED, ImageLayout::TRANSFER_DST_OPTIMAL,
            self.command_pool, *self.graphics_queue.lock(), ImageAspectFlags::COLOR, mip_levels, None);
            image.copy_buffer_to_image(staging.buffer, width, height, self.command_pool, *self.graphics_queue.lock(), None);
            image.generate_mipmap(ImageAspectFlags::COLOR, mip_levels, self.command_pool, *self.graphics_queue.lock(), None);
            image.create_sampler(mip_levels);
            image
        }
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
            self.logical_device.destroy_descriptor_set_layout(self.sampler_descriptor_set_layout, None);
            self.logical_device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.allocator.write().unwrap().destroy();
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            let debug_loader = DebugUtils::new(&self.entry, &self.instance);
            debug_loader.destroy_debug_utils_messenger(self.debug_messenger, None);
            self.instance.destroy_instance(None);
            log::info!("Successfully dropped graphics.");
        }
    }
}