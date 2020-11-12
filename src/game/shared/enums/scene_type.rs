#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug, Hash)]
pub enum SceneType {
    Title,
    Lobby,
    Game,
}
