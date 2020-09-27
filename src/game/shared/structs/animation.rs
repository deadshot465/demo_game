use glam::{Quat, Vec3A, Mat4};
use gltf::animation::Interpolation;

use crate::game::shared::structs::Joint;

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

macro_rules! interpolate {
    ($p0: expr, $p1: expr, $m0: expr, $m1: expr, $time: expr) => {{
        let t_pow2 = $time * $time;
        let t_pow3 = $time * $time;
        (2.0 * t_pow3 - 3.0 * t_pow2 + 1.0) * $p0 +
            (t_pow3 - 2.0 * t_pow2 + $time) * $m0 +
            (-2.0 * t_pow3 + 3.0 * t_pow2) * $p1 +
            (t_pow3 - t_pow2) * $m1
    }}
}

pub fn generate_joint_transforms(animation: &Animation, frame: f32, root_joint: &Joint, local_transform: Mat4, buffer: &mut [Mat4; 500]) {
    let mut translation = root_joint.translation.clone();
    let mut rotation = root_joint.rotation.clone();
    let mut scale = root_joint.scale.clone();

    for channel in animation.channels.iter() {
        if root_joint.node_index == channel.target_node_index {
            match (&channel.outputs, &channel.interpolation) {
                (ChannelOutputs::Translations(translations), Interpolation::Linear) => {
                    let (index_prev, index_next, amount) = index_linear(channel, frame);
                    let prev = translations[index_prev];
                    let next = translations[index_next];
                    translation = prev.lerp(next, amount);
                },
                (ChannelOutputs::Translations(translations), Interpolation::Step) => {
                    let output_index = index_step(channel, frame);
                    translation = translations[output_index];
                },
                (ChannelOutputs::Translations(translations), Interpolation::CubicSpline) => {
                    translation = match index_cubic_spline(channel, frame) {
                        CubicSplineIndex::Clamped { index } => {
                            translations[index * 3 + 1]
                        },
                        CubicSplineIndex::Interpolate { index_prev, index_next, time, range } => {
                            // previous spline vertex
                            let p0 = translations[index_prev * 3 + 1];
                            // next spline vertex
                            let p1 = translations[index_next * 3 + 1];
                            // previous output tangent
                            let m0: Vec3A = translations[index_prev * 3 + 2] * range;
                            // next output tangent
                            let m1: Vec3A = translations[index_next * 3 + 0] * range;
                            let result: Vec3A = interpolate!(p0, p1, m0, m1, time);
                            result
                        }
                    };
                },
                (ChannelOutputs::Rotations(rotations), Interpolation::Linear) => {
                    let (index_prev, index_next, amount) = index_linear(channel, frame);
                    let prev = rotations[index_prev];
                    let next = rotations[index_next];
                    rotation = prev.slerp(next, amount);
                },
                (ChannelOutputs::Rotations(rotations), Interpolation::Step) => {
                    let output_index = index_step(channel, frame);
                    rotation = rotations[output_index];
                },
                (ChannelOutputs::Rotations(rotations), Interpolation::CubicSpline) => {
                    rotation = match index_cubic_spline(channel, frame) {
                        CubicSplineIndex::Clamped { index } => {
                            rotations[index * 3 + 1]
                        },
                        CubicSplineIndex::Interpolate { index_prev, index_next, time, range } => {
                            // previous spline vertex
                            let p0 = rotations[index_prev * 3 + 1];
                            // next spline vertex
                            let p1 = rotations[index_next * 3 + 1];
                            // previous output tangent
                            let m0 = Quat::from_xyzw(
                                rotations[index_prev * 3 + 2].x() * range,
                                rotations[index_prev * 3 + 2].y() * range,
                                rotations[index_prev * 3 + 2].z() * range,
                                rotations[index_prev * 3 + 2].w() * range
                            );
                            // next output tangent
                            let m1 = Quat::from_xyzw(
                                rotations[index_next * 3 + 0].x() * range,
                                rotations[index_next * 3 + 0].y() * range,
                                rotations[index_next * 3 + 0].z() * range,
                                rotations[index_next * 3 + 0].w() * range
                            );
                            let p0_vector: glam::Vec4 = glam::Vec4::new(
                                p0.x(), p0.y(), p0.z(), p0.w()
                            );
                            let p1_vector: glam::Vec4 = glam::Vec4::new(
                                p1.x(), p1.y(), p1.z(), p1.w()
                            );
                            let m0_vector: glam::Vec4 = glam::Vec4::new(
                                m0.x(), m0.y(), m0.z(), m0.w()
                            );
                            let m1_vector: glam::Vec4 = glam::Vec4::new(
                                m1.x(), m1.y(), m1.z(), m1.w()
                            );
                            let result: glam::Vec4 = interpolate!(p0_vector, p1_vector, m0_vector, m1_vector, time);
                            Quat::from_xyzw(result.x(), result.y(), result.z(), result.w())
                        }
                    };
                    rotation = rotation.normalize();
                },
                (ChannelOutputs::Scales(scales), Interpolation::Linear) => {
                    let (index_prev, index_next, amount) = index_linear(channel, frame);
                    let prev = scales[index_prev];
                    let next = scales[index_next];
                    scale = prev.lerp(next, amount);
                }
                (ChannelOutputs::Scales(scales), Interpolation::Step) => {
                    let output_index = index_step(channel, frame);
                    scale = scales[output_index];
                },
                (ChannelOutputs::Scales(scales), Interpolation::CubicSpline) => {
                    scale = match index_cubic_spline(channel, frame) {
                        CubicSplineIndex::Clamped { index } => {
                            scales[index * 3 + 1]
                        },
                        CubicSplineIndex::Interpolate { index_prev, index_next, time, range } => {
                            // previous spline vertex
                            let p0 = scales[index_prev * 3 + 1];
                            // next spline vertex
                            let p1 = scales[index_next * 3 + 1];
                            // previous output tangent
                            let m0: Vec3A = scales[index_prev * 3 + 2] * range;
                            // next output tangent
                            let m1: Vec3A = scales[index_next * 3 + 0] * range;
                            let result: Vec3A = interpolate!(p0, p1, m0, m1, time);
                            result
                        }
                    };
                }
            }
        }
    }
    let rotation = Mat4::from_quat(rotation);
    let transform = local_transform *
        Mat4::from_translation(glam::Vec3::from(translation)) *
        rotation *
        Mat4::from_scale(glam::Vec3::from(scale));
    let final_transform = transform * root_joint.inverse_bind_matrices;
    buffer[root_joint.index] = final_transform;
    for child in root_joint.children.iter() {
        generate_joint_transforms(animation, frame, child, transform.clone(), buffer);
    }
}

