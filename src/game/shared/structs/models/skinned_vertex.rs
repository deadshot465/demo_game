use crate::game::structs::Vertex;
use ash::vk::{
    Format, VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
};
use glam::{Vec2, Vec3A, Vec4};
use std::convert::TryFrom;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SkinnedVertex {
    pub vertex: Vertex,
    pub joints: Vec4,
    pub weights: Vec4,
}

impl SkinnedVertex {
    pub fn new(position: Vec3A, normal: Vec3A, uv: Vec2, joints: Vec4, weights: Vec4) -> Self {
        SkinnedVertex {
            vertex: Vertex {
                position,
                normal,
                uv,
            },
            joints,
            weights,
        }
    }

    pub fn get_binding_description(
        binding: u32,
        input_rate: VertexInputRate,
    ) -> VertexInputBindingDescription {
        let desc = VertexInputBindingDescription::builder()
            .binding(binding)
            .input_rate(input_rate)
            .stride(u32::try_from(std::mem::size_of::<SkinnedVertex>()).unwrap())
            .build();
        desc
    }

    pub fn get_attribute_description(binding: u32) -> Vec<VertexInputAttributeDescription> {
        let mut descs = Vertex::get_attribute_description(binding);
        descs.push(
            VertexInputAttributeDescription::builder()
                .binding(binding)
                .offset(u32::try_from(memoffset::offset_of!(SkinnedVertex, joints)).unwrap())
                .format(Format::R32G32B32A32_SFLOAT)
                .location(3)
                .build(),
        );
        descs.push(
            VertexInputAttributeDescription::builder()
                .binding(binding)
                .offset(u32::try_from(memoffset::offset_of!(SkinnedVertex, weights)).unwrap())
                .format(Format::R32G32B32A32_SFLOAT)
                .location(4)
                .build(),
        );
        descs
    }
}
