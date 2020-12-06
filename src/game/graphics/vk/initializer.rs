use crate::game::enums::ImageFormat;
use crate::game::graphics::vk::Graphics;
use crate::game::structs::{Directional, ViewProjection};
use crate::game::traits::Mappable;
use crate::game::util::{
    end_one_time_command_buffer, get_single_time_command_buffer, interpolate_alpha,
};
use crate::game::Camera;
use anyhow::Context;
use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::{Surface, Swapchain};
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::vk::*;
use ash::{Entry, Instance};
use ash_window::enumerate_required_extensions;
use crossbeam::sync::ShardedLock;
use image::GenericImageView;
use parking_lot::{Mutex, RwLock};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::ffi::{c_void, CStr, CString};
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};
use vk_mem::Allocator;

pub struct Initializer {}

impl Initializer {
    pub fn create_instance(
        debug: bool,
        enabled_layers: &[CString],
        entry: &Entry,
        window: &winit::window::Window,
    ) -> anyhow::Result<Instance> {
        let app_name = CString::new("Demo Engine Rust")?;
        let engine_name = CString::new("Demo Engine")?;
        let app_info = ApplicationInfo::builder()
            .api_version(make_version(1, 2, 0))
            .application_name(&*app_name)
            .application_version(make_version(0, 0, 1))
            .engine_name(&*engine_name)
            .engine_version(make_version(0, 0, 1));

        let extensions = Self::get_required_extensions(debug, window, entry)?;
        let layers = enabled_layers
            .iter()
            .map(|s| s.as_ptr())
            .collect::<Vec<_>>();

        let extension_ptrs = extensions.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

        let mut instance_info = InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(extension_ptrs.as_slice());

        if debug {
            instance_info = instance_info.enabled_layer_names(layers.as_slice());
        }

        unsafe {
            let instance = entry
                .create_instance(&instance_info, None)
                .expect("Failed to create Vulkan instance.");
            log::info!("Vulkan instance successfully created.");
            Ok(instance)
        }
    }

