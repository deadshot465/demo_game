use crate::game::shared::traits::Scene;
use std::cell::RefCell;

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

    pub fn initialize(&self) {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().initialize();
        }
    }

    pub async fn load_content(&self) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().load_content().await?;
        }
        Ok(())
    }

    pub async fn wait_for_all_tasks(&self) {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().wait_for_all_tasks().await;
        }
    }

    pub async fn update(&self, delta_time: f64) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow_mut().update(delta_time).await?;
        }
        Ok(())
    }

    pub async fn render(&self, delta_time: f64, handle: &tokio::runtime::Handle) -> anyhow::Result<()> {
        let current_index = self.current_index;
        if let Some(scene) = self.scenes.get(current_index) {
            scene.borrow().render(delta_time, handle).await?;
        }
        Ok(())
    }

    pub fn register_scene<T>(&mut self, scene: T)
    where
        T: Scene + 'static,
    {
        self.scenes.push(RefCell::new(Box::new(scene)));
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
}
