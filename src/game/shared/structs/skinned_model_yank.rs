use anyhow::Context;
use ash::vk::{CommandBuffer, CommandPool};
use crossbeam::sync::ShardedLock;
use glam::{Quat, Vec2, Vec3, Vec3A, Vec4, Mat4};
use gltf::{Node, Scene, scene::Transform};
use gltf::animation::util::ReadOutputs;
use image::{ImageFormat, ImageDecoder, EncodableLayout, GenericImageView};
use image::ColorType;
use image::jpeg::JpegDecoder;
use image::png::PngDecoder;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};
use tokio::task::JoinHandle;

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{Vertex, SkinnedMesh, Animation, SkinnedVertex, SkinnedPrimitive, ChannelOutputs, Channel};
use crate::game::structs::Joint;
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::shared::util::handle_rgb_bgr;
use rand::AsByteSliceMut;

#[allow(dead_code)]
pub struct SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
    pub color: Vec4,
    pub skinned_meshes: Vec<SkinnedMesh<BufferType, CommandType, TextureType>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub model_index: usize,
    pub animations: HashMap<String, Animation>,
    graphics: Weak<ShardedLock<GraphicsType>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType> SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn read_raw_data(file_name: &str) -> gltf::Gltf {
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
        gltf
    }

    fn process_animation(gltf_data: &gltf::Gltf) -> HashMap<String, Animation> {
        let blob = gltf_data.blob.as_ref().unwrap();
        let mut animations = HashMap::new();
        for animation in gltf_data.animations() {
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
        println!("Animations: {:?}", &animations);
        animations
    }

    fn create_skinned_model(file_name: &str, gltf_data: &gltf::Gltf,
                                graphics: Arc<ShardedLock<GraphicsType>>,
                                textures: &Vec<TextureType>,
                                position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let blob = gltf_data.blob.as_ref().unwrap();
        let scene = gltf_data.default_scene();
        let meshes: Vec<SkinnedMesh<BufferType, CommandType, TextureType>>;
        if let Some(root_scene) = scene {
            meshes = Self::process_root_nodes(root_scene, blob, &textures);
        }
        else {
            meshes = Self::process_root_nodes(gltf_data.scenes().nth(0).unwrap(), blob, &textures);
        }
        let animations = Self::process_animation(gltf_data);
        let model = SkinnedModel {
            position,
            scale,
            rotation,
            color,
            is_disposed: false,
            model_name: file_name.to_string(),
            model_index: 0,
            animations,
            graphics: Arc::downgrade(&graphics),
            skinned_meshes: meshes
        };
        model
    }

    fn process_root_nodes(scene: Scene, buffer_data: &Vec<u8>, textures: &Vec<TextureType>) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut sub_meshes = Self::process_node(node, &buffer_data, textures, Mat4::identity());
            meshes.append(&mut sub_meshes);
        }
        println!("Skinned mesh count: {:?}", meshes.len());
        meshes
    }

    fn process_node(node: Node, buffer_data: &Vec<u8>, textures: &Vec<TextureType>, local_transform: Mat4) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
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

    fn process_skinned_mesh(node: &Node, mesh: gltf::Mesh, buffer_data: &Vec<u8>, local_transform: Mat4, textures: &Vec<TextureType>) -> SkinnedMesh<BufferType, CommandType, TextureType> {
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
                is_disposed: false,
                command_pool: None,
                command_buffer: None
            };
            skinned_primitives.push(_primitive);
        }

        println!("Skinned primitive count: {:?}", skinned_primitives.len());
        println!("Root joint: {:?}", &root_joint);
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

