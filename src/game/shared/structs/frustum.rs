use glam::{Mat4, Vec3, Vec4};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct FrustumSide(usize);

impl FrustumSide {
    pub const LEFT: Self = Self(0);
    pub const RIGHT: Self = Self(1);
    pub const TOP: Self = Self(2);
    pub const BOTTOM: Self = Self(3);
    pub const BACK: Self = Self(4);
    pub const FRONT: Self = Self(5);
}

#[repr(C)]
pub struct Frustum {
    pub planes: [Vec4; 6],
}

impl Frustum {
    pub fn update(&mut self, matrix: Mat4) {
        let vectors = matrix.to_cols_array_2d();
        self.planes[FrustumSide::LEFT.0] = Vec4::new(
            vectors[0][3] + vectors[0][0],
            vectors[1][3] + vectors[1][0],
            vectors[2][3] + vectors[2][0],
            vectors[3][3] + vectors[3][0]
        );
        self.planes[FrustumSide::RIGHT.0] = Vec4::new(
            vectors[0][3] - vectors[0][0],
            vectors[1][3] - vectors[1][0],
            vectors[2][3] - vectors[2][0],
            vectors[3][3] - vectors[3][0]
        );
        self.planes[FrustumSide::TOP.0] = Vec4::new(
            vectors[0][3] - vectors[0][1],
            vectors[1][3] - vectors[1][1],
            vectors[2][3] - vectors[2][1],
            vectors[3][3] - vectors[3][1]
        );
        self.planes[FrustumSide::BOTTOM.0] = Vec4::new(
            vectors[0][3] + vectors[0][1],
            vectors[1][3] + vectors[1][1],
            vectors[2][3] + vectors[2][1],
            vectors[3][3] + vectors[3][1]
        );
        self.planes[FrustumSide::BACK.0] = Vec4::new(
            vectors[0][3] + vectors[0][2],
            vectors[1][3] + vectors[1][2],
            vectors[2][3] + vectors[2][2],
            vectors[3][3] + vectors[3][2]
        );
        self.planes[FrustumSide::FRONT.0] = Vec4::new(
            vectors[0][3] - vectors[0][2],
            vectors[1][3] - vectors[1][2],
            vectors[2][3] - vectors[2][2],
            vectors[3][3] - vectors[3][2]
        );
        for plane in self.planes.iter_mut() {
            let sum: f32 = plane.x() * plane.x() + plane.y() * plane.y() + plane.z() * plane.z();
            let length = sum.sqrt();
            *plane /= length;
        }
    }

    pub fn check_sphere(&self, position: Vec3, radius: f32) -> bool {
        for plane in self.planes.iter() {
            if (plane.x() * position.x()) +
                (plane.y() * position.y()) +
                (plane.z() * position.z()) + plane.w() < -radius {
                return false;
            }
        }
        true
    }
}