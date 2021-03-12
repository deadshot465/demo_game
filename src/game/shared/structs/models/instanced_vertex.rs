use crate::game::shared::structs::Vertex;
use ash::vk::{Format, VertexInputAttributeDescription};
use glam::Vec3A;
use std::convert::TryFrom;

/// ワールド行列を作るためのインスタンスデータ<br />
/// Instance data for creating world matrices.
#[derive(Copy, Clone, Debug)]
pub struct InstanceData {
    pub translation: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
}

impl Default for InstanceData {
    fn default() -> Self {
        InstanceData {
            translation: Vec3A::default(),
            scale: Vec3A::default(),
            rotation: Vec3A::default(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct InstancedVertex {
    pub vertex: Vertex,
    pub instance_data: InstanceData,
}

impl InstancedVertex {
    pub fn get_attribute_description(binding: u32) -> Vec<VertexInputAttributeDescription> {
        let mut descs = Vertex::get_attribute_description(binding);
        let instance_binding = binding + 1;
        descs.push(VertexInputAttributeDescription {
            location: 3,
            binding: instance_binding,
            format: Format::R32G32B32_SFLOAT,
            offset: 0,
        });
        descs.push(VertexInputAttributeDescription {
            location: 4,
            binding: instance_binding,
            format: Format::R32G32B32_SFLOAT,
            offset: u32::try_from(memoffset::offset_of!(InstanceData, scale)).unwrap(),
        });
        descs.push(VertexInputAttributeDescription {
            location: 5,
            binding: instance_binding,
            format: Format::R32G32B32_SFLOAT,
            offset: u32::try_from(memoffset::offset_of!(InstanceData, rotation)).unwrap(),
        });
        descs
    }
}
