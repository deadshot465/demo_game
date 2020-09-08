use std::cell::RefCell;
use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::util::get_random_string;
use crate::game::shared::structs::Model;
use crate::game::graphics::vk::{Graphics, Buffer};
use glam::{Vec3A, Vec4};

pub struct ResourceManager<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> {
    pub models: Vec<*mut Model<GraphicsType, BufferType>>,
    resource: Vec<RefCell<Box<dyn Disposable>>>,
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> ResourceManager<GraphicsType, BufferType> {
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
        self.resource.push(RefCell::new(Box::new(resource)));
        let mutable = self.resource.last_mut().unwrap();
        let boxed = mutable.get_mut().as_mut();
        boxed.set_name(name);
        let ptr = boxed as *mut _ as *mut U;
        ptr
    }

    pub fn add_model(&mut self, model: Model<GraphicsType, BufferType>) {
        let name = model.model_name.clone();
        let _model = self.add_resource_with_name(model, name);
        self.models.push(_model);
    }

    pub fn get_resource<U>(&self, resource_name: &str) -> *const U
        where U: Disposable {
        let item = self.resource.iter()
            .find(|r| (*r).borrow().get_name() == resource_name);
        if let Some(res) = item {
            let _res = res.borrow();
            let ptr = _res.as_ref() as *const _ as *const U;
            ptr
        }
        else {
            std::ptr::null()
        }
    }

    pub fn remove_resource(&mut self, resource_name: &str) {
        let mut res: Option<&RefCell<Box<dyn Disposable>>> = None;
        let mut _index = 0_usize;
        for (index, item) in self.resource.iter().enumerate() {
            if item.borrow().get_name() == resource_name {
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

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> Drop for ResourceManager<GraphicsType, BufferType> {
    fn drop(&mut self) {
        log::info!("Dropping Resource Manager...");
        unsafe {
            for resource in self.resource.iter_mut() {
                resource.as_ptr().as_mut().unwrap().dispose();
            }
        }
        self.resource.clear();
    }
}