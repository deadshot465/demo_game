use glam::Vec4;
use bytemuck::{Pod, Zeroable};

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct PushConstant {
    pub texture_index: usize,
    pub object_color: Vec4,
}

impl PushConstant {
    pub fn null() -> Self {
        PushConstant {
            texture_index: 0,
            object_color: Vec4::new(0.0, 0.0, 0.0, 0.0)
        }
    }

    pub fn new(texture_index: usize, object_color: Vec4) -> Self {
        PushConstant {
            texture_index,
            object_color
        }
    }
}

unsafe impl Zeroable for PushConstant {}
unsafe impl Pod for PushConstant {}