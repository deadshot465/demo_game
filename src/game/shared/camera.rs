use glam::{Mat4, Vec3, Vec3A};
use winit::event::VirtualKeyCode;

const MIN_DISTANCE: f32 = 5.0;
const MAX_DISTANCE: f32 = 15.0;
const DISTANCE: f32 = 12.0;
const HEIGHT: f32 = 0.75;

#[derive(Copy, Clone, Debug)]
pub enum CameraType {
    Watch(Vec3A),
    Directional(Vec3A),
    Chase(Vec3A),
    TPS(Vec3A, f32),
    FPS(Vec3A, f32),
}

pub struct Camera {
    pub position: Vec3A,
    pub target: Vec3A,
    pub width: f64,
    pub height: f64,
    pub current_type: CameraType,
    pub projection: Mat4,
    default_position: Vec3A,
}

impl Camera {
    pub fn new(width: f64, height: f64) -> Self {
        let mut camera = Camera {
            position: Vec3A::new(0.0, 10.0, -10.0),
            target: Vec3A::new(0.0, 0.0, 0.0),
            width,
            height,
            current_type: CameraType::Watch(Vec3A::new(0.0, 0.0, 0.0)),
            projection: Mat4::identity(),
            default_position: Vec3A::new(0.0, 10.0, -15.0),
        };
        camera.set_perspective(70.0_f32.to_radians(), (width / height) as f32, 0.1, 1000.0);
        camera
    }

    pub fn update(&mut self, _camera_type: CameraType, key: VirtualKeyCode) {
        /*match camera_type {
            CameraType::Watch(pos) => self.watch(pos),
            CameraType::Directional(pos) => self.directional(pos),
            CameraType::Chase(pos) => self.chase(pos),
            CameraType::TPS(pos, angle) => self.tps(pos, angle),
            CameraType::FPS(pos, angle) => self.fps(pos, angle),
        }*/
        self.move_camera(key);
    }

    pub fn set_orthographic(&mut self, width: f32, height: f32, near: f32, far: f32) -> Mat4 {
        self.projection = Mat4::orthographic_rh(0.0, width, height, 0.0, near, far);
        self.projection
    }

    pub fn set_perspective(&mut self, fov: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        self.projection = Mat4::perspective_rh(fov, aspect, near, far);
        self.projection
    }

    pub fn get_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(
            Vec3::from(self.position),
            Vec3::from(self.target),
            Vec3::new(0.0, -1.0, 0.0),
        )
    }

    pub fn get_projection_matrix(&self) -> Mat4 {
        self.projection
    }

    pub fn update_window(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
        self.set_perspective(70.0_f32.to_radians(), (width / height) as f32, 0.1, 1000.0);
    }

    fn move_camera(&mut self, key: VirtualKeyCode) {
        let x: f32 = self.position.x();
        let y: f32 = self.position.y();
        let z: f32 = self.position.z();
        let tx: f32 = self.target.x();
        let ty: f32 = self.target.y();
        let tz: f32 = self.target.z();
        match key {
            VirtualKeyCode::A => self.position = Vec3A::new(x - 0.1, y, z),
            VirtualKeyCode::J => self.target = Vec3A::new(tx - 0.1, ty, tz),
            VirtualKeyCode::D => self.position = Vec3A::new(x + 0.1, y, z),
            VirtualKeyCode::L => self.target = Vec3A::new(tx + 0.1, ty, tz),
            VirtualKeyCode::W => self.position = Vec3A::new(x, y + 0.1, z),
            VirtualKeyCode::I => self.target = Vec3A::new(tx, ty + 0.1, tz),
            VirtualKeyCode::S => self.position = Vec3A::new(x, y - 0.1, z),
            VirtualKeyCode::K => self.target = Vec3A::new(tx, ty - 0.1, tz),
            VirtualKeyCode::Q => self.position = Vec3A::new(x, y, z - 0.1),
            VirtualKeyCode::U => self.target = Vec3A::new(tx, ty, tz - 0.1),
            VirtualKeyCode::E => self.position = Vec3A::new(x, y, z + 0.1),
            VirtualKeyCode::O => self.target = Vec3A::new(tx, ty, tz + 0.1),
            _ => (),
        }
    }

    fn watch(&mut self, player_pos: Vec3A) {
        self.position = self.default_position;
        self.target = player_pos;
    }

    fn directional(&mut self, player_pos: Vec3A) {
        self.position = Vec3A::new(
            player_pos.x() + 8.0,
            player_pos.y() + 5.0,
            player_pos.z() - 8.0,
        );
        self.target = player_pos;
    }

    fn chase(&mut self, player_pos: Vec3A) {
        let mut dx: f32 = player_pos.x() - self.position.x();
        let mut dz: f32 = player_pos.z() - self.position.z();
        let distance = (dx * dx + dz * dz).sqrt();

        if distance < MIN_DISTANCE {
            dx /= distance;
            dz /= distance;
            self.position = Vec3A::new(
                player_pos.x() - MIN_DISTANCE * dx,
                self.position.y(),
                player_pos.z() - MIN_DISTANCE * dz,
            );
        }
        if distance > MAX_DISTANCE {
            dx /= distance;
            dz /= distance;
            self.position = Vec3A::new(
                player_pos.x() - MAX_DISTANCE * dx,
                self.position.y(),
                player_pos.z() - MAX_DISTANCE * dz,
            );
        }
        self.target = player_pos;
    }

    fn tps(&mut self, player_pos: Vec3A, player_angle: f32) {
        let dx = player_angle.sin();
        let dz = player_angle.cos();
        self.position = Vec3A::new(
            player_pos.x() - DISTANCE * dx,
            self.position.y(),
            player_pos.z() - DISTANCE * dz,
        );
        self.target = player_pos;
    }

    fn fps(&mut self, player_pos: Vec3A, player_angle: f32) {
        let dx = player_angle.sin();
        let dz = player_angle.cos();
        self.position = Vec3A::new(player_pos.x(), player_pos.y() + HEIGHT, player_pos.z());
        self.target = Vec3A::new(
            self.position.x() + dx,
            self.position.y(),
            self.position.z() + dz,
        );
    }
}
