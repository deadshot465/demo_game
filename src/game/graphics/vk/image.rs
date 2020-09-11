use ash::vk::*;
use ash::version::{DeviceV1_0, InstanceV1_0};
use std::ffi::c_void;
use std::sync::Arc;
use std::convert::TryFrom;
use crate::game::util::{get_single_time_command_buffer, end_one_time_command_buffer};
use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::traits::mappable::Mappable;

#[derive(Clone)]
pub struct Image {
    pub image_view: ImageView,
    pub sampler: Sampler,
    pub device_memory: DeviceMemory,
    pub mapped_memory: *mut c_void,
    pub width: u32,
    pub height: u32,
    image: ash::vk::Image,
    logical_device: Arc<ash::Device>,
    is_disposed: bool,
}

impl Image {
    pub fn new(instance: &ash::Instance, device: Arc<ash::Device>,
               physical_device: PhysicalDevice,
               usage_flag: ImageUsageFlags,
               memory_properties: MemoryPropertyFlags, format: Format,
               sample_count: SampleCountFlags, extent: Extent2D, image_type: ImageType,
               mip_levels: u32, aspect_flags: ImageAspectFlags) -> Self {
        let extent = Extent3D::builder()
            .height(extent.height)
            .width(extent.width)
            .depth(1)
            .build();
        let create_info = ImageCreateInfo::builder()
            .usage(usage_flag)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .format(format)
            .extent(extent)
            .array_layers(1)
            .image_type(image_type)
            .initial_layout(ImageLayout::UNDEFINED)
            .mip_levels(mip_levels)
            .samples(sample_count)
            .tiling(ImageTiling::OPTIMAL)
            .build();
        unsafe {
            let image = device.create_image(&create_info, None)
                .expect("Failed to create image.");
            log::info!("Image successfully created.");
            let mut image = Image {
                image,
                logical_device: device,
                image_view: ImageView::null(),
                is_disposed: false,
                sampler: Sampler::null(),
                device_memory: DeviceMemory::null(),
                mapped_memory: std::ptr::null_mut(),
                width: extent.width,
                height: extent.height
            };
            image.allocate_memory(instance, physical_device, memory_properties);
            image.create_image_view(format, aspect_flags, mip_levels);
            image
        }
    }

    pub fn from_image(image: ash::vk::Image, device: Arc<ash::Device>, format: Format,
                      aspect_flags: ImageAspectFlags, mip_levels: u32) -> Self {
        let mut image = Image {
            image,
            logical_device: device,
            image_view: ImageView::null(),
            is_disposed: false,
            sampler: Sampler::null(),
            device_memory: DeviceMemory::null(),
            mapped_memory: std::ptr::null_mut(),
            width: 0,
            height: 0
        };
        image.create_image_view(format, aspect_flags, mip_levels);
        image
    }