    pub fn create_debug_messenger(instance: &Instance, entry: &Entry) -> DebugUtilsMessengerEXT {
        let create_info = DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
            )
            .message_type(DebugUtilsMessageTypeFlagsEXT::all())
            .pfn_user_callback(Some(Self::debug_callback));
        let debug_utils_loader = DebugUtils::new(entry, instance);
        unsafe {
            let messenger = debug_utils_loader
                .create_debug_utils_messenger(&create_info, None)
                .expect("Failed to create debug messenger.");
            log::info!("Debug messenger successfully created.");
            messenger
        }
    }

    pub fn create_surface(
        window: &winit::window::Window,
        entry: &Entry,
        instance: &Instance,
    ) -> anyhow::Result<SurfaceKHR> {
        unsafe {
            let surface = ash_window::create_surface(entry, instance, window, None)
                .with_context(|| "Failed to create surface.")?;
            Ok(surface)
        }
    }

    pub fn create_logical_device(
        instance: &Instance,
        physical_device: &super::PhysicalDevice,
        enabled_layers: &[CString],
        debug: bool,
    ) -> (ash::Device, Queue, Queue, Queue) {
        let layers = enabled_layers
            .iter()
            .map(|s| s.as_ptr())
            .collect::<Vec<_>>();
        let mut extensions = vec![Swapchain::name()];
        /*if debug {
            extensions.push(ash::vk::NvDeviceDiagnosticCheckpointsFn::name());
        }*/
        let extensions = extensions.iter().map(|e| e.as_ptr()).collect::<Vec<_>>();
        let features = PhysicalDeviceFeatures::builder()
            .tessellation_shader(physical_device.feature_support.tessellation_shader)
            .shader_sampled_image_array_dynamic_indexing(
                physical_device
                    .feature_support
                    .shader_sampled_image_array_dynamic_indexing,
            )
            .sampler_anisotropy(physical_device.feature_support.sampler_anisotropy)
            .sample_rate_shading(physical_device.feature_support.sample_rate_shading)
            .geometry_shader(physical_device.feature_support.geometry_shader)
            .shader_clip_distance(physical_device.feature_support.shader_clip_distance);
        let mut indexing_features = PhysicalDeviceDescriptorIndexingFeatures::builder()
            .runtime_descriptor_array(physical_device.feature_support.runtime_descriptor_array)
            .descriptor_binding_partially_bound(
                physical_device
                    .feature_support
                    .descriptor_binding_partially_bound,
            );
        let mut queue_create_infos = vec![];
        let mut unique_indices = HashSet::new();
        unique_indices.insert(
            physical_device
                .queue_indices
                .graphics_family
                .unwrap_or_default(),
        );
        unique_indices.insert(
            physical_device
                .queue_indices
                .present_family
                .unwrap_or_default(),
        );
        unique_indices.insert(
            physical_device
                .queue_indices
                .compute_family
                .unwrap_or_default(),
        );
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
                .create_device(physical_device.physical_device, &create_info, None)
                .expect("Failed to create logical device.");
            let graphics_queue = device.get_device_queue(
                physical_device
                    .queue_indices
                    .graphics_family
                    .unwrap_or_default(),
                0,
            );
            let present_queue = device.get_device_queue(
                physical_device
                    .queue_indices
                    .present_family
                    .unwrap_or_default(),
                0,
            );
            let compute_queue = device.get_device_queue(
                physical_device
                    .queue_indices
                    .compute_family
                    .unwrap_or_default(),
                0,
            );
            log::info!("Device queue successfully acquired.");
            log::info!("Logical device successfully created.");
            (device, graphics_queue, present_queue, compute_queue)
        }
    }

    pub fn create_swapchain(
        surface_loader: &Surface,
        surface: SurfaceKHR,
        physical_device: &super::PhysicalDevice,
        window: &winit::window::Window,
        instance: &Instance,
        device: Weak<ash::Device>,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> super::Swapchain {
        super::Swapchain::new(
            surface_loader,
            surface,
            physical_device.physical_device,
            window,
            physical_device.queue_indices,
            instance,
            device,
            allocator,
        )
    }

    pub fn choose_depth_format(
        depth_formats: Vec<Format>,
        tiling: ImageTiling,
        feature_flags: FormatFeatureFlags,
        instance: &Instance,
        physical_device: &super::PhysicalDevice,
    ) -> Format {
        for format in depth_formats.iter() {
            unsafe {
                let format_properties = instance.get_physical_device_format_properties(
                    physical_device.physical_device,
                    *format,
                );
                if tiling == ImageTiling::LINEAR
                    && ((format_properties.linear_tiling_features & feature_flags) == feature_flags)
                {
                    return *format;
                }
                if tiling == ImageTiling::OPTIMAL
                    && ((format_properties.optimal_tiling_features & feature_flags)
                        == feature_flags)
                {
                    return *format;
                }
            }
        }
        depth_formats[0]
    }

    pub fn get_depth_format(
        instance: &Instance,
        physical_device: &super::PhysicalDevice,
    ) -> Format {
        Self::choose_depth_format(
            vec![
                Format::D32_SFLOAT,
                Format::D24_UNORM_S8_UINT,
                Format::D16_UNORM,
            ],
            ImageTiling::OPTIMAL,
            FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
            instance,
            physical_device,
        )
    }

    pub fn create_depth_image(
        device: Weak<ash::Device>,
        format: Format,
        extent: Extent2D,
        command_pool: CommandPool,
        graphics_queue: Queue,
        sample_count: SampleCountFlags,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> super::Image {
        let mut image = super::Image::new(
            device,
            ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            MemoryPropertyFlags::DEVICE_LOCAL,
            format,
            sample_count,
            extent,
            ImageType::TYPE_2D,
            1,
            ImageAspectFlags::DEPTH,
            allocator,
        );
        image.transition_layout(
            ImageLayout::UNDEFINED,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            command_pool,
            graphics_queue,
            ImageAspectFlags::DEPTH,
            1,
            None,
        );
        log::info!("Depth image successfully created.");
        image
    }

    pub fn get_sample_count(
        instance: &Instance,
        physical_device: &super::PhysicalDevice,
    ) -> SampleCountFlags {
        unsafe {
            let properties =
                instance.get_physical_device_properties(physical_device.physical_device);
            let sample_count: SampleCountFlags =
                properties.limits.sampled_image_color_sample_counts;
            let supported_samples: SampleCountFlags;
            supported_samples =
                if (sample_count & SampleCountFlags::TYPE_64) == SampleCountFlags::TYPE_64 {
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

    pub fn create_msaa_image(
        device: Weak<ash::Device>,
        format: Format,
        extent: Extent2D,
        command_pool: CommandPool,
        graphics_queue: Queue,
        sample_count: SampleCountFlags,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> super::Image {
        let mut image = super::image::Image::new(
            device,
            ImageUsageFlags::TRANSIENT_ATTACHMENT | ImageUsageFlags::COLOR_ATTACHMENT,
            MemoryPropertyFlags::DEVICE_LOCAL,
            format,
            sample_count,
            extent,
            ImageType::TYPE_2D,
            1,
            ImageAspectFlags::COLOR,
            allocator,
        );
        image.transition_layout(
            ImageLayout::UNDEFINED,
            ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            command_pool,
            graphics_queue,
            ImageAspectFlags::COLOR,
            1,
            None,
        );
        log::info!("Msaa image successfully created.");
        image
    }

    pub fn create_view_projection(
        camera: &Camera,
        device: Weak<ash::Device>,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> anyhow::Result<super::Buffer> {
        let vp_size = std::mem::size_of::<ViewProjection>();
        let view_projection =
            ViewProjection::new(camera.get_view_matrix(), camera.get_projection_matrix());
        unsafe {
            let mut vp_buffer = super::buffer::Buffer::new(
                device,
                DeviceSize::try_from(vp_size)?,
                BufferUsageFlags::UNIFORM_BUFFER,
                MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
                allocator,
            );
            let mapped = vp_buffer.map_memory(u64::try_from(vp_size)?, 0);
            std::ptr::copy_nonoverlapping(
                &view_projection as *const _ as *const c_void,
                mapped,
                vp_size,
            );
            Ok(vp_buffer)
        }
    }

    pub fn create_directional_light(
        directional: &Directional,
        device: Weak<ash::Device>,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> anyhow::Result<super::Buffer> {
        let dl_size = std::mem::size_of::<Directional>();
        unsafe {
            let mut dl_buffer = super::buffer::Buffer::new(
                device,
                DeviceSize::try_from(dl_size)?,
                BufferUsageFlags::UNIFORM_BUFFER,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                allocator,
            );
            let mapped = dl_buffer.map_memory(u64::try_from(dl_size)?, 0);
            std::ptr::copy(directional as *const _ as *const c_void, mapped, dl_size);
            Ok(dl_buffer)
        }
    }

    pub fn create_ssbo_descriptor_set_layout(device: &ash::Device) -> DescriptorSetLayout {
        let layout_bindings = vec![DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::STORAGE_BUFFER)
            .stage_flags(ShaderStageFlags::VERTEX)
            .build()];
        let create_info =
            DescriptorSetLayoutCreateInfo::builder().bindings(layout_bindings.as_slice());
        unsafe {
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&create_info, None)
                .expect("Failed to create descriptor set layout for ssbo.");
            log::info!("Descriptor set layout for ssbo successfully created.");
            descriptor_set_layout
        }
    }

    pub fn allocate_command_buffers(
        device: &ash::Device,
        command_pool: CommandPool,
        image_count: u32,
    ) -> Vec<CommandBuffer> {
        let command_buffer_info = CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(image_count)
            .level(CommandBufferLevel::PRIMARY);
        unsafe {
            device
                .allocate_command_buffers(&command_buffer_info)
                .expect("Failed to allocate command buffers.")
        }
    }

    pub fn create_sync_object(device: &ash::Device) -> (Fence, Semaphore, Semaphore) {
        let fence_info = FenceCreateInfo::builder().flags(FenceCreateFlags::SIGNALED);
        let semaphore_info = SemaphoreCreateInfo::builder();
        unsafe {
            let fence = device
                .create_fence(&fence_info, None)
                .expect("Failed to create fence.");
            let acquired_semaphore = device
                .create_semaphore(&semaphore_info, None)
                .expect("Failed to create semaphore.");
            let completed_semaphore = device
                .create_semaphore(&semaphore_info, None)
                .expect("Failed to create semaphore.");
            log::info!("Sync objects successfully created.");
            (fence, acquired_semaphore, completed_semaphore)
        }
    }

    pub fn create_image_from_file(
        file_name: &str,
        graphics: Arc<RwLock<ManuallyDrop<Graphics>>>,
        command_pool: Arc<Mutex<CommandPool>>,
        sampler_address_mode: SamplerAddressMode,
    ) -> anyhow::Result<(Arc<ShardedLock<super::Image>>, usize)> {
        let resource_manager = graphics.read().resource_manager.clone();
        let resource_manager = match resource_manager.upgrade() {
            None => panic!("Failed to upgrade resource manager."),
            Some(rm) => rm,
        };
        let image = image::open(file_name)?;
        let buffer_size;
        let bytes = match image.color() {
            image::ColorType::Bgr8 | image::ColorType::Rgb8 => {
                buffer_size = image.width() * image.height() * 4;
                interpolate_alpha(
                    image.to_bytes(),
                    image.width(),
                    image.height(),
                    buffer_size as usize,
                )
            }
            _ => {
                let bytes = image.to_bytes();
                buffer_size = bytes.len() as u32;
                bytes
            }
        };
        let width = image.width();
        let height = image.height();
        let color_type = image.color();
        use crossbeam::channel::*;
        use image::ColorType;
        let (texture_send, texture_recv) = bounded(5);
        rayon::spawn(move || {
            let result = Self::create_image_from_raw(
                bytes,
                buffer_size as u64,
                width,
                height,
                match color_type {
                    ColorType::Rgb8 => ImageFormat::ColorType(ColorType::Rgba8),
                    ColorType::Bgr8 => ImageFormat::ColorType(ColorType::Bgra8),
                    _ => ImageFormat::ColorType(color_type),
                },
                graphics,
                command_pool,
                sampler_address_mode,
            );
            texture_send
                .send(result)
                .expect("Failed to send texture result.");
        });
        let texture = texture_recv.recv()??;
        let mut rm_lock = resource_manager.write();
        let image = rm_lock.add_texture(texture);
        let texture_index = rm_lock.get_texture_count() - 1;
        Ok((image, texture_index))
    }

    pub fn create_image_from_raw(
        image_data: Vec<u8>,
        buffer_size: DeviceSize,
        width: u32,
        height: u32,
        format: ImageFormat,
        graphics: Arc<RwLock<ManuallyDrop<Graphics>>>,
        command_pool: Arc<Mutex<ash::vk::CommandPool>>,
        sampler_address_mode: SamplerAddressMode,
    ) -> anyhow::Result<super::Image> {
        let lock = graphics.read();
        let device = lock.logical_device.clone();
        let allocator = lock.allocator.clone();
        let image_format = match format {
            ImageFormat::GltfFormat(gltf_format) => match gltf_format {
                gltf::image::Format::B8G8R8A8 => ash::vk::Format::B8G8R8A8_UNORM,
                gltf::image::Format::R8G8B8A8 => ash::vk::Format::R8G8B8A8_UNORM,
                _ => lock.swapchain.format.format,
            },
            ImageFormat::VkFormat(vk_format) => vk_format,
            ImageFormat::ColorType(color_type) => match color_type {
                image::ColorType::Bgra8 => ash::vk::Format::B8G8R8A8_UNORM,
                image::ColorType::Rgba8 => ash::vk::Format::R8G8B8A8_UNORM,
                image::ColorType::L16 => ash::vk::Format::R16_UNORM,
                _ => lock.swapchain.format.format,
            },
        };
        let cmd_buffer = get_single_time_command_buffer(device.as_ref(), *command_pool.lock());

        let mut staging = super::Buffer::new(
            Arc::downgrade(&device),
            buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
            Arc::downgrade(&allocator),
        );
        unsafe {
            let mapped = staging.map_memory(buffer_size, 0);
            std::ptr::copy_nonoverlapping(
                image_data.as_ptr() as *const c_void,
                mapped,
                buffer_size as usize,
            );
        }
        let width = width as f32;
        let height = height as f32;
        let mip_levels = width.max(height).log2().floor() as u32;
        let mut image = super::Image::new(
            Arc::downgrade(&device),
            ImageUsageFlags::TRANSFER_SRC
                | ImageUsageFlags::TRANSFER_DST
                | ImageUsageFlags::SAMPLED,
            MemoryPropertyFlags::DEVICE_LOCAL,
            image_format,
            SampleCountFlags::TYPE_1,
            Extent2D::builder()
                .width(width as u32)
                .height(height as u32)
                .build(),
            ImageType::TYPE_2D,
            mip_levels,
            ImageAspectFlags::COLOR,
            Arc::downgrade(&allocator),
        );
        let pool_lock = command_pool.lock();
        image.transition_layout(
            ImageLayout::UNDEFINED,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            *pool_lock,
            *lock.graphics_queue.lock(),
            ImageAspectFlags::COLOR,
            mip_levels,
            Some(cmd_buffer),
        );
        image.copy_buffer_to_image(
            staging.buffer,
            width as u32,
            height as u32,
            *pool_lock,
            *lock.graphics_queue.lock(),
            Some(cmd_buffer),
        );
        unsafe {
            image.generate_mipmap(
                ImageAspectFlags::COLOR,
                mip_levels,
                *pool_lock,
                *lock.graphics_queue.lock(),
                Some(cmd_buffer),
            );
        }
        image.create_sampler(mip_levels, sampler_address_mode);
        end_one_time_command_buffer(
            cmd_buffer,
            device.as_ref(),
            *pool_lock,
            *lock.graphics_queue.lock(),
        );
        Ok(image)
    }

    fn get_required_extensions(
        debug: bool,
        window: &winit::window::Window,
        entry: &Entry,
    ) -> anyhow::Result<Vec<CString>> {
        let extensions = enumerate_required_extensions(window)
            .with_context(|| "Failed to enumerate required extensions.")?;
        let mut extensions = extensions
            .into_iter()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();
        if debug {
            let instance_extensions = entry.enumerate_instance_extension_properties()?;
            let nv_checkpoint_extension =
                std::ffi::CString::new("VK_KHR_get_physical_device_properties2")
                    .expect("Failed to construct extension name.");
            let mut required_debug_extensions = vec![DebugUtils::name().to_owned()];
            //required_debug_extensions.push(nv_checkpoint_extension);
            for extension in instance_extensions.iter() {
                let extension_name = extension.extension_name.as_ptr();
                unsafe {
                    let extension_name = std::ffi::CStr::from_ptr(extension_name).to_owned();
                    for required_extension in required_debug_extensions.iter() {
                        if required_extension.to_str()? == extension_name.to_str()? {
                            extensions.push(extension_name.clone());
                            log::warn!("Instance extension enabled: {}", extension_name.to_str()?);
                        }
                    }
                }
            }
        }
        Ok(extensions)
    }

    unsafe extern "system" fn debug_callback(
        severity: DebugUtilsMessageSeverityFlagsEXT,
        _message_type: DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
        _p_user_data: *mut c_void,
    ) -> Bool32 {
        let message = CStr::from_ptr((*p_callback_data).p_message);
        if let Ok(msg) = message.to_str() {
            if msg.starts_with("Device Extension") {
                return FALSE;
            }
            match severity {
                DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::info!("{}", msg),
                DebugUtilsMessageSeverityFlagsEXT::WARNING => log::warn!("{}", msg),
                DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("{}", msg),
                _ => (),
            }
        }
        FALSE
    }
}
