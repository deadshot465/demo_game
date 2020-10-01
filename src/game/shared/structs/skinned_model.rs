use ash::vk::{CommandBuffer, CommandPool, CommandBufferInheritanceInfo, CommandBufferBeginInfo, CommandBufferUsageFlags, PipelineBindPoint, ShaderStageFlags, IndexType};
use crossbeam::sync::ShardedLock;
use glam::{Quat, Vec2, Vec3, Vec3A, Vec4, Mat4};
use gltf::{Node, Scene, scene::Transform};
use gltf::animation::util::ReadOutputs;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};
use tokio::task::JoinHandle;

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::enums::{create_sampler_resource, ShaderType, SamplerResource};
use crate::game::shared::structs::{Vertex, SkinnedMesh, Animation, SkinnedVertex, SkinnedPrimitive, ChannelOutputs, Channel, SSBO, generate_joint_transforms};
use crate::game::structs::{Joint, PushConstant};
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::util::read_raw_data;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::convert::TryFrom;
use ash::version::DeviceV1_0;

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
    current_frame: f32,
}

impl<GraphicsType, BufferType, CommandType, TextureType> SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Clone + Disposable {
    fn create_model(file_name: &str, document: gltf::Document, buffers: Vec<gltf::buffer::Data>,
                    images: Vec<Arc<ShardedLock<TextureType>>>,
                    graphics: Weak<ShardedLock<GraphicsType>>,
                    position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let meshes = Self::process_model(&document, &buffers, images);
        let animations = Self::process_animation(&document, &buffers);
        for (name, _) in animations.iter() {
            println!("{}", &name);
        }
        SkinnedModel {
            position,
            scale,
            rotation,
            color,
            skinned_meshes: meshes,
            is_disposed: false,
            model_name: file_name.to_string(),
            model_index: 0,
            animations,
            graphics,
            current_frame: 0.0,
        }
    }

