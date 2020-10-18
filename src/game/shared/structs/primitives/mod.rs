use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::structs::Model;
use crate::game::traits::{Disposable, GraphicsBase};
use ash::vk::CommandBuffer;
use crossbeam::channel::*;

#[derive(Copy, Clone, Debug)]
pub enum PrimitiveType {
    Rect,
}

pub struct GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    pub is_disposed: bool,
    pub model: Model<GraphicsType, BufferType, CommandType, TextureType>,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    //pub fn create_primitive() -> Self {}
}

impl GeometricPrimitive<Graphics, Buffer, CommandBuffer, Image> {
    //pub fn new() -> anyhow::Result<Receiver<Self>> {}
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    fn dispose(&mut self) {
        if self.is_disposed {
            return;
        }
        self.model.dispose();
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        self.model.get_name()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.model.set_name(name)
    }
}
