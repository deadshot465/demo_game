use crate::game::shared::traits::{Scene, GraphicsBase};
use glam::{Vec3A, Vec4};
use std::sync::{RwLock, Weak};
use crate::game::ResourceManager;
use crate::game::traits::Disposable;
use crate::game::shared::structs::Model;
use crate::game::graphics::vk::{Graphics, Buffer};
use ash::vk::CommandBuffer;

pub struct GameScene<GraphicsType: 'static + GraphicsBase<BufferType, CommandType>, BufferType: 'static + Disposable + Clone, CommandType: 'static> {
    graphics: Weak<RwLock<GraphicsType>>,
    resource_manager: Weak<RwLock<ResourceManager<GraphicsType, BufferType, CommandType>>>,
    scene_name: String,
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType>, BufferType: 'static + Disposable + Clone, CommandType: 'static> GameScene<GraphicsType, BufferType, CommandType> {
    pub fn new(resource_manager: Weak<RwLock<ResourceManager<GraphicsType, BufferType, CommandType>>>, graphics: Weak<RwLock<GraphicsType>>) -> Self {
        GameScene {
            graphics,
            resource_manager,
            scene_name: String::from("GAME_SCENE")
        }
    }
}

impl Scene for GameScene<Graphics, Buffer, CommandBuffer> {
    fn initialize(&mut self) {

    }

    fn load_content(&mut self) {
        self.add_model("./models/tank/tank.gltf", Vec3A::new(1.5, 0.0, 1.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(0.0, 0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0));
    }

    fn update(&mut self, _delta_time: u64) {
    }

    fn render(&self, _delta_time: u64) {
        let arc = self.resource_manager.upgrade();
        let arc_2 = self.graphics.upgrade();
        if let Some(resource_manager) = arc {
            if let Some(graphics) = arc_2 {
                let resource_lock = resource_manager.read().unwrap();
                let graphics_lock = graphics.read().unwrap();
                let cmd_buffers = graphics_lock.get_commands();
                let models = &resource_lock.models;
                for buffer in cmd_buffers.iter() {
                    for model in models.iter() {
                        unsafe {
                            model.as_ref().unwrap().render(*buffer);
                        }
                    }
                }
                drop(graphics_lock);
                drop(resource_lock);
            }
        }
    }

    fn get_scene_name(&self) -> &str {
        self.scene_name.as_str()
    }

    fn set_scene_name(&mut self, scene_name: &str) {
        self.scene_name = scene_name.to_string();
    }

    fn add_model(&self, file_name: &str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) {
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            log::error!("Resource manager has been destroyed.");
            return;
        }
        let resource_manager = resource_manager.unwrap();
        let mut lock = resource_manager
            .write()
            .expect("Failed to lock resource manager.");
        unsafe {
            let item = lock.models.iter()
                .find(|m| (*m).as_ref().unwrap().get_name() == file_name);
            if let Some(m) = item {
                let _m = m.as_ref().unwrap();
                let mut model = Model::from(_m);
                model.position = position;
                model.scale = scale;
                let x: f32 = rotation.x();
                let y: f32 = rotation.y();
                let z: f32 = rotation.z();
                model.rotation = Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians());
                model.color = color;
                lock.add_model(model);
            }
            else {
                let model = Model::new(file_name, self.graphics.clone(), position, scale, rotation, color);
                lock.add_model(model);
            }
        }
        drop(lock);
        drop(resource_manager);
    }
}