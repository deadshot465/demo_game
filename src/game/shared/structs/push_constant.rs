use bytemuck::{Pod, Zeroable};
use glam::Vec4;

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct PushConstant {
    pub texture_index: usize,
    pub object_color: Vec4,
    pub model_index: usize,
    pub sky_color: Vec4,
}

impl PushConstant {
    pub fn null() -> Self {
        PushConstant {
            texture_index: 0,
            object_color: Vec4::zero(),
            model_index: 0,
            sky_color: Vec4::zero(),
        }
    }

    pub fn new(texture_index: usize, object_color: Vec4, model_index: usize, sky_color: Vec4) -> Self {
        PushConstant {
            texture_index,
            object_color,
            model_index,
            sky_color,
        }
    }
}

unsafe impl Zeroable for PushConstant {}
unsafe impl Pod for PushConstant {}
