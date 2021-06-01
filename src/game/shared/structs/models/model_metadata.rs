use glam::{Mat4, Vec4};

/// モデルのメタデータ。SSBOに保存されます。<br />
/// Metadata of models, stored in the primary SSBO.
#[derive(Copy, Clone, Debug)]
pub struct ModelMetaData {
    pub world_matrix: Mat4,
    pub object_color: Vec4,
    pub reflectivity: f32,
    pub shine_damper: f32,
}

impl ModelMetaData {
    pub fn new(
        world_matrix: Mat4,
        object_color: Vec4,
        reflectivity: f32,
        shine_damper: f32,
    ) -> Self {
        ModelMetaData {
            world_matrix,
            object_color,
            reflectivity,
            shine_damper,
        }
    }

    pub fn identity() -> Self {
        ModelMetaData {
            world_matrix: Mat4::IDENTITY,
            object_color: Vec4::ONE,
            reflectivity: 1.0,
            shine_damper: 1.0,
        }
    }
}
