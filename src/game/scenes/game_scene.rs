use crate::game::shared::traits::Scene;
use glam::{Vec3A, Vec4};
use std::sync::{Arc, RwLock};
use crate::game::ResourceManager;
use crate::game::traits::Disposable;
use crate::game::shared::structs::Model;

pub struct GameScene<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> {
    graphics: Arc<RwLock<GraphicsType>>,
    resource_manager: Arc<RwLock<ResourceManager<GraphicsType, BufferType>>>,
    scene_name: String,
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> GameScene<GraphicsType, BufferType> {
    pub fn new(resource_manager: Arc<RwLock<ResourceManager<GraphicsType, BufferType>>>, graphics: Arc<RwLock<GraphicsType>>) -> Self {
        GameScene {
            graphics,
            resource_manager,
            scene_name: String::from("GAME_SCENE")
        }
    }
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> Scene for GameScene<GraphicsType, BufferType> {
    fn initialize(&mut self) {

    }

    fn load_content(&mut self) {
        self.add_model("./models/tank/tank.gltf", Vec3A::new(1.5, 0.0, 1.5),
                       Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(0.0, 0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0));
    }

    fn update(&mut self, delta_time: u64) {

    }

    fn render(&self, delta_time: u64) {

    }

    fn get_scene_name(&self) -> &str {
        self.scene_name.as_str()
    }

    fn set_scene_name(&mut self, scene_name: &str) {
        self.scene_name = scene_name.to_string();
    }

    fn add_model(&self, file_name: &str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) {
        let mut lock = self.resource_manager
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
    }
}