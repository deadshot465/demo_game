use crate::game::shared::structs::{Vertex, SkinnedMesh, Animation, SkinnedVertex, SkinnedPrimitive, ChannelOutputs, Channel};
use glam::{Quat, Vec2, Vec3, Vec3A, Vec4, Mat4};
use crate::game::traits::{Disposable, GraphicsBase};
use std::sync::{Weak};
use std::marker::PhantomData;
use crossbeam::sync::ShardedLock;
use std::collections::HashMap;
use image::ImageFormat;
use gltf::{Node, Scene, scene::Transform};
use crate::game::structs::Joint;
use std::mem::ManuallyDrop;
use gltf::animation::util::ReadOutputs;

pub struct SkinnedModel<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> {
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
    pub color: Vec4,
    pub skinned_meshes: Vec<SkinnedMesh<BufferType, TextureType>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub model_index: usize,
    pub animations: HashMap<String, Animation>,
    graphics: Weak<ShardedLock<GraphicsType>>,
    phantom: PhantomData<&'static CommandType>,
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> SkinnedModel<GraphicsType, BufferType, CommandType, TextureType> {
    pub fn load_skinned_mesh(file_name: &str, graphics: Weak<ShardedLock<GraphicsType>>,
                             position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let raw_data = match std::fs::read(file_name) {
            Ok(d) => d,
            Err(e) => {
                log::error!("Error reading the model: {}", e.to_string());
                vec![]
            }
        };
        if raw_data.is_empty() {
            panic!("Raw data is empty. Abort.");
        }
        let gltf = gltf::Gltf::from_slice(raw_data.as_slice()).unwrap();
        let blob = gltf.blob.as_ref().unwrap();

        let mut textures = vec![];
        for texture in gltf.textures() {
            match texture.source().source() {
                gltf::image::Source::View {
                    view, mime_type
                } => {
                    let slice = &blob[view.offset()..view.offset() + view.length() - 1];
                    let png = image::load_from_memory_with_format(slice, if mime_type == "image/jpeg" {
                        ImageFormat::Jpeg
                    } else {
                        ImageFormat::Png
                    });
                    let png = match png {
                        Ok(d) => Some(d),
                        Err(e) => {
                            log::error!("Failed to load image from the memory. Error: {}", e.to_string());
                            None
                        }
                    };
                    let png = png.unwrap();
                    let image = match png.as_bgra8() {
                        Some(d) => d,
                        None => {
                            panic!("Cannot convert the image to BGRA8 format.");
                        }
                    };
                    let buffer_size = image.height() * image.width() * 4;
                    let data = image.to_vec();
                    let _graphics = graphics.upgrade();
                    if let Some(g) = _graphics {
                        let lock = g.read().unwrap();
                        let img = lock.create_image(data.as_slice(), buffer_size as u64, image.width(), image.height());
                        textures.push(img);
                    }
                },
                _ => {
                    log::error!("The format has to be a binary GLTF.");
                }
            }
        }

        let scene = gltf.default_scene();
        let meshes: Vec<SkinnedMesh<BufferType, TextureType>>;
        if let Some(root_scene) = scene {
            meshes = Self::process_root_nodes(root_scene, blob, &textures);
        }
        else {
            meshes = Self::process_root_nodes(gltf.scenes().nth(0).unwrap(), blob, &textures);
        }

        let mut animations = HashMap::new();
        for animation in gltf.animations() {
            if let Some(name) = animation.name() {
                let mut channels = vec![];
                for channel in animation.channels() {
                    let target = channel.target();
                    let target_node_index = target.node().index();
                    let sampler = channel.sampler();
                    let interpolation = sampler.interpolation();
                    let reader = channel.reader(|buffer| {
                        match buffer.source() {
                            gltf::buffer::Source::Bin => (),
                            _ => {
                                unimplemented!("The format has to be a binary GLTF.");
                            }
                        }
                        Some(&blob)
                    });
                    let inputs = reader.read_inputs()
                        .unwrap()
                        .collect::<Vec<_>>();
                    let outputs = match reader.read_outputs().unwrap() {
                        ReadOutputs::Translations(translations) => {
                            ChannelOutputs::Translations(translations.map(|x| Vec3A::from(x)).collect())
                        },
                        ReadOutputs::Rotations(rotations) => {
                            ChannelOutputs::Rotations(rotations.into_f32()
                                .map(|r| Quat::from(r))
                                .collect())
                        },
                        ReadOutputs::Scales(scales) => {
                            ChannelOutputs::Scales(scales.map(|s| Vec3A::from(s))
                                .collect())
                        },
                        ReadOutputs::MorphTargetWeights(_) => {
                            unimplemented!("glTF property::MorphTargetWeights is unimplemented.")
                        }
                    };
                    channels.push(Channel {
                        target_node_index,
                        inputs,
                        outputs,
                        interpolation
                    });
                }
                animations.insert(name.to_string(), Animation {
                    channels
                });
            }
            else {
                log::error!("glTF animation cannot be loaded as it has no name.");
            }
        }

        let model = SkinnedModel {
            position,
            scale,
            rotation,
            color,
            is_disposed: false,
            model_name: file_name.to_string(),
            model_index: 0,
            animations,
            graphics,
            phantom: PhantomData,
            skinned_meshes: meshes
        };
        model
    }

    fn process_root_nodes(scene: Scene, buffer_data: &Vec<u8>, textures: &Vec<TextureType>) -> Vec<SkinnedMesh<BufferType, TextureType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut sub_meshes = Self::process_node(node, &buffer_data, textures, Mat4::identity());
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_node(node: Node, buffer_data: &Vec<u8>, textures: &Vec<TextureType>, local_transform: Mat4) -> Vec<SkinnedMesh<BufferType, TextureType>> {
        let mut meshes = vec![];
        let (t, r, s) = node.transform().decomposed();
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );
        let transform = local_transform * transform;
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_skinned_mesh(&node, mesh, &buffer_data, transform.clone(), textures));
        }
        for _node in node.children() {
            let mut sub_meshes = Self::process_node(_node, &buffer_data, textures, transform.clone());
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_skinned_mesh(node: &Node, mesh: gltf::Mesh, buffer_data: &Vec<u8>, local_transform: Mat4, textures: &Vec<TextureType>) -> SkinnedMesh<BufferType, TextureType> {
        let mut root_joint = None;
        if let Some(skin) = node.skin() {
            let joints: Vec<_> = skin.joints().collect();
            if !joints.is_empty() {
                let reader = skin.reader(|buffer| {
                    match buffer.source() {
                        gltf::buffer::Source::Bin => (),
                        gltf::buffer::Source::Uri(_) => {
                            log::error!("URI-based skins are not supported.");
                        }
                    }
                    Some(&buffer_data)
                });
                let ibm: Vec<Mat4> = reader.read_inverse_bind_matrices()
                    .unwrap()
                    .map(|r| Mat4::from_cols_array_2d(&r))
                    .collect();
                let node_to_joints_lookup: Vec<_> = joints.iter().map(|n| n.index()).collect();
                root_joint = Some(Self::process_skeleton(
                    node, buffer_data, &node_to_joints_lookup, ibm.as_slice(), Mat4::identity()
                ));
            }
        }

        let mut skinned_primitives = vec![];
        for primitive in mesh.primitives() {
            match primitive.mode() {
                gltf::json::mesh::Mode::Triangles => (),
                _ => {
                    log::error!("The primitive topology has to be triangles.");
                }
            }
            let reader = primitive.reader(|buffer| {
                match buffer.source() {
                    gltf::buffer::Source::Bin => (),
                    _ => {
                        log::error!("The format has to be a binary GLTF.");
                    }
                }
                Some(&buffer_data)
            });
            let indices = reader.read_indices()
                .unwrap()
                .into_u32()
                .map(|x| x)
                .collect::<Vec<_>>();
            let positions = reader.read_positions();
            let normals = reader.read_normals();
            let uvs = reader.read_tex_coords(0);
            let joints = reader.read_joints(0);
            let weights = reader.read_weights(0);
            let skinned_vertices = match (positions, normals, uvs, joints, weights) {
                (Some(positions), Some(normals), Some(uvs), Some(joints), Some(weights)) => {
                    let vertices = positions
                        .zip(normals)
                        .zip(uvs.into_f32())
                        .zip(joints.into_u16())
                        .zip(weights.into_f32())
                        .map(|((((pos, normals), uv), joints), weights)| SkinnedVertex {
                            vertex: Vertex {
                                position: Vec3A::from(pos),
                                normal: Vec3A::from(normals),
                                uv: Vec2::from(uv)
                            },
                            joints: Some(Vec4::new(joints[0] as f32, joints[1] as f32, joints[2] as f32, joints[3] as f32)),
                            weights: Some(Vec4::from(weights))
                        })
                        .collect::<Vec<_>>();
                    vertices
                },
                (positions, normals, uvs, joints, weights) => {
                    unimplemented!("This method doesn't support loading static meshes. Positions: {:?}, Normals: {:?}, UVs: {:?}, Joints: {:?}, Weights: {:?}", positions.is_some(), normals.is_some(), uvs.is_some(), joints.is_some(), weights.is_some());
                }
            };

            let texture_index = primitive.material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|x| x.texture().index());
            let texture = texture_index.and_then(|x| textures.get(x).cloned());

            let _primitive = SkinnedPrimitive {
                vertices: skinned_vertices,
                indices,
                vertex_buffer: None::<ManuallyDrop<BufferType>>,
                index_buffer: None::<ManuallyDrop<BufferType>>,
                texture: texture.map(|t| ManuallyDrop::new(t)),
                is_disposed: false
            };
            skinned_primitives.push(_primitive);
        }

        SkinnedMesh {
            primitives: skinned_primitives,
            is_disposed: false,
            transform: local_transform,
            root_joint
        }
    }

    fn process_skeleton(node: &Node, buffer_data: &[u8], node_to_joints_lookup: &[usize], inverse_bind_matrices: &[Mat4], local_transform: Mat4) -> Joint {
        let mut children = vec![];
        let node_index = node.index();
        let index = node_to_joints_lookup.iter()
            .enumerate()
            .find(|(_, x)| **x == node_index)
            .unwrap()
            .0;
        let name = node.name().unwrap_or("");
        let ibm = inverse_bind_matrices[index];
        let (t, r, s) = node.transform().decomposed();
        let node_transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );
        let pose_transform = local_transform * node_transform;
        for child in node.children() {
            children.push(Self::process_skeleton(&child, buffer_data, node_to_joints_lookup, inverse_bind_matrices, pose_transform.clone()));
        }
        let ibm = ibm.clone();
        let (t, r, s) = match node.transform() {
            Transform::Matrix { matrix } => {
                let mat = Mat4::from_cols_array_2d(&matrix);
                let (t, r, s) = mat.to_scale_rotation_translation();
                (t, r, s)
            },
            Transform::Decomposed { translation, rotation, scale } => {
                let translation = Vec3::from(translation);
                let quaternion = Quat::from(rotation);
                let scale = Vec3::from(scale);
                (translation, quaternion, scale)
            }
        };

        Joint {
            name: name.to_string(),
            node_index,
            index,
            children,
            inverse_bind_matrices: ibm,
            translation: Vec3A::from(t),
            rotation: r,
            scale: Vec3A::from(s),
        }
    }
}