use crate::game::shared::enums::SceneType;
use crate::game::shared::traits::Scene;
use std::cell::RefCell;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

pub struct SceneManager {
    pub current_index: usize,
    scenes: Vec<RefCell<Box<dyn Scene + 'static>>>,
}

impl Default for SceneManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneManager {
    pub fn new() -> Self {
        SceneManager {
            current_index: 0,
            scenes: vec![],
        }
    }

    pub fn create_ssbo(&self) -> anyhow::Result<()> {
        let current_index = self.current_index;
        self.scenes
            .get(current_index)
            .expect("Failed to get current scene.")
            .borrow()
            .create_ssbo()?;
        Ok(())
    }

    pub fn get_command_buffers(&self) {
        let current_index = self.current_index;
        self.scenes
            .get(current_index)
            .expect("Failed to get current scene.")
            .borrow()
            .get_command_buffers();
    }

    pub fn get_scene_model_count(&self) -> Arc<AtomicUsize> {
        let current_index = self.current_index;
        self.scenes
            .get(current_index)
            .expect("Failed to get current scene.")
            .borrow()
            .get_model_count()
    }

    pub fn initialize(&self) {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().initialize();
        }
    }

    pub fn load_content(&self) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().load_content()?;
        }
        Ok(())
    }

    pub fn register_scene<T>(&mut self, scene: T) -> usize
    where
        T: Scene + 'static,
    {
        let index = self.scenes.len();
        self.scenes.push(RefCell::new(Box::new(scene)));
        index
    }

    pub fn render(&self, delta_time: f64) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow().render(delta_time)?;
        }
        Ok(())
    }

    pub fn set_current_scene_by_index(&mut self, index: usize) {
        self.current_index = index;
    }

    pub fn set_current_scene_by_name(&mut self, name: &str) {
        let mut index = 0_usize;
        let mut found = false;
        let _ = self.scenes.iter().enumerate().find(|s| {
            if (*s).1.borrow().get_scene_name() == name {
                index = (*s).0;
                found = true;
                true
            } else {
                false
            }
        });
        if found {
            self.current_index = index;
        }
    }

    pub fn switch_scene(&mut self, index: usize) {
        self.set_current_scene_by_index(index);
        self.initialize();
    }

    pub fn update(&self, delta_time: f64) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().update(delta_time)?;
        }
        Ok(())
    }

    pub fn wait_for_all_tasks(&self) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().wait_for_all_tasks()?;
        }
        Ok(())
    }
}
