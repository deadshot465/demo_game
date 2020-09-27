use ash::vk::CommandBuffer;
use async_trait::async_trait;
use crossbeam::sync::ShardedLock;
use glam::{Vec3A, Vec4};
use std::sync::Weak;
use tokio::task::JoinHandle;

use crate::game::graphics::vk::{Graphics, Buffer, Image};
use crate::game::shared::structs::{Model, SkinnedModel};
use crate::game::shared::traits::{Scene, GraphicsBase};
use crate::game::traits::Disposable;
use crate::game::ResourceManager;

pub struct GameScene<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    graphics: Weak<ShardedLock<GraphicsType>>,
    resource_manager: Weak<ShardedLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    scene_name: String,
    model_tasks: Vec<JoinHandle<Model<GraphicsType, BufferType, CommandType, TextureType>>>,
    skinned_model_tasks: Vec<JoinHandle<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>>,
    model_count: usize
}

impl<GraphicsType, BufferType, CommandType, TextureType> GameScene<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    pub fn new(resource_manager: Weak<ShardedLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>, graphics: Weak<ShardedLock<GraphicsType>>) -> Self {
        GameScene {
            graphics,
            resource_manager,
            scene_name: String::from("GAME_SCENE"),
            model_tasks: vec![],
            model_count: 0,
            skinned_model_tasks: vec![],
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
        /*self.add_model("./models/tank/tank.gltf", Vec3A::new(1.5, 0.0, 1.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 90.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 1.0));
        self.add_model("./models/tank/tank.gltf", Vec3A::new(-1.5, 0.0, -1.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 225.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0));
        self.add_model("./models/tank/tank.gltf", Vec3A::new(2.5, 0.0, 2.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 270.0, 0.0), Vec4::new(1.0, 1.0, 1.0, 1.0));*/
        self.add_model("./models/mr.incredible/Mr.Incredible.glb", Vec3A::new(0.0, 0.0, 0.0),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(0.0, 0.0, 0.0), Vec4::new(1.0, 1.0, 1.0, 1.0));
        self.add_model("./models/bison/output.gltf", Vec3A::new(0.0, 0.0, 0.0),
                       Vec3A::new(400.0, 400.0, 400.0), Vec3A::new(0.0, 90.0, 90.0), Vec4::new(1.0, 1.0, 1.0, 1.0));
        self.add_skinned_model("./models/cesiumMan/CesiumMan.glb", Vec3A::new(-1.5, 0.0, -1.5),
                       Vec3A::new(2.0, 2.0, 2.0), Vec3A::new(0.0, 180.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0));
    }

    fn update(&mut self, delta_time: f64) {
        let graphics_arc = self.graphics.upgrade().unwrap();
        let mut graphics_lock = graphics_arc.write().unwrap();
        graphics_lock.update(delta_time);
    }

    fn render(&self, _delta_time: f64) {
        /*let arc = self.resource_manager.upgrade();
        let arc_2 = self.graphics.upgrade();
        if let Some(resource_manager) = arc {
            if let Some(graphics) = arc_2 {
                let resource_lock = resource_manager.read().unwrap();
                let graphics_lock = graphics.read().unwrap();
                let models = &resource_lock.models;
                let frame_buffers = &graphics_lock.frame_buffers;
                for buffer in frame_buffers.iter() {
                    for model in models.iter() {
                        let model_lock = model.lock();
                        unsafe {
                            model.as_ref().unwrap().render(*buffer);
                        }
                    }
                }
                drop(graphics_lock);
                drop(resource_lock);
            }
        }*/
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
        let lock = resource_manager
            .read()
            .expect("Failed to lock resource manager.");
        let item = lock.models.iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .map(|m| m.clone());
        if let Some(m) = item {
            let mut model = Model::from(&*m.lock());
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
            drop(lock);
            let task = Model::new(file_name, self.graphics.clone(), position, scale, rotation, color, self.model_count);
            self.model_tasks.push(task);
        }
        self.model_count += 1;
        drop(resource_manager);
    }

    fn add_skinned_model(&mut self, file_name: &'static str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) {
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            log::error!("Resource manager has been destroyed.");
            return;
        }
        let resource_manager = resource_manager.unwrap();
        let lock = resource_manager
            .read()
            .expect("Failed to lock resource manager.");
        let item = lock.skinned_models.iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .map(|m| m.clone());
        if let Some(m) = item {
            let mut model = SkinnedModel::from(&*m.lock());
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
            lock.add_skinned_model(model);
            drop(lock);
        }
        else {
            drop(lock);
            let task = SkinnedModel::new(file_name, self.graphics.clone(), position, scale, rotation, color, self.model_count);
            self.skinned_model_tasks.push(task);
        }
        self.model_count += 1;
        drop(resource_manager);
    }

    async fn wait_for_all_tasks(&mut self) {
        let model_tasks = &mut self.model_tasks;
        let skinned_model_tasks = &mut self.skinned_model_tasks;
        let mut models = vec![];
        let mut skinned_models = vec![];
        for task in model_tasks.into_iter() {
            let model = task.await.unwrap();
            models.push(model);
        }
        for task in skinned_model_tasks.into_iter() {
            let model = task.await.unwrap();
            skinned_models.push(model);
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
        for model in skinned_models.into_iter() {
            lock.add_skinned_model(model);
        }
        drop(lock);
        drop(rm);
        self.model_tasks.clear();
    }
}