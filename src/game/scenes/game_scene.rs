use ash::vk::CommandBuffer;
use async_trait::async_trait;
use glam::{Vec3A, Vec4};
use std::sync::Weak;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{Model, SkinnedModel, Terrain};
use crate::game::shared::traits::{GraphicsBase, Scene};
use crate::game::traits::Disposable;
use crate::game::ResourceManager;

pub struct GameScene<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    graphics: Weak<RwLock<GraphicsType>>,
    resource_manager:
        Weak<RwLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    scene_name: String,
    model_count: usize,
    model_tasks: Vec<JoinHandle<Model<GraphicsType, BufferType, CommandType, TextureType>>>,
    skinned_model_tasks:
        Vec<JoinHandle<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>>,
    terrain_tasks: Vec<JoinHandle<Terrain<GraphicsType, BufferType, CommandType, TextureType>>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    GameScene<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub fn new(
        resource_manager: Weak<
            RwLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>,
        >,
        graphics: Weak<RwLock<GraphicsType>>,
    ) -> Self {
        GameScene {
            graphics,
            resource_manager,
            scene_name: String::from("GAME_SCENE"),
            model_tasks: vec![],
            model_count: 0,
            skinned_model_tasks: vec![],
            terrain_tasks: vec![],
        }
    }
}

impl GameScene<Graphics, Buffer, CommandBuffer, Image> {
    pub async fn generate_terrain(&mut self, grid_x: i32, grid_z: i32) -> anyhow::Result<()> {
        let model_index = self.model_count;
        let terrain = Terrain::new(
            grid_x,
            grid_z,
            model_index,
            self.resource_manager.clone(),
            self.graphics.clone(),
        )
        .await?;
        self.terrain_tasks.push(terrain);
        self.model_count += 1;
        Ok(())
    }
}

#[async_trait]
impl Scene for GameScene<Graphics, Buffer, CommandBuffer, Image> {
    fn initialize(&mut self) {}

    async fn load_content(&mut self) -> anyhow::Result<()> {
        /*self.add_skinned_model(
            "./models/nathan/Nathan.glb",
            Vec3A::new(-1.5, 0.0, -1.5),
            Vec3A::new(1.0, 1.0, 1.0),
            Vec3A::new(0.0, 0.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;
        self.add_model(
            "./models/ak/output.gltf",
            Vec3A::new(2.5, 0.0, 2.5),
            Vec3A::new(1.0, 1.0, 1.0),
            Vec3A::new(0.0, 0.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;*/
        /*self.add_model(
            "./models/tank/tank.gltf",
            Vec3A::new(0.0, 0.0, 0.0),
            Vec3A::new(1.0, 1.0, 1.0),
            Vec3A::new(90.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 1.0),
        ).await?;*/
        /*self.add_model("./models/tank/tank.gltf", Vec3A::new(1.5, 0.0, 1.5),
        Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 90.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 1.0));*/
        self.add_model(
            "./models/mr.incredible/Mr.Incredible.glb",
            Vec3A::new(0.0, 0.0, -400.0),
            Vec3A::new(1.0, 1.0, 1.0),
            Vec3A::new(0.0, 0.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )
        .await?;
        self.add_model(
            "./models/bison/output.gltf",
            Vec3A::new(0.0, 0.0, -400.0),
            Vec3A::new(400.0, 400.0, 400.0),
            Vec3A::new(0.0, 90.0, 90.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )
        .await?;
        self.add_skinned_model(
            "./models/cesiumMan/CesiumMan.glb",
            Vec3A::new(-3.5, 0.0, -400.0),
            Vec3A::new(2.0, 2.0, 2.0),
            Vec3A::new(0.0, 180.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )
        .await?;
        self.generate_terrain(0, -1).await?;
        self.generate_terrain(-1, -1).await?;
        Ok(())
    }

    async fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let graphics_arc = self.graphics.upgrade().unwrap();
        let mut graphics_lock = graphics_arc.write().await;
        graphics_lock.update(delta_time).await?;
        Ok(())
    }

    async fn render(&self, _delta_time: f64) -> anyhow::Result<()> {
        let graphics = self
            .graphics
            .upgrade()
            .expect("Failed to upgrade Weak of Graphics for rendering.");
        {
            let mut graphics_lock = graphics.write().await;
            graphics_lock.render().await?;
        }
        Ok(())
    }

    fn get_scene_name(&self) -> &str {
        self.scene_name.as_str()
    }

    fn set_scene_name(&mut self, scene_name: &str) {
        self.scene_name = scene_name.to_string();
    }

    async fn add_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()> {
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            return Err(anyhow::anyhow!("Resource manager has been destroyed."));
        }
        let resource_manager = resource_manager.unwrap();
        let lock = resource_manager.read().await;
        let item = lock
            .models
            .iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .cloned();
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
            let mut lock = resource_manager.write().await;
            lock.add_model(model);
            drop(lock);
        } else {
            drop(lock);
            let task = Model::new(
                file_name,
                self.graphics.clone(),
                position,
                scale,
                rotation,
                color,
                self.model_count,
            )
            .await?;
            self.model_tasks.push(task);
        }
        self.model_count += 1;
        drop(resource_manager);
        Ok(())
    }

    async fn add_skinned_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()> {
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            return Err(anyhow::anyhow!("Resource manager has been destroyed."));
        }
        let resource_manager = resource_manager.unwrap();
        let lock = resource_manager.read().await;
        let item = lock
            .skinned_models
            .iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .cloned();
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
            let mut lock = resource_manager.write().await;
            lock.add_skinned_model(model);
            drop(lock);
        } else {
            drop(lock);
            let task = SkinnedModel::new(
                file_name,
                self.graphics.clone(),
                position,
                scale,
                rotation,
                color,
                self.model_count,
            )
            .await?;
            self.skinned_model_tasks.push(task);
        }
        self.model_count += 1;
        drop(resource_manager);
        Ok(())
    }

    async fn wait_for_all_tasks(&mut self) -> anyhow::Result<()> {
        let model_tasks = &mut self.model_tasks;
        let skinned_model_tasks = &mut self.skinned_model_tasks;
        let terrain_tasks = &mut self.terrain_tasks;
        let mut models = vec![];
        let mut skinned_models = vec![];
        let mut terrains = vec![];
        for task in model_tasks.iter_mut() {
            let model = task.await?;
            models.push(model);
        }
        for task in skinned_model_tasks.iter_mut() {
            let model = task.await?;
            skinned_models.push(model);
        }
        for task in terrain_tasks.iter_mut() {
            let terrain = task.await?;
            terrains.push(terrain);
        }
        let rm = self.resource_manager.upgrade();
        if rm.is_none() {
            return Err(anyhow::anyhow!(
                "Failed to lock resource manager for waiting tasks."
            ));
        }
        let rm = rm.unwrap();
        let mut lock = rm.write().await;
        for model in models.into_iter() {
            lock.add_model(model);
        }
        for model in skinned_models.into_iter() {
            lock.add_skinned_model(model);
        }
        for terrain in terrains.into_iter() {
            lock.add_terrain(terrain);
        }
        drop(lock);
        drop(rm);
        self.model_tasks.clear();
        self.skinned_model_tasks.clear();
        self.terrain_tasks.clear();
        Ok(())
    }
}
