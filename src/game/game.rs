use std::sync::{Arc, RwLock, atomic::{
    AtomicU32, Ordering
}};
use crossbeam::sync::ShardedLock;
use winit::{
    event_loop::{
        EventLoop
    },
    window::WindowBuilder
};
use crate::game::{ResourceManager, Camera, SceneManager, GameScene};
use crate::game::graphics::vk::{Graphics, Buffer, Image};
use crate::game::traits::Disposable;
use crate::game::shared::traits::GraphicsBase;
use ash::vk::CommandBuffer;
use crate::game::graphics::dx12 as DX12;
use winapi::um::d3d12::ID3D12CommandList;

pub struct Game<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> {
    pub window: Arc<RwLock<winit::window::Window>>,
    pub resource_manager: Arc<RwLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub camera: Arc<RwLock<Camera>>,
    pub graphics: Arc<ShardedLock<GraphicsType>>,
    pub scene_manager: SceneManager,
    current_index: AtomicU32,
}

impl Game<Graphics, Buffer, CommandBuffer, Image> {
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
            graphics: Arc::new(ShardedLock::new(graphics)),
            scene_manager: SceneManager::new(),
            current_index: AtomicU32::new(0),
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
        self.scene_manager.wait_for_all_tasks().await;
        let mut lock = self.graphics.write().unwrap();
        lock.initialize().await;
        drop(lock);
        let lock = self.resource_manager.read().unwrap();
        lock.create_sampler_resource();
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
        let lock = self.graphics.read().unwrap();
        let index = lock.render(self.current_index.load(Ordering::SeqCst));
        self.current_index.store(index, Ordering::SeqCst);
        drop(lock);
    }
}

impl Game<DX12::Graphics, DX12::Resource, ID3D12CommandList, DX12::Resource> {
    pub unsafe fn new(title: &str, width: f64, height: f64, event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Arc::new(RwLock::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ResourceManager::new()));
        let graphics = DX12::Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager));
        Game {
            window: Arc::new(RwLock::new(window)),
            resource_manager,
            camera,
            graphics: Arc::new(ShardedLock::new(graphics)),
            scene_manager: SceneManager::new(),
            current_index: AtomicU32::new(0),
        }
    }

    pub fn initialize(&mut self) -> bool {
        true
    }

    pub async fn load_content(&mut self) {
        ()
    }

    pub fn update(&self) {

    }

    pub fn render(&self) {

    }
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> Drop for Game<GraphicsType, BufferType, CommandType, TextureType> {
    fn drop(&mut self) {
        log::info!("Dropping game...");
    }
}