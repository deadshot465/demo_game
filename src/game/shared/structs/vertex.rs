use ash::{
    vk::{
        Format,
        VertexInputAttributeDescription,
        VertexInputBindingDescription,
        VertexInputRate,
    }
};
use glam::{Vec2, Vec3A, Vec4};
use std::convert::TryFrom;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: Vec3A,
    pub normal: Vec3A,
    pub tex_coord: Vec2,
    pub uv: Option<Vec2>,
    pub joints: Option<Vec4>,
    pub weights: Option<Vec4>,
}

impl Vertex {
    pub fn new(position: Vec3A, normal: Vec3A, tex_coord: Vec2) -> Self {
        Vertex {
            position,
            normal,
            tex_coord,
            uv: None,
            joints: None,
            weights: None,
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
            .offset(u32::try_from(memoffset::offset_of!(Vertex, normal)).unwrap())
            .format(Format::R32G32B32_SFLOAT)
            .location(1)
            .build());
        descs.push(VertexInputAttributeDescription::builder()
            .binding(binding)
            .offset(u32::try_from(memoffset::offset_of!(Vertex, tex_coord)).unwrap())
            .format(Format::R32G32_SFLOAT)
            .location(2)
            .build());
        descs
    }
}