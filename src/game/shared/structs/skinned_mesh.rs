use crate::game::traits::Disposable;
use crate::game::shared::structs::{Joint, SkinnedVertex};
use glam::{Mat4};
use std::mem::ManuallyDrop;

pub struct SkinnedPrimitive<BufferType: 'static + Disposable, TextureType: 'static + Clone + Disposable> {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Option<ManuallyDrop<TextureType>>,
    pub is_disposed: bool,
}

pub struct SkinnedMesh<BufferType: 'static + Disposable, TextureType: 'static + Clone + Disposable> {
    pub primitives: Vec<SkinnedPrimitive<BufferType, TextureType>>,
    pub is_disposed: bool,
    pub transform: Mat4,
    pub root_joint: Option<Joint>
}

