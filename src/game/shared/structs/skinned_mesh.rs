use crossbeam::sync::ShardedLock;
use glam::Mat4;
use parking_lot::Mutex;
use std::mem::ManuallyDrop;
use std::sync::Arc;

use crate::game::graphics::vk::{Buffer, Image};
use crate::game::shared::enums::{SamplerResource, ShaderType};
use crate::game::shared::structs::{Joint, SkinnedVertex, SSBO};
use crate::game::traits::Disposable;
use ash::vk::CommandBuffer;

#[derive(Clone, Debug)]
pub struct SkinnedPrimitive<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Option<Arc<ShardedLock<TextureType>>>,
    pub texture_index: usize,
    pub is_disposed: bool,
    pub command_pool: Option<Arc<Mutex<ash::vk::CommandPool>>>,
    pub command_buffer: Option<CommandType>,
    pub sampler_resource: Option<SamplerResource>,
    pub shader_type: ShaderType,
}

unsafe impl<BufferType, CommandType, TextureType> Send
    for SkinnedPrimitive<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
}
unsafe impl<BufferType, CommandType, TextureType> Sync
    for SkinnedPrimitive<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
}

impl SkinnedPrimitive<Buffer, CommandBuffer, Image> {
    pub fn get_vertex_buffer(&self) -> ash::vk::Buffer {
        self.vertex_buffer.as_ref().unwrap().buffer
    }

    pub fn get_index_buffer(&self) -> ash::vk::Buffer {
        self.index_buffer.as_ref().unwrap().buffer
    }
}

impl<BufferType, CommandType, TextureType> Drop
    for SkinnedPrimitive<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<BufferType, CommandType, TextureType> Disposable
    for SkinnedPrimitive<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    fn dispose(&mut self) {
        unsafe {
            if let Some(buffer) = self.vertex_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.index_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
        }
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        unimplemented!()
    }

    fn set_name(&mut self, _name: String) -> &str {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct SkinnedMesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    pub primitives: Vec<SkinnedPrimitive<BufferType, CommandType, TextureType>>,
    pub is_disposed: bool,
    pub transform: Mat4,
    pub root_joint: Option<Joint>,
    pub ssbo: Option<SSBO>,
    pub model_index: usize,
}

impl<BufferType, CommandType, TextureType> Drop
    for SkinnedMesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped skinned mesh.");
        }
    }
}

impl<BufferType, CommandType, TextureType> Disposable
    for SkinnedMesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    fn dispose(&mut self) {
        for primitive in self.primitives.iter_mut() {
            primitive.dispose();
        }
        if let Some(ssbo) = self.ssbo.as_mut() {
            ssbo.dispose();
        }
        self.is_disposed = true;
        log::info!("Successfully disposed skinned mesh.");
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        unimplemented!()
    }

    fn set_name(&mut self, _name: String) -> &str {
        unimplemented!()
    }
}
