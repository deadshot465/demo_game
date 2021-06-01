use bytemuck::{Pod, Zeroable};
use glam::Vec4;

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct PushConstant {
    pub texture_index: usize,
    pub model_index: usize,
    pub sky_color: Vec4,
}

impl PushConstant {
    pub fn null() -> Self {
        PushConstant {
            texture_index: 0,
            model_index: 0,
            sky_color: Vec4::ZERO,
        }
    }

    pub fn new(texture_index: usize, model_index: usize, sky_color: Vec4) -> Self {
        PushConstant {
            texture_index,
            model_index,
            sky_color,
        }
    }
}

unsafe impl Zeroable for PushConstant {}
unsafe impl Pod for PushConstant {}