/*fn cubic_spline_interpolate<T>(p0: T, p1: T, m0: T, m1: T, time: f32) -> T
    where T: Mul<f32> {
    let t_pow2 = time * time;
    let t_pow3 = t_pow2 * time;

    (2.0 * t_pow3 - 3.0 * t_pow2 + 1.0) * p0 +
        (t_pow3 - 2.0 * t_pow2 + time) * m0 +
        (-2.0 * t_pow3 + 3.0 * t_pow2) * p1 +
        (t_pow3 - t_pow2) * m1
}*/

fn index_step(channel: &Channel, frame: f32) -> usize {
    // 60 fps
    let seconds = frame % 60.0;
    if seconds < *channel.inputs.first().unwrap() || channel.inputs.len() < 2 {
        return 0;
    }
    for (i, window) in channel.inputs.windows(2).enumerate() {
        let input_prev = window[0];
        let input_next = window[1];
        if seconds >= input_prev && seconds < input_next {
            return i;
        }
    }
    channel.inputs.len() - 1
}

fn index_linear(channel: &Channel, frame: f32) -> (usize, usize, f32) {
    // 60 fps
    let seconds = frame % 60.0;
    if seconds < *channel.inputs.first().unwrap() || channel.inputs.len() < 2 {
        return (0, 0, 0.0);
    }
    for (i, window) in channel.inputs.windows(2).enumerate() {
        let input_prev = window[0];
        let input_next = window[1];
        if seconds >= input_prev && seconds < input_next {
            let amount = (seconds - input_prev) / (input_next - input_prev);
            return (i, i + 1, amount)
        }
    }
    let last = channel.inputs.len() - 1;
    (last, last, 0.0)
}

enum CubicSplineIndex {
    Clamped { index: usize },
    Interpolate { index_prev: usize, index_next: usize, time: f32, range: f32 }
}

fn index_cubic_spline(channel: &Channel, frame: f32) -> CubicSplineIndex {
    // 60 fps
    let seconds = frame / 60.0;
    if seconds < *channel.inputs.first().unwrap() || channel.inputs.len() < 2 {
        return CubicSplineIndex::Clamped { index: 0 };
    }
    for (i, window) in channel.inputs.windows(2).enumerate() {
        let input_prev = window[0];
        let input_next = window[1];
        if seconds >= input_prev && seconds < input_next {
            let range = input_next - input_prev;
            let time = (seconds - input_prev) / range;
            return CubicSplineIndex::Interpolate {
                index_prev: i,
                index_next: i + 1,
                time,
                range
            };
        }
    }
    let index = channel.inputs.len() - 1;
    CubicSplineIndex::Clamped { index }
}