use glam::Mat4;

pub struct ViewProjection {
    pub view: Mat4,
    pub projection: Mat4,
}

impl ViewProjection {
    pub fn new(view: Mat4, projection: Mat4) -> Self {
        ViewProjection {
            view,
            projection
        }
    }
}