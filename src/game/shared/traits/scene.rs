use crate::game::structs::PrimitiveType;
use glam::{Vec3A, Vec4};

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
    ) -> anyhow::Result<()>;
    fn add_skinned_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()>;
    fn add_geometric_primitive(
        &mut self,
        primitive_type: PrimitiveType,
        texture_name: Option<&'static str>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()>;
    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()>;
}
