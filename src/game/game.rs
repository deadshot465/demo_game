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
use crate::game::traits::Disposable;
use crate::game::shared::traits::GraphicsBase;
use ash::vk::CommandBuffer;

pub struct Game<GraphicsType: 'static + GraphicsBase<BufferType, CommandType>, BufferType: 'static + Disposable + Clone, CommandType: 'static> {
    pub window: Arc<RwLock<winit::window::Window>>,
    pub resource_manager: Arc<RwLock<ResourceManager<GraphicsType, BufferType, CommandType>>>,
    pub camera: Arc<RwLock<Camera>>,
    pub graphics: Arc<RwLock<Graphics>>,
    pub scene_manager: SceneManager,
}

impl Game<Graphics, Buffer, CommandBuffer> {
    pub fn new(title: &str, width: f64, height: f64, event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Arc::new(RwLock::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ResourceManager::new()));
        let graphics = Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager));
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
            Arc::downgrade(&self.resource_manager),
            Arc::downgrade(&self.graphics)
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

    pub fn update(&self) {
        /*let mut lock = self.graphics.write().unwrap();
        lock.update();
        self.scene_manager.update(0);
        drop(lock);*/
    }

    pub fn render(&self) {
        let mut lock = self.graphics.write().unwrap();
        let index = lock.render();
        lock.current_index = index;
        drop(lock);
    }
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType>, BufferType: 'static + Disposable + Clone, CommandType: 'static> Drop for Game<GraphicsType, BufferType, CommandType> {
    fn drop(&mut self) {
        log::info!("Dropping game...");
    }
}