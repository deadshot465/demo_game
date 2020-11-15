use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use glam::{Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use slotmap::{DefaultKey, SlotMap};
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use crate::game::enums::ShaderType;
use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::enums::SceneType;
use crate::game::shared::structs::{
    Counts, GeometricPrimitive, InstanceData, InstancedModel, Model, PositionInfo, PrimitiveType,
    SkinnedModel, Terrain, WaitableTasks,
};
use crate::game::shared::traits::{GraphicsBase, Renderable, Scene};
use crate::game::shared::util::HeightGenerator;
use crate::game::traits::Disposable;
use crate::game::ResourceManager;
use std::collections::HashMap;

pub struct GameScene<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
    resource_manager: Weak<
        RwLock<ManuallyDrop<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    >,
    scene_name: String,
    counts: Counts,
    height_generator: Arc<ShardedLock<HeightGenerator>>,
    scene_type: SceneType,
    entities: std::rc::Weak<RefCell<SlotMap<DefaultKey, usize>>>,
    current_entities: HashMap<String, DefaultKey>,
    render_components: Vec<
        Arc<Mutex<Box<dyn Renderable<GraphicsType, BufferType, CommandType, TextureType> + Send>>>,
    >,
    waitable_tasks: WaitableTasks<GraphicsType, BufferType, CommandType, TextureType>,
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
            RwLock<
                ManuallyDrop<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>,
            >,
        >,
        graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
        entities: std::rc::Weak<RefCell<SlotMap<DefaultKey, usize>>>,
    ) -> Self {
        GameScene {
            graphics,
            resource_manager,
            scene_name: String::from("GAME_SCENE"),
            counts: Counts::new(),
            height_generator: Arc::new(ShardedLock::new(HeightGenerator::new())),
            waitable_tasks: WaitableTasks::new(),
            scene_type: SceneType::Game,
            entities,
            current_entities: HashMap::new(),
            render_components: Vec::new(),
        }
    }

    fn add_entity(&mut self, entity_name: &str) -> DefaultKey {
        let entities = self
            .entities
            .upgrade()
            .expect("Failed to upgrade entities handle.");
        self.counts.entity_count += 1;
        let mut entities_lock = entities.borrow_mut();
        let entity = entities_lock.insert(self.counts.entity_count);
        self.current_entities
            .insert(entity_name.to_string(), entity);
        entity
    }
}

impl GameScene<Graphics, Buffer, CommandBuffer, Image> {
    pub fn generate_terrain(
        &mut self,
        grid_x: i32,
        grid_z: i32,
        entity: DefaultKey,
    ) -> anyhow::Result<()> {
        let model_index = self.counts.model_count.fetch_add(1, Ordering::SeqCst);
        let ssbo_index = self.counts.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let mut height_generator = self
            .height_generator
            .write()
            .expect("Failed to lock height generator.");
        let vertex_count = Terrain::<Graphics, Buffer, CommandBuffer, Image>::VERTEX_COUNT;
        height_generator.set_offsets(grid_x as i32, grid_z as i32, vertex_count as i32);
        drop(height_generator);
        let ratio = std::env::var("RATIO").unwrap().parse::<f32>().unwrap();
        let terrain = Terrain::new(
            grid_x,
            grid_z,
            model_index,
            ssbo_index,
            self.graphics.clone(),
            self.height_generator.clone(),
            ratio,
            ratio,
            ratio,
            entity,
        )?;
        self.waitable_tasks.terrain_tasks.push(terrain);
        Ok(())
    }

