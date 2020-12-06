pub mod graphics;
pub mod scenes;
pub mod shared;
pub mod ui;
pub use scenes::*;
pub use shared::*;
pub use ui::*;

use ash::vk::CommandBuffer;
use parking_lot::RwLock;
use slotmap::{DefaultKey, SlotMap};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
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
use crate::game::scenes::title_scene::TitleScene;
use crate::game::shared::enums::SceneType;
use crate::game::shared::traits::GraphicsBase;
use crate::game::shared::util::get_random_string;
use crate::game::traits::Disposable;
use crate::game::{Camera, GameScene, ResourceManager, SceneManager};
use rand::prelude::IteratorRandom;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, VirtualKeyCode};

pub struct Game<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub window: Rc<RefCell<winit::window::Window>>,
    pub camera: Rc<RefCell<Camera>>,
    pub graphics: Arc<RwLock<ManuallyDrop<GraphicsType>>>,
    pub scene_manager: SceneManager,
    pub ui_system: UISystemHandle<GraphicsType, BufferType, CommandType, TextureType>,
    pub current_scene: SceneType,
    resource_manager: ResourceManagerHandle<GraphicsType, BufferType, CommandType, TextureType>,
    entities: Rc<RefCell<SlotMap<DefaultKey, usize>>>,
    network_system: Arc<tokio::sync::RwLock<NetworkSystem>>,
    scenes: HashMap<SceneType, usize>,
    room_state_receiver: Option<crossbeam::channel::Receiver<bool>>,
}

