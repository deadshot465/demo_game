use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::enums::{SceneType, ShaderType};
use crate::game::shared::structs::{PrimitiveType, WaitableTasks};
use crate::game::structs::Model;
use crate::game::traits::{Disposable, GraphicsBase, Scene};
use crate::game::ResourceManager;
use ash::vk::CommandBuffer;
use crossbeam::channel::*;
use glam::f32::{Vec3A, Vec4};
use parking_lot::RwLock;
use slotmap::{DefaultKey, SlotMap};
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

pub struct TitleScene<GraphicsType, BufferType, CommandType, TextureType>
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
    model_count: Arc<AtomicUsize>,
    ssbo_count: AtomicUsize,
    waitable_tasks: WaitableTasks<GraphicsType, BufferType, CommandType, TextureType>,
    scene_type: SceneType,
    entities: std::rc::Weak<RefCell<SlotMap<DefaultKey, usize>>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    TitleScene<GraphicsType, BufferType, CommandType, TextureType>
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
        TitleScene {
            graphics,
            resource_manager,
            scene_name: String::from("TITLE_SCENE"),
            model_count: Arc::new(AtomicUsize::new(0)),
            ssbo_count: AtomicUsize::new(0),
            waitable_tasks: WaitableTasks::new(),
            scene_type: SceneType::Title,
            entities,
        }
    }
}

impl TitleScene<Graphics, Buffer, CommandBuffer, Image> {}

impl Scene for TitleScene<Graphics, Buffer, CommandBuffer, Image> {
    fn initialize(&mut self) {}

    fn get_scene_name(&self) -> &str {
        &self.scene_name
    }

    fn set_scene_name(&mut self, scene_name: &str) {
        self.scene_name = scene_name.to_string();
    }

    fn load_content(&mut self) -> anyhow::Result<()> {
        self.add_model(
            "./models/merkava_tank/scene.gltf",
            Vec3A::new(1.5, 0.0, 1.5),
            Vec3A::new(0.01, 0.01, 0.01),
            Vec3A::new(0.0, 0.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        )?;
        Ok(())
    }

    fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let graphics = self
            .graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle.");
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
            let _ = graphics_lock.render(self.scene_type);
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
            model.update_model_indices(self.model_count.clone());
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
                self.model_count.clone(),
                ssbo_index,
                true,
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
            lock.add_model(self.scene_type, model);
        }
        drop(lock);
        drop(rm);
        self.waitable_tasks.clear();
        Ok(())
    }

    fn get_model_count(&self) -> Arc<AtomicUsize> {
        self.model_count.clone()
    }

    fn get_scene_type(&self) -> SceneType {
        self.scene_type
    }

    fn create_ssbo(&self) -> anyhow::Result<()> {
        let resource_manager = self
            .resource_manager
            .upgrade()
            .expect("Failed to upgrade resource manager handle.");
        let resource_lock = resource_manager.read();
        resource_lock.create_ssbo(self.scene_type)?;
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
