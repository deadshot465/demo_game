use glam::{Vec3A, Vec4};

/// 指向性ライト<br />
/// Directional lighting
#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct Directional {
    diffuse: Vec4,
    light_position: Vec3A,
    ambient_intensity: f32,
    specular_intensity: f32,
}

impl Directional {
    pub fn new(
        diffuse: Vec4,
        light_position: Vec3A,
        ambient_intensity: f32,
        specular_intensity: f32,
    ) -> Self {
        Directional {
            diffuse,
            light_position,
            ambient_intensity,
            specular_intensity,
        }
    }
}
