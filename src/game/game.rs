use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use std::sync::{Arc, atomic::{
    AtomicU32
}};
#[cfg(target_os = "windows")]
use winapi::um::d3d12::ID3D12GraphicsCommandList;
use winit::{
    event_loop::{
        EventLoop
    },
    window::WindowBuilder
};
#[cfg(target_os = "windows")]
use wio::com::ComPtr;

use crate::game::{ResourceManager, Camera, SceneManager, GameScene};
#[cfg(target_os = "windows")]
use crate::game::graphics::dx12 as DX12;
use crate::game::graphics::vk::{Graphics, Buffer, Image};
use crate::game::shared::traits::GraphicsBase;
use crate::game::traits::Disposable;

pub struct Game<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    pub window: Arc<ShardedLock<winit::window::Window>>,
    pub resource_manager: Arc<ShardedLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub camera: Arc<ShardedLock<Camera>>,
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
        let camera = Arc::new(ShardedLock::new(Camera::new(width, height)));
        let resource_manager = Arc::new(ShardedLock::new(ResourceManager::new()));
        let graphics = Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager));
        Game {
            window: Arc::new(ShardedLock::new(window)),
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
    }

    pub fn update(&self) {
        /*let mut lock = self.graphics.write().unwrap();
        lock.update();
        self.scene_manager.update(0);
        drop(lock);*/
    }

    pub fn render(&mut self) {
        let lock = self.graphics.read().unwrap();
        self.current_index.store(lock.render(), std::sync::atomic::Ordering::SeqCst);
        drop(lock);
    }
}

#[cfg(target_os = "windows")]
impl Game<DX12::Graphics, DX12::Resource, ComPtr<ID3D12GraphicsCommandList>, DX12::Resource> {
    pub unsafe fn new(title: &str, width: f64, height: f64, event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Arc::new(ShardedLock::new(Camera::new(width, height)));
        let resource_manager = Arc::new(ShardedLock::new(ResourceManager::new()));
        let graphics = DX12::Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager));
        Game {
            window: Arc::new(ShardedLock::new(window)),
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

impl<GraphicsType, BufferType, CommandType, TextureType> Drop for Game<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn drop(&mut self) {
        log::info!("Dropping game...");
    }
}