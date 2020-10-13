use async_trait::async_trait;
use glam::{Vec3A, Vec4};

#[async_trait]
pub trait Scene {
    fn initialize(&mut self);
    fn get_scene_name(&self) -> &str;
    fn set_scene_name(&mut self, scene_name: &str);
    async fn load_content(&mut self) -> anyhow::Result<()>;
    async fn update(&mut self, delta_time: f64) -> anyhow::Result<()>;
    async fn render(&self, delta_time: f64) -> anyhow::Result<()>;
    async fn add_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()>;
    async fn add_skinned_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()>;
    async fn wait_for_all_tasks(&mut self);
}