    fn process_model(document: &gltf::Document, buffers: &Vec<gltf::buffer::Data>, images: Vec<Arc<ShardedLock<TextureType>>>) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let meshes = if let Some(scene) = document.default_scene() {
            Self::process_root_nodes(scene, buffers, images)
        } else {
            Self::process_root_nodes(document.scenes().nth(0).unwrap(), buffers, images)
        };
        log::info!("Skinned model mesh count: {}", meshes.len());
        meshes
    }

    fn process_root_nodes(scene: Scene, buffers: &Vec<gltf::buffer::Data>, images: Vec<Arc<ShardedLock<TextureType>>>) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut sub_meshes = Self::process_node(node, &buffers, &images, Mat4::identity());
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_node(node: Node, buffers: &Vec<gltf::buffer::Data>, images: &Vec<Arc<ShardedLock<TextureType>>>, local_transform: Mat4) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let mut meshes = Vec::with_capacity(10);
        let (t, r, s) = node.transform().decomposed();
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );
        let transform = local_transform * transform;
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_skinned_mesh(&node, mesh, buffers, transform.clone(), images));
        }
        for _node in node.children() {
            let mut sub_meshes = Self::process_node(_node, buffers, images, transform.clone());
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_skinned_mesh(node: &Node, mesh: gltf::Mesh, buffers: &Vec<gltf::buffer::Data>, local_transform: Mat4, images: &Vec<Arc<ShardedLock<TextureType>>>) -> SkinnedMesh<BufferType, CommandType, TextureType> {
        let mut root_joint = None;
        if let Some(skin) = node.skin() {
            let joints: Vec<_> = skin.joints().collect();
            if !joints.is_empty() {
                let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
                let ibm: Vec<Mat4> = reader.read_inverse_bind_matrices()
                    .unwrap()
                    .map(|r| Mat4::from_cols_array_2d(&r))
                    .collect();
                let node_to_joints_lookup: Vec<_> = joints.iter().map(|n| n.index()).collect();
                root_joint = Some(Self::process_skeleton(
                    &joints[0], &node_to_joints_lookup, ibm.as_slice(), Mat4::identity()
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
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
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
                            joints: Vec4::new(joints[0] as f32, joints[1] as f32, joints[2] as f32, joints[3] as f32),
                            weights: Vec4::from(weights)
                        })
                        .collect::<Vec<_>>();
                    vertices
                },
                (Some(positions), Some(normals), Some(uvs), None, None) => {
                    let vertices = positions
                        .zip(normals)
                        .zip(uvs.into_f32())
                        .map(|((pos, normals), uv)| SkinnedVertex {
                            vertex: Vertex {
                                position: Vec3A::from(pos),
                                normal: Vec3A::from(normals),
                                uv: Vec2::from(uv)
                            },
                            joints: Vec4::zero(),
                            weights: Vec4::zero(),
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
            let texture = texture_index
                .and_then(|x| images.get(x).cloned());

            let _primitive = SkinnedPrimitive {
                vertices: skinned_vertices,
                indices,
                vertex_buffer: None::<ManuallyDrop<BufferType>>,
                index_buffer: None::<ManuallyDrop<BufferType>>,
                texture,
                is_disposed: false,
                command_pool: None,
                command_buffer: None,
                sampler_resource: None,
            };
            skinned_primitives.push(_primitive);
        }

        SkinnedMesh {
            primitives: skinned_primitives,
            is_disposed: false,
            transform: local_transform,
            root_joint,
            ssbo: None,
        }
    }

    fn process_skeleton(node: &Node, node_to_joints_lookup: &[usize], inverse_bind_matrices: &[Mat4], local_transform: Mat4) -> Joint {
        let mut children = vec![];
        let node_index = node.index();
        let index = node_to_joints_lookup.iter()
            .enumerate()
            .find(|(_, x)| **x == node_index);
        let index = index.unwrap().0;
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
            let skeleton = Self::process_skeleton(&child, node_to_joints_lookup, inverse_bind_matrices, pose_transform.clone());
            children.push(skeleton);
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
            inverse_bind_matrices: ibm,
            translation: Vec3A::from(t),
            rotation: r,
            scale: Vec3A::from(s),
            children
        }
    }

    fn process_animation(document: &gltf::Document, buffers: &Vec<gltf::buffer::Data>) -> HashMap<String, Animation> {
        let mut animations = HashMap::new();
        for (index, animation) in document.animations().enumerate() {
            let name = if let Some(n) = animation.name() {
                n.to_string()
            } else {
                format!("default{}", index)
            };
            let mut channels = vec![];
            for channel in animation.channels() {
                let target = channel.target();
                let target_node_index = target.node().index();
                let sampler = channel.sampler();
                let interpolation = sampler.interpolation();
                let reader = channel.reader(|buffer| {
                    Some(&buffers[buffer.index()])
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
            animations.insert(name, Animation {
                channels,
                current_time: 0.0
            });
        }
        log::info!("Animation count: {}", animations.len());
        animations
    }

    pub fn get_world_matrix(&self) -> Mat4 {
        let world = Mat4::identity();
        let scale = Mat4::from_scale(glam::Vec3::from(self.scale));
        let translation = Mat4::from_translation(glam::Vec3::from(self.position));
        let rotate = Mat4::from_rotation_ypr(self.rotation.y(), self.rotation.x(), self.rotation.z());
        world * translation * rotate * scale
    }
}

impl SkinnedModel<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(file_name: &'static str, graphics: Weak<ShardedLock<Graphics>>,
               position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4, model_index: usize) -> anyhow::Result<JoinHandle<Self>> {
        log::info!("Loading skinned model from glTF {}...", file_name);
        let graphics_arc = graphics.upgrade().unwrap();
        let model = tokio::spawn(async move {
            let graphics_clone = graphics_arc;
            let thread_count: usize;
            let command_pool: Arc<Mutex<CommandPool>>;
            {
                let graphics_lock = graphics_clone.read().unwrap();
                thread_count = graphics_lock.thread_pool.thread_count;
                command_pool = graphics_lock
                    .thread_pool
                    .threads[model_index % thread_count]
                    .command_pool
                    .clone();
            }
            log::info!("Skinned model index: {}, Command pool: {:?}", model_index, command_pool);
            let (document, buffers, images) = read_raw_data(file_name).unwrap();
            let textures = Graphics::create_gltf_textures(images, graphics_clone.clone(), command_pool.clone()).await.unwrap();
            let mut loaded_model = Self::create_model(file_name, document, buffers, textures, graphics.clone(), position, scale, rotation, color);
            {
                let graphics_lock = graphics_clone.read().unwrap();
                for mesh in loaded_model.skinned_meshes.iter_mut() {
                    for primitive in mesh.primitives.iter_mut() {
                        primitive.command_pool = Some(command_pool.clone());
                        let command_buffer = graphics_lock
                            .create_secondary_command_buffer(command_pool.clone());
                        primitive.command_buffer = Some(command_buffer);
                    }
                }
            }
            loaded_model.create_buffers(graphics_clone.clone(), command_pool.clone()).await.unwrap();
            loaded_model
        });
        Ok(model)
    }

    async fn create_buffers(&mut self, graphics: Arc<ShardedLock<Graphics>>, command_pool: Arc<Mutex<CommandPool>>) -> anyhow::Result<()> {
        let mut handles = HashMap::new();
        for (index, mesh) in self.skinned_meshes.iter_mut().enumerate() {
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
                let (vertex_buffer, index_buffer) = handle.await??;
                primitive.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                primitive.index_buffer = Some(ManuallyDrop::new(index_buffer));
            }
        }
        Ok(())
    }

    pub async fn create_ssbo(&mut self) {
        let mut ssbo_handles = HashMap::new();
        let graphics = self.graphics.upgrade().unwrap();
        for (index, _) in self.skinned_meshes.iter_mut().enumerate() {
            let entry = ssbo_handles.entry(index).or_insert(vec![]);
            let graphics_clone = graphics.clone();
            entry.push(tokio::spawn(async move {
                let buffer = [Mat4::identity(); 500];
                let ssbo = SSBO::new(graphics_clone, &buffer);
                ssbo
            }));
        }
        for (index, mesh) in self.skinned_meshes.iter_mut().enumerate() {
            let ssbos = ssbo_handles.get_mut(&index).unwrap();
            for ssbo in ssbos.iter_mut() {
                let item = ssbo.await.unwrap();
                mesh.ssbo = Some(item);
                break;
            }
        }
    }

    pub fn create_sampler_resource(&mut self) {
        let graphics_arc = self.graphics.upgrade().unwrap();
        let graphics_lock = graphics_arc.read().unwrap();
        for mesh in self.skinned_meshes.iter_mut() {
            for primitive in mesh.primitives.iter_mut() {
                if primitive.texture.is_none() {
                    continue;
                }
                let texture_clone = primitive.texture
                    .clone()
                    .unwrap();
                let texture = texture_clone.read().unwrap();
                let sampler_resource = create_sampler_resource(
                    Arc::downgrade(&graphics_lock.logical_device),
                    graphics_lock.sampler_descriptor_set_layout,
                    *graphics_lock.descriptor_pool.lock(), &*texture
                );
                primitive.sampler_resource = Some(sampler_resource);
            }
        }
    }

    pub fn update(&mut self, delta_time: f64) {
        let mut animation_name = String::new();
        for (name, _) in self.animations.iter() {
            animation_name = name.clone();
            break;
        }
        let animation = self.animations.get_mut(&animation_name).unwrap();
        animation.current_time += delta_time as f32;
        let animation_end_time = *animation.channels
            .last().unwrap()
            .inputs.last()
            .unwrap();
        if animation.current_time > animation_end_time {
            animation.current_time -= animation_end_time;
        }
        let buffer_size = std::mem::size_of::<Mat4>() * 500;
        for mesh in self.skinned_meshes.iter_mut() {
            let mut buffer = [Mat4::identity(); 500];
            let local_transform = mesh.transform;
            match mesh.root_joint.as_ref() {
                Some(joint) => generate_joint_transforms(
                    animation, animation.current_time,
                    joint,
                    local_transform, &mut buffer),
                None => continue
            }
            let mapped = mesh.ssbo.as_ref().unwrap().buffer.mapped_memory;
            unsafe {
                std::ptr::copy_nonoverlapping(buffer.as_ptr() as *const std::ffi::c_void, mapped, buffer_size);
            }
        }

    }

    pub fn render(&self, inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
                  dynamic_alignment: u64,
                  push_constant: PushConstant, viewport: ash::vk::Viewport, scissor: ash::vk::Rect2D) {
        let dynamic_offset = dynamic_alignment *
            ash::vk::DeviceSize::try_from(self.model_index).unwrap();
        let dynamic_offset = u32::try_from(dynamic_offset).unwrap();
        let graphics_ptr = self.graphics.upgrade().unwrap();
        let graphics_lock = graphics_ptr.read().unwrap();
        unsafe {
            let mut push_constant = push_constant;
            for mesh in self.skinned_meshes.iter() {
                let pipeline = graphics_lock.pipeline
                    .get_pipeline(ShaderType::AnimatedModel, 0);
                let inheritance = inheritance_info.load(Ordering::SeqCst)
                    .as_ref()
                    .unwrap();
                for primitive in mesh.primitives.iter() {
                    let command_buffer_begin_info = CommandBufferBeginInfo::builder()
                        .inheritance_info(inheritance)
                        .flags(CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                        .build();
                    let command_buffer = primitive.command_buffer.unwrap();
                    let result = graphics_lock
                        .logical_device
                        .begin_command_buffer(command_buffer, &command_buffer_begin_info);
                    if let Err(e) = result {
                        log::error!("Error beginning secondary command buffer: {}", e.to_string());
                    }
                    graphics_lock.logical_device.cmd_set_viewport(command_buffer, 0, &[viewport]);
                    graphics_lock.logical_device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                    graphics_lock.logical_device.cmd_bind_pipeline(
                        command_buffer, PipelineBindPoint::GRAPHICS,
                        pipeline
                    );
                    let pipeline_layout = graphics_lock
                        .pipeline.get_pipeline_layout(ShaderType::AnimatedModel);
                    push_constant.object_color = self.color;
                    let casted = bytemuck::cast::<PushConstant, [u8; 32]>(push_constant);
                    graphics_lock.logical_device
                        .cmd_push_constants(command_buffer, pipeline_layout,
                                            ShaderStageFlags::FRAGMENT, 0, &casted[0..]);
                    let vertex_buffers = [
                        primitive.get_vertex_buffer()
                    ];
                    let index_buffer = primitive.get_index_buffer();
                    graphics_lock.logical_device.cmd_bind_descriptor_sets(
                        command_buffer, PipelineBindPoint::GRAPHICS,
                        pipeline_layout, 0,
                        &[graphics_lock.descriptor_sets[0]], &[dynamic_offset, dynamic_offset]
                    );
                    if let Some(sampler_resource) = primitive.sampler_resource.as_ref() {
                        match sampler_resource {
                            SamplerResource::DescriptorSet(set) => {
                                graphics_lock.logical_device
                                    .cmd_bind_descriptor_sets(
                                        command_buffer, PipelineBindPoint::GRAPHICS,
                                        pipeline_layout, 1, &[*set], &[]
                                    );
                            },
                        }
                    }
                    if let Some(ssbo) = mesh.ssbo.as_ref() {
                        graphics_lock.logical_device
                            .cmd_bind_descriptor_sets(
                                command_buffer, PipelineBindPoint::GRAPHICS,
                                pipeline_layout, 2, &[ssbo.descriptor_set],
                                &[]
                            );
                    }
                    graphics_lock.logical_device
                        .cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers[0..], &[0]);
                    graphics_lock.logical_device
                        .cmd_bind_index_buffer(command_buffer, index_buffer, 0, IndexType::UINT32);
                    graphics_lock.logical_device
                        .cmd_draw_indexed(
                            command_buffer, primitive.indices.len() as u32,
                            1, 0, 0, 0
                        );
                    let result = graphics_lock.logical_device.end_command_buffer(command_buffer);
                    if let Err(e) = result {
                        log::error!("Error ending command buffer: {}", e.to_string());
                    }
                }
            }
        }
        drop(graphics_lock);
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
            graphics: model.graphics.clone(),
            current_frame: 0.0,
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