use crossbeam::sync::ShardedLock;
use serde::{Deserialize, Serialize};
use std::mem::ManuallyDrop;
use std::sync::Arc;

use crate::game::shared::enums::ShaderType;
use crate::game::shared::traits::disposable::Disposable;
use crate::game::structs::Vertex;
use crate::game::{graphics, CommandData};

/// Primitive > Mesh > Model<br />
/// Primitiveはメッシュを構成します。<br />
/// メッシュはモデルを構成します。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Primitive {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture_index: Option<usize>,
    pub is_disposed: bool,
}

/// Mesh > Model<br />
/// メッシュはモデルを構成します。
#[derive(Clone)]
pub struct Mesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    CommandType: 'static,
    TextureType: 'static + Clone + Disposable,
{
    pub primitives: Vec<Primitive>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Vec<Arc<ShardedLock<TextureType>>>,
    pub is_disposed: bool,
    pub command_data: CommandData<CommandType>,
    pub shader_type: ShaderType,
    pub model_index: usize,
}

impl Mesh<graphics::vk::Buffer, ash::vk::CommandBuffer, graphics::vk::Image> {
    pub fn new(primitives: Vec<Primitive>) -> Self {
        Mesh {
            primitives,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false,
            texture: vec![],
            shader_type: ShaderType::BasicShader,
            model_index: 0,
            command_data: std::collections::HashMap::new(),
        }
    }

    pub fn get_vertex_buffer(&self) -> ash::vk::Buffer {
        if let Some(buffer) = self.vertex_buffer.as_ref() {
            buffer.buffer
        } else {
            panic!("Vertex buffer is not yet created.");
        }
    }

    pub fn get_index_buffer(&self) -> ash::vk::Buffer {
        if let Some(buffer) = self.index_buffer.as_ref() {
            buffer.buffer
        } else {
            panic!("Index buffer is not yet created.");
        }
    }
}

unsafe impl<BufferType, CommandType, TextureType> Send
    for Mesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    TextureType: 'static + Clone + Disposable,
{
}
unsafe impl<BufferType, CommandType, TextureType> Sync
    for Mesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    TextureType: 'static + Clone + Disposable,
{
}

impl<BufferType, CommandType, TextureType> Drop for Mesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped mesh.");
        }
    }
}

impl<BufferType, CommandType, TextureType> Disposable for Mesh<BufferType, CommandType, TextureType>
where
    BufferType: 'static + Clone + Disposable,
    TextureType: 'static + Clone + Disposable,
{
    fn dispose(&mut self) {
        unsafe {
            if let Some(buffer) = self.index_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.vertex_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
        }
        self.is_disposed = true;
        log::info!("Successfully disposed mesh.");
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
