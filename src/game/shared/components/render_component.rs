use crate::game::shared::structs::{GeometricPrimitive, Model, SkinnedModel, Terrain};
use crate::game::traits::{Disposable, GraphicsBase};

pub enum RenderComponent<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    Static(Model<GraphicsType, BufferType, CommandType, TextureType>),
    Skinned(SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>),
    Terrain(Terrain<GraphicsType, BufferType, CommandType, TextureType>),
    Primitive(GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>),
}
