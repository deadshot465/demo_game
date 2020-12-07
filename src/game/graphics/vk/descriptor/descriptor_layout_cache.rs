use ash::version::DeviceV1_0;
use ash::vk::{DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Weak;

/// 描述子レイアウトに関する情報。<br />
/// この構造体は`HashMap`のキーとして使用されているので`PartialEq, Eq, Hash`を実装する。<br />
/// Information about a descriptor set layout.<br />
/// This struct is used as keys in a HashMap, so it implements `PartialEq, Eq` and `Hash`.
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
                if binding_self.binding != binding_other.binding
                    || binding_self.descriptor_type != binding_other.descriptor_type
                    || binding_self.descriptor_count != binding_other.descriptor_count
                    || binding_self.stage_flags != binding_other.stage_flags
                {
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

/// 描述子レイアウトのキャッシュ。<br />
/// レイアウトは既存であればそのままキャッシュを使う。<br />
/// 既存ではないなら新しいレイアウトを作る。<br />
/// Caches for descriptor layout.<br />
/// If there are existing descriptor layouts, use caches.<br />
/// If not, create new layouts.
pub struct DescriptorLayoutCache {
    logical_device: Weak<ash::Device>,
    layout_cache: HashMap<DescriptorLayoutInfo, DescriptorSetLayout>,
}

impl DescriptorLayoutCache {
    /// コンストラクター。<br />
    /// Constructor.
    pub fn new(device: Weak<ash::Device>) -> Self {
        DescriptorLayoutCache {
            logical_device: device,
            layout_cache: HashMap::new(),
        }
    }

    /// 描述子レイアウトを作成する。<br />
    /// Create a descriptor layout.
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
        // Copy from the direct info struct into our own one.
        for i in 0..info.binding_count {
            layout_info.bindings.push(bindings[i as usize]);

            // Check that the bindings are in strict increasing order.
            if bindings[i as usize].binding > last_binding as u32 {
                last_binding = bindings[i as usize].binding as i32;
            } else {
                sorted = false;
            }
        }

        // Sort the bindings if they are not in order.
        if !sorted {
            layout_info
                .bindings
                .sort_unstable_by(|a, b| a.binding.cmp(&b.binding));
        }

        // Try to grab from cache.
        if let Some(layout) = self.layout_cache.get(&layout_info) {
            *layout
        } else {
            // Create a new one (not found)
            let device = self
                .logical_device
                .upgrade()
                .expect("Failed to upgrade device handle.");
            let layout = unsafe {
                device
                    .create_descriptor_set_layout(info, None)
                    .expect("Failed to create descriptor set layout.")
            };
            // Add to cache.
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
        self.layout_cache.clear();
    }
}
