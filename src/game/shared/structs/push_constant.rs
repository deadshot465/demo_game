use bytemuck::{Pod, Zeroable};
use glam::Vec4;

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct PushConstant {
    pub texture_index: usize,
    pub object_color: Vec4,
    pub model_index: usize,
}

impl PushConstant {
    pub fn null() -> Self {
        PushConstant {
            texture_index: 0,
            object_color: Vec4::new(0.0, 0.0, 0.0, 0.0),
            model_index: 0,
        }
    }

    pub fn new(texture_index: usize, object_color: Vec4, model_index: usize) -> Self {
        PushConstant {
            texture_index,
            object_color,
            model_index
        }
    }
}

unsafe impl Zeroable for PushConstant {}
unsafe impl Pod for PushConstant {}