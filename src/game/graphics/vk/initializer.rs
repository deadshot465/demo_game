use anyhow::Context;
use ash::{Entry, Instance};
use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::{Swapchain, Surface};
use ash::version::{EntryV1_0, InstanceV1_0, DeviceV1_0};
use ash::vk::*;
use ash_window::enumerate_required_extensions;
use crossbeam::sync::ShardedLock;
use std::collections::HashSet;
use std::ffi::{CString, c_void, CStr};
use std::sync::Weak;
use vk_mem::Allocator;

pub struct Initializer {}

impl Initializer {
    pub fn create_instance(debug: bool, enabled_layers: &[CString], entry: &Entry, window: &winit::window::Window) -> anyhow::Result<Instance> {
        let app_name = CString::new("Demo Engine Rust")?;
        let engine_name = CString::new("Demo Engine")?;
        let app_info = ApplicationInfo::builder()
            .api_version(make_version(1, 2, 0))
            .application_name(&*app_name)
            .application_version(make_version(0, 0, 1))
            .engine_name(&*engine_name)
            .engine_version(make_version(0, 0, 1));

        let extensions = Self::get_required_extensions(debug, window)?;
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

        unsafe {
            let instance = entry.create_instance(&instance_info, None)
                .expect("Failed to create Vulkan instance.");
            log::info!("Vulkan instance successfully created.");
            Ok(instance)
        }
    }

    pub fn create_debug_messenger(instance: &Instance, entry: &Entry) -> DebugUtilsMessengerEXT {
        let create_info = DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(DebugUtilsMessageSeverityFlagsEXT::ERROR |
                DebugUtilsMessageSeverityFlagsEXT::WARNING |
                DebugUtilsMessageSeverityFlagsEXT::VERBOSE)
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

    pub fn create_surface(window: &winit::window::Window, entry: &Entry, instance: &Instance) -> anyhow::Result<SurfaceKHR> {
        unsafe {
            let surface = ash_window::create_surface(entry, instance, window, None)
                .with_context(|| "Failed to create surface.")?;
            Ok(surface)
        }
    }

    pub fn create_logical_device(instance: &Instance, physical_device: &super::PhysicalDevice,
                             enabled_layers: &[CString], debug: bool) -> (ash::Device, Queue, Queue, Queue) {
        let layers = enabled_layers.iter().map(|s| {
            s.as_ptr()
        }).collect::<Vec<_>>();
        let extensions = vec![Swapchain::name()];
        let extensions = extensions.into_iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();
        let features = PhysicalDeviceFeatures::builder()
            .tessellation_shader(physical_device.feature_support.tessellation_shader)
            .shader_sampled_image_array_dynamic_indexing(physical_device.feature_support.shader_sampled_image_array_dynamic_indexing)
            .sampler_anisotropy(physical_device.feature_support.sampler_anisotropy)
            .sample_rate_shading(physical_device.feature_support.sample_rate_shading)
            .geometry_shader(physical_device.feature_support.geometry_shader);
        let mut indexing_features = PhysicalDeviceDescriptorIndexingFeatures::builder()
            .runtime_descriptor_array(physical_device.feature_support.runtime_descriptor_array)
            .descriptor_binding_partially_bound(physical_device.feature_support.descriptor_binding_partially_bound);
        let mut queue_create_infos = vec![];
        let mut unique_indices = HashSet::new();
        unique_indices.insert(physical_device.queue_indices.graphics_family.unwrap_or_default());
        unique_indices.insert(physical_device.queue_indices.present_family.unwrap_or_default());
        unique_indices.insert(physical_device.queue_indices.compute_family.unwrap_or_default());
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
            let graphics_queue = device.get_device_queue(physical_device
                                                             .queue_indices
                                                             .graphics_family.unwrap_or_default(), 0);
            let present_queue = device.get_device_queue(physical_device
                                                            .queue_indices
                                                            .present_family.unwrap_or_default(), 0);
            let compute_queue = device.get_device_queue(physical_device
                                                            .queue_indices
                                                            .compute_family.unwrap_or_default(), 0);
            log::info!("Device queue successfully acquired.");
            log::info!("Logical device successfully created.");
            (device, graphics_queue, present_queue, compute_queue)
        }
    }

    pub fn create_swapchain(surface_loader: &Surface, surface: SurfaceKHR,
                        physical_device: &super::PhysicalDevice, window: &winit::window::Window,
                        instance: &Instance, device: Weak<ash::Device>, allocator: Weak<ShardedLock<Allocator>>) -> super::Swapchain {
        super::Swapchain::new(
            surface_loader, surface, physical_device.physical_device, window,
            physical_device.queue_indices, instance, device, allocator
        )
    }

    pub fn choose_depth_format(depth_formats: Vec<Format>, tiling: ImageTiling, feature_flags: FormatFeatureFlags, instance: &Instance, physical_device: &super::PhysicalDevice) -> Format {
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

    pub fn get_depth_format(instance: &Instance, physical_device: &super::PhysicalDevice) -> Format {
        Self::choose_depth_format(vec![Format::D32_SFLOAT, Format::D24_UNORM_S8_UINT, Format::D16_UNORM],
                                  ImageTiling::OPTIMAL, FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT, instance, physical_device)
    }

    pub fn create_depth_image(device: Weak<ash::Device>,
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

    pub fn get_sample_count(instance: &Instance, physical_device: &super::PhysicalDevice) -> SampleCountFlags {
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

    pub fn create_msaa_image(device: Weak<ash::Device>,
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

    fn get_required_extensions(debug: bool, window: &winit::window::Window) -> anyhow::Result<Vec<*const i8>> {
        let extensions = enumerate_required_extensions(window)
            .with_context(|| "Failed to enumerate required extensions.")?;
        let mut extensions = extensions.into_iter()
            .map(|extension| extension.as_ptr())
            .collect::<Vec<_>>();
        if debug {
            extensions.push(DebugUtils::name().as_ptr());
        }
        Ok(extensions)
    }

    unsafe extern "system" fn debug_callback(severity: DebugUtilsMessageSeverityFlagsEXT,
                                             _message_type: DebugUtilsMessageTypeFlagsEXT,
                                             p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
                                             _p_user_data: *mut c_void) -> Bool32 {
        let message = CStr::from_ptr((*p_callback_data).p_message);
        if let Ok(msg) = message.to_str() {
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