use ash::vk::CommandBuffer;
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Vec3A, Vec4};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use crate::game::enums::ShaderType;
use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{
    GeometricPrimitive, Model, PrimitiveType, SkinnedModel, Terrain,
};
use crate::game::shared::traits::{GraphicsBase, Scene};
use crate::game::shared::util::HeightGenerator;
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
    model_count: Arc<AtomicUsize>,
    ssbo_count: AtomicUsize,
    height_generator: Arc<ShardedLock<HeightGenerator>>,
    model_tasks: Vec<Receiver<Model<GraphicsType, BufferType, CommandType, TextureType>>>,
    skinned_model_tasks:
        Vec<Receiver<SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>>,
    terrain_tasks: Vec<Receiver<Terrain<GraphicsType, BufferType, CommandType, TextureType>>>,
    geometric_primitive_tasks:
        Vec<Receiver<GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>>>,
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
            model_count: Arc::new(AtomicUsize::new(0)),
            height_generator: Arc::new(ShardedLock::new(HeightGenerator::new())),
            model_tasks: vec![],
            skinned_model_tasks: vec![],
            terrain_tasks: vec![],
            geometric_primitive_tasks: vec![],
            ssbo_count: AtomicUsize::new(0),
        }
    }
}

impl GameScene<Graphics, Buffer, CommandBuffer, Image> {
    pub fn generate_terrain(&mut self, grid_x: i32, grid_z: i32) -> anyhow::Result<()> {
        let model_index = self.model_count.fetch_add(1, Ordering::SeqCst);
        let ssbo_index = self.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let mut height_generator = self
            .height_generator
            .write()
            .expect("Failed to lock height generator.");
        let vertex_count = Terrain::<Graphics, Buffer, CommandBuffer, Image>::VERTEX_COUNT;
        height_generator.set_offsets(grid_x, grid_z, vertex_count as i32);
        drop(height_generator);
        let terrain = Terrain::new(
            grid_x,
            grid_z,
            model_index,
            ssbo_index,
            self.graphics.clone(),
            self.height_generator.clone(),
            0.5,
            0.5,
            0.5,
        )?;
        self.terrain_tasks.push(terrain);
        Ok(())
    }
}

impl Scene for GameScene<Graphics, Buffer, CommandBuffer, Image> {
    fn initialize(&mut self) {}

    fn get_scene_name(&self) -> &str {
        self.scene_name.as_str()
    }

    fn set_scene_name(&mut self, scene_name: &str) {
        self.scene_name = scene_name.to_string();
    }

    fn load_content(&mut self) -> anyhow::Result<()> {
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
            Vec3A::new(-3.5, 0.0, 0.0),
            Vec3A::new(2.0, 2.0, 2.0),
            Vec3A::new(0.0, 180.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;
        let water_pos = std::env::var("WATER_POS")?.parse::<f32>()?;
        let water_height = std::env::var("WATER_HEIGHT")?.parse::<f32>()?;
        let water_scale = std::env::var("WATER_SCALE")?.parse::<f32>()?;
        self.add_geometric_primitive(
            PrimitiveType::Rect,
            None,
            Vec3A::new(water_pos, water_height, water_pos),
            Vec3A::new(water_scale, 1.0, water_scale),
            Vec3A::zero(),
            Vec4::new(0.0, 0.0, 1.0, 1.0),
            Some(ShaderType::Water),
        )?;
        self.generate_terrain(0, 0)?;
        Ok(())
    }

    fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let graphics_arc = self.graphics.upgrade().unwrap();
        let mut graphics_lock = graphics_arc.write();
        graphics_lock.update(delta_time)?;
        Ok(())
    }

    fn render(&self, _delta_time: f64) -> anyhow::Result<()> {
        let graphics = self
            .graphics
            .upgrade()
            .expect("Failed to upgrade Weak of Graphics for rendering.");
        {
            let graphics_lock = graphics.read();
            graphics_lock.render()?;
        }
        Ok(())
    }