impl Game<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        title: &str,
        width: f64,
        height: f64,
        event_loop: &EventLoop<()>,
        network_system: NetworkSystem,
    ) -> anyhow::Result<Self> {
        let window = Rc::new(RefCell::new(
            WindowBuilder::new()
                .with_title(title)
                .with_inner_size(winit::dpi::LogicalSize::new(width, height))
                .with_resizable(false)
                .build(event_loop)
                .expect("Failed to create window."),
        ));
        let camera = Rc::new(RefCell::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ManuallyDrop::new(ResourceManager::new())));
        let graphics = Graphics::new(
            std::rc::Rc::downgrade(&window),
            camera.clone(),
            Arc::downgrade(&resource_manager),
        )?;
        Ok(Game {
            window,
            resource_manager,
            camera,
            graphics: Arc::new(RwLock::new(ManuallyDrop::new(graphics))),
            scene_manager: SceneManager::new(),
            ui_system: None,
            entities: Rc::new(RefCell::new(SlotMap::new())),
            network_system: Arc::new(tokio::sync::RwLock::new(network_system)),
            scenes: HashMap::new(),
            current_scene: SceneType::TITLE,
            room_state_receiver: None,
        })
    }

    pub fn end_input(&self) {
        if let Some(ui) = self.ui_system.as_ref() {
            ui.borrow_mut().end_input();
        }
    }

    pub fn initialize(&mut self) -> bool {
        let title_scene = TitleScene::new(
            Arc::downgrade(&self.resource_manager),
            Arc::downgrade(&self.graphics),
            Rc::downgrade(&self.entities),
        );
        let game_scene = GameScene::new(
            Arc::downgrade(&self.resource_manager),
            Arc::downgrade(&self.graphics),
            Rc::downgrade(&self.entities),
            Arc::downgrade(&self.network_system),
        );
        let title_scene_index = self.scene_manager.register_scene(title_scene);
        let game_scene_index = self.scene_manager.register_scene(game_scene);
        self.scene_manager.switch_scene(title_scene_index);
        self.scenes.insert(SceneType::TITLE, title_scene_index);
        self.scenes.insert(SceneType::GAME, game_scene_index);
        true
    }

    pub fn input_button(&self, button: MouseButton, x: f64, y: f64, element_state: ElementState) {
        if let Some(ui) = self.ui_system.as_ref() {
            ui.borrow_mut().input_button(button, x, y, element_state);
        }
    }

    pub fn input_key(&self, key: VirtualKeyCode, element_state: ElementState) {
        if let Some(ui) = self.ui_system.as_ref() {
            ui.borrow_mut().input_key(key, element_state);
        }
    }

    pub fn input_motion(&self, x: f64, y: f64) {
        if let Some(ui) = self.ui_system.as_ref() {
            ui.borrow_mut().input_motion(x, y);
        }
    }

    pub fn input_scroll(&self, mouse_scroll_delta: MouseScrollDelta) {
        if let Some(ui) = self.ui_system.as_ref() {
            ui.borrow_mut().input_scroll(mouse_scroll_delta);
        }
    }

    pub fn input_unicode(&self, c: char) {
        if let Some(ui) = self.ui_system.as_ref() {
            ui.borrow_mut().input_unicode(c);
        }
    }

    pub async fn load_content(&mut self) -> anyhow::Result<()> {
        self.scene_manager.load_content().await?;
        self.scene_manager.wait_for_all_tasks()?;
        if self.ui_system.is_none() {
            let graphics_lock = self.graphics.read();
            let ui_manager = Rc::new(RefCell::new(ManuallyDrop::new(UISystem::new(
                &*graphics_lock,
            ))));
            drop(graphics_lock);
            let mut graphics_lock = self.graphics.write();
            graphics_lock.ui_manager = Some(Rc::downgrade(&ui_manager));
            self.ui_system = Some(ui_manager);
        }

        {
            let PhysicalSize { width, height } = self.window.borrow().inner_size();
            let mut graphics_lock = self.graphics.write();
            let is_initialized = graphics_lock.is_initialized();
            if !is_initialized {
                graphics_lock.initialize_scene_resource(self.current_scene, false)?;
                graphics_lock.initialize_pipelines()?;
            } else {
                graphics_lock.recreate_swapchain(width, height, self.current_scene)?;
            }
        }

        self.scene_manager.create_ssbo()?;
        self.scene_manager.get_command_buffers();

        Ok(())
    }

    pub fn render(&mut self, delta_time: f64) -> anyhow::Result<()> {
        self.scene_manager.render(delta_time)?;
        Ok(())
    }

    pub fn start_input(&self) {
        if let Some(ui) = self.ui_system.as_ref() {
            let mut borrowed = ui.borrow_mut();
            borrowed.start_input();
        }
    }

    pub async fn update(&mut self, delta_time: f64) -> anyhow::Result<()> {
        let old_scene = self.current_scene;
        let mut new_scene = self.current_scene;
        if let Some(ui_system) = self.ui_system.as_ref() {
            let mut borrowed = ui_system.borrow_mut();
            match old_scene {
                SceneType::TITLE => {
                    let player = borrowed.draw_title_ui(self.network_system.clone()).await?;
                    if let Some(p) = player {
                        log::info!("Successfully logged in as {}.", &p.email);
                        new_scene = SceneType::GAME;
                    }
                }
                SceneType::GAME => borrowed.draw_game_ui(self.network_system.clone()).await?,
                _ => (),
            }
        }

        let load_game = if let Some(recv) = self.room_state_receiver.as_ref() {
            recv.try_recv().is_ok()
        } else {
            false
        };
        if load_game {
            let is_owner = {
                let ns = self.network_system.read().await;
                if let Some(player) = ns.logged_user.as_ref() {
                    if let Some(state) = player.state.as_ref() {
                        state.is_owner
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            {
                let mut ns = self.network_system.write().await;
                let entity = self.scene_manager.add_entity("GameTerrain");
                if is_owner {
                    let primitive = self.scene_manager.generate_terrain(0, 0, None, entity)?;
                    ns.start_game(primitive).await?;
                } else {
                    self.scene_manager.generate_terrain(
                        0,
                        0,
                        Some(ns.get_terrain().await?),
                        entity,
                    )?;
                }
                ns.progress_game().await?;
            }
            self.load_content().await?;
        }

        if old_scene != new_scene {
            let receiver = match new_scene {
                SceneType::GAME => {
                    let mut network_system = self.network_system.write().await;
                    let rooms = network_system.get_rooms().await?;
                    if rooms.is_empty() {
                        let room_id = get_random_string(7);
                        Some(
                            network_system
                                .register_player(room_id, "Test Room".into(), true)
                                .await?,
                        )
                    } else {
                        let available_rooms = rooms
                            .iter()
                            .filter(|r| !r.started && r.current_players < r.max_players)
                            .collect::<Vec<_>>();
                        let randomly_selected_room = {
                            let mut rng = rand::thread_rng();
                            available_rooms.iter().choose(&mut rng)
                        };

                        let room = randomly_selected_room.expect("Failed to get available room.");

                        Some(
                            network_system
                                .register_player(
                                    room.room_id.clone(),
                                    room.room_name.clone(),
                                    false,
                                )
                                .await?,
                        )
                    }
                }
                _ => None,
            };

            self.room_state_receiver = receiver;
            self.switch_scene(new_scene).await?;
        }

        self.scene_manager.update(delta_time).await?;
        Ok(())
    }

    async fn switch_scene(&mut self, scene_type: SceneType) -> anyhow::Result<()> {
        self.current_scene = scene_type;
        let scene_index = self
            .scenes
            .get(&scene_type)
            .expect("Failed to get scene index.");
        self.scene_manager.switch_scene(*scene_index);
        if scene_type != SceneType::GAME {
            self.load_content().await
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
impl Game<DX12::Graphics, DX12::Resource, ComPtr<ID3D12GraphicsCommandList>, DX12::Resource> {
    pub unsafe fn new(
        title: &str,
        width: f64,
        height: f64,
        event_loop: &EventLoop<()>,
        network_system: NetworkSystem,
    ) -> Self {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window.");
        let camera = Rc::new(RefCell::new(Camera::new(width, height)));
        let resource_manager = Arc::new(RwLock::new(ManuallyDrop::new(ResourceManager::new())));
        let graphics =
            DX12::Graphics::new(&window, camera.clone(), Arc::downgrade(&resource_manager));
        Game {
            window: Rc::new(RefCell::new(window)),
            resource_manager,
            camera,
            graphics: Arc::new(RwLock::new(ManuallyDrop::new(graphics))),
            scene_manager: SceneManager::new(),
            ui_system: None,
            entities: Rc::new(RefCell::new(SlotMap::new())),
            network_system: Arc::new(tokio::sync::RwLock::new(network_system)),
            scenes: HashMap::new(),
            current_scene: SceneType::TITLE,
            room_state_receiver: None,
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
        let initialized = self.graphics.read().is_initialized();
        if initialized {
            self.graphics.write().set_disposing();
            unsafe {
                self.graphics.read().wait_idle();
            }
        }
        unsafe {
            {
                if let Some(ui_manager) = self.ui_system.as_ref() {
                    let mut borrowed = ui_manager.borrow_mut();
                    ManuallyDrop::drop(&mut *borrowed);
                }
            }
            {
                let mut resource_manager = self.resource_manager.write();
                ManuallyDrop::drop(&mut *resource_manager);
            }
            {
                let mut graphics = self.graphics.write();
                ManuallyDrop::drop(&mut *graphics);
            }
        }
    }
}
