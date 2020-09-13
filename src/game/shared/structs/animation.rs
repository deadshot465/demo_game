use gltf::animation::Interpolation;
use glam::{Quat, Vec3A};

#[derive(Clone, Debug)]
pub enum ChannelOutputs {
    Translations(Vec<Vec3A>),
    Rotations(Vec<Quat>),
    Scales(Vec<Vec3A>)
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub target_node_index: usize,
    pub inputs: Vec<f32>,
    pub outputs: ChannelOutputs,
    pub interpolation: Interpolation,
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub channels: Vec<Channel>,
}