use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::game::enums::SceneType;
use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::traits::Renderable;
use crate::game::shared::util::get_random_string;
use crate::game::traits::GraphicsBase;
use crate::game::LockableRenderable;

pub struct ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub textures: Vec<Arc<ShardedLock<TextureType>>>,
    pub command_buffers: HashMap<SceneType, HashMap<usize, Vec<CommandType>>>,
    pub model_queue: HashMap<
        SceneType,
        Vec<LockableRenderable<GraphicsType, BufferType, CommandType, TextureType>>,
    >,
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
            textures: vec![],
            command_buffers: HashMap::new(),
            model_queue: HashMap::new(),
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

    pub fn add_texture(&mut self, texture: TextureType) -> Arc<ShardedLock<TextureType>> {
        let texture_wrapped = Arc::new(ShardedLock::new(texture));
        self.textures.push(texture_wrapped.clone());
        texture_wrapped
    }

    pub fn get_model_count(&self) -> usize {
        let mut count = 0;
        self.model_queue
            .iter()
            .for_each(|(_, model_queue)| count += model_queue.len());
        count
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
    pub fn create_ssbo(&self, scene_type: SceneType) -> anyhow::Result<()> {
        let current_model_queue = self
            .model_queue
            .get(&scene_type)
            .expect("Failed to get model queue of the current scene.");
        for model in current_model_queue.iter() {
            let mut model_lock = model.lock();
            model_lock.create_ssbo()?;
        }
        Ok(())
    }

    pub fn get_all_command_buffers(&mut self, scene_type: SceneType) {
        let inflight_frame_count = std::env::var("INFLIGHT_BUFFER_COUNT")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let current_model_queue = self
            .model_queue
            .get(&scene_type)
            .expect("Failed to get model queue of the current scene.");
        for i in 0..inflight_frame_count {
            let model_command_buffers = current_model_queue
                .iter()
                .map(|m| m.lock().get_command_buffers(i))
                .flatten()
                .collect::<Vec<_>>();
            let current_scene = self
                .command_buffers
                .entry(scene_type)
                .or_insert_with(HashMap::new);
            let entry = current_scene.entry(i).or_insert(vec![]);
            *entry = model_command_buffers;
        }
    }

    pub fn add_model(
        &mut self,
        scene_type: SceneType,
        model: impl Renderable<Graphics, Buffer, CommandBuffer, Image> + Send + 'static,
    ) -> LockableRenderable<Graphics, Buffer, CommandBuffer, Image> {
        let model_queue = self.model_queue.entry(scene_type).or_insert_with(Vec::new);
        model_queue.push(Arc::new(Mutex::new(Box::new(model))));
        let reference = model_queue.last().cloned().unwrap();
        reference
    }

    pub fn add_clone(
        &mut self,
        scene_type: SceneType,
        model: Box<dyn Renderable<Graphics, Buffer, CommandBuffer, Image> + Send + 'static>,
    ) -> LockableRenderable<Graphics, Buffer, CommandBuffer, Image> {
        let model_queue = self
            .model_queue
            .get_mut(&scene_type)
            .expect("Failed to get model queue of the specified scene.");
        model_queue.push(Arc::new(Mutex::new(model)));
        let reference = model_queue.last().cloned().unwrap();
        reference
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

        for (_, model_queue) in self.model_queue.iter() {
            for model in model_queue.iter() {
                let mut model_lock = model.lock();
                model_lock.dispose();
            }
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
