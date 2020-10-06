use ash::vk::TRUE;
use ash::{
    extensions::khr::{Surface, Swapchain},
    version::{InstanceV1_0, InstanceV1_1},
    vk::{
        PhysicalDeviceDescriptorIndexingFeatures, PhysicalDeviceFeatures2,
        PhysicalDeviceProperties, PhysicalDeviceType, QueueFlags, SurfaceKHR,
    },
    Instance,
};
use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::c_char;

#[derive(Copy, Clone, Debug)]
pub struct QueueIndices {
    pub graphics_family: Option<u32>,
    pub present_family: Option<u32>,
    pub compute_family: Option<u32>,
}

#[derive(Copy, Clone, Debug)]
pub struct FeatureSupport {
    pub geometry_shader: bool,
    pub tessellation_shader: bool,
    pub sample_rate_shading: bool,
    pub sampler_anisotropy: bool,
    pub shader_sampled_image_array_dynamic_indexing: bool,
    pub runtime_descriptor_array: bool,
    pub descriptor_binding_partially_bound: bool,
}

pub struct PhysicalDevice {
    pub physical_device: ash::vk::PhysicalDevice,
    pub queue_indices: QueueIndices,
    pub device_properties: PhysicalDeviceProperties,
    pub feature_support: FeatureSupport,
}

impl Default for QueueIndices {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueIndices {
    pub fn new() -> Self {
        QueueIndices {
            graphics_family: None,
            present_family: None,
            compute_family: None,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.graphics_family.is_some()
            && self.present_family.is_some()
            && self.compute_family.is_some()
    }
}

impl PhysicalDevice {
    pub fn new(instance: &Instance, surface_loader: &Surface, surface: SurfaceKHR) -> Self {
        let (device, queue_indices, properties) =
            PhysicalDevice::get_physical_device(instance, surface_loader, surface);
        unsafe {
            let features = instance.get_physical_device_features(device);

            let mut indexing_feature = PhysicalDeviceDescriptorIndexingFeatures::default();
            let mut features2 = PhysicalDeviceFeatures2::default();
            features2.p_next = &mut indexing_feature as *mut _ as *mut std::ffi::c_void;
            instance.get_physical_device_features2(device, &mut features2);

            let feature_support = FeatureSupport {
                geometry_shader: features.geometry_shader == TRUE,
                tessellation_shader: features.tessellation_shader == TRUE,
                sample_rate_shading: features.sample_rate_shading == TRUE,
                sampler_anisotropy: features.sampler_anisotropy == TRUE,
                shader_sampled_image_array_dynamic_indexing: features
                    .shader_sampled_image_array_dynamic_indexing
                    == TRUE,
                runtime_descriptor_array: indexing_feature.runtime_descriptor_array == TRUE,
                descriptor_binding_partially_bound: indexing_feature
                    .descriptor_binding_partially_bound
                    == TRUE,
            };

            log::info!("Geometry shader: {}", feature_support.geometry_shader);
            log::info!(
                "Tessellation shader: {}",
                feature_support.tessellation_shader
            );
            log::info!(
                "Sample rate shading: {}",
                feature_support.sample_rate_shading
            );
            log::info!("Sampler Anisotropy: {}", feature_support.sampler_anisotropy);
            log::info!(
                "Shader sampled image array dynamic indexing: {}",
                feature_support.shader_sampled_image_array_dynamic_indexing
            );
            log::info!(
                "Runtime descriptor array: {}",
                feature_support.runtime_descriptor_array
            );
            log::info!(
                "Descriptor binding partially bound: {}",
                feature_support.descriptor_binding_partially_bound
            );

            PhysicalDevice {
                physical_device: device,
                queue_indices,
                device_properties: properties,
                feature_support,
            }
        }
    }

    fn get_queue_indices(
        instance: &Instance,
        surface_loader: &Surface,
        device: ash::vk::PhysicalDevice,
        surface: SurfaceKHR,
    ) -> QueueIndices {
        let mut queue_indices = QueueIndices::new();
        unsafe {
            let queue_families = instance.get_physical_device_queue_family_properties(device);

            for item in queue_families.iter().enumerate() {
                let surface_support = surface_loader
                    .get_physical_device_surface_support(device, item.0 as u32, surface)
                    .expect("Failed to query surface support.");

                if item.1.queue_count > 0
                    && ((item.1.queue_flags & QueueFlags::GRAPHICS) == QueueFlags::GRAPHICS)
                {
                    queue_indices.graphics_family = Some(item.0 as u32);
                }

                if item.1.queue_count > 0 && surface_support {
                    queue_indices.present_family = Some(item.0 as u32);
                }

                if item.1.queue_count > 0
                    && ((item.1.queue_flags & QueueFlags::COMPUTE) != QueueFlags::COMPUTE)
                {
                    queue_indices.compute_family = Some(item.0 as u32);
                }

                if queue_indices.is_ready() {
                    break;
                }
            }
        }
        queue_indices
    }

    fn check_extension_support(instance: &Instance, device: ash::vk::PhysicalDevice) -> bool {
        let mut required_extension = HashSet::new();
        required_extension.insert(Swapchain::name());
        unsafe {
            let extensions = instance
                .enumerate_device_extension_properties(device)
                .expect("Failed to enumerate physical device extensions.");
            for extension in extensions.iter() {
                let name = extension.extension_name.as_ptr() as *const c_char;
                let name = CStr::from_ptr(name);
                required_extension.remove(&name);
            }
        }
        required_extension.is_empty()
    }

    fn is_device_suitable(
        instance: &Instance,
        surface_loader: &Surface,
        device: ash::vk::PhysicalDevice,
        surface: SurfaceKHR,
    ) -> (bool, Option<QueueIndices>) {
        let queue_indices =
            PhysicalDevice::get_queue_indices(instance, surface_loader, device, surface);
        if !queue_indices.is_ready() {
            return (false, None);
        }
        unsafe {
            let properties = instance.get_physical_device_properties(device);
            let raw_name = properties.device_name.as_ptr() as *const c_char;
            let name = CStr::from_ptr(raw_name);
            log::info!("{}", name.to_str().unwrap());

            let result = PhysicalDevice::check_extension_support(instance, device);
            (result, Some(queue_indices))
        }
    }

    fn get_physical_device(
        instance: &Instance,
        surface_loader: &Surface,
        surface: SurfaceKHR,
    ) -> (
        ash::vk::PhysicalDevice,
        QueueIndices,
        PhysicalDeviceProperties,
    ) {
        let mut selected_device = ash::vk::PhysicalDevice::null();
        let mut queue_indices = QueueIndices::new();
        unsafe {
            let physical_devices = instance
                .enumerate_physical_devices()
                .expect("Failed to enumerate available physical devices.");
            for device in physical_devices.iter() {
                let (res, _queue_indices) =
                    PhysicalDevice::is_device_suitable(instance, surface_loader, *device, surface);
                if !res {
                    continue;
                }
                queue_indices = _queue_indices.unwrap();
                selected_device = *device;
                let properties = instance.get_physical_device_properties(*device);
                if properties.device_type == PhysicalDeviceType::DISCRETE_GPU {
                    return (selected_device, queue_indices, properties);
                }
            }
        }
        (
            selected_device,
            queue_indices,
            PhysicalDeviceProperties::builder().build(),
        )
    }
}