    pub fn add_instanced_model(
        &mut self,
        file_name: &'static str,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        instance_count: usize,
        entity: DefaultKey,
    ) -> anyhow::Result<()> {
        let ssbo_index = self.counts.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let mut instance_data = vec![];
        instance_data.resize(instance_count, InstanceData::default());
        let mut x_offset = 0.0;
        let mut z_offset = 0.0;
        for (index, data) in instance_data.iter_mut().enumerate() {
            if index % 25 == 0 {
                z_offset += 25.0;
                x_offset = 0.0;
            }
            (*data).translation = Vec3A::new(x_offset, 0.0, z_offset);
            (*data).rotation = Vec3A::zero();
            (*data).scale = Vec3A::one();
            x_offset += 25.0;
        }
        let task = InstancedModel::new(
            file_name,
            self.graphics.clone(),
            position,
            scale,
            rotation,
            color,
            self.counts.model_count.clone(),
            ssbo_index,
            instance_data,
            entity,
        )?;
        self.waitable_tasks.instanced_model_tasks.push(task);
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
        let ssbo_index = self.counts.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            return Err(anyhow::anyhow!("Resource manager has been destroyed."));
        }
        let resource_manager = resource_manager.unwrap();
        let mut lock = resource_manager.write();
        let current_model_queue = lock
            .model_queue
            .entry(self.scene_type)
            .or_insert_with(Vec::new);
        let item = current_model_queue
            .iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .cloned();
        drop(lock);
        if let Some(m) = item {
            let mut model = (*m.lock()).clone();
            let x: f32 = rotation.x();
            let y: f32 = rotation.y();
            let z: f32 = rotation.z();
            model.set_position_info(PositionInfo {
                position,
                scale,
                rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
            });
            let mut metadata = model.get_model_metadata();
            metadata.world_matrix = model.get_world_matrix();
            metadata.object_color = color;
            model.set_model_metadata(metadata);
            model.set_ssbo_index(ssbo_index);
            model.update_model_indices(self.counts.model_count.clone());
            let mut lock = resource_manager.write();
            lock.add_clone(self.scene_type, model);
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
                self.counts.model_count.clone(),
            )?;
            self.waitable_tasks.skinned_model_tasks.push(task);
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
        entity: DefaultKey,
    ) -> anyhow::Result<()> {
        let model_index = self.counts.model_count.fetch_add(1, Ordering::SeqCst);
        let ssbo_index = self.counts.ssbo_count.fetch_add(1, Ordering::SeqCst);
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
            entity,
        )?;
        self.waitable_tasks.geometric_primitive_tasks.push(task);
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
        let mr_incredible = self.add_entity("Mr.Incredible");
        self.add_model(
            "./models/mr.incredible/Mr.Incredible.glb",
            Vec3A::new(-5.0, 0.0, 5.0),
            Vec3A::new(1.0, 1.0, 1.0),
            Vec3A::new(0.0, 0.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            mr_incredible,
        )?;
        let bison = self.add_entity("Bison");
        self.add_model(
            "./models/bison/output.gltf",
            Vec3A::new(0.0, 0.0, 5.0),
            Vec3A::new(400.0, 400.0, 400.0),
            Vec3A::new(0.0, 90.0, 90.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            bison,
        )?;
        self.add_skinned_model(
            "./models/cesiumMan/CesiumMan.glb",
            Vec3A::new(5.0, 0.0, 5.0),
            Vec3A::new(2.0, 2.0, 2.0),
            Vec3A::new(0.0, 180.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;
        //let water_pos = std::env::var("WATER_POS")?.parse::<f32>()?;
        //let water_height = std::env::var("WATER_HEIGHT")?.parse::<f32>()?;
        //let water_scale = std::env::var("WATER_SCALE")?.parse::<f32>()?;
        /*self.add_geometric_primitive(
            PrimitiveType::Rect,
            None,
            Vec3A::new(water_pos, water_height, water_pos),
            Vec3A::new(water_scale, 1.0, water_scale),
            Vec3A::zero(),
            Vec4::new(0.0, 0.0, 1.0, 1.0),
            Some(ShaderType::Water),
        )?;*/
        let instance_count = std::env::var("INSTANCE_COUNT")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        if instance_count > 0 {
            let dragon = self.add_entity("Dragon");
            self.add_instanced_model(
                "models/stanford_dragon/stanford-dragon.glb",
                Vec3A::new(0.0, 5.0, 0.0),
                Vec3A::one(),
                Vec3A::zero(),
                Vec4::new(0.72, 0.43, 0.47, 1.0),
                instance_count,
                dragon,
            )?;
        }
        //self.generate_terrain(0, 0)?;
        Ok(())
    }

    fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let graphics = self.graphics.upgrade().unwrap();
        let mut graphics_lock = graphics.write();
        graphics_lock.update(delta_time, self.scene_type)?;
        Ok(())
    }

    fn render(&self, _delta_time: f64) -> anyhow::Result<()> {
        let graphics = self
            .graphics
            .upgrade()
            .expect("Failed to upgrade Weak of Graphics for rendering.");
        {
            let graphics_lock = graphics.read();
            let _ = graphics_lock.render(&self.render_components);
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
        entity: DefaultKey,
    ) -> anyhow::Result<()> {
        let ssbo_index = self.counts.ssbo_count.fetch_add(1, Ordering::SeqCst);
        let resource_manager = self.resource_manager.upgrade();
        if resource_manager.is_none() {
            return Err(anyhow::anyhow!("Resource manager has been destroyed."));
        }
        let resource_manager = resource_manager.unwrap();
        let mut lock = resource_manager.write();
        let current_model_queue = lock
            .model_queue
            .entry(self.scene_type)
            .or_insert_with(Vec::new);
        let item = current_model_queue
            .iter()
            .find(|m| (*m).lock().get_name() == file_name)
            .cloned();
        drop(lock);
        if let Some(m) = item {
            let mut model = (*m.lock()).clone();
            let x: f32 = rotation.x();
            let y: f32 = rotation.y();
            let z: f32 = rotation.z();
            model.set_position_info(PositionInfo {
                position,
                scale,
                rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
            });
            let mut metadata = model.get_model_metadata();
            metadata.world_matrix = model.get_world_matrix();
            metadata.object_color = color;
            model.set_model_metadata(metadata);
            model.set_ssbo_index(ssbo_index);
            model.update_model_indices(self.counts.model_count.clone());
            let mut lock = resource_manager.write();
            lock.add_clone(self.scene_type, model);
            drop(lock);
        } else {
            let task = Model::new(
                file_name,
                self.graphics.clone(),
                position,
                scale,
                rotation,
                color,
                self.counts.model_count.clone(),
                ssbo_index,
                true,
                entity,
            )?;
            self.waitable_tasks.model_tasks.push(task);
        }
        drop(resource_manager);
        Ok(())
    }

    fn wait_for_all_tasks(&mut self) -> anyhow::Result<()> {
        let completed_tasks = self.waitable_tasks.wait_for_all_tasks()?;
        let rm = self.resource_manager.upgrade();
        if rm.is_none() {
            return Err(anyhow::anyhow!(
                "Failed to lock resource manager for waiting tasks."
            ));
        }
        let rm = rm.unwrap();
        let mut lock = rm.write();

        for model in completed_tasks.models.into_iter() {
            self.render_components
                .push(lock.add_model(self.scene_type, model));
        }
        for model in completed_tasks.skinned_models.into_iter() {
            self.render_components
                .push(lock.add_model(self.scene_type, model));
        }
        for terrain in completed_tasks.terrains.into_iter() {
            self.render_components
                .push(lock.add_model(self.scene_type, terrain));
        }
        for primitive in completed_tasks.geometric_primitives.into_iter() {
            self.render_components
                .push(lock.add_model(self.scene_type, primitive));
        }
        for instance in completed_tasks.instances.into_iter() {
            self.render_components
                .push(lock.add_model(self.scene_type, instance));
        }
        drop(lock);
        drop(rm);
        self.waitable_tasks.clear();
        Ok(())
    }

    fn get_model_count(&self) -> Arc<AtomicUsize> {
        self.counts.model_count.clone()
    }

    fn get_scene_type(&self) -> SceneType {
        self.scene_type
    }

    fn create_ssbo(&self) -> anyhow::Result<()> {
        for renderable in self.render_components.iter() {
            renderable.lock().create_ssbo()?;
        }
        Ok(())
    }

    fn get_command_buffers(&self) {
        let resource_manager = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade resource manager handle.");
        let mut resource_lock = resource_manager.write();
        resource_lock.get_all_command_buffers(self.scene_type);
    }
}
