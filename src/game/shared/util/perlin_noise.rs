use rand::prelude::*;

/// Translation of Ken Perlin's JAVA implementation (http://mrl.nyu.edu/~perlin/noise/)
/// Modified from -
/// Sascha Willems' Texture3D sample (https://github.com/SaschaWillems/Vulkan/tree/master/examples/texture3d)
/// 2D Perlin Noise (https://stackoverflow.com/questions/8659351/2d-perlin-noise)
/// Calculate PerlinNoise faster with an optimization to grad() (http://riven8192.blogspot.com/2010/08/calculate-perlinnoise-twice-as-fast.html)
/// Understanding Perlin Noise (https://adrianb.io/2014/08/09/perlinnoise.html)
pub struct PerlinNoise {
    permutations: [u8; 512],
}

impl Default for PerlinNoise {
    fn default() -> Self {
        Self::new()
    }
}

impl PerlinNoise {
    pub fn new() -> Self {
        let mut permutation_lookup = [0_u8; 256];
        for (i, item) in permutation_lookup.iter_mut().enumerate() {
            *item = i as u8;
        }
        let mut rng = rand::thread_rng();
        permutation_lookup.shuffle(&mut rng);
        let mut permutations = [0; 512];
        /*let permutation_lookup = [
            151, 160, 137, 91, 90, 15,
            131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23,
            190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33,
            88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166,
            77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244,
            102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196,
            135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123,
            5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42,
            223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
            129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228,
            251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107,
            49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254,
            138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180
        ];*/
        permutations[256..(256 + 256)].clone_from_slice(&permutation_lookup[..256]);
        permutations[..256].clone_from_slice(&permutation_lookup[..256]);
        PerlinNoise { permutations }
    }

    pub fn noise(&self, mut x: f64, mut y: f64) -> f64 {
        let permutations = &self.permutations;
        // Find unit square that contains point
        let x_temp = (x.floor() as usize) & 255;
        let y_temp = (y.floor() as usize) & 255;

        // Find relative x, y, z of point in square
        x -= x.floor();
        y -= y.floor();

        // Compute fade curves for each of x, y
        let u = Self::fade(x);
        let v = Self::fade(y);

        // Hash coordinates of the 6 square corners
        let a = (permutations[x_temp]) as usize + y_temp;
        let aa = (permutations[a]) as usize;
        let ab = (permutations[a + 1]) as usize;
        let b = (permutations[x_temp + 1]) as usize + y_temp;
        let ba = (permutations[b]) as usize;
        let bb = (permutations[b + 1]) as usize;

        // And add blended results for 6 corners of the square
        /*Self::lerp(
            w,
            Self::lerp(
                v,
                Self::lerp(
                    u,
                    Self::grad(
                        self.permutations[aa] as i32,
                        x,
                        y,
                        z,
                    ),
                    Self::grad(
                        self.permutations[ba] as i32,
                        x - 1.0,
                        y,
                        z,
                    )
                ),
                Self::lerp(
                    u,
                    Self::grad(
                        self.permutations[ab] as i32,
                        x,
                        y - 1.0,
                        z
                    ),
                    Self::grad(
                        self.permutations[bb] as i32,
                        x - 1.0,
                        y - 1.0,
                        z
                    )
                )
            ),
            Self::lerp(
                v,
                Self::lerp(
                    u,
                    Self::grad(
                        self.permutations[aa + 1] as i32,
                        x,
                        y,
                        z - 1.0
                    ),
                    Self::grad(
                        self.permutations[ba + 1] as i32,
                        x - 1.0,
                        y,
                        z - 1.0
                    )
                ),
                Self::lerp(
                    u,
                    Self::grad(
                        self.permutations[ab + 1] as i32,
                        x,
                        y - 1.0,
                        z - 1.0
                    ),
                    Self::grad(
                        self.permutations[bb + 1] as i32,
                        x - 1.0,
                        y - 1.0,
                        z - 1.0
                    )
                )
            )
        );*/
        Self::lerp(
            v,
            Self::lerp(
                u,
                Self::grad(permutations[aa] as i32, x, y, 0.0),
                Self::grad(permutations[ba] as i32, x - 1.0, y, 0.0),
            ),
            Self::lerp(
                u,
                Self::grad(permutations[ab] as i32, x, y - 1.0, 0.0),
                Self::grad(permutations[bb] as i32, x - 1.0, y - 1.0, 0.0),
            ),
        )
    }

    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    fn lerp(t: f64, a: f64, b: f64) -> f64 {
        a + t * (b - a)
    }

    fn grad(hash: i32, x: f64, y: f64, z: f64) -> f64 {
        /*let h = hash & 15;
        let u = if h < 8 {
            x
        } else {
            y
        };
        let v = if h < 4 {
            y
        } else {
            if h == 12 || h == 14 {
                x
            } else {
                z
            }
        };
        let mut result = if (h & 1) == 0 {
            u
        } else {
            -u
        };
        result += if (h & 2) == 0 {
            v
        } else {
            -v
        };
        result*/

        match hash & 0xF {
            0x0 => x + y,
            0x1 => -x + y,
            0x2 => x - y,
            0x3 => -x - y,
            0x4 => x + z,
            0x5 => -x + z,
            0x6 => x - z,
            0x7 => -x - z,
            0x8 => y + z,
            0x9 => -y + z,
            0xA => y - z,
            0xB => -y - z,
            0xC => y + x,
            0xD => -y + z,
            0xE => y - x,
            0xF => -y - z,
            _ => 0.0,
        }
    }
}
