use async_trait::async_trait;
use crate::game::shared::traits::{Scene, GraphicsBase};
use glam::{Vec3A, Vec4};
use std::sync::{RwLock, Weak};
use crate::game::ResourceManager;
use crate::game::traits::Disposable;
use crate::game::shared::structs::Model;
use crate::game::graphics::vk::{Graphics, Buffer, Image};
use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use tokio::task::JoinHandle;

pub struct GameScene<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static + Clone, TextureType: 'static + Clone + Disposable> {
    graphics: Weak<ShardedLock<GraphicsType>>,
    resource_manager: Weak<RwLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    scene_name: String,
    tasks: Vec<JoinHandle<Model<GraphicsType, BufferType, CommandType, TextureType>>>,
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static + Clone, TextureType: 'static + Clone + Disposable> GameScene<GraphicsType, BufferType, CommandType, TextureType> {
    pub fn new(resource_manager: Weak<RwLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>, graphics: Weak<ShardedLock<GraphicsType>>) -> Self {
        GameScene {
            graphics,
            resource_manager,
            scene_name: String::from("GAME_SCENE"),
            tasks: vec![],
        }
    }
}

#[async_trait]
impl Scene for GameScene<Graphics, Buffer, CommandBuffer, Image> {
    fn initialize(&mut self) {

    }

    fn load_content(&mut self) {
        self.add_model("./models/tank/tank.gltf", Vec3A::new(0.0, 0.0, 0.0),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 0.0, 0.0), Vec4::new(0.0, 0.0, 1.0, 1.0));
        self.add_model("./models/tank/tank.gltf", Vec3A::new(1.5, 0.0, 1.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 90.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 1.0));
        /*self.add_model("./models/tank/tank.gltf", Vec3A::new(-1.5, 0.0, -1.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 225.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0));
        self.add_model("./models/tank/tank.gltf", Vec3A::new(2.5, 0.0, 2.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 270.0, 0.0), Vec4::new(1.0, 1.0, 1.0, 1.0));*/
        /*self.add_model("./models/mr.incredible/Mr.Incredible.glb", Vec3A::new(0.0, 0.0, 0.0),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(0.0, 0.0, 0.0), Vec4::new(1.0, 1.0, 1.0, 1.0));*/
        self.add_model("./models/bison/output.gltf", Vec3A::new(0.0, 0.0, 0.0),
                       Vec3A::new(400.0, 400.0, 400.0), Vec3A::new(0.0, 90.0, 90.0), Vec4::new(1.0, 1.0, 1.0, 1.0));
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

    fn add_model(&mut self, file_name: &'static str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) {
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            log::error!("Resource manager has been destroyed.");
            return;
        }
        let resource_manager = resource_manager.unwrap();
        unsafe {
            let lock = resource_manager
                .read()
                .expect("Failed to lock resource manager.");
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
                model.model_index = lock.get_model_count();
                drop(lock);
                let mut lock = resource_manager
                    .write()
                    .unwrap();
                lock.add_model(model);
                drop(lock);
            }
            else {
                let model_index = lock.get_model_count();
                drop(lock);
                let task = Model::new(file_name, self.graphics.clone(), position, scale, rotation, color, model_index);
                self.tasks.push(task);
            }
        }
        drop(resource_manager);
    }

    async fn wait_for_all_tasks(&mut self) {
        let tasks = &mut self.tasks;
        let mut models = vec![];
        for task in tasks.into_iter() {
            let model = task.await.unwrap();
            models.push(model);
        }
        let rm = self.resource_manager.upgrade();
        if rm.is_none() {
            log::error!("Failed to lock resource manager for waiting tasks.");
            return;
        }
        let rm = rm.unwrap();
        let mut lock = rm.write().unwrap();
        for model in models.into_iter() {
            lock.add_model(model);
        }
        drop(lock);
        drop(rm);
        self.tasks.clear();
    }
}