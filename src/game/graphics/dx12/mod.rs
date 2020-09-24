#[cfg(target_os = "windows")]
pub mod command_queue;
#[cfg(target_os = "windows")]
pub mod descriptor_heap;
#[cfg(target_os = "windows")]
pub mod graphics;
#[cfg(target_os = "windows")]
pub mod pipeline;
#[cfg(target_os = "windows")]
pub mod resource;
#[cfg(target_os = "windows")]
pub mod swapchain;
#[cfg(target_os = "windows")]
pub use command_queue::CommandQueue;
#[cfg(target_os = "windows")]
pub use descriptor_heap::DescriptorHeap;
#[cfg(target_os = "windows")]
pub use graphics::Graphics;
#[cfg(target_os = "windows")]
pub use pipeline::Pipeline;
#[cfg(target_os = "windows")]
pub use resource::*;
#[cfg(target_os = "windows")]
pub use swapchain::SwapChain;