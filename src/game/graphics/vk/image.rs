use ash::version::DeviceV1_0;
use ash::vk::*;
use crossbeam::sync::ShardedLock;
use std::convert::TryFrom;
use std::ffi::c_void;
use std::sync::Weak;
use vk_mem::{
    Allocation, AllocationCreateFlags, AllocationCreateInfo, AllocationInfo, Allocator, MemoryUsage,
};

use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::traits::mappable::Mappable;
use crate::game::util::{end_one_time_command_buffer, get_single_time_command_buffer};

#[derive(Clone)]
pub struct Image {
    pub image_view: ImageView,
    pub sampler: Sampler,
    pub device_memory: DeviceMemory,
    pub mapped_memory: *mut c_void,
    pub width: u32,
    pub height: u32,
    image: ash::vk::Image,
    logical_device: Weak<ash::Device>,
    is_disposed: bool,
    allocator: Weak<ShardedLock<Allocator>>,
    allocation: Allocation,
    allocation_info: Option<AllocationInfo>,
}

unsafe impl Send for Image {}
unsafe impl Sync for Image {}

impl Image {
    pub fn new(
        device: Weak<ash::Device>,
        usage_flag: ImageUsageFlags,
        memory_properties: MemoryPropertyFlags,
        format: Format,
        sample_count: SampleCountFlags,
        extent: Extent2D,
        image_type: ImageType,
        mip_levels: u32,
        aspect_flags: ImageAspectFlags,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> Self {
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

        let allocation_info = AllocationCreateInfo {
            usage: MemoryUsage::GpuOnly,
            flags: if (memory_properties & MemoryPropertyFlags::HOST_VISIBLE)
                == MemoryPropertyFlags::HOST_VISIBLE
                && (memory_properties & MemoryPropertyFlags::HOST_COHERENT)
                    == MemoryPropertyFlags::HOST_COHERENT
            {
                AllocationCreateFlags::MAPPED
            } else {
                AllocationCreateFlags::NONE
            },
            required_flags: memory_properties,
            preferred_flags: MemoryPropertyFlags::empty(),
            memory_type_bits: 0,
            pool: None,
            user_data: None,
        };
        let arc = allocator.upgrade().unwrap();
        let lock = arc.read().unwrap();
        let (image, allocation, allocation_info) = lock
            .create_image(&create_info, &allocation_info)
            .expect("Failed to create image using the VMA allocator.");
        drop(lock);
        let device_memory = allocation_info.get_device_memory();
        let mapped = allocation_info.get_mapped_data();
        let _device = device.upgrade().unwrap();
        let mut image = Image {
            image,
            logical_device: device,
            image_view: ImageView::null(),
            is_disposed: false,
            sampler: Sampler::null(),
            device_memory,
            mapped_memory: mapped as *mut c_void,
            width: extent.width,
            height: extent.height,
            allocator,
            allocation,
            allocation_info: Some(allocation_info),
        };
        image.create_image_view(_device.as_ref(), format, aspect_flags, mip_levels);
        image
    }

    pub fn from_image(
        image: ash::vk::Image,
        device: Weak<ash::Device>,
        format: Format,
        aspect_flags: ImageAspectFlags,
        mip_levels: u32,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> Self {
        let _device = device.upgrade().unwrap();
        let mut image = Image {
            image,
            logical_device: device,
            image_view: ImageView::null(),
            is_disposed: false,
            allocator,
            allocation: Allocation::null(),
            sampler: Sampler::null(),
            device_memory: DeviceMemory::null(),
            mapped_memory: std::ptr::null_mut(),
            width: 0,
            height: 0,
            allocation_info: None,
        };
        image.create_image_view(_device.as_ref(), format, aspect_flags, mip_levels);
        image
    }

    pub fn transition_layout(
        &mut self,
        old_layout: ImageLayout,
        new_layout: ImageLayout,
        command_pool: CommandPool,
        graphics_queue: Queue,
        aspect_flags: ImageAspectFlags,
        mip_levels: u32,
        command_buffer: Option<CommandBuffer>,
    ) {
        let mut barrier = ImageMemoryBarrier::builder()
            .image(self.image)
            .subresource_range(
                ImageSubresourceRange::builder()
                    .level_count(mip_levels)
                    .layer_count(1)
                    .base_mip_level(0)
                    .base_array_layer(0)
                    .aspect_mask(aspect_flags)
                    .build(),
            )
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .old_layout(old_layout)
            .new_layout(new_layout);

        let mut old_stage = PipelineStageFlags::empty();
        let mut new_stage = PipelineStageFlags::empty();

        match (old_layout, new_layout) {
            (ImageLayout::UNDEFINED, ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => {
                barrier = barrier.dst_access_mask(
                    AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE,
                );
                old_stage = PipelineStageFlags::TOP_OF_PIPE;
                new_stage = PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            }
            (ImageLayout::UNDEFINED, ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL) => {
                barrier = barrier.dst_access_mask(
                    AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                        | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                );
                old_stage = PipelineStageFlags::TOP_OF_PIPE;
                new_stage = PipelineStageFlags::EARLY_FRAGMENT_TESTS;
            }
            (ImageLayout::UNDEFINED, ImageLayout::TRANSFER_DST_OPTIMAL) => {
                barrier = barrier.dst_access_mask(AccessFlags::TRANSFER_WRITE);
                old_stage = PipelineStageFlags::TOP_OF_PIPE;
                new_stage = PipelineStageFlags::TRANSFER;
            }
            (_, _) => (),
        }

        unsafe {
            let device = self.logical_device.upgrade().unwrap();
            let cmd_buffer = if let Some(buffer) = command_buffer {
                buffer
            } else {
                get_single_time_command_buffer(device.as_ref(), command_pool)
            };
            device.cmd_pipeline_barrier(
                cmd_buffer,
                old_stage,
                new_stage,
                DependencyFlags::empty(),
                &[],
                &[],
                &[barrier.build()],
            );
            if command_buffer.is_none() {
                end_one_time_command_buffer(
                    cmd_buffer,
                    device.as_ref(),
                    command_pool,
                    graphics_queue,
                );
            }
        }
    }

    pub fn create_sampler(&mut self, mip_levels: u32, sampler_address_mode: SamplerAddressMode) {
        let create_info = SamplerCreateInfo::builder()
            .address_mode_u(sampler_address_mode)
            .address_mode_v(sampler_address_mode)
            .address_mode_w(sampler_address_mode)
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
            self.sampler = self
                .logical_device
                .upgrade()
                .unwrap()
                .create_sampler(&create_info, None)
                .expect("Failed to create sampler.");
            log::info!("Successfully created sampler.");
        }
    }

    pub fn copy_buffer_to_image(
        &self,
        source_buffer: Buffer,
        width: u32,
        height: u32,
        command_pool: CommandPool,
        graphics_queue: Queue,
        command_buffer: Option<CommandBuffer>,
    ) {
        let extent = Extent3D::builder()
            .height(height)
            .width(width)
            .depth(1)
            .build();

        let copy_info = BufferImageCopy::builder()
            .image_extent(extent)
            .image_subresource(
                ImageSubresourceLayers::builder()
                    .base_array_layer(0)
                    .layer_count(1)
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .build(),
            )
            .build();

        let device = self.logical_device.upgrade().unwrap();
        let cmd_buffer = if let Some(buffer) = command_buffer {
            buffer
        } else {
            get_single_time_command_buffer(device.as_ref(), command_pool)
        };
        unsafe {
            device.cmd_copy_buffer_to_image(
                cmd_buffer,
                source_buffer,
                self.image,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copy_info],
            );
        }
        if command_buffer.is_none() {
            end_one_time_command_buffer(cmd_buffer, device.as_ref(), command_pool, graphics_queue);
        }
    }

    fn create_image_view(
        &mut self,
        device: &ash::Device,
        format: Format,
        aspect_flags: ImageAspectFlags,
        mip_levels: u32,
    ) {
        let create_info = ImageViewCreateInfo::builder()
            .image(self.image)
            .format(format)
            .components(
                ComponentMapping::builder()
                    .r(ComponentSwizzle::IDENTITY)
                    .g(ComponentSwizzle::IDENTITY)
                    .b(ComponentSwizzle::IDENTITY)
                    .a(ComponentSwizzle::IDENTITY)
                    .build(),
            )
            .subresource_range(
                ImageSubresourceRange::builder()
                    .aspect_mask(aspect_flags)
                    .base_array_layer(0)
                    .base_mip_level(0)
                    .layer_count(1)
                    .level_count(mip_levels)
                    .build(),
            )
            .view_type(ImageViewType::TYPE_2D)
            .build();

        unsafe {
            self.image_view = device
                .create_image_view(&create_info, None)
                .expect("Failed to create image view.");
        }
    }

    pub unsafe fn generate_mipmap(
        &mut self,
        aspect_flags: ImageAspectFlags,
        mip_levels: u32,
        command_pool: CommandPool,
        graphics_queue: Queue,
        command_buffer: Option<CommandBuffer>,
    ) {
        let mut barrier = ImageMemoryBarrier::builder()
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .subresource_range(
                ImageSubresourceRange::builder()
                    .aspect_mask(aspect_flags)
                    .base_array_layer(0)
                    .layer_count(1)
                    .level_count(1)
                    .build(),
            )
            .image(self.image)
            .build();

        let device = self.logical_device.upgrade().unwrap();
        let cmd_buffer = if let Some(buffer) = command_buffer {
            buffer
        } else {
            get_single_time_command_buffer(device.as_ref(), command_pool)
        };

        let mut width = i32::try_from(self.width).unwrap();
        let mut height = i32::try_from(self.height).unwrap();

        for i in 1..mip_levels {
            barrier.subresource_range.base_mip_level = i - 1;
            barrier.src_access_mask = AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = AccessFlags::TRANSFER_READ;
            barrier.old_layout = ImageLayout::TRANSFER_DST_OPTIMAL;
            barrier.new_layout = ImageLayout::TRANSFER_SRC_OPTIMAL;

            device.cmd_pipeline_barrier(
                cmd_buffer,
                PipelineStageFlags::TRANSFER,
                PipelineStageFlags::TRANSFER,
                DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );

            let mut image_blit = ImageBlit::builder()
                .dst_subresource(
                    ImageSubresourceLayers::builder()
                        .layer_count(1)
                        .base_array_layer(0)
                        .aspect_mask(aspect_flags)
                        .mip_level(i)
                        .build(),
                )
                .src_subresource(
                    ImageSubresourceLayers::builder()
                        .mip_level(i - 1)
                        .aspect_mask(aspect_flags)
                        .layer_count(1)
                        .base_array_layer(0)
                        .build(),
                )
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
            image_blit.dst_offsets[1].x = if width > 1 { width / 2 } else { 1 };
            image_blit.dst_offsets[1].y = if height > 1 { height / 2 } else { 1 };
            image_blit.dst_offsets[1].z = 1;

            device.cmd_blit_image(
                cmd_buffer,
                self.image,
                ImageLayout::TRANSFER_SRC_OPTIMAL,
                self.image,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                &[image_blit],
                Filter::LINEAR,
            );

            barrier.src_access_mask = AccessFlags::TRANSFER_READ;
            barrier.dst_access_mask = AccessFlags::SHADER_READ;
            barrier.old_layout = ImageLayout::TRANSFER_SRC_OPTIMAL;
            barrier.new_layout = ImageLayout::SHADER_READ_ONLY_OPTIMAL;

            device.cmd_pipeline_barrier(
                cmd_buffer,
                PipelineStageFlags::TRANSFER,
                PipelineStageFlags::FRAGMENT_SHADER,
                DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            width = if width > 1 { width / 2 } else { width };
            height = if height > 1 { height / 2 } else { height };
        }
        barrier.subresource_range.base_mip_level = mip_levels - 1;
        barrier.src_access_mask = AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = AccessFlags::SHADER_READ;
        barrier.old_layout = ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.new_layout = ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        device.cmd_pipeline_barrier(
            cmd_buffer,
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::FRAGMENT_SHADER,
            DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
        if command_buffer.is_none() {
            end_one_time_command_buffer(cmd_buffer, device.as_ref(), command_pool, graphics_queue);
        }
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
        if self.is_disposed {
            return;
        }
        let device = self.logical_device.upgrade().unwrap();
        unsafe {
            if !self.mapped_memory.is_null() {
                self.unmap_memory();
            }
            if self.sampler != Sampler::null() {
                device.destroy_sampler(self.sampler, None);
            }
            if self.image_view != ImageView::null() {
                device.destroy_image_view(self.image_view, None);
            }
            if self.device_memory != DeviceMemory::null() {
                self.allocator
                    .upgrade()
                    .unwrap()
                    .read()
                    .unwrap()
                    .destroy_image(self.image, &self.allocation)
                    .expect("Failed to destroy image.");
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
    fn map_memory(&mut self, _device_size: u64, _offset: u64) -> *mut c_void {
        if self.mapped_memory.is_null() {
            self.mapped_memory =
                self.allocator
                    .upgrade()
                    .unwrap()
                    .read()
                    .unwrap()
                    .map_memory(&self.allocation)
                    .expect("Failed to map device memory.") as *mut c_void;
        }
        self.mapped_memory
    }

    fn unmap_memory(&mut self) {
        self.allocator
            .upgrade()
            .unwrap()
            .read()
            .unwrap()
            .unmap_memory(&self.allocation)
            .expect("Failed to unmap memory.");
    }
}
