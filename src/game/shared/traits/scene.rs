use crate::game::shared::enums::SceneType;
use crate::game::shared::structs::Primitive;
use async_trait::async_trait;
use glam::{Vec3A, Vec4};
use slotmap::DefaultKey;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use winit::event::{ElementState, VirtualKeyCode};

#[async_trait]
pub trait Scene: Sync {
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

    fn create_ssbo(&self) -> anyhow::Result<()>;

    fn generate_terrain(
        &mut self,
        _grid_x: f32,
        _grid_z: f32,
        _primitive: Option<Primitive>,
    ) -> anyhow::Result<Primitive> {
        Ok(Primitive {
            vertices: vec![],
            indices: vec![],
            texture_index: None,
            is_disposed: false,
        })
    }

    fn get_command_buffers(&self);
    fn get_model_count(&self) -> Arc<AtomicUsize>;
    fn get_scene_name(&self) -> &str;
    fn get_scene_type(&self) -> SceneType;
    fn initialize(&mut self);
    fn is_loaded(&self) -> bool;
    async fn input_key(&self, _key: VirtualKeyCode, _element_state: ElementState) {}
    async fn load_content(&mut self) -> anyhow::Result<()>;
    fn render(&self, delta_time: f64) -> anyhow::Result<()>;
    fn set_scene_name(&mut self, scene_name: &str);
    async fn update(&self, delta_time: f64) -> anyhow::Result<()>;
    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()>;
}
