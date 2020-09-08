use ash::{
    vk::{
        Format,
        VertexInputAttributeDescription,
        VertexInputBindingDescription,
        VertexInputRate,
    }
};
use glam::{Vec2, Vec3A};
use std::convert::TryFrom;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    position: Vec3A,
    normal: Vec3A,
    tex_coord: Vec2
}

impl Vertex {
    pub fn new(position: Vec3A, normal: Vec3A, tex_coord: Vec2) -> Self {
        Vertex {
            position,
            normal,
            tex_coord
        }
    }

    pub fn get_binding_description(binding: u32, input_rate: VertexInputRate) -> VertexInputBindingDescription {
        let desc = VertexInputBindingDescription::builder()
            .binding(binding)
            .input_rate(input_rate)
            .stride(u32::try_from(std::mem::size_of::<Vertex>()).unwrap())
            .build();
        desc
    }

    pub fn get_attribute_description(binding: u32) -> Vec<VertexInputAttributeDescription> {
        let mut descs = vec![];
        descs.push(VertexInputAttributeDescription::builder()
            .binding(binding)
            .offset(0)
            .format(Format::R32G32B32_SFLOAT)
            .location(0)
            .build());
        descs.push(VertexInputAttributeDescription::builder()
            .binding(binding)
            .offset(0)
            .format(Format::R32G32B32_SFLOAT)
            .location(1)
            .build());
        descs.push(VertexInputAttributeDescription::builder()
            .binding(binding)
            .offset(0)
            .format(Format::R32G32_SFLOAT)
            .location(2)
            .build());
        descs
    }
}