    fn add_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()> {
        let ssbo_index = self.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            return Err(anyhow::anyhow!("Resource manager has been destroyed."));
        }
        let resource_manager = resource_manager.unwrap();
        let lock = resource_manager.read();
        let item = lock
            .model_queue
            .iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .cloned();
        drop(lock);
        if let Some(m) = item {
            let mut model = (*m.lock()).clone();
            model.set_position(position);
            model.set_scale(scale);
            let x: f32 = rotation.x();
            let y: f32 = rotation.y();
            let z: f32 = rotation.z();
            model.set_rotation(Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()));
            let mut metadata = model.get_model_metadata();
            metadata.world_matrix = model.get_world_matrix();
            metadata.object_color = color;
            model.set_model_metadata(metadata);
            model.set_ssbo_index(ssbo_index);
            let mut lock = resource_manager.write();
            lock.add_clone(model);
            drop(lock);
        } else {
            let task = Model::new(
                file_name,
                self.graphics.clone(),
                position,
                scale,
                rotation,
                color,
                self.model_count.clone(),
                ssbo_index,
            )?;
            self.model_tasks.push(task);
        }
        drop(resource_manager);
        Ok(())
    }

    fn add_skinned_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
    ) -> anyhow::Result<()> {
        let ssbo_index = self.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            return Err(anyhow::anyhow!("Resource manager has been destroyed."));
        }
        let resource_manager = resource_manager.unwrap();
        let lock = resource_manager.read();
        let item = lock
            .model_queue
            .iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .cloned();
        drop(lock);
        if let Some(m) = item {
            let mut model = (*m.lock()).clone();
            model.set_position(position);
            model.set_scale(scale);
            let x: f32 = rotation.x();
            let y: f32 = rotation.y();
            let z: f32 = rotation.z();
            model.set_rotation(Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()));
            let mut metadata = model.get_model_metadata();
            metadata.world_matrix = model.get_world_matrix();
            metadata.object_color = color;
            model.set_model_metadata(metadata);
            model.set_ssbo_index(ssbo_index);
            let mut lock = resource_manager.write();
            lock.add_clone(model);
            drop(lock);
        } else {
            let task = SkinnedModel::new(
                file_name,
                self.graphics.clone(),
                position,
                scale,
                rotation,
                color,
                ssbo_index,
                self.model_count.clone(),
            )?;
            self.skinned_model_tasks.push(task);
        }
        drop(resource_manager);
        Ok(())
    }

    fn add_geometric_primitive(
        &mut self,
        primitive_type: PrimitiveType,
        texture_name: Option<&'static str>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        shader_type: Option<ShaderType>,
    ) -> anyhow::Result<()> {
        let model_index = self.model_count.fetch_add(1, Ordering::SeqCst);
        let ssbo_index = self.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let task = GeometricPrimitive::new(
            self.graphics.clone(),
            primitive_type,
            texture_name,
            model_index,
            ssbo_index,
            position,
            scale,
            rotation,
            color,
            shader_type,
        )?;
        self.geometric_primitive_tasks.push(task);
        Ok(())
    }

    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()> {
        let model_tasks = &mut self.model_tasks;
        let skinned_model_tasks = &mut self.skinned_model_tasks;
        let terrain_tasks = &mut self.terrain_tasks;
        let primitive_tasks = &mut self.geometric_primitive_tasks;
        let mut models = vec![];
        let mut skinned_models = vec![];
        let mut terrains = vec![];
        let mut primitives = vec![];
        for task in model_tasks.iter_mut() {
            let model = task.recv()?;
            models.push(model);
        }
        for task in skinned_model_tasks.iter_mut() {
            let model = task.recv()?;
            skinned_models.push(model);
        }
        for task in terrain_tasks.iter_mut() {
            let terrain = task.recv()?;
            terrains.push(terrain);
        }
        for task in primitive_tasks.iter_mut() {
            let primitive = task.recv()?;
            primitives.push(primitive);
        }
        let rm = self.resource_manager.upgrade();
        if rm.is_none() {
            return Err(anyhow::anyhow!(
                "Failed to lock resource manager for waiting tasks."
            ));
        }
        let rm = rm.unwrap();
        let mut lock = rm.write();
        for model in models.into_iter() {
            lock.add_model(model);
        }
        for model in skinned_models.into_iter() {
            lock.add_model(model);
        }
        for terrain in terrains.into_iter() {
            lock.add_model(terrain);
        }
        for primitive in primitives.into_iter() {
            lock.add_model(primitive);
        }
        drop(lock);
        drop(rm);
        self.model_tasks.clear();
        self.skinned_model_tasks.clear();
        self.terrain_tasks.clear();
        self.geometric_primitive_tasks.clear();
        Ok(())
    }
}
