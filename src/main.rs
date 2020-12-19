#[cfg(target_os = "windows")]
use demo_game_rs::game::graphics::dx12 as DX12;
use demo_game_rs::game::graphics::vk as VK;
//use demo_game_rs::game::shared::structs::PushConstant;
use demo_game_rs::game::{Game, NetworkSystem};
use env_logger::Builder;
use log::LevelFilter;
use std::time;
#[cfg(target_os = "windows")]
use winapi::um::d3d12::ID3D12GraphicsCommandList;
use winit::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_os = "windows")]
use wio::com::ComPtr;

fn main() -> anyhow::Result<()> {
    //println!("{}", std::mem::size_of::<PushConstant>());
    //println!("{}", std::mem::size_of::<usize>());
    //return Ok(());
    dotenv::dotenv().ok();
    let log_level = dotenv::var("LOG").unwrap();
    Builder::new()
        .filter(
            None,
            match log_level.as_str() {
                "trace" => LevelFilter::Trace,
                "info" => LevelFilter::Info,
                "warn" => LevelFilter::Warn,
                "debug" => LevelFilter::Debug,
                "error" => LevelFilter::Error,
                _ => LevelFilter::Off,
            },
        )
        .default_format()
        .init();
    let api = dotenv::var("API").unwrap();
    log::info!("Using API: {}", &api);
    let mut rt = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()?;
    let event_loop = EventLoop::new();
    let mut last_second = time::Instant::now();
    let mut current_time = time::Instant::now();
    let mut frame_count = 0_u32;
    let mut delta_time = 0.0_f64;
    let network_system = rt.block_on(async {
        NetworkSystem::new()
            .await
            .expect("Failed to initialize network system.")
    });
    match api.as_str() {
        "VULKAN" => {
            let mut game = std::mem::ManuallyDrop::new(Game::<
                VK::Graphics,
                VK::Buffer,
                ash::vk::CommandBuffer,
                VK::Image,
            >::new(
                "Demo game",
                1280.0,
                720.0,
                &event_loop,
                network_system,
            )?);
            if game.initialize() {
                rt.block_on(async {
                    game.load_content().await.expect("Failed to load content.");
                });
            }
            log::info!("Game content loaded.");
            let mut mouse_x = 0.0;
            let mut mouse_y = 0.0;
            event_loop.run(move |event, _target, control_flow| {
                let game = &mut game;
                let rt = &mut rt;
                match event {
                    Event::NewEvents(_) => {
                        delta_time = current_time.elapsed().as_secs_f64();
                        current_time = time::Instant::now();
                        frame_count += 1;
                        let elapsed = last_second.elapsed().as_secs_f64();
                        if elapsed > 1.0 {
                            game.window.borrow().set_title(&format!(
                                "Demo Engine / FPS: {} / Frame Time: {}",
                                frame_count,
                                1000 / frame_count
                            ));
                            frame_count = 0;
                            last_second = time::Instant::now();
                        }
                        game.start_input();
                    }
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            unsafe {
                                std::mem::ManuallyDrop::drop(game);
                            }
                            *control_flow = ControlFlow::Exit;
                        }
                        WindowEvent::ReceivedCharacter(c) => {
                            game.input_unicode(c);
                        }
                        WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    virtual_keycode: Some(virtual_key_code),
                                    state,
                                    ..
                                },
                            ..
                        } => match virtual_key_code {
                            VirtualKeyCode::Escape => {
                                unsafe {
                                    game.is_terminating = true;
                                    std::mem::ManuallyDrop::drop(game);
                                }
                                *control_flow = ControlFlow::Exit;
                            }
                            _ => {
                                /*let mut camera = game.camera.borrow_mut();
                                println!(
                                    "Position: {}, Target: {}",
                                    camera.position, camera.target
                                );
                                camera.update(
                                    CameraType::Watch(glam::Vec3A::zero()),
                                    virtual_key_code,
                                );*/
                                rt.block_on(async {
                                    game.input_key(virtual_key_code, state).await;
                                });
                            }
                        },
                        WindowEvent::CursorMoved {
                            position: winit::dpi::PhysicalPosition { x, y },
                            ..
                        } => {
                            mouse_x = x;
                            mouse_y = y;
                            game.input_motion(x, y);
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            game.input_button(button, mouse_x, mouse_y, state);
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            game.input_scroll(delta);
                        }
                        WindowEvent::Resized(winit::dpi::PhysicalSize { width, height }) => {
                            let current_scene = game.current_scene;
                            game.graphics
                                .write()
                                .recreate_swapchain(width, height, current_scene)
                                .expect("Failed to recreate swapchain.");
                            if width > 0 && height > 0 {
                                game.scene_manager
                                    .create_ssbo()
                                    .expect("Failed to create SSBO for skinned models.");
                            }
                        }
                        _ => (),
                    },
                    Event::MainEventsCleared => {
                        game.end_input();
                        rt.block_on(async {
                            game.update(delta_time)
                                .await
                                .expect("Failed to update the game.");
                        });
                        game.render(delta_time).expect("Failed to render the game.");
                    }
                    _ => (),
                }
            });
        }
        "DX12" => {
            #[cfg(target_os = "windows")]
            unsafe {
                let mut game = std::mem::ManuallyDrop::new(Game::<
                    DX12::Graphics,
                    DX12::Resource,
                    ComPtr<ID3D12GraphicsCommandList>,
                    DX12::Resource,
                >::new(
                    "Demo game",
                    1280.0,
                    720.0,
                    &event_loop,
                    network_system,
                ));
                if game.initialize() {
                    game.load_content();
                }
                println!("Game content loaded.");
                event_loop.run(move |event, _target, control_flow| {
                    //*control_flow = ControlFlow::Poll;
                    let game = &mut game;
                    match event {
                        Event::WindowEvent { event, .. } => match event {
                            WindowEvent::CloseRequested => {
                                std::mem::ManuallyDrop::drop(game);
                                *control_flow = ControlFlow::Exit;
                            }
                            WindowEvent::KeyboardInput {
                                input:
                                    KeyboardInput {
                                        virtual_keycode: Some(virtual_key_code),
                                        ..
                                    },
                                ..
                            } => match virtual_key_code {
                                VirtualKeyCode::Escape => {
                                    std::mem::ManuallyDrop::drop(game);
                                    *control_flow = ControlFlow::Exit;
                                }
                                _ => (),
                            },
                            _ => (),
                        },
                        _ => (),
                    }
                });
            }
            #[cfg(not(target_os = "windows"))]
            panic!("DirectX is not supported on non-Windows operating systems.");
        }
        _ => (),
    }
    Ok(())
}
