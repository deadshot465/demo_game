use crate::game::shared::structs::CompletedTasks;
use crate::game::structs::{GeometricPrimitive, InstancedModel, Model, SkinnedModel, Terrain};
use crate::game::traits::{Disposable, GraphicsBase};
use crossbeam::channel::*;

/// モデルの読み込み及びシェイプや地形を生成するとき、より効率的に実行するため、読み込み開始の時点は全てをタスク化しました。<br />
/// 処理完了する際にタスクを待つことができるような仕様です。<br />
/// When reading models or generating shapes and terrains, to increase the performance, all reading/generating will return tasks.<br />
/// When all reading/generating functions are invoked, the tasks can then be waited.
pub struct WaitableTasks<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub model_tasks: Vec<Receiver<Model<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub skinned_model_tasks:
        Vec<Receiver<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub terrain_tasks: Vec<Receiver<Terrain<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub geometric_primitive_tasks:
        Vec<Receiver<GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub instanced_model_tasks:
        Vec<Receiver<InstancedModel<GraphicsType, BufferType, CommandType, TextureType>>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType> Default
    for WaitableTasks<GraphicsType, BufferType, CommandType, TextureType>
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
    WaitableTasks<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub fn new() -> Self {
        WaitableTasks {
            model_tasks: vec![],
            skinned_model_tasks: vec![],
            terrain_tasks: vec![],
            geometric_primitive_tasks: vec![],
            instanced_model_tasks: vec![],
        }
    }

    pub fn wait_for_all_tasks(
        &mut self,
    ) -> anyhow::Result<CompletedTasks<GraphicsType, BufferType, CommandType, TextureType>> {
        let model_tasks = &mut self.model_tasks;
        let skinned_model_tasks = &mut self.skinned_model_tasks;
        let terrain_tasks = &mut self.terrain_tasks;
        let primitive_tasks = &mut self.geometric_primitive_tasks;
        let instance_tasks = &mut self.instanced_model_tasks;
        let mut models = vec![];
        let mut skinned_models = vec![];
        let mut terrains = vec![];
        let mut primitives = vec![];
        let mut instances = vec![];
        for task in model_tasks.iter_mut() {
            let model = task.recv()?;
            models.push(model);
        }
        for task in skinned_model_tasks.iter_mut() {
            let model = task.recv()?;
            skinned_models.push(model);
        }
        for task in terrain_tasks.iter_mut() {
            let terrain = task.recv()?;
            terrains.push(terrain);
        }
        for task in primitive_tasks.iter_mut() {
            let primitive = task.recv()?;
            primitives.push(primitive);
        }
        for task in instance_tasks.iter_mut() {
            let instance = task.recv()?;
            instances.push(instance);
        }
        Ok(CompletedTasks {
            models,
            skinned_models,
            terrains,
            geometric_primitives: primitives,
            instances,
        })
    }

    pub fn clear(&mut self) {
        self.model_tasks.clear();
        self.skinned_model_tasks.clear();
        self.terrain_tasks.clear();
        self.geometric_primitive_tasks.clear();
        self.instanced_model_tasks.clear();
    }
}
