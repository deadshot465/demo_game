use ash::vk::{CommandBuffer, SamplerAddressMode};
use async_trait::async_trait;
use crossbeam::sync::ShardedLock;
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
    model_tasks: Vec<JoinHandle<Model<GraphicsType, BufferType, CommandType, TextureType>>>,
    skinned_model_tasks:
        Vec<JoinHandle<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>>,
    model_count: usize,
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
        }
    }
}

impl GameScene<Graphics, Buffer, CommandBuffer, Image> {
    pub async fn generate_terrain(&mut self) -> anyhow::Result<()> {
        let graphics = self
            .graphics
            .upgrade()
            .expect("Failed to upgrade the weak pointer of Graphics.");
        let command_pool = graphics
            .read()
            .await
            .thread_pool
            .get_idle_command_pool();
        let image = Graphics::create_image_from_file(
            "textures/TexturesCom_Grass0150_1_seamless_S.jpg",
            graphics.clone(),
            command_pool,
            SamplerAddressMode::REPEAT,
        )
        .await?;
        log::info!("Terrain texture successfully created.");
        let resource_manager = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade Weak of resource manager for creating terrain.");
        let texture_index: usize;
        let model_index = self.model_count;
        {
            let resource_lock = resource_manager.read().await;
            texture_index = resource_lock.get_texture_count() - 1;
        }
        let mut terrain = Terrain::new(
            0,
            0,
            texture_index,
            image,
            graphics.clone(),
        );
        terrain.model.model_index = model_index;
        {
            let graphics_lock = graphics.read().await;
            let thread_count = graphics_lock.thread_pool.thread_count;
            let command_pool = graphics_lock.thread_pool
                .threads[model_index % thread_count]
                .command_pool
                .clone();
            let command_buffer = graphics_lock
                .create_secondary_command_buffer(command_pool.clone());
            terrain.model.meshes[0].command_pool = Some(command_pool);
            terrain.model.meshes[0].command_buffer = Some(command_buffer);
        }
        terrain.create_buffers().await?;
        {
            let mut resource_lock = resource_manager
                .write()
                .await;
            resource_lock.add_terrain(terrain);
        }
        self.model_count += 1;
        log::info!("Terrain successfully generated.");
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
        )?;*/
        /*self.add_model("./models/tank/tank.gltf", Vec3A::new(1.5, 0.0, 1.5),
        Vec3A::new(1.0, 1.0, 1.0), Vec3A::new(90.0, 90.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 1.0));*/
        /*self.add_model(
            "./models/mr.incredible/Mr.Incredible.glb",
            Vec3A::new(0.0, 0.0, 0.0),
            Vec3A::new(1.0, 1.0, 1.0),
            Vec3A::new(0.0, 0.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;
        self.add_model(
            "./models/bison/output.gltf",
            Vec3A::new(0.0, 0.0, 0.0),
            Vec3A::new(400.0, 400.0, 400.0),
            Vec3A::new(0.0, 90.0, 90.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;
        self.add_skinned_model(
            "./models/cesiumMan/CesiumMan.glb",
            Vec3A::new(-1.5, 0.0, -1.5),
            Vec3A::new(2.0, 2.0, 2.0),
            Vec3A::new(0.0, 180.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;*/
        self.generate_terrain().await?;
        Ok(())
    }

    async fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let graphics_arc = self.graphics.upgrade().unwrap();
        let mut graphics_lock = graphics_arc.write().await;
        graphics_lock.update(delta_time).await?;
        Ok(())
    }

    async fn render(&self, _delta_time: f64, handle: &tokio::runtime::Handle) -> anyhow::Result<()> {
        let graphics = self.graphics.upgrade()
            .expect("Failed to upgrade Weak of Graphics for rendering.");
        {
            let mut graphics_lock = graphics.write().await;
            graphics_lock.render(handle).await?;
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
        let lock = resource_manager
            .read()
            .await;
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
            ).await?;
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
        let lock = resource_manager
            .read()
            .await;
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
            ).await?;
            self.skinned_model_tasks.push(task);
        }
        self.model_count += 1;
        drop(resource_manager);
        Ok(())
    }

    async fn wait_for_all_tasks(&mut self) {
        let model_tasks = &mut self.model_tasks;
        let skinned_model_tasks = &mut self.skinned_model_tasks;
        let mut models = vec![];
        let mut skinned_models = vec![];
        for task in model_tasks.iter_mut() {
            let model = task.await.unwrap();
            models.push(model);
        }
        for task in skinned_model_tasks.iter_mut() {
            let model = task.await.unwrap();
            skinned_models.push(model);
        }
        let rm = self.resource_manager.upgrade();
        if rm.is_none() {
            log::error!("Failed to lock resource manager for waiting tasks.");
            return;
        }
        let rm = rm.unwrap();
        let mut lock = rm.write().await;
        for model in models.into_iter() {
            lock.add_model(model);
        }
        for model in skinned_models.into_iter() {
            lock.add_skinned_model(model);
        }
        drop(lock);
        drop(rm);
        self.model_tasks.clear();
    }
}
