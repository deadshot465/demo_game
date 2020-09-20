use parking_lot::Mutex;
use std::sync::Arc;

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::Model;
use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::util::get_random_string;
use crate::game::traits::GraphicsBase;

pub struct ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    pub models: Vec<Arc<Mutex<Model<GraphicsType, BufferType, CommandType, TextureType>>>>,
    resource: Vec<Arc<Mutex<Box<dyn Disposable>>>>,
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Send for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable { }
unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Sync for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable { }

impl<GraphicsType, BufferType, CommandType, TextureType> ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    pub fn new() -> Self {
        ResourceManager {
            resource: vec![],
            models: vec![],
        }
    }

    pub fn add_resource<U: 'static>(&mut self, resource: U) -> *mut U
        where U: Disposable {
        let name = get_random_string(7);
        self.add_resource_with_name(resource, name)
    }

    pub fn add_resource_with_name<U: 'static>(&mut self, resource: U, name: String) -> *mut U
        where U: Disposable {
        self.resource.push(Arc::new(Mutex::new(Box::new(resource))));
        let mutable = self.resource.last_mut().cloned().unwrap();
        let mut boxed = mutable.lock();
        boxed.set_name(name);
        let ptr = boxed.as_mut() as *mut _ as *mut U;
        ptr
    }

    pub fn add_model(&mut self, model: Model<GraphicsType, BufferType, CommandType, TextureType>) {
        let name = model.model_name.clone();
        let model = Arc::new(Mutex::new(model));
        let mut model_lock = model.lock();
        model_lock.model_index = self.models.len();
        model_lock.model_name = name;
        drop(model_lock);
        self.models.push(model);
    }

    pub fn get_model_count(&self) -> usize {
        self.models.len()
    }

    pub fn get_resource<U>(&self, resource_name: &str) -> *const U
        where U: Disposable {
        let item = self.resource.iter()
            .find(|r| (*r).lock().get_name() == resource_name);
        if let Some(res) = item {
            let resource_lock = res.lock();
            let ptr = resource_lock.as_ref() as *const _ as *const U;
            ptr
        }
        else {
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
        if let Some(_) = res {
            self.resource.remove(_index);
        }
    }
}

impl ResourceManager<Graphics, Buffer, ash::vk::CommandBuffer, Image> {
    pub fn create_sampler_resource(&self) {
        for model in self.models.iter() {
            model.lock().create_sampler_resource();
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop for ResourceManager<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn drop(&mut self) {
        log::info!("Dropping Resource Manager...");
        for model in self.models.iter() {
            let mut model_lock = model.lock();
            if model_lock.is_disposed() {
                continue;
            }
            model_lock.dispose();
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