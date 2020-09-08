use glam::{Vec3A, Vec4};

pub trait Scene {
    fn initialize(&mut self);
    fn load_content(&mut self);
    fn update(&mut self, delta_time: u64);
    fn render(&self, delta_time: u64);
    fn get_scene_name(&self) -> &str;
    fn set_scene_name(&mut self, scene_name: &str);
    fn add_model(&self, file_name: &str, position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4);
}