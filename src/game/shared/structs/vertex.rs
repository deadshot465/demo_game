use ash::vk::{
    Format, VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
};
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3A};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[repr(C)]
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub position: Vec3A,
    pub normal: Vec3A,
    pub uv: Vec2,
}

impl Default for Vertex {
    fn default() -> Self {
        Vertex {
            position: Vec3A::zero(),
            normal: Vec3A::zero(),
            uv: Vec2::zero(),
        }
    }
}

impl Vertex {
    pub fn new(position: Vec3A, normal: Vec3A, uv: Vec2) -> Self {
        Vertex {
            position,
            normal,
            uv,
        }
    }

    pub fn get_binding_description(
        binding: u32,
        stride_size: u32,
        input_rate: VertexInputRate,
    ) -> VertexInputBindingDescription {
        let desc = VertexInputBindingDescription::builder()
            .binding(binding)
            .input_rate(input_rate)
            .stride(stride_size)
            .build();
        desc
    }

    pub fn get_attribute_description(binding: u32) -> Vec<VertexInputAttributeDescription> {
        let mut descs = vec![];
        descs.push(
            VertexInputAttributeDescription::builder()
                .binding(binding)
                .offset(0)
                .format(Format::R32G32B32_SFLOAT)
                .location(0)
                .build(),
        );
        descs.push(
            VertexInputAttributeDescription::builder()
                .binding(binding)
                .offset(u32::try_from(memoffset::offset_of!(Vertex, normal)).unwrap())
                .format(Format::R32G32B32_SFLOAT)
                .location(1)
                .build(),
        );
        descs.push(
            VertexInputAttributeDescription::builder()
                .binding(binding)
                .offset(u32::try_from(memoffset::offset_of!(Vertex, uv)).unwrap())
                .format(Format::R32G32_SFLOAT)
                .location(2)
                .build(),
        );
        descs
    }
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}
