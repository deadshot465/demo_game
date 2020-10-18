use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{Model, SkinnedModel, Terrain};
use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::util::get_random_string;
use crate::game::traits::GraphicsBase;

type ModelList<GraphicsType, BufferType, CommandType, TextureType> =
    Vec<Arc<Mutex<Model<GraphicsType, BufferType, CommandType, TextureType>>>>;
type SkinnedModelList<GraphicsType, BufferType, CommandType, TextureType> =
    Vec<Arc<Mutex<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>>>;
type TerrainList<GraphicsType, BufferType, CommandType, TextureType> =
    Vec<Arc<Mutex<Terrain<GraphicsType, BufferType, CommandType, TextureType>>>>;

pub struct ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub models: ModelList<GraphicsType, BufferType, CommandType, TextureType>,
    pub skinned_models: SkinnedModelList<GraphicsType, BufferType, CommandType, TextureType>,
    pub textures: Vec<Arc<ShardedLock<TextureType>>>,
    pub terrains: TerrainList<GraphicsType, BufferType, CommandType, TextureType>,
    pub command_buffers: Vec<CommandType>,
    resource: Vec<Arc<Mutex<Box<dyn Disposable>>>>,
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Send
    for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}
unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Sync
    for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

impl<GraphicsType, BufferType, CommandType, TextureType> Default
    for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
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
    ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub fn new() -> Self {
        ResourceManager {
            resource: vec![],
            models: vec![],
            skinned_models: vec![],
            textures: vec![],
            terrains: vec![],
            command_buffers: vec![],
        }
    }

    pub fn add_resource<U: 'static>(&mut self, resource: U) -> *mut U
    where
        U: Disposable,
    {
        let name = get_random_string(7);
        self.add_resource_with_name(resource, name)
    }

    pub fn add_resource_with_name<U: 'static>(&mut self, resource: U, name: String) -> *mut U
    where
        U: Disposable,
    {
        self.resource.push(Arc::new(Mutex::new(Box::new(resource))));
        let mutable = self.resource.last_mut().cloned().unwrap();
        let mut boxed = mutable.lock();
        boxed.set_name(name);
        let ptr = boxed.as_mut() as *mut _ as *mut U;
        ptr
    }

    pub fn add_model(
        &mut self,
        model: Model<GraphicsType, BufferType, CommandType, TextureType>,
    ) -> Arc<Mutex<Model<GraphicsType, BufferType, CommandType, TextureType>>> {
        let model = Arc::new(Mutex::new(model));
        self.models.push(model.clone());
        model
    }

    pub fn add_skinned_model(
        &mut self,
        model: SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>,
    ) {
        let model = Arc::new(Mutex::new(model));
        self.skinned_models.push(model);
    }

    pub fn add_texture(&mut self, texture: TextureType) -> Arc<ShardedLock<TextureType>> {
        let texture_wrapped = Arc::new(ShardedLock::new(texture));
        self.textures.push(texture_wrapped.clone());
        texture_wrapped
    }

    pub fn add_terrain(
        &mut self,
        terrain: Terrain<GraphicsType, BufferType, CommandType, TextureType>,
    ) -> Arc<Mutex<Terrain<GraphicsType, BufferType, CommandType, TextureType>>> {
        let terrain_wrapped = Arc::new(Mutex::new(terrain));
        self.terrains.push(terrain_wrapped.clone());
        terrain_wrapped
    }

    pub fn get_model_count(&self) -> usize {
        self.models.len()
    }
    pub fn get_skinned_model_count(&self) -> usize {
        self.skinned_models.len()
    }
    pub fn get_total_model_count(&self) -> usize {
        self.get_model_count() + self.get_skinned_model_count()
    }
    pub fn get_texture_count(&self) -> usize {
        self.textures.len()
    }

    pub fn get_resource<U>(&self, resource_name: &str) -> *const U
    where
        U: Disposable,
    {
        let item = self
            .resource
            .iter()
            .find(|r| (*r).lock().get_name() == resource_name);
        if let Some(res) = item {
            let resource_lock = res.lock();
            let ptr = resource_lock.as_ref() as *const _ as *const U;
            ptr
        } else {
            std::ptr::null()
        }
    }

    pub fn remove_resource(&mut self, resource_name: &str) {
        let mut res: Option<&Arc<Mutex<Box<dyn Disposable>>>> = None;
        let mut _index = 0_usize;
        for (index, item) in self.resource.iter().enumerate() {
            if item.lock().get_name() == resource_name {
                res = Some(item);
                _index = index;
                break;
            }
        }
        if res.is_some() {
            self.resource.remove(_index);
        }
    }
}

impl ResourceManager<Graphics, Buffer, CommandBuffer, Image> {
    pub fn create_ssbo(&mut self) -> anyhow::Result<()> {
        for model in self.skinned_models.iter_mut() {
            let mut model_lock = model.lock();
            model_lock.create_ssbo()?;
        }
        Ok(())
    }

    pub fn get_all_command_buffers(&mut self) {
        let mut model_command_buffers = self
            .models
            .iter()
            .map(|model| {
                let mesh_command_buffers = model
                    .lock()
                    .meshes
                    .iter()
                    .map(|mesh| mesh.command_buffer.unwrap())
                    .collect::<Vec<_>>();
                mesh_command_buffers
            })
            .flatten()
            .collect::<Vec<_>>();
        let mut skinned_model_command_buffers = self
            .skinned_models
            .iter()
            .map(|model| {
                let primitive_command_buffers = model
                    .lock()
                    .skinned_meshes
                    .iter()
                    .map(|mesh| {
                        mesh.primitives
                            .iter()
                            .map(|primitive| primitive.command_buffer.unwrap())
                            .collect::<Vec<_>>()
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                primitive_command_buffers
            })
            .flatten()
            .collect::<Vec<_>>();
        let mut terrain_command_buffers = self
            .terrains
            .iter()
            .map(|terrain| {
                let mesh_command_buffers = terrain
                    .lock()
                    .model
                    .meshes
                    .iter()
                    .map(|mesh| mesh.command_buffer.unwrap())
                    .collect::<Vec<_>>();
                mesh_command_buffers
            })
            .flatten()
            .collect::<Vec<_>>();
        model_command_buffers.append(&mut skinned_model_command_buffers);
        model_command_buffers.append(&mut terrain_command_buffers);
        self.command_buffers = model_command_buffers;
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        log::info!("Dropping Resource Manager...");
        for texture in self.textures.iter() {
            let mut texture_lock = texture.write().unwrap();
            if texture_lock.is_disposed() {
                continue;
            }
            texture_lock.dispose();
        }

        for model in self.skinned_models.iter() {
            let mut model_lock = model.lock();
            if model_lock.is_disposed() {
                continue;
            }
            model_lock.dispose();
        }
        for model in self.models.iter() {
            let mut model_lock = model.lock();
            if model_lock.is_disposed() {
                continue;
            }
            model_lock.dispose();
        }
        for terrain in self.terrains.iter() {
            let mut terrain_lock = terrain.lock();
            if terrain_lock.is_disposed() {
                continue;
            }
            terrain_lock.dispose();
        }
        for resource in self.resource.iter() {
            let mut resource_lock = resource.lock();
            if resource_lock.is_disposed() {
                continue;
            }
            resource_lock.dispose();
        }
        log::info!("Successfully dropped resource manager.");
    }
}
