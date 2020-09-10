use demo_game_rs::game::Game;
use env_logger::Builder;
use log::LevelFilter;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::event::{Event, WindowEvent, KeyboardInput, VirtualKeyCode};
use demo_game_rs::game::shared::structs::PushConstant;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    Builder::new()
        .filter(None, LevelFilter::Trace)
        .default_format()
        .init();
    log::info!("Using API: {}", dotenv::var("API").unwrap().as_str());
    let event_loop = EventLoop::new();
    let mut game = Game::new("Demo game", 1280.0, 720.0, &event_loop);
    if game.initialize() {
        game.load_content().await;
    }
    println!("Game content loaded.");
    event_loop.run(move |event, _target, control_flow| {
        //*control_flow = ControlFlow::Poll;
        let mut game = &mut game;
        game.update();
        game.render();
        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    },
                    WindowEvent::KeyboardInput { input: KeyboardInput {
                        virtual_keycode: Some(virtual_key_code),
                        ..
                    }, .. } => {
                        match virtual_key_code {
                            VirtualKeyCode::Escape => *control_flow = ControlFlow::Exit,
                            _ => ()
                        }
                    },
                    _ => ()
                }
            },
            _ => ()
        }
    });
}
