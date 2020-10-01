use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use std::sync::{Arc, atomic::{
    AtomicU32
}};
use std::time;
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
    last_frame_time: time::Instant,
    current_time: time::Instant,
    frame_count: u32,
}

impl Game<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(title: &str, width: f64, height: f64, event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Arc::new(ShardedLock::new(Camera::new(width, height)));
        let resource_manager = Arc::new(ShardedLock::new(ResourceManager::new()));
        let graphics = Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager))?;
        Ok(Game {
            window: Arc::new(ShardedLock::new(window)),
            resource_manager,
            camera,
            graphics: Arc::new(ShardedLock::new(graphics)),
            scene_manager: SceneManager::new(),
            current_index: AtomicU32::new(0),
            last_frame_time: time::Instant::now(),
            current_time: time::Instant::now(),
            frame_count: 0,
        })
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

    pub async fn load_content(&mut self) -> anyhow::Result<()> {
        self.scene_manager.load_content()?;
        self.scene_manager.wait_for_all_tasks().await;
        let mut lock = self.graphics.write().unwrap();
        lock.initialize().await?;
        drop(lock);
        let mut resource_lock = self.resource_manager.write().unwrap();
        resource_lock.create_sampler_resource();
        resource_lock.create_ssbo().await;
        drop(resource_lock);
        Ok(())
    }

    pub fn update(&mut self) -> anyhow::Result<()> {
        let delta_time = self.current_time.elapsed().as_secs_f64();
        self.scene_manager.update(delta_time)?;
        self.current_time = time::Instant::now();
        self.frame_count += 1;
        let elapsed = self.last_frame_time.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            self.window.read().unwrap().set_title(&format!("Demo Game / FPS: {}", self.frame_count));
            self.frame_count = 0;
            self.last_frame_time = time::Instant::now();
        }
        Ok(())
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        let lock = self.graphics.read().unwrap();
        self.current_index.store(lock.render()?, std::sync::atomic::Ordering::SeqCst);
        drop(lock);
        Ok(())
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
            current_time: time::Instant::now(),
            last_frame_time: time::Instant::now(),
            frame_count: 0,
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