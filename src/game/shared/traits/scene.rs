use async_trait::async_trait;
use glam::{Vec3A, Vec4};

#[async_trait]
pub trait Scene {
    fn initialize(&mut self);
    fn load_content(&mut self);
    fn update(&mut self, delta_time: u64);
    fn render(&self, delta_time: u64);
    fn get_scene_name(&self) -> &str;
    fn set_scene_name(&mut self, scene_name: &str);
    fn add_model(&mut self, file_name: &'static str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4);
    fn add_skinned_model(&mut self, file_name: &'static str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4);
    async fn wait_for_all_tasks(&mut self);
}