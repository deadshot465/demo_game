#[cfg(target_os = "windows")]
use demo_game_rs::game::graphics::dx12 as DX12;
use demo_game_rs::game::graphics::vk as VK;
use demo_game_rs::game::Game;
use env_logger::Builder;
use log::LevelFilter;
#[cfg(target_os = "windows")]
use winapi::um::d3d12::ID3D12GraphicsCommandList;
use winit::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_os = "windows")]
use wio::com::ComPtr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    let event_loop = EventLoop::new();
    match api.as_str() {
        "VULKAN" => {
            let mut game =
                Game::<VK::Graphics, VK::Buffer, ash::vk::CommandBuffer, VK::Image>::new(
                    "Demo game",
                    1280.0,
                    720.0,
                    &event_loop,
                )?;
            if game.initialize() {
                game.load_content().await?;
            }
            log::info!("Game content loaded.");
            event_loop.run(move |event, _target, control_flow| {
                //*control_flow = ControlFlow::Poll;
                let game = &mut game;
                game.update().unwrap();
                game.render().unwrap();
                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
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
                            VirtualKeyCode::Escape => *control_flow = ControlFlow::Exit,
                            _ => (),
                        },
                        _ => (),
                    },
                    _ => (),
                }
            });
        }
        "DX12" => {
            #[cfg(target_os = "windows")]
            unsafe {
                let mut game = Game::<
                    DX12::Graphics,
                    DX12::Resource,
                    ComPtr<ID3D12GraphicsCommandList>,
                    DX12::Resource,
                >::new("Demo game", 1280.0, 720.0, &event_loop);
                if game.initialize() {
                    game.load_content().await;
                }
                println!("Game content loaded.");
                event_loop.run(move |event, _target, control_flow| {
                    //*control_flow = ControlFlow::Poll;
                    let game = &mut game;
                    game.update();
                    game.render();
                    match event {
                        Event::WindowEvent { event, .. } => match event {
                            WindowEvent::CloseRequested => {
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
                                VirtualKeyCode::Escape => *control_flow = ControlFlow::Exit,
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
