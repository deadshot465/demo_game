use crate::game::graphics::vk::{DescriptorAllocator, DescriptorLayoutCache};
use ash::version::DeviceV1_0;
use ash::vk::{
    DescriptorBufferInfo, DescriptorImageInfo, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType, ShaderStageFlags,
    WriteDescriptorSet,
};

/// 簡単に描述子セットを設定できるための描述子ビルダー。<br />
/// An easy-to-use descriptor builder used to setup the descriptor set.
pub struct DescriptorBuilder<'a> {
    layout_cache: &'a mut DescriptorLayoutCache,
    allocator: &'a mut DescriptorAllocator,
    writes: Vec<WriteDescriptorSet>,
    bindings: Vec<DescriptorSetLayoutBinding>,
}

impl<'a> DescriptorBuilder<'a> {
    /// ビルダーパターンより描述子セットを作成する。<br />
    /// これはビルダーのエントリーポイントです。<br />
    /// Create/write a descriptor set based on builder pattern.<br />
    /// This is the entry point of the builder.
    pub fn builder(
        layout_cache: &'a mut DescriptorLayoutCache,
        allocator: &'a mut DescriptorAllocator,
    ) -> Self {
        DescriptorBuilder {
            layout_cache,
            allocator,
            writes: vec![],
            bindings: vec![],
        }
    }

    /// バッファをバインドする。<br />
    /// この関数は描述子セットにバッファに追加する。<br />
    /// Bind to a buffer.<br />
    /// This function will add a buffer to the descriptor set.
    pub fn bind_buffer(
        mut self,
        binding: u32,
        descriptor_count: Option<u32>,
        buffer_info: &'a [DescriptorBufferInfo],
        descriptor_type: DescriptorType,
        stage_flags: ShaderStageFlags,
    ) -> Self {
        let new_binding = DescriptorSetLayoutBinding::builder()
            .descriptor_count(descriptor_count.unwrap_or(1))
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .binding(binding)
            .build();
        self.bindings.push(new_binding);

        let new_write = WriteDescriptorSet::builder()
            .descriptor_type(descriptor_type)
            .buffer_info(buffer_info)
            .dst_array_element(0)
            .dst_binding(binding)
            .build();
        self.writes.push(new_write);
        self
    }

    /// イメージをバインドする。<br />
    /// この関数は描述子セットにイメージに追加する。<br />
    /// Bind to an image.<br />
    /// This function will add an image to the descriptor set.
    pub fn bind_image(
        mut self,
        binding: u32,
        descriptor_count: Option<u32>,
        image_info: &'a [DescriptorImageInfo],
        descriptor_type: DescriptorType,
        stage_flags: ShaderStageFlags,
    ) -> Self {
        let new_binding = DescriptorSetLayoutBinding::builder()
            .descriptor_count(descriptor_count.unwrap_or(1))
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .binding(binding)
            .build();
        self.bindings.push(new_binding);

        let new_write = WriteDescriptorSet::builder()
            .descriptor_type(descriptor_type)
            .image_info(image_info)
            .dst_array_element(0)
            .dst_binding(binding)
            .build();
        self.writes.push(new_write);
        self
    }

    /// ビルダーを終えて配置された描述子セットとそのセットのレイアウトを返す。<br />
    /// Ends the builder and returns allocated descriptor set and its descriptor set layout.
    pub fn build(mut self) -> Option<(DescriptorSet, DescriptorSetLayout)> {
        // Build layout first.
        let layout_info =
            DescriptorSetLayoutCreateInfo::builder().bindings(self.bindings.as_slice());
        let layout = self.layout_cache.create_descriptor_layout(&layout_info);

        // Allocate the descriptor set.
        let descriptor_set = self.allocator.allocate(layout);
        if let Some(set) = descriptor_set {
            // Write descriptor sets
            for write in self.writes.iter_mut() {
                write.dst_set = set;
            }
            unsafe {
                let device = self
                    .allocator
                    .logical_device
                    .upgrade()
                    .expect("Failed to upgrade device handle.");
                device.update_descriptor_sets(self.writes.as_slice(), &[]);
            }
            Some((set, layout))
        } else {
            None
        }
    }
}
