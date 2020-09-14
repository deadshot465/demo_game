use crate::game::structs::Vertex;
use glam::{Vec2, Vec3A, Vec4};

#[derive(Copy, Clone, Debug)]
pub struct SkinnedVertex {
    pub vertex: Vertex,
    pub joints: Option<Vec4>,
    pub weights: Option<Vec4>,
}

impl SkinnedVertex {
    pub fn new(position: Vec3A, normal: Vec3A, uv: Vec2, joints: Option<Vec4>, weights: Option<Vec4>) -> Self {
        SkinnedVertex {
            vertex: Vertex {
                position,
                normal,
                uv,
            },
            joints,
            weights
        }
    }
}