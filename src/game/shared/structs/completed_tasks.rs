use crate::game::structs::{GeometricPrimitive, InstancedModel, Model, SkinnedModel, Terrain};
use crate::game::traits::{Disposable, GraphicsBase};

pub struct CompletedTasks<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub models: Vec<Model<GraphicsType, BufferType, CommandType, TextureType>>,
    pub skinned_models: Vec<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>,
    pub terrains: Vec<Terrain<GraphicsType, BufferType, CommandType, TextureType>>,
    pub geometric_primitives:
        Vec<GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>>,
    pub instances: Vec<InstancedModel<GraphicsType, BufferType, CommandType, TextureType>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType> Default
    for CompletedTasks<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    CompletedTasks<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub fn new() -> Self {
        CompletedTasks {
            models: vec![],
            skinned_models: vec![],
            terrains: vec![],
            geometric_primitives: vec![],
            instances: vec![],
        }
    }
}
