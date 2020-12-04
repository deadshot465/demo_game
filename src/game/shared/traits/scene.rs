use crate::game::shared::enums::SceneType;
use crate::game::shared::structs::Primitive;
use async_trait::async_trait;
use glam::{Vec3A, Vec4};
use slotmap::DefaultKey;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[async_trait]
pub trait Scene {
    fn initialize(&mut self);
    fn get_scene_name(&self) -> &str;
    fn set_scene_name(&mut self, scene_name: &str);
    async fn load_content(&mut self) -> anyhow::Result<()>;
    async fn update(&mut self, delta_time: f64) -> anyhow::Result<()>;
    fn render(&self, delta_time: f64) -> anyhow::Result<()>;
    fn add_entity(&mut self, entity_name: &str) -> DefaultKey;
    fn add_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        entity: DefaultKey,
    ) -> anyhow::Result<()>;

    fn generate_terrain(
        &mut self,
        _grid_x: i32,
        _grid_z: i32,
        _primitive: Option<Primitive>,
        _entity: DefaultKey,
    ) -> anyhow::Result<Primitive> {
        Ok(Primitive {
            vertices: vec![],
            indices: vec![],
            texture_index: None,
            is_disposed: false,
        })
    }

    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()>;
    fn get_model_count(&self) -> Arc<AtomicUsize>;
    fn get_scene_type(&self) -> SceneType;
    fn create_ssbo(&self) -> anyhow::Result<()>;
    fn get_command_buffers(&self);
}
