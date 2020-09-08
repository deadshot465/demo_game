use ash::{
    extensions::khr::Surface,
    vk::*
};
use super::physical_device::QueueIndices;
use std::sync::Arc;

pub struct Swapchain {
    pub swapchain: SwapchainKHR,
    pub extent: Extent2D,
    pub format: SurfaceFormatKHR,
    pub present_mode: PresentModeKHR,
    pub swapchain_images: Vec<super::Image>,
    swapchain_loader: ash::extensions::khr::Swapchain,
    capabilities: SurfaceCapabilitiesKHR,
}

impl Swapchain {
    pub fn new(surface_loader: &Surface,
               surface: SurfaceKHR,
               physical_device: PhysicalDevice,
               window: &winit::window::Window,
               queue_indices: QueueIndices, instance: &ash::Instance, device: Arc<ash::Device>) -> Self {
        let (capabilities, formats, present_modes) = Swapchain::get_swapchain_details(surface_loader, surface, physical_device);
        let mut swapchain = Swapchain {
            swapchain: SwapchainKHR::null(),
            capabilities,
            extent: Swapchain::choose_extent(&capabilities, window),
            format: Swapchain::choose_format(&formats),
            present_mode: Swapchain::choose_present_mode(&present_modes),
            swapchain_loader: ash::extensions::khr::Swapchain::new(instance, device.as_ref()),
            swapchain_images: vec![]
        };
        swapchain.create_swapchain(surface, queue_indices);
        unsafe {
            let images = swapchain.swapchain_loader.get_swapchain_images(swapchain.swapchain)
                .expect("Failed to acquire swapchain images.");
            let format = swapchain.format.format;
            for image in images.into_iter() {
                let img = super::Image::from_image(image, device.clone(), format, ImageAspectFlags::COLOR, 1);
                swapchain.swapchain_images.push(img);
            }
        }
        swapchain
    }

    fn choose_format(formats: &Vec<SurfaceFormatKHR>) -> SurfaceFormatKHR {
        for format in formats.iter() {
            if format.format == Format::B8G8R8A8_UNORM &&
                format.color_space == ColorSpaceKHR::SRGB_NONLINEAR {
                return *format;
            }
        }
        formats[0]
    }

    fn choose_extent(capabilities: &SurfaceCapabilitiesKHR, window: &winit::window::Window) -> Extent2D {
        if capabilities.min_image_extent.width != u32::max_value() {
            capabilities.current_extent
        }
        else {
            let inner_size = window.inner_size();
            let actual_width: u32;
            let actual_height: u32;
            match inner_size {
                winit::dpi::PhysicalSize {
                    width, height
                } => {
                    actual_width = if width < capabilities.min_image_extent.width {
                        capabilities.min_image_extent.width
                    } else if width > capabilities.max_image_extent.width {
                        capabilities.max_image_extent.width
                    } else {
                        width
                    };

                    actual_height = if height < capabilities.min_image_extent.height {
                        capabilities.min_image_extent.height
                    } else if height > capabilities.max_image_extent.height {
                        capabilities.max_image_extent.height
                    } else {
                        height
                    };
                }
            }
            let extent = Extent2D::builder()
                .width(actual_width)
                .height(actual_height)
                .build();
            extent
        }
    }

    fn choose_present_mode(present_modes: &Vec<PresentModeKHR>) -> PresentModeKHR {
        let mut fifo_support = false;
        for mode in present_modes.iter() {
            match *mode {
                PresentModeKHR::MAILBOX => {
                    return *mode;
                },
                PresentModeKHR::FIFO => {
                    fifo_support = true;
                },
                _ => ()
            }
        }
        if fifo_support {
            PresentModeKHR::FIFO
        } else {
            PresentModeKHR::IMMEDIATE
        }
    }

    fn get_swapchain_details(surface_loader: &Surface, surface: SurfaceKHR, physical_device: PhysicalDevice) -> (SurfaceCapabilitiesKHR, Vec<SurfaceFormatKHR>, Vec<PresentModeKHR>) {
        unsafe {
            let capabilities = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .expect("Failed to get surface capabilities");
            let formats = surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .expect("Failed to get surface formats.");
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .expect("Failed to get available present modes.");
            (capabilities, formats, present_modes)
        }
    }

    fn create_swapchain(&mut self, surface: SurfaceKHR,
                        queue_indices: QueueIndices) {
        let min_image_count = if self.capabilities.max_image_count <= 0 {
            self.capabilities.min_image_count + 1
        } else if self.capabilities.min_image_count + 1 > self.capabilities.max_image_count {
            self.capabilities.max_image_count
        } else {
            self.capabilities.min_image_count + 1
        };

        log::info!("Present mode: {:?}", self.present_mode);
        log::info!("Color space: {:?}", self.format.color_space);
        log::info!("Swapchain format: {:?}", self.format.format);
        log::info!("Min image count: {}", min_image_count);

        let mut create_info = SwapchainCreateInfoKHR::builder()
            .min_image_count(min_image_count)
            .present_mode(self.present_mode)
            .surface(surface)
            .clipped(false)
            .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
            .image_array_layers(1)
            .image_color_space(self.format.color_space)
            .image_extent(self.extent)
            .image_format(self.format.format)
            .pre_transform(self.capabilities.current_transform)
            .image_usage(ImageUsageFlags::COLOR_ATTACHMENT);

        let indices = vec![
            queue_indices.graphics_family.unwrap(),
            queue_indices.present_family.unwrap()
        ];

        if indices[0] != indices[1] {
            create_info = create_info.image_sharing_mode(SharingMode::CONCURRENT)
                .queue_family_indices(&indices)
        }
        else {
            create_info = create_info.image_sharing_mode(SharingMode::EXCLUSIVE);
        }

        unsafe {
            self.swapchain = self.swapchain_loader.create_swapchain(&create_info, None)
                .expect("Failed to create swapchain.");
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            log::info!("Swapchain successfully dropped.");
        }
    }
}