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
    /// シーンの中にエンティティを追加する。<br />
    /// Add an entity to this scene.
    fn add_entity(&mut self, entity_name: &str) -> DefaultKey;

    /// シーンの中に一般的なモデルを追加する。<br />
    /// Add a common model to this scene.
    fn add_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        entity: DefaultKey,
    ) -> anyhow::Result<()>;

    /// シーンの中に存在しているモデルのSSBOを作成する。<br />
    /// Create SSBOs of all models existing in this scene.
    fn create_ssbo(&self) -> anyhow::Result<()>;

    /// 地形を生成する。<br />
    /// Generate a terrain.
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

    /// このシーンの中に存在しているモデルのコマンドバッファを取得する。<br />
    /// Get command buffers of models existing in this scene.
    fn get_command_buffers(&self);

    /// このシーンの中に存在しているモデルの個数を取得する。<br />
    /// Get count of models existing in this scene.
    fn get_model_count(&self) -> Arc<AtomicUsize>;

    /// シーンの名前を取得する。<br />
    /// Get this scene's name.
    fn get_scene_name(&self) -> &str;

    /// シーンのタイプを取得する。<br />
    /// Get this scene's type.
    fn get_scene_type(&self) -> SceneType;

    /// シーンを初期化する。<br />
    /// Initialize the scene.
    fn initialize(&mut self);

    /// シーンはロード済みなのかどうか。<br />
    /// Is this scene loaded?
    fn is_loaded(&self) -> bool;

    /// キーが押されたらのコールバック。<br />
    /// Callback when a key is pressed.
    async fn input_key(&self, _key: VirtualKeyCode, _element_state: ElementState) {}

    /// シーンコンテンツをロードする。<br />
    /// Load contents in this scene.
    async fn load_content(&mut self) -> anyhow::Result<()>;

    /// シーンを描画する。<br />
    /// Render the scene.
    fn render(&self, delta_time: f64) -> anyhow::Result<()>;

    /// シーンの名前を設定する。<br />
    /// Set this scene's name.
    fn set_scene_name(&mut self, scene_name: &str);

    /// シーンを更新する。<br />
    /// Update the scene.
    async fn update(&self, delta_time: f64) -> anyhow::Result<()>;

    /// 全てのタスクを待つ。<br />
    /// Wait for all tasks in this scene.
    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()>;
}
