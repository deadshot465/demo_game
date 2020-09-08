use std::cell::{RefCell};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use winit::{
    event::{
        ElementState,
        Event,
        KeyboardInput,
        VirtualKeyCode,
        WindowEvent,
    },
    event_loop::{
        ControlFlow,
        EventLoop
    },
    window::WindowBuilder
};
use crate::game::{ResourceManager, Camera, SceneManager, GameScene};
use crate::game::graphics::vk::{Graphics, Buffer};
use crate::game::shared::traits::Scene;
use crate::game::traits::Disposable;

pub struct Game<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> {
    pub window: Arc<RwLock<winit::window::Window>>,
    pub resource_manager: Arc<RwLock<ResourceManager<GraphicsType, BufferType>>>,
    pub camera: Arc<RwLock<Camera>>,
    pub graphics: Arc<RwLock<Graphics>>,
    pub scene_manager: SceneManager,
}

impl Game<Graphics, Buffer> {
    pub fn new(title: &str, width: f64, height: f64, event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Arc::new(RwLock::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ResourceManager::new()));
        let graphics = Graphics::new(&window, camera.clone(), resource_manager.clone());
        Game {
            window: Arc::new(RwLock::new(window)),
            resource_manager,
            camera,
            graphics: Arc::new(RwLock::new(graphics)),
            scene_manager: SceneManager::new(),
        }
    }

    pub fn initialize(&mut self) -> bool {
        let game_scene = GameScene::new(
            self.resource_manager.clone(),
            self.graphics.clone()
        );
        self.scene_manager.register_scene(game_scene);
        self.scene_manager.set_current_scene_by_index(0);
        self.scene_manager.initialize();
        true
    }

    pub async fn load_content(&mut self) {
        self.scene_manager.load_content();
        let mut lock = self.graphics.write().unwrap();
        lock.initialize().await;
        drop(lock);
        let lock = self.graphics.read().unwrap();
        lock.begin_draw();
        self.scene_manager.render(0);
        lock.end_draw();
        drop(lock);
    }
}