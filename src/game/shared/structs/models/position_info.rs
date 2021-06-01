use glam::Vec3A;

#[derive(Copy, Clone, Debug)]
pub struct PositionInfo {
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
}

impl Default for PositionInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl PositionInfo {
    pub fn new() -> Self {
        PositionInfo {
            position: Vec3A::ZERO,
            scale: Vec3A::ZERO,
            rotation: Vec3A::ZERO,
        }
    }
}