impl SkinnedModel<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(file_name: &'static str, graphics: Weak<ShardedLock<Graphics>>,
               position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4, model_index: usize) -> JoinHandle<Self> {
        log::info!("Loading skinned model {}...", file_name);
        let graphics_arc = graphics.upgrade().unwrap();
        let model = tokio::spawn(async move {
            let graphics = graphics_arc;
            let thread_count: usize;
            let command_pool: Arc<Mutex<CommandPool>>;
            {
                let graphics_lock = graphics.read().unwrap();
                thread_count = graphics_lock.thread_pool.thread_count;
                command_pool = graphics_lock
                    .thread_pool
                    .threads[model_index % thread_count]
                    .command_pool
                    .clone();
            }
            log::info!("Skinned model index: {}, Command pool: {:?}", model_index, command_pool);
            let data = Self::read_raw_data(file_name);
            let textures = Self::create_texture(&data, graphics.clone(), command_pool.clone()).unwrap();
            let mut loaded_model = Self::create_skinned_model(file_name, &data, graphics.clone(), &textures, position, scale, rotation, color);
            {
                let graphics_lock = graphics.read().unwrap();
                for mesh in loaded_model.skinned_meshes.iter_mut() {
                    for primitive in mesh.primitives.iter_mut() {
                        primitive.command_pool = Some(command_pool.clone());
                        let command_buffer = graphics_lock
                            .create_secondary_command_buffer(command_pool.clone());
                        primitive.command_buffer = Some(command_buffer);
                    }
                }
            }
            loaded_model.create_buffers(graphics.clone(), command_pool.clone()).await;
            loaded_model
        });
        model
    }

    async fn create_buffers(&mut self, graphics: Arc<ShardedLock<Graphics>>, command_pool: Arc<Mutex<CommandPool>>) {
        let mut handles = HashMap::new();
        for (index, mesh) in self.skinned_meshes.iter_mut().enumerate() {
            handles.insert(index, vec![]);
            for primitive in mesh.primitives.iter_mut() {
                let graphics_clone = graphics.clone();
                let vertices = primitive.vertices.clone();
                let indices = primitive.indices.clone();
                let cmd_pool = command_pool.clone();
                let handle = tokio::spawn(async move {
                    Graphics::create_buffer(
                        graphics_clone, vertices,
                        indices, cmd_pool
                    ).await
                });
                let entry = handles.entry(index).or_insert(vec![]);
                (*entry).push(handle);
            }
        }
        for (index, mesh) in self.skinned_meshes.iter_mut().enumerate() {
            let mesh_handles = handles.get_mut(&index).unwrap();
            let zipped = mesh.primitives.iter_mut()
                .zip(mesh_handles.iter_mut())
                .collect::<Vec<_>>();
            for (primitive, handle) in zipped {
                let (vertex_buffer, index_buffer) = handle.await.unwrap();
                primitive.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                primitive.index_buffer = Some(ManuallyDrop::new(index_buffer));
            }
        }
    }

    fn create_texture(gltf_data: &gltf::Gltf, graphics: Arc<ShardedLock<Graphics>>, command_pool: Arc<Mutex<CommandPool>>) -> anyhow::Result<Vec<Image>> {
        let blob = gltf_data.blob.as_ref().unwrap();
        let mut textures = vec![];
        for texture in gltf_data.textures() {
            match texture.source().source() {
                gltf::image::Source::View {
                    view, mime_type
                } => {
                    println!("Mime type: {}", mime_type);
                    let slice = &blob[view.offset()..view.offset() + view.length() - 1];
                    let width: u32;
                    let height: u32;
                    let data: Vec<u8>;
                    let color_type: image::ColorType;
                    match mime_type {
                        "image/jpeg" => {
                            let decoder = JpegDecoder::new(slice)
                                .with_context(|| "Error creating JPEG decoder.")?;
                            let (w, h) = decoder.dimensions();
                            width = w;
                            height = h;
                            let decode_type = decoder.color_type();
                            let mut buffer: Vec<u8> = vec![];
                            buffer.resize(decoder.total_bytes() as usize, 0);
                            decoder.read_image(&mut *buffer)
                                .with_context(|| "Error reading JPEG image into the buffer.")?;
                            let (result, new_color_type) = handle_rgb_bgr(decode_type, buffer, (w * h * 4) as usize, w, h);
                            data = result;
                            color_type = new_color_type;
                        },
                        "image/png" => {
                            let decoder = PngDecoder::new(slice)
                                .with_context(|| "Error creating PNG decoder.")?;
                            let (w, h) = decoder.dimensions();
                            width = w;
                            height = h;
                            let mut buffer: Vec<u8> = vec![];
                            buffer.resize(decoder.total_bytes() as usize, 0);
                            let decode_type = decoder.color_type();
                            decoder.read_image(&mut *buffer)
                                .with_context(|| "Error reading PNG image into the buffer.")?;
                            let (result, new_color_type) = handle_rgb_bgr(decode_type, buffer, (w * h * 4) as usize, w, h);
                            data = result;
                            color_type = new_color_type;
                        },
                        _ => {
                            panic!("Unsupported image format.");
                        }
                    }
                    let gltf_format = match color_type {
                        ColorType::Bgra8 => gltf::image::Format::B8G8R8A8,
                        ColorType::Rgba8 => gltf::image::Format::R8G8B8A8,
                        _ => panic!("Unsupported GLTF image format.")
                    };
                    let buffer_size = width * height * 4;
                    let img = Graphics::create_image(
                        data, buffer_size as u64, width,
                        height, gltf_format,
                        graphics.clone(), command_pool.clone()
                    );
                    textures.push(img);
                },
                _ => {
                    log::error!("The format has to be a binary GLTF.");
                }
            }
        }
        Ok(textures)
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> From<&SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>> for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn from(model: &SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>) -> Self {
        loop {
            if model.skinned_meshes.iter()
                .all(|mesh| mesh.primitives.iter()
                    .all(|primitive| {
                        primitive.vertex_buffer.is_some() && primitive.index_buffer.is_some()
                    })) {
                break;
            }
        }
        SkinnedModel {
            position: model.position,
            scale: model.scale,
            rotation: model.rotation,
            color: model.color,
            skinned_meshes: model.skinned_meshes.to_vec(),
            is_disposed: true,
            model_name: model.model_name.clone(),
            model_index: 0,
            animations: model.animations.clone(),
            graphics: model.graphics.clone()
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped skinned model.");
        }
        else {
            log::warn!("Skinned model is already dropped.");
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn dispose(&mut self) {
        log::info!("Disposing skinned model...Skinned model: {}, Model index: {}", self.model_name.as_str(), self.model_index);
        for mesh in self.skinned_meshes.iter_mut() {
            mesh.dispose();
        }
        self.is_disposed = true;
        log::info!("Successfully disposed skinned model.");
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        self.model_name.as_str()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.model_name = name;
        self.model_name.as_str()
    }
}