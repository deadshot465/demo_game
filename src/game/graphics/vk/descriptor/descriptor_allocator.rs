use ash::version::DeviceV1_0;
use ash::vk::{
    DescriptorPool, DescriptorPoolCreateFlags, DescriptorPoolCreateInfo, DescriptorPoolResetFlags,
    DescriptorPoolSize, DescriptorSet, DescriptorSetAllocateInfo, DescriptorSetLayout,
    DescriptorType,
};
use std::sync::Weak;

struct PoolSizes {
    pub sizes: Vec<(DescriptorType, f32)>,
}

pub struct DescriptorAllocator {
    pub logical_device: Weak<ash::Device>,
    descriptor_sizes: PoolSizes,
    used_pools: Vec<DescriptorPool>,
    free_pools: Vec<DescriptorPool>,
    current_pool: DescriptorPool,
}

impl DescriptorAllocator {
    pub fn new(device: Weak<ash::Device>) -> Self {
        DescriptorAllocator {
            descriptor_sizes: PoolSizes {
                sizes: vec![
                    (DescriptorType::SAMPLER, 0.5),
                    (DescriptorType::COMBINED_IMAGE_SAMPLER, 4.0),
                    (DescriptorType::SAMPLED_IMAGE, 4.0),
                    (DescriptorType::STORAGE_IMAGE, 1.0),
                    (DescriptorType::UNIFORM_TEXEL_BUFFER, 1.0),
                    (DescriptorType::STORAGE_TEXEL_BUFFER, 1.0),
                    (DescriptorType::UNIFORM_BUFFER, 2.0),
                    (DescriptorType::STORAGE_BUFFER, 2.0),
                    (DescriptorType::UNIFORM_BUFFER_DYNAMIC, 1.0),
                    (DescriptorType::STORAGE_BUFFER_DYNAMIC, 1.0),
                    (DescriptorType::INPUT_ATTACHMENT, 0.5),
                ],
            },
            used_pools: vec![],
            free_pools: vec![],
            current_pool: DescriptorPool::null(),
            logical_device: device,
        }
    }

    pub fn allocate(
        &mut self,
        descriptor_set: &mut DescriptorSet,
        layout: DescriptorSetLayout,
    ) -> bool {
        let device = self
            .logical_device
            .upgrade()
            .expect("Failed to upgrade device handle.");

        if self.current_pool == DescriptorPool::null() {
            let pool = self.grab_pool(&device);
            self.current_pool = pool;
            self.used_pools.push(self.current_pool);
        }

        let layouts = [layout];
        let allocate_info = DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.current_pool)
            .set_layouts(&layouts[..]);
        unsafe {
            let result = device.allocate_descriptor_sets(&allocate_info);
            let mut reallocate = false;
            match result {
                Ok(set) => {
                    *descriptor_set = set[0];
                    return true;
                }
                Err(e) => match e {
                    ash::vk::Result::ERROR_FRAGMENTED_POOL
                    | ash::vk::Result::ERROR_OUT_OF_POOL_MEMORY => reallocate = true,
                    _ => return false,
                },
            }

            if reallocate {
                let pool = self.grab_pool(&device);
                self.current_pool = pool;
                self.used_pools.push(self.current_pool);
                return match device.allocate_descriptor_sets(&allocate_info) {
                    Ok(set) => {
                        *descriptor_set = set[0];
                        true
                    }
                    Err(_) => false,
                };
            }
        }
        false
    }

    pub fn reset_pool(&mut self) {
        let device = self
            .logical_device
            .upgrade()
            .expect("Failed to upgrade device handle.");
        unsafe {
            for pool in self.used_pools.iter() {
                device
                    .reset_descriptor_pool(*pool, DescriptorPoolResetFlags::empty())
                    .expect("Failed to reset descriptor pool.");
            }
        }
        let mut used_tools = vec![];
        used_tools.append(&mut self.used_pools);
        self.free_pools = used_tools;
        self.current_pool = DescriptorPool::null();
    }

    fn grab_pool(&mut self, device: &ash::Device) -> DescriptorPool {
        if !self.free_pools.is_empty() {
            self.free_pools
                .pop()
                .expect("Failed to pop the last pool from descriptor allocator.")
        } else {
            Self::create_pool(
                device,
                &self.descriptor_sizes,
                1000,
                DescriptorPoolCreateFlags::empty(),
            )
        }
    }

    fn create_pool(
        device: &ash::Device,
        pool_sizes: &PoolSizes,
        count: u32,
        flags: DescriptorPoolCreateFlags,
    ) -> DescriptorPool {
        let mut sizes = Vec::with_capacity(pool_sizes.sizes.len());
        for (descriptor_type, size) in pool_sizes.sizes.iter() {
            sizes.push(
                DescriptorPoolSize::builder()
                    .descriptor_count(*size as u32 * count)
                    .ty(*descriptor_type)
                    .build(),
            )
        }
        let pool_info = DescriptorPoolCreateInfo::builder()
            .pool_sizes(&sizes)
            .flags(flags)
            .max_sets(count);

        unsafe {
            device
                .create_descriptor_pool(&pool_info, None)
                .expect("Failed to create descriptor pool.")
        }
    }
}

impl Drop for DescriptorAllocator {
    fn drop(&mut self) {
        let device = self
            .logical_device
            .upgrade()
            .expect("Failed to upgrade the device handle.");
        unsafe {
            for pool in self.free_pools.iter() {
                device.destroy_descriptor_pool(*pool, None);
            }
            for pool in self.used_pools.iter() {
                device.destroy_descriptor_pool(*pool, None);
            }
        }
    }
}
