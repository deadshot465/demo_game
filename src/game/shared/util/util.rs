use ash::Device;
use ash::vk::{CommandPool, CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, CommandBufferBeginInfo, CommandBufferUsageFlags, Queue, SubmitInfo, Fence};
use ash::version::DeviceV1_0;
use rand::prelude::*;

const ALPHANUMERICS: &'static str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

pub fn get_random_string(length: usize) -> String {
    if length > ALPHANUMERICS.len() {
        String::new()
    }
    else {
        let mut rng = thread_rng();
        let sample = ALPHANUMERICS.chars().choose_multiple(&mut rng, length);
        let result: String = sample.into_iter().collect();
        result
    }
}

pub fn get_single_time_command_buffer(device: &Device, command_pool: CommandPool) -> CommandBuffer {
    let allocate_info = CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .command_buffer_count(1)
        .level(CommandBufferLevel::PRIMARY)
        .build();
    unsafe {
        let command_buffer = device.allocate_command_buffers(&allocate_info)
            .expect("Failed to allocate command buffer.");
        let buffer = command_buffer[0];
        let begin_info = CommandBufferBeginInfo::builder()
            .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        device.begin_command_buffer(buffer, &begin_info)
            .expect("Failed to begin command buffer.");
        buffer
    }
}

pub fn end_one_time_command_buffer(cmd_buffer: CommandBuffer, device: &Device,
                                   command_pool: CommandPool, graphics_queue: Queue) {
    unsafe {
        device.end_command_buffer(cmd_buffer)
            .expect("Failed to end command buffer.");
        let command_buffers = [cmd_buffer];
        let submit_info = SubmitInfo::builder()
            .command_buffers(&command_buffers)
            .build();
        let submit_infos = [submit_info];
        device.queue_submit(graphics_queue, &submit_infos, Fence::null())
            .expect("Failed to submit the queue.");
        device.queue_wait_idle(graphics_queue).expect("Failed to wait for queue.");
        device.free_command_buffers(command_pool, &command_buffers);
    }
}