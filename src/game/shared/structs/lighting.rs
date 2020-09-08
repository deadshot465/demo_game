use glam::{Vec3A, Vec4};

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct Directional {
    diffuse: Vec4,
    light_direction: Vec3A,
    ambient_intensity: f32,
    specular_intensity: f32,
}

impl Directional {
    pub fn new(diffuse: Vec4, light_direction: Vec3A, ambient_intensity: f32, specular_intensity: f32) -> Self {
        Directional {
            diffuse,
            light_direction,
            ambient_intensity,
            specular_intensity,
        }
    }
}