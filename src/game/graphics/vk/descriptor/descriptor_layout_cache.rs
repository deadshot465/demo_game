use ash::version::DeviceV1_0;
use ash::vk::{DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Weak;

struct DescriptorLayoutInfo {
    pub bindings: Vec<DescriptorSetLayoutBinding>,
}

impl PartialEq for DescriptorLayoutInfo {
    fn eq(&self, other: &Self) -> bool {
        if other.bindings.len() != self.bindings.len() {
            false
        } else {
            // Compare each of the bindings is the same. Bindings are sorted so they will match.
            let iterator = self.bindings.iter().zip(other.bindings.iter());
            for (binding_self, binding_other) in iterator {
                if binding_self.binding != binding_other.binding {
                    return false;
                } else if binding_self.descriptor_type != binding_other.descriptor_type {
                    return false;
                } else if binding_self.descriptor_count != binding_other.descriptor_count {
                    return false;
                } else if binding_self.stage_flags != binding_other.stage_flags {
                    return false;
                }
            }
            true
        }
    }
}

impl Eq for DescriptorLayoutInfo {}

impl Hash for DescriptorLayoutInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for binding in self.bindings.iter() {
            binding.stage_flags.hash(state);
            binding.binding.hash(state);
            binding.descriptor_count.hash(state);
            binding.descriptor_type.hash(state);
            binding.p_immutable_samplers.hash(state);
        }
    }
}

pub struct DescriptorLayoutCache {
    logical_device: Weak<ash::Device>,
    layout_cache: HashMap<DescriptorLayoutInfo, DescriptorSetLayout>,
}

impl DescriptorLayoutCache {
    pub fn new(device: Weak<ash::Device>) -> Self {
        DescriptorLayoutCache {
            logical_device: device,
            layout_cache: HashMap::new(),
        }
    }

    pub fn create_descriptor_layout(
        &mut self,
        info: &DescriptorSetLayoutCreateInfo,
    ) -> DescriptorSetLayout {
        let mut layout_info = DescriptorLayoutInfo {
            bindings: Vec::with_capacity(info.binding_count as usize),
        };
        let mut sorted = true;
        let mut last_binding = -1_i32;
        let bindings =
            unsafe { std::slice::from_raw_parts(info.p_bindings, info.binding_count as usize) };
        for i in 0..info.binding_count {
            layout_info.bindings.push(bindings[i as usize]);
            if bindings[i as usize].binding > last_binding as u32 {
                last_binding = bindings[i as usize].binding as i32;
            } else {
                sorted = false;
            }
        }

        if !sorted {
            layout_info
                .bindings
                .sort_unstable_by(|a, b| a.binding.cmp(&b.binding));
        }

        if let Some(layout) = self.layout_cache.get(&layout_info) {
            *layout
        } else {
            let device = self
                .logical_device
                .upgrade()
                .expect("Failed to upgrade device handle.");
            let layout = unsafe {
                device
                    .create_descriptor_set_layout(info, None)
                    .expect("Failed to create descriptor set layout.")
            };
            self.layout_cache.insert(layout_info, layout);
            layout
        }
    }
}

impl Drop for DescriptorLayoutCache {
    fn drop(&mut self) {
        let device = self
            .logical_device
            .upgrade()
            .expect("Failed to upgrade device handle.");
        unsafe {
            for (_, layout) in self.layout_cache.iter() {
                device.destroy_descriptor_set_layout(*layout, None);
            }
        }
    }
}
