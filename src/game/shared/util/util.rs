use ash::Device;
use ash::version::DeviceV1_0;
use ash::vk::{CommandPool, CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, CommandBufferBeginInfo, CommandBufferUsageFlags, Queue, SubmitInfo, Fence};
use rand::prelude::*;
use winapi::ctypes::c_void;
use winapi::shared::winerror::{HRESULT, FAILED};

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

pub fn handle_rgb_bgr(color_type: image::ColorType, data: Vec<u8>, data_size: usize, width: u32, height: u32) -> (Vec<u8>, image::ColorType) {
    let pixels = data;
    match color_type {
        image::ColorType::Bgr8 | image::ColorType::Rgb8 => {
            let mut rgba_pixels: Vec<u8> = vec![];
            let mut rgba_index = 0;
            let mut rgb_index = 0;
            rgba_pixels.resize(data_size, 0);
            for _ in 0..(width * height) {
                rgba_pixels[rgba_index] = pixels[rgb_index];
                rgba_pixels[rgba_index + 1] = pixels[rgb_index + 1];
                rgba_pixels[rgba_index + 2] = pixels[rgb_index + 2];
                rgba_pixels[rgba_index + 3] = 255;
                rgba_index += 4;
                rgb_index += 3;
            }
            let new_color_type = if color_type == image::ColorType::Bgr8 {
                image::ColorType::Bgra8
            } else {
                image::ColorType::Rgba8
            };
            (rgba_pixels, new_color_type)
        },
        image::ColorType::Rgba8 | image::ColorType::Bgra8 => {
            (pixels, color_type)
        }
        _ => {
            panic!("Unsupported color type: {:?}", color_type);
        }
    }
}

pub fn get_nullptr() -> *mut c_void {
    std::ptr::null_mut() as *mut c_void
}

pub fn log_error(result: HRESULT, msg: &str) {
    if FAILED(result) {
        log::error!("{} Error: {}.", msg, result);
        panic!("{} Error: {}.", msg, result);
    }
}