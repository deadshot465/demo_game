use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk::*;

#[derive(Copy, Clone, Debug)]
pub struct Texture {
    pub image: Image,
    pub image_view: ImageView,
    pub device_memory: DeviceMemory,
}

impl Texture {
    pub fn new(
        width: u32,
        height: u32,
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        memory_properties: MemoryPropertyFlags,
    ) -> Self {
        let image_info = ImageCreateInfo::builder()
            .format(Format::R8G8B8A8_UNORM)
            .samples(SampleCountFlags::TYPE_1)
            .initial_layout(ImageLayout::UNDEFINED)
            .mip_levels(1)
            .extent(
                Extent3D::builder()
                    .height(height)
                    .width(width)
                    .depth(1)
                    .build(),
            )
            .array_layers(1)
            .image_type(ImageType::TYPE_2D)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .tiling(ImageTiling::OPTIMAL)
            .usage(ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST);

        unsafe {
            let image = device
                .create_image(&image_info, None)
                .expect("Failed to create image for Nuklear.");
            let memory_requirements = device.get_image_memory_requirements(image);
            let allocation_info = MemoryAllocateInfo::builder()
                .allocation_size(memory_requirements.size)
                .memory_type_index(Self::find_memory_type_index(
                    instance,
                    physical_device,
                    memory_requirements.memory_type_bits,
                    memory_properties,
                ));
            let device_memory = device
                .allocate_memory(&allocation_info, None)
                .expect("Failed to allocate memory for the image for Nuklear.");
            device
                .bind_image_memory(image, device_memory, 0)
                .expect("Failed to bind image memory for Nuklear.");

            Texture {
                image,
                image_view: ImageView::null(),
                device_memory,
            }
        }
    }

    pub fn copy_buffer_to_image(
        &self,
        device: &ash::Device,
        buffer: Buffer,
        command_pool: CommandPool,
        graphics_queue: Queue,
        width: u32,
        height: u32,
    ) {
        let command_allocate_info = CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .level(CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd_buffer = unsafe {
            let cmd_buffers = device
                .allocate_command_buffers(&command_allocate_info)
                .expect("Failed to allocate command buffer.");
            cmd_buffers[0]
        };

        let fence_info = FenceCreateInfo::builder();
        let fence = unsafe {
            device
                .create_fence(&fence_info, None)
                .expect("Failed to create fence.")
        };

        let begin_info =
            CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            device
                .begin_command_buffer(cmd_buffer, &begin_info)
                .expect("Failed to begin command buffer.");
        }

        let copy_region = vec![BufferImageCopy::builder()
            .image_subresource(
                ImageSubresourceLayers::builder()
                    .mip_level(0)
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .base_array_layer(0)
                    .build(),
            )
            .image_extent(
                Extent3D::builder()
                    .width(width)
                    .height(height)
                    .depth(1)
                    .build(),
            )
            .build()];

        let mut barrier = ImageMemoryBarrier::builder()
            .image(self.image)
            .dst_access_mask(AccessFlags::TRANSFER_WRITE)
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
            .old_layout(ImageLayout::UNDEFINED)
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .subresource_range(
                ImageSubresourceRange::builder()
                    .base_array_layer(0)
                    .layer_count(1)
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .build(),
            )
            .build();

        unsafe {
            device.cmd_pipeline_barrier(
                cmd_buffer,
                PipelineStageFlags::TOP_OF_PIPE,
                PipelineStageFlags::TRANSFER,
                DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            device.cmd_copy_buffer_to_image(
                cmd_buffer,
                buffer,
                self.image,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                copy_region.as_slice(),
            );
            barrier.old_layout = ImageLayout::TRANSFER_DST_OPTIMAL;
            barrier.new_layout = ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            barrier.src_access_mask = AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = AccessFlags::SHADER_READ;
            device.cmd_pipeline_barrier(
                cmd_buffer,
                PipelineStageFlags::TRANSFER,
                PipelineStageFlags::FRAGMENT_SHADER,
                DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );

            device
                .end_command_buffer(cmd_buffer)
                .expect("Failed to end command buffer.");
            let cmd_buffers = [cmd_buffer];
            let submit_info = vec![SubmitInfo::builder()
                .command_buffers(&cmd_buffers[0..])
                .build()];
            device
                .queue_submit(graphics_queue, submit_info.as_slice(), fence)
                .expect("Failed to submit queue for execution.");
            let fences = vec![fence];
            device
                .wait_for_fences(fences.as_slice(), true, u64::MAX)
                .expect("Failed to wait for fence.");
            device.free_command_buffers(command_pool, &cmd_buffers[0..]);
            device.destroy_fence(fence, None);
        }
    }

    pub fn create_image_view(&mut self, device: &ash::Device) {
        let image_view_info = ImageViewCreateInfo::builder()
            .image(self.image)
            .subresource_range(
                ImageSubresourceRange::builder()
                    .level_count(1)
                    .base_mip_level(0)
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .base_array_layer(0)
                    .build(),
            )
            .format(Format::R8G8B8A8_UNORM)
            .components(
                ComponentMapping::builder()
                    .r(ComponentSwizzle::R)
                    .g(ComponentSwizzle::G)
                    .b(ComponentSwizzle::B)
                    .a(ComponentSwizzle::A)
                    .build(),
            )
            .view_type(ImageViewType::TYPE_2D);

        unsafe {
            self.image_view = device
                .create_image_view(&image_view_info, None)
                .expect("Failed to create image view.");
        }
    }

    fn find_memory_type_index(
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        memory_type: u32,
        memory_properties: MemoryPropertyFlags,
    ) -> u32 {
        unsafe {
            let properties = instance.get_physical_device_memory_properties(physical_device);
            for i in 0..properties.memory_type_count {
                if ((memory_type & (1 << i)) != 0)
                    && ((properties.memory_types[i as usize].property_flags & memory_properties)
                        == memory_properties)
                {
                    return i;
                }
            }
        }
        0
    }
}
