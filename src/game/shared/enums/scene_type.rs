/// シーンのタイプ<br />
/// Scene types.
#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug, Hash)]
pub struct SceneType(pub(crate) u32);

impl SceneType {
    pub(crate) const TITLE: Self = Self(0);
    pub(crate) const LOBBY: Self = Self(1);
    pub(crate) const GAME: Self = Self(2);
}

impl PartialEq<u32> for SceneType {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}
