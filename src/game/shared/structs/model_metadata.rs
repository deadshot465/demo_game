use glam::{Mat4, Vec4};

#[derive(Copy, Clone, Debug)]
pub struct ModelMetaData {
    pub world_matrix: Mat4,
    pub object_color: Vec4,
}

impl ModelMetaData {
    pub fn new(world_matrix: Mat4, object_color: Vec4) -> Self {
        ModelMetaData {
            world_matrix,
            object_color,
        }
    }

    pub fn identity() -> Self {
        ModelMetaData {
            world_matrix: Mat4::identity(),
            object_color: Vec4::one(),
        }
    }
}