use glam::{Mat4, Quat, Vec3A};

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub node_index: usize,
    pub index: usize,
    pub children: Vec<Joint>,
    pub inverse_bind_matrices: Mat4,
    pub translation: Vec3A,
    pub rotation: Quat,
    pub scale: Vec3A,
}