    pub fn transition_layout(&mut self, old_layout: ImageLayout, new_layout: ImageLayout,
                             command_pool: CommandPool, graphics_queue: Queue,
                             aspect_flags: ImageAspectFlags, mip_levels: u32) {
        let mut barrier = ImageMemoryBarrier::builder()
            .image(self.image)
            .subresource_range(ImageSubresourceRange::builder()
                .level_count(mip_levels)
                .layer_count(1)
                .base_mip_level(0)
                .base_array_layer(0)
                .aspect_mask(aspect_flags)
                .build())
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .old_layout(old_layout)
            .new_layout(new_layout);

        let mut old_stage = PipelineStageFlags::empty();
        let mut new_stage = PipelineStageFlags::empty();

        match (old_layout, new_layout) {
            (ImageLayout::UNDEFINED, ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => {
                barrier = barrier.dst_access_mask(AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE);
                old_stage = PipelineStageFlags::TOP_OF_PIPE;
                new_stage = PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            },
            (ImageLayout::UNDEFINED, ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL) => {
                barrier = barrier.dst_access_mask(AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);
                old_stage = PipelineStageFlags::TOP_OF_PIPE;
                new_stage = PipelineStageFlags::EARLY_FRAGMENT_TESTS;
            },
            (ImageLayout::UNDEFINED, ImageLayout::TRANSFER_DST_OPTIMAL) => {
                barrier = barrier.dst_access_mask(AccessFlags::TRANSFER_WRITE);
                old_stage = PipelineStageFlags::TOP_OF_PIPE;
                new_stage = PipelineStageFlags::TRANSFER;
            },
            (_, _) => ()
        }

        unsafe {
            let cmd_buffer = get_single_time_command_buffer(self.logical_device.as_ref(), command_pool);
            self.logical_device
                .cmd_pipeline_barrier(cmd_buffer, old_stage,
                                      new_stage, DependencyFlags::empty(),
                                      &[], &[], &[barrier.build()]);
            end_one_time_command_buffer(cmd_buffer, self.logical_device.as_ref(), command_pool, graphics_queue);
        }
    }

    pub fn create_sampler(&mut self, mip_levels: u32) {
        let create_info = SamplerCreateInfo::builder()
            .address_mode_u(SamplerAddressMode::REPEAT)
            .address_mode_v(SamplerAddressMode::REPEAT)
            .address_mode_w(SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .border_color(BorderColor::FLOAT_OPAQUE_BLACK)
            .compare_enable(false)
            .compare_op(CompareOp::ALWAYS)
            .mag_filter(Filter::LINEAR)
            .max_anisotropy(16.0)
            .max_lod(mip_levels as f32)
            .min_filter(Filter::LINEAR)
            .min_lod(0.0)
            .mip_lod_bias(0.0)
            .mipmap_mode(SamplerMipmapMode::LINEAR)
            .unnormalized_coordinates(false)
            .build();
         unsafe {
             self.sampler = self.logical_device.create_sampler(&create_info, None)
                 .expect("Failed to create sampler.");
         }
    }

    pub fn copy_buffer_to_image(&self, source_buffer: Buffer, width: u32, height: u32, command_pool: CommandPool, graphics_queue: Queue) {
        let extent = Extent3D::builder()
            .height(height)
            .width(width)
            .depth(1)
            .build();

        let copy_info = BufferImageCopy::builder()
            .image_extent(extent)
            .image_subresource(ImageSubresourceLayers::builder()
                .base_array_layer(0)
                .layer_count(1)
                .aspect_mask(ImageAspectFlags::COLOR)
                .mip_level(0)
                .build())
            .build();

        let cmd_buffer = get_single_time_command_buffer(self.logical_device.as_ref(), command_pool);
        unsafe {
            self.logical_device
                .cmd_copy_buffer_to_image(cmd_buffer, source_buffer, self.image, ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_info]);
        }
        end_one_time_command_buffer(cmd_buffer, self.logical_device.as_ref(), command_pool, graphics_queue);
    }

    fn create_image_view(&mut self, format: Format, aspect_flags: ImageAspectFlags, mip_levels: u32) {
        let create_info = ImageViewCreateInfo::builder()
            .image(self.image)
            .format(format)
            .components(ComponentMapping::builder()
                .r(ComponentSwizzle::IDENTITY)
                .g(ComponentSwizzle::IDENTITY)
                .b(ComponentSwizzle::IDENTITY)
                .a(ComponentSwizzle::IDENTITY)
                .build())
            .subresource_range(ImageSubresourceRange::builder()
                .aspect_mask(aspect_flags)
                .base_array_layer(0)
                .base_mip_level(0)
                .layer_count(1)
                .level_count(mip_levels)
                .build())
            .view_type(ImageViewType::TYPE_2D)
            .build();

        unsafe {
            self.image_view = self.logical_device.create_image_view(&create_info, None)
                .expect("Failed to create image view.");
        }
    }

    pub unsafe fn generate_mipmap(&mut self, aspect_flags: ImageAspectFlags, mip_levels: u32, command_pool: CommandPool, graphics_queue: Queue) {
        let mut barrier = ImageMemoryBarrier::builder()
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .subresource_range(ImageSubresourceRange::builder()
                .aspect_mask(aspect_flags)
                .base_array_layer(0)
                .layer_count(1)
                .level_count(mip_levels)
                .build())
            .image(self.image)
            .build();

        let cmd_buffer = get_single_time_command_buffer(
            self.logical_device.as_ref(), command_pool);

        let mut width = i32::try_from(self.width).unwrap();
        let mut height = i32::try_from(self.height).unwrap();

        for i in 1..mip_levels {
            barrier.subresource_range.base_mip_level = i - 1;
            barrier.src_access_mask = AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = AccessFlags::TRANSFER_READ;
            barrier.old_layout = ImageLayout::TRANSFER_DST_OPTIMAL;
            barrier.new_layout = ImageLayout::TRANSFER_SRC_OPTIMAL;

            self.logical_device.cmd_pipeline_barrier(cmd_buffer, PipelineStageFlags::TRANSFER,
                                            PipelineStageFlags::TRANSFER, DependencyFlags::empty(), &[],
                                            &[], &[barrier]);

            let mut image_blit = ImageBlit::builder()
                .dst_subresource(ImageSubresourceLayers::builder()
                    .layer_count(1)
                    .base_array_layer(0)
                    .aspect_mask(aspect_flags)
                    .mip_level(i)
                    .build())
                .src_subresource(ImageSubresourceLayers::builder()
                    .mip_level(i - 1)
                    .aspect_mask(aspect_flags)
                    .layer_count(1)
                    .base_array_layer(0)
                    .build())
                .build();
            image_blit.src_offsets[0].x = 0;
            image_blit.src_offsets[0].y = 0;
            image_blit.src_offsets[0].z = 0;
            image_blit.src_offsets[1].x = width;
            image_blit.src_offsets[1].y = height;
            image_blit.src_offsets[1].z = 1;

            image_blit.dst_offsets[0].x = 0;
            image_blit.dst_offsets[0].y = 0;
            image_blit.dst_offsets[0].z = 0;
            image_blit.dst_offsets[1].x = if width > 1 {
                width / 2
            } else {
                1
            };
            image_blit.dst_offsets[1].y = if height > 1 {
                height / 2
            } else {
                1
            };
            image_blit.dst_offsets[1].z = 1;

            self.logical_device.cmd_blit_image(cmd_buffer, self.image, ImageLayout::TRANSFER_SRC_OPTIMAL,
                                      self.image, ImageLayout::TRANSFER_DST_OPTIMAL, &[image_blit], Filter::LINEAR);

            barrier.src_access_mask = AccessFlags::TRANSFER_READ;
            barrier.dst_access_mask = AccessFlags::SHADER_READ;
            barrier.old_layout = ImageLayout::TRANSFER_SRC_OPTIMAL;
            barrier.new_layout = ImageLayout::SHADER_READ_ONLY_OPTIMAL;

            self.logical_device.cmd_pipeline_barrier(cmd_buffer, PipelineStageFlags::TRANSFER,
                                            PipelineStageFlags::FRAGMENT_SHADER, DependencyFlags::empty(), &[],
                                            &[], &[barrier]);
            width = if width > 1 {
                width / 2
            } else {
                width
            };
            height = if height > 1 {
                height / 2
            } else {
                height
            };
        }
        barrier.subresource_range.base_mip_level = mip_levels - 1;
        barrier.src_access_mask = AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = AccessFlags::SHADER_READ;
        barrier.old_layout = ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.new_layout = ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        self.logical_device.cmd_pipeline_barrier(cmd_buffer, PipelineStageFlags::TRANSFER,
                                        PipelineStageFlags::FRAGMENT_SHADER, DependencyFlags::empty(),
                                        &[], &[], &[barrier]);
        end_one_time_command_buffer(cmd_buffer, self.logical_device.as_ref(), command_pool, graphics_queue);
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl Disposable for Image {
    fn dispose(&mut self) {
        log::info!("Disposing image...");
        if self.is_disposed {
            return
        }
        unsafe {
            if !self.mapped_memory.is_null() {
                self.unmap_memory();
            }
            if self.device_memory != DeviceMemory::null() {
                self.logical_device
                    .free_memory(self.device_memory, None);
            }
            if self.sampler != Sampler::null() {
                self.logical_device.destroy_sampler(self.sampler, None)
            }
            if self.image_view != ImageView::null() {
                self.logical_device.destroy_image_view(self.image_view, None);
            }
            if self.device_memory != DeviceMemory::null() {
                self.logical_device.destroy_image(self.image, None);
            }
        }
        self.is_disposed = true;
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

impl Mappable for Image {
    fn allocate_memory(&mut self, instance: &ash::Instance, physical_device: PhysicalDevice, memory_properties: MemoryPropertyFlags) -> DeviceMemory {
        unsafe {
            let requirements = self.logical_device.get_image_memory_requirements(self.image);
            let device_memory = self.map_device_memory(instance,
                                                       &requirements, physical_device, memory_properties);
            self.logical_device.bind_image_memory(self.image, device_memory, 0)
                .expect("Failed to bind image memory.");
            self.device_memory = device_memory;
            self.device_memory
        }
    }

    fn map_memory(&mut self, device_size: u64, offset: u64) -> *mut c_void {
        unsafe {
            self.mapped_memory = self.logical_device
                .map_memory(self.device_memory, offset, device_size, MemoryMapFlags::empty())
                .expect("Failed to map device memory.");
            self.mapped_memory
        }
    }

    fn unmap_memory(&mut self) {
        unsafe {
            self.logical_device.unmap_memory(self.device_memory);
        }
    }

    fn get_memory_type_index(&self, instance: &ash::Instance, physical_device: PhysicalDevice, memory_type: u32, memory_properties: MemoryPropertyFlags) -> u32 {
        unsafe {
            let properties = instance.get_physical_device_memory_properties(physical_device);
            for i in 0..properties.memory_type_count {
                if (memory_type & (1 << i)) != 0 &&
                    ((properties.memory_types[i as usize].property_flags & memory_properties) == memory_properties) {
                    return i as u32;
                }
            }
        }
        0
    }

    fn map_device_memory(&mut self, instance: &ash::Instance, memory_requirements: &MemoryRequirements, physical_device: PhysicalDevice, memory_properties: MemoryPropertyFlags) -> DeviceMemory {
        let memory_type_index = self.get_memory_type_index(instance, physical_device,
                                                           memory_requirements.memory_type_bits, memory_properties);
        let allocate_info = MemoryAllocateInfo::builder()
            .memory_type_index(memory_type_index)
            .allocation_size(memory_requirements.size)
            .build();
        unsafe {
            let device_memory = self.logical_device
                .allocate_memory(&allocate_info, None)
                .expect("Failed to allocate device memory.");
            device_memory
        }
    }
}