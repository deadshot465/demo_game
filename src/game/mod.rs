pub mod graphics;
pub mod scenes;
pub mod shared;
pub use scenes::*;
pub use shared::*;

use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use parking_lot::RwLock;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use winapi::um::d3d12::ID3D12GraphicsCommandList;
use winit::{event_loop::EventLoop, window::WindowBuilder};
#[cfg(target_os = "windows")]
use wio::com::ComPtr;

#[cfg(target_os = "windows")]
use crate::game::graphics::dx12 as DX12;
use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::traits::GraphicsBase;
use crate::game::traits::Disposable;
use crate::game::{Camera, GameScene, ResourceManager, SceneManager};

pub struct Game<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub window: Arc<ShardedLock<winit::window::Window>>,
    pub resource_manager:
        Arc<RwLock<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>,
    pub camera: Rc<RefCell<Camera>>,
    pub graphics: Arc<RwLock<GraphicsType>>,
    pub scene_manager: SceneManager,
}

impl Game<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        title: &str,
        width: f64,
        height: f64,
        event_loop: &EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Rc::new(RefCell::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ResourceManager::new()));
        let graphics = Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager))?;
        Ok(Game {
            window: Arc::new(ShardedLock::new(window)),
            resource_manager,
            camera,
            graphics: Arc::new(RwLock::new(graphics)),
            scene_manager: SceneManager::new(),
        })
    }

    pub fn initialize(&mut self) -> bool {
        let game_scene = GameScene::new(
            Arc::downgrade(&self.resource_manager),
            Arc::downgrade(&self.graphics),
        );
        self.scene_manager.register_scene(game_scene);
        self.scene_manager.set_current_scene_by_index(0);
        self.scene_manager.initialize();
        true
    }

    pub fn load_content(&mut self) -> anyhow::Result<()> {
        self.scene_manager.load_content()?;
        self.scene_manager.wait_for_all_tasks()?;
        let mut lock = self.graphics.write();
        lock.initialize()?;
        drop(lock);
        let mut lock = self.resource_manager.write();
        lock.create_ssbo()?;
        lock.get_all_command_buffers();
        drop(lock);
        Ok(())
    }

    pub fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        self.scene_manager.update(delta_time)?;
        Ok(())
    }

    pub fn render(&mut self, delta_time: f64) -> anyhow::Result<()> {
        self.scene_manager.render(delta_time)?;
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
        let camera = Rc::new(RefCell::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ResourceManager::new()));
        let graphics =
            DX12::Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager));
        Game {
            window: Arc::new(ShardedLock::new(window)),
            resource_manager,
            camera,
            graphics: Arc::new(RwLock::new(graphics)),
            scene_manager: SceneManager::new(),
        }
    }

    pub fn initialize(&mut self) -> bool {
        true
    }

    pub fn load_content(&mut self) {}

    pub fn update(&self) {}

    pub fn render(&self) {}
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for Game<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        log::info!("Dropping game...");
        let initialized = self.graphics.read().is_initialized();
        if initialized {
            self.graphics.write().set_disposing();
            unsafe {
                self.graphics.read().wait_idle();
            }
        }
    }
}
