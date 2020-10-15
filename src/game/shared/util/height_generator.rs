use rand::prelude::*;
use crate::game::shared::util::PerlinNoise;

/// TODO: Offset is not working so tiling is currently not possible.
pub struct HeightGenerator {
    seed: i32,
    perlin_noise: PerlinNoise,
    x_offset: i32,
    z_offset: i32,
}

impl HeightGenerator {
    const AMPLITUDE: f32 = 75.0;
    const OCTAVES: i32 = 3;
    const ROUGHNESS: f32 = 0.3;

    pub fn new() -> Self {
        let mut rng = thread_rng();
        let seed = rng.gen_range(0, 1_000_000_000);
        HeightGenerator {
            seed,
            perlin_noise: PerlinNoise::new(),
            x_offset: 0,
            z_offset: 0,
        }
    }

    pub fn set_offsets(&mut self, grid_x: i32, grid_z: i32, vertex_count: i32) {
        self.x_offset = grid_x * (vertex_count - 1);
        self.z_offset = grid_z * (vertex_count - 1);
    }

    pub fn generate_height(&self, x: f32, z: f32) -> f32 {
        let mut total = 0.0;
        let d = 2.0_f32.powi(Self::OCTAVES - 1);
        //let x_offset = self.x_offset as f32;
        //let z_offset = self.z_offset as f32;
        for i in 0..Self::OCTAVES {
            let frequency = 2.0_f32.powi(i) / d;
            let amplitude = Self::ROUGHNESS.powi(i) * Self::AMPLITUDE;
            total += self.get_interpolated_noise(x * frequency, z * frequency) * amplitude;
        }
        total
    }

    fn interpolate(a: f32, b: f32, blend: f32) -> f32 {
        let theta = blend * std::f32::consts::PI;
        let f = (1.0 - theta.cos()) * 0.5;
        a * (1.0 - f) + b * f
    }

    fn get_interpolated_noise(&self, x: f32, z: f32) -> f32 {
        let x_floor = x.floor();
        let z_floor = z.floor();
        let fractional_x = x - x_floor;
        let fractional_z = z - z_floor;
        let v1 = self.get_smooth_noise(x_floor, z_floor);
        let v2 = self.get_smooth_noise(x_floor + 1.0, z_floor);
        let v3 = self.get_smooth_noise(x_floor, z_floor + 1.0);
        let v4 = self.get_smooth_noise(x_floor + 1.0, z_floor + 1.0);
        let i1 = Self::interpolate(v1, v2, fractional_x);
        let i2 = Self::interpolate(v3, v4, fractional_x);
        Self::interpolate(i1, i2, fractional_z)
    }

    fn get_smooth_noise(&self, x: f32, z: f32) -> f32 {
        let corners = (
            self.get_noise(x - 1.0, z - 1.0)
                + self.get_noise(x + 1.0, z - 1.0)
                + self.get_noise(x - 1.0, z + 1.0)
                + self.get_noise(x + 1.0, z + 1.0)
        ) / 16.0;
        let sides = (
            self.get_noise(x - 1.0, z)
                + self.get_noise(x + 1.0, z)
                + self.get_noise(x, z - 1.0)
                + self.get_noise(x, z + 1.0)
        ) / 8.0;
        let center = self.get_noise(x, z) / 4.0;
        corners + sides + center
    }

    fn get_noise(&self, x: f32, z: f32) -> f32 {
        let mut rng = rand::rngs::StdRng::seed_from_u64(
            (x * 49362.0 + z * 325176.0 + (self.seed as f32)) as u64
        );
        let x = rng.gen_range(0.0_f64, 1.0_f64) * 2.0 - 1.0;
        let z = rng.gen_range(0.0_f64, 1.0_f64) * 2.0 - 1.0;
        self.perlin_noise.noise(x as f64, z as f64) as f32
        //rng.gen_range(0.0, 1.0) * 2.0 - 1.0
    }
}