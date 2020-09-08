use demo_game_rs::game::Game;
use env_logger::Builder;
use log::LevelFilter;
use winit::event_loop::EventLoop;

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
    event_loop.run(move |event, _target, control_flow| {
        game.scene_manager.update(0);
        game.scene_manager.render(0);
    });
}
