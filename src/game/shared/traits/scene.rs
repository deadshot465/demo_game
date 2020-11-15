use crate::game::shared::enums::SceneType;
use glam::{Vec3A, Vec4};
//use parking_lot::RwLock;
//use std::mem::ManuallyDrop;
use slotmap::DefaultKey;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

pub trait Scene {
    fn initialize(&mut self);
    fn get_scene_name(&self) -> &str;
    fn set_scene_name(&mut self, scene_name: &str);
    fn load_content(&mut self) -> anyhow::Result<()>;
    fn update(&mut self, delta_time: f64) -> anyhow::Result<()>;
    fn render(&self, delta_time: f64) -> anyhow::Result<()>;
    fn add_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        entity: DefaultKey,
    ) -> anyhow::Result<()>;
    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()>;
    fn get_model_count(&self) -> Arc<AtomicUsize>;
    fn get_scene_type(&self) -> SceneType;
    fn create_ssbo(&self) -> anyhow::Result<()>;
    fn get_command_buffers(&self);
}
