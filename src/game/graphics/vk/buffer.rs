use ash::version::DeviceV1_0;
use ash::{
    vk::{
        BufferCopy, BufferCreateInfo, BufferUsageFlags, CommandBuffer, CommandPool, DeviceMemory,
        DeviceSize, MemoryPropertyFlags, Queue, SharingMode,
    },
    Device,
};
use crossbeam::sync::ShardedLock;
use std::ffi::c_void;
use std::sync::Weak;
use vk_mem::*;

use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::traits::mappable::Mappable;
use crate::game::util::{end_one_time_command_buffer, get_single_time_command_buffer};

/// Vulkanのバッファをラップする。<br />
/// Wraps Vulkan buffer.
#[derive(Clone)]
pub struct Buffer {
    /// 生のVulkanバッファ。<br />
    /// Raw Vulkan buffer.
    pub buffer: ash::vk::Buffer,

    /// デバイスメモリー。<br />
    /// Device memory.
    pub device_memory: DeviceMemory,

    /// 常にマップしているメモリー。<br />
    /// `std::ptr::copy_nonoverlapping`に使います。<br />
    /// Consistently mapped memory.<br />
    /// Used in `std::ptr::copy_nonoverlapping`
    pub mapped_memory: *mut c_void,

    /// このバッファは既に解放したのかどうか。<br />
    /// Is this buffer already released?
    pub is_disposed: bool,

    /// バッファのサイズ。<br />
    /// Buffer size.
    pub buffer_size: DeviceSize,

    /// ロジカルデバイスのハンドル。<br />
    /// Handle to the logical device.
    logical_device: Weak<Device>,

    /// VMAメモリー配置器のWeakポインタ。<br />
    /// VMA memory allocator's weak pointer.
    allocator: Weak<ShardedLock<Allocator>>,

    /// メモリー配置。<br />
    /// Memory allocation.
    allocation: Allocation,

    /// メモリー配置の情報。<br />
    /// Memory allocation's info.
    allocation_info: Option<AllocationInfo>,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

impl Buffer {
    ///　コンストラクター。<br />
    /// Constructor.
    pub fn new(
        device: Weak<Device>,
        buffer_size: DeviceSize,
        usage_flag: BufferUsageFlags,
        memory_properties: MemoryPropertyFlags,
        allocator: Weak<ShardedLock<Allocator>>,
    ) -> Self {
        let create_info = BufferCreateInfo::builder()
            .sharing_mode(SharingMode::EXCLUSIVE)
            .size(buffer_size)
            .usage(usage_flag)
            .build();
        let allocation_info = AllocationCreateInfo {
            usage: match usage_flag {
                BufferUsageFlags::TRANSFER_SRC => MemoryUsage::CpuOnly,
                x if (x & BufferUsageFlags::VERTEX_BUFFER) != BufferUsageFlags::empty()
                    || (x & BufferUsageFlags::INDEX_BUFFER) != BufferUsageFlags::empty() =>
                {
                    MemoryUsage::GpuOnly
                }
                _ => MemoryUsage::CpuToGpu,
            },
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
        let (buffer, allocation, allocation_info) = lock
            .create_buffer(&create_info, &allocation_info)
            .expect("Failed to create buffer from VMA allocator.");
        drop(lock);
        let device_memory = allocation_info.get_device_memory();
        let mapped = allocation_info.get_mapped_data();
        Buffer {
            logical_device: device,
            buffer,
            device_memory,
            mapped_memory: mapped as *mut c_void,
            is_disposed: false,
            buffer_size,
            allocation,
            allocation_info: Some(allocation_info),
            allocator,
        }
    }

    /// バッファ元からこのバッファにコピーする。<br />
    /// Copy buffer from another buffer.
    pub fn copy_buffer(
        &self,
        src_buffer: &Buffer,
        buffer_size: DeviceSize,
        command_pool: CommandPool,
        graphics_queue: Queue,
        command_buffer: Option<CommandBuffer>,
    ) {
        unsafe {
            let device = self.logical_device.upgrade();
            if let Some(d) = device {
                let copy_info = BufferCopy::builder()
                    .src_offset(0)
                    .size(buffer_size)
                    .dst_offset(0);
                let cmd_buffer = if let Some(buffer) = command_buffer {
                    buffer
                } else {
                    get_single_time_command_buffer(d.as_ref(), command_pool)
                };
                d.cmd_copy_buffer(
                    cmd_buffer,
                    src_buffer.buffer,
                    self.buffer,
                    &[copy_info.build()],
                );
                if command_buffer.is_none() {
                    end_one_time_command_buffer(
                        cmd_buffer,
                        d.as_ref(),
                        command_pool,
                        graphics_queue,
                    );
                }
            }
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl Disposable for Buffer {
    fn dispose(&mut self) {
        if self.is_disposed {
            return;
        }
        if !self.mapped_memory.is_null() {
            self.unmap_memory();
        }
        if self.device_memory != DeviceMemory::null() {
            self.allocator
                .upgrade()
                .unwrap()
                .read()
                .unwrap()
                .destroy_buffer(self.buffer, &self.allocation)
                .expect("Failed to destroy buffer.");
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

impl Mappable for Buffer {
    fn map_memory(&mut self, _device_size: u64, _offset: u64) -> *mut c_void {
        if self.mapped_memory.is_null()
            && self
                .allocation_info
                .as_ref()
                .unwrap()
                .get_mapped_data()
                .is_null()
        {
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
        if self
            .allocation_info
            .as_ref()
            .unwrap()
            .get_mapped_data()
            .is_null()
            && !self.mapped_memory.is_null()
        {
            self.allocator
                .upgrade()
                .unwrap()
                .read()
                .unwrap()
                .unmap_memory(&self.allocation)
                .expect("Failed to unmap memory.");
        }
    }
}
