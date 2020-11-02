use ash::vk::{
    CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferUsageFlags,
    CommandPool, DescriptorSet, IndexType, PipelineBindPoint, Rect2D, ShaderStageFlags, Viewport,
};
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
use gltf::animation::util::ReadOutputs;
use gltf::{scene::Transform, Node, Scene};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};

use crate::game::graphics::vk::{Buffer, Graphics, Image, Pipeline, ThreadPool};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{
    generate_joint_transforms, Animation, Channel, ChannelOutputs, ModelMetaData, SkinnedMesh,
    SkinnedPrimitive, SkinnedVertex, Vertex, SSBO,
};
use crate::game::shared::traits::Renderable;
use crate::game::structs::{Joint, PushConstant};
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::util::read_raw_data;
use ash::version::DeviceV1_0;
use ash::Device;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub struct SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
    pub model_metadata: ModelMetaData,
    pub skinned_meshes: Vec<Arc<Mutex<SkinnedMesh<BufferType, CommandType, TextureType>>>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub ssbo_index: usize,
    pub animations: HashMap<String, Animation>,
    graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn create_model(
        file_name: &str,
        model_index: Arc<AtomicUsize>,
        ssbo_index: usize,
        document: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        images: Vec<Arc<ShardedLock<TextureType>>>,
        graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        texture_index_offset: usize,
    ) -> Self {
        let meshes = Self::process_model(
            &document,
            &buffers,
            images,
            texture_index_offset,
            model_index,
        );
        let meshes = meshes
            .into_iter()
            .map(|m| Arc::new(Mutex::new(m)))
            .collect::<Vec<_>>();
        let animations = Self::process_animation(&document, &buffers);
        for (name, _) in animations.iter() {
            log::info!("Animation: {}", &name);
        }
        SkinnedModel {
            position,
            scale,
            rotation,
            model_metadata: ModelMetaData {
                world_matrix: Mat4::identity(),
                object_color: color,
                reflectivity: 1.0,
                shine_damper: 10.0,
            },
            skinned_meshes: meshes,
            is_disposed: false,
            model_name: file_name.to_string(),
            ssbo_index,
            animations,
            graphics,
        }
    }

    fn process_model(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: Vec<Arc<ShardedLock<TextureType>>>,
        texture_index_offset: usize,
        model_index: Arc<AtomicUsize>,
    ) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let meshes = if let Some(scene) = document.default_scene() {
            Self::process_root_nodes(scene, buffers, images, texture_index_offset, model_index)
        } else {
            Self::process_root_nodes(
                document.scenes().next().unwrap(),
                buffers,
                images,
                texture_index_offset,
                model_index,
            )
        };
        log::info!("Skinned model mesh count: {}", meshes.len());
        meshes
    }

    fn process_root_nodes(
        scene: Scene,
        buffers: &[gltf::buffer::Data],
        images: Vec<Arc<ShardedLock<TextureType>>>,
        texture_index_offset: usize,
        model_index: Arc<AtomicUsize>,
    ) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut sub_meshes = Self::process_node(
                node,
                buffers,
                &images,
                Mat4::identity(),
                texture_index_offset,
                model_index.clone(),
            );
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_node(
        node: Node,
        buffers: &[gltf::buffer::Data],
        images: &[Arc<ShardedLock<TextureType>>],
        local_transform: Mat4,
        texture_index_offset: usize,
        model_index: Arc<AtomicUsize>,
    ) -> Vec<SkinnedMesh<BufferType, CommandType, TextureType>> {
        let mut meshes = Vec::with_capacity(10);
        let (t, r, s) = node.transform().decomposed();
        let transform =
            Mat4::from_scale_rotation_translation(Vec3::from(s), Quat::from(r), Vec3::from(t));
        let transform = local_transform * transform;
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_skinned_mesh(
                &node,
                mesh,
                buffers,
                transform,
                images,
                texture_index_offset,
                model_index.clone(),
            ));
        }
        for _node in node.children() {
            let mut sub_meshes = Self::process_node(
                _node,
                buffers,
                images,
                transform,
                texture_index_offset,
                model_index.clone(),
            );
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_skinned_mesh(
        node: &Node,
        mesh: gltf::Mesh,
        buffers: &[gltf::buffer::Data],
        local_transform: Mat4,
        images: &[Arc<ShardedLock<TextureType>>],
        texture_index_offset: usize,
        model_index: Arc<AtomicUsize>,
    ) -> SkinnedMesh<BufferType, CommandType, TextureType> {
        let mut root_joint = None;
        if let Some(skin) = node.skin() {
            let joints: Vec<_> = skin.joints().collect();
            if !joints.is_empty() {
                let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
                let ibm: Vec<Mat4> = reader
                    .read_inverse_bind_matrices()
                    .unwrap()
                    .map(|r| Mat4::from_cols_array_2d(&r))
                    .collect();
                let node_to_joints_lookup: Vec<_> = joints.iter().map(|n| n.index()).collect();
                root_joint = Some(Self::process_skeleton(
                    &joints[0],
                    &node_to_joints_lookup,
                    ibm.as_slice(),
                    Mat4::identity(),
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
            let indices = reader
                .read_indices()
                .unwrap()
                .into_u32()
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
                                uv: Vec2::from(uv),
                            },
                            joints: Vec4::new(
                                joints[0] as f32,
                                joints[1] as f32,
                                joints[2] as f32,
                                joints[3] as f32,
                            ),
                            weights: Vec4::from(weights),
                        })
                        .collect::<Vec<_>>();
                    vertices
                }
                (Some(positions), Some(normals), Some(uvs), None, None) => {
                    let vertices = positions
                        .zip(normals)
                        .zip(uvs.into_f32())
                        .map(|((pos, normals), uv)| SkinnedVertex {
                            vertex: Vertex {
                                position: Vec3A::from(pos),
                                normal: Vec3A::from(normals),
                                uv: Vec2::from(uv),
                            },
                            joints: Vec4::zero(),
                            weights: Vec4::zero(),
                        })
                        .collect::<Vec<_>>();
                    vertices
                }
                (positions, normals, uvs, joints, weights) => {
                    unimplemented!("This method doesn't support loading static meshes. Positions: {:?}, Normals: {:?}, UVs: {:?}, Joints: {:?}, Weights: {:?}", positions.is_some(), normals.is_some(), uvs.is_some(), joints.is_some(), weights.is_some());
                }
            };

            let texture_index = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|x| x.texture().index());
            let texture = texture_index.and_then(|x| images.get(x).cloned());
            let texture_index = texture_index.map(|index| index + texture_index_offset);

            let shader_type = if texture.is_none() {
                ShaderType::BasicShaderWithoutTexture
            } else {
                ShaderType::AnimatedModel
            };
            let skinned_primitive = SkinnedPrimitive {
                vertices: skinned_vertices,
                indices,
                vertex_buffer: None::<ManuallyDrop<BufferType>>,
                index_buffer: None::<ManuallyDrop<BufferType>>,
                texture,
                texture_index: texture_index.unwrap_or_default(),
                is_disposed: false,
                command_data: std::collections::HashMap::new(),
                sampler_resource: None,
                shader_type,
            };
            skinned_primitives.push(skinned_primitive);
        }

        SkinnedMesh {
            primitives: skinned_primitives,
            is_disposed: false,
            transform: local_transform,
            root_joint,
            ssbo: None,
            model_index: model_index.fetch_add(1, Ordering::SeqCst),
        }
    }

    fn process_skeleton(
        node: &Node,
        node_to_joints_lookup: &[usize],
        inverse_bind_matrices: &[Mat4],
        local_transform: Mat4,
    ) -> Joint {
        let mut children = vec![];
        let node_index = node.index();
        let index = node_to_joints_lookup
            .iter()
            .enumerate()
            .find(|(_, x)| **x == node_index);
        let index = index.unwrap().0;
        let name = node.name().unwrap_or("");
        let ibm = inverse_bind_matrices[index];
        let (t, r, s) = node.transform().decomposed();
        let node_transform =
            Mat4::from_scale_rotation_translation(Vec3::from(s), Quat::from(r), Vec3::from(t));
        let pose_transform = local_transform * node_transform;
        for child in node.children() {
            let skeleton = Self::process_skeleton(
                &child,
                node_to_joints_lookup,
                inverse_bind_matrices,
                pose_transform,
            );
            children.push(skeleton);
        }
        let ibm = ibm;
        let (t, r, s) = match node.transform() {
            Transform::Matrix { matrix } => {
                let mat = Mat4::from_cols_array_2d(&matrix);
                let (t, r, s) = mat.to_scale_rotation_translation();
                (t, r, s)
            }
            Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => {
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
            children,
        }
    }

    fn process_animation(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> HashMap<String, Animation> {
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
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                let inputs = reader.read_inputs().unwrap().collect::<Vec<_>>();
                let outputs = match reader.read_outputs().unwrap() {
                    ReadOutputs::Translations(translations) => {
                        ChannelOutputs::Translations(translations.map(Vec3A::from).collect())
                    }
                    ReadOutputs::Rotations(rotations) => {
                        ChannelOutputs::Rotations(rotations.into_f32().map(Quat::from).collect())
                    }
                    ReadOutputs::Scales(scales) => {
                        ChannelOutputs::Scales(scales.map(Vec3A::from).collect())
                    }
                    ReadOutputs::MorphTargetWeights(_) => {
                        unimplemented!("glTF property::MorphTargetWeights is unimplemented.")
                    }
                };
                channels.push(Channel {
                    target_node_index,
                    inputs,
                    outputs,
                    interpolation,
                });
            }
            animations.insert(
                name,
                Animation {
                    channels,
                    current_time: 0.0,
                },
            );
        }
        log::info!("Animation count: {}", animations.len());
        animations
    }
}

impl SkinnedModel<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        file_name: &'static str,
        graphics: Weak<RwLock<ManuallyDrop<Graphics>>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        ssbo_index: usize,
        model_index: Arc<AtomicUsize>,
    ) -> anyhow::Result<Receiver<Self>> {
        log::info!("Loading skinned model from glTF {}...", file_name);
        let graphics_arc = graphics.upgrade().unwrap();
        let (model_send, model_recv) = bounded(5);
        rayon::spawn(move || {
            let graphics_arc = graphics_arc;
            let command_pool: Arc<Mutex<CommandPool>>;
            {
                let graphics = graphics_arc.read();
                command_pool = graphics.get_idle_command_pool();
            }
            log::info!("Skinned model index: {}", ssbo_index);
            let (document, buffers, images) =
                read_raw_data(file_name).expect("Failed to read raw data from glTF.");
            let (textures, texture_index_offset) =
                Graphics::create_gltf_textures(images, graphics_arc.clone(), command_pool.clone())
                    .expect("Failed to create glTF textures.");
            let mut loaded_model = Self::create_model(
                file_name,
                model_index,
                ssbo_index,
                document,
                buffers,
                textures,
                graphics,
                position,
                scale,
                rotation,
                color,
                texture_index_offset,
            );
            loaded_model.model_metadata.world_matrix = loaded_model.get_world_matrix();
            {
                let graphics_lock = graphics_arc.read();
                let inflight_frame_count = std::env::var("INFLIGHT_BUFFER_COUNT")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                for mesh in loaded_model.skinned_meshes.iter_mut() {
                    let mut mesh_lock = mesh.lock();
                    let model_index = mesh_lock.model_index;
                    for primitive in mesh_lock.primitives.iter_mut() {
                        for i in 0..inflight_frame_count {
                            let (pool, command_buffer) =
                                Graphics::get_command_pool_and_secondary_command_buffer(
                                    &*graphics_lock,
                                    model_index,
                                    i,
                                );
                            let entry = primitive
                                .command_data
                                .entry(i)
                                .or_insert((None, CommandBuffer::null()));
                            (*entry) = (Some(pool), command_buffer);
                        }
                    }
                }
            }
            loaded_model
                .create_buffers(graphics_arc)
                .expect("Failed to create buffers for skinned model.");
            model_send
                .send(loaded_model)
                .expect("Failed to send model result.");
        });
        Ok(model_recv)
    }

    fn create_buffers(
        &mut self,
        graphics: Arc<RwLock<ManuallyDrop<Graphics>>>,
    ) -> anyhow::Result<()> {
        let mut handles = HashMap::new();
        for (index, mesh) in self.skinned_meshes.iter().enumerate() {
            let mut mesh_lock = mesh.lock();
            for primitive in mesh_lock.primitives.iter_mut() {
                let graphics_clone = graphics.clone();
                let vertices = primitive.vertices.clone();
                let indices = primitive.indices.clone();
                let cmd_pool = primitive
                    .command_data
                    .get(&0)
                    .map(|(pool, _)| pool.clone().unwrap())
                    .unwrap();
                let (buffer_send, buffer_recv) = bounded(5);
                rayon::spawn(move || {
                    let result = Graphics::create_vertex_and_index_buffer(
                        graphics_clone,
                        vertices,
                        indices,
                        cmd_pool,
                    );
                    buffer_send
                        .send(result)
                        .expect("Failed to send buffer result.");
                });
                let entry = handles.entry(index).or_insert_with(Vec::new);
                (*entry).push(buffer_recv);
            }
        }
        for (index, mesh) in self.skinned_meshes.iter().enumerate() {
            let mesh_handles = handles.get_mut(&index).unwrap();
            let mut mesh_lock = mesh.lock();
            let zipped = mesh_lock
                .primitives
                .iter_mut()
                .zip(mesh_handles.iter_mut())
                .collect::<Vec<_>>();
            for (primitive, handle) in zipped {
                let (vertex_buffer, index_buffer) = handle.recv()??;
                primitive.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                primitive.index_buffer = Some(ManuallyDrop::new(index_buffer));
            }
        }
        Ok(())
    }
}

/*impl<GraphicsType, BufferType, CommandType, TextureType>
    From<&SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>>
    for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn from(model: &SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>) -> Self {
        loop {
            let is_buffer_completed = model.skinned_meshes.iter().all(|mesh| {
                mesh.lock().primitives.iter().all(|primitive| {
                    primitive.vertex_buffer.is_some() && primitive.index_buffer.is_some()
                })
            });
            if is_buffer_completed {
                break;
            }
        }
        SkinnedModel {
            position: model.position,
            scale: model.scale,
            rotation: model.rotation,
            model_metadata: model.model_metadata,
            skinned_meshes: model.skinned_meshes.to_vec(),
            is_disposed: true,
            model_name: model.model_name.clone(),
            ssbo_index: 0,
            animations: model.animations.clone(),
            graphics: model.graphics.clone(),
        }
    }
}*/

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Send
    for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Sync
    for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

impl<GraphicsType, BufferType, CommandType, TextureType> Clone
    for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn clone(&self) -> Self {
        loop {
            let is_buffer_completed = self.skinned_meshes.iter().all(|m| {
                let mesh_lock = m.lock();
                let completed = mesh_lock
                    .primitives
                    .iter()
                    .all(|p| p.vertex_buffer.is_some() && p.index_buffer.is_some());
                completed
            });
            if is_buffer_completed {
                break;
            }
        }
        SkinnedModel {
            position: self.position,
            scale: self.scale,
            rotation: self.rotation,
            model_metadata: self.model_metadata,
            skinned_meshes: self.skinned_meshes.clone(),
            is_disposed: true,
            model_name: self.model_name.clone(),
            ssbo_index: 0,
            animations: self.animations.clone(),
            graphics: self.graphics.clone(),
        }
    }
}

impl Renderable<Graphics, Buffer, CommandBuffer, Image>
    for SkinnedModel<Graphics, Buffer, CommandBuffer, Image>
{
    fn update(&mut self, delta_time: f64) {
        let mut keys = self.animations.keys();
        let animation_name = keys.next().cloned().unwrap();
        let animation = self.animations.get_mut(&animation_name).unwrap();
        animation.current_time += delta_time as f32;
        let animation_end_time = *animation.channels.last().unwrap().inputs.last().unwrap();
        if animation.current_time > animation_end_time {
            animation.current_time -= animation_end_time;
        }
        let buffer_size = std::mem::size_of::<Mat4>() * 500;
        for mesh in self.skinned_meshes.iter() {
            let mesh_lock = mesh.lock();
            let mut buffer = [Mat4::identity(); 500];
            let local_transform = mesh_lock.transform;
            match mesh_lock.root_joint.as_ref() {
                Some(joint) => generate_joint_transforms(
                    animation,
                    animation.current_time,
                    joint,
                    local_transform,
                    &mut buffer,
                ),
                None => continue,
            }
            let mapped = mesh_lock.ssbo.as_ref().unwrap().buffer.mapped_memory;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    buffer.as_ptr() as *const std::ffi::c_void,
                    mapped,
                    buffer_size,
                );
            }
        }
    }

    fn render(
        &self,
        inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
        push_constant: PushConstant,
        viewport: Viewport,
        scissor: Rect2D,
        device: Arc<Device>,
        pipeline: Arc<ShardedLock<ManuallyDrop<Pipeline>>>,
        descriptor_set: DescriptorSet,
        thread_pool: Arc<ThreadPool>,
        frame_index: usize,
    ) {
        let thread_count = thread_pool.thread_count;
        let pipeline_layout = pipeline
            .read()
            .expect("Failed to lock pipeline when acquiring pipeline layout.")
            .get_pipeline_layout(ShaderType::AnimatedModel);
        let pipeline = pipeline
            .read()
            .expect("Failed to lock pipeline when getting the graphics pipeline.")
            .get_pipeline(ShaderType::AnimatedModel, 0);
        let mut push_constant = push_constant;
        push_constant.model_index = self.ssbo_index;
        unsafe {
            for mesh in self.skinned_meshes.iter() {
                let mesh_clone = mesh.clone();
                let mesh_lock = mesh.lock();
                let model_index = mesh_lock.model_index;
                drop(mesh_lock);
                let inheritance_clone = inheritance_info.clone();
                let device_clone = device.clone();
                thread_pool.threads[model_index % thread_count]
                    .add_job(move || {
                        let device = device_clone;
                        let inheritance =
                            inheritance_clone.load(Ordering::SeqCst).as_ref().unwrap();
                        let mesh = mesh_clone;
                        let mesh_lock = mesh.lock();
                        for primitive in mesh_lock.primitives.iter() {
                            let command_buffer_begin_info = CommandBufferBeginInfo::builder()
                                .inheritance_info(inheritance)
                                .flags(CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                                .build();
                            let (_, command_buffer) =
                                primitive.command_data.get(&frame_index).unwrap();
                            let command_buffer = *command_buffer;
                            let result = device
                                .begin_command_buffer(command_buffer, &command_buffer_begin_info);
                            if let Err(e) = result {
                                log::error!(
                                    "Error beginning secondary command buffer: {}",
                                    e.to_string()
                                );
                            }
                            device.cmd_set_viewport(command_buffer, 0, &[viewport]);
                            device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                            device.cmd_bind_pipeline(
                                command_buffer,
                                PipelineBindPoint::GRAPHICS,
                                pipeline,
                            );
                            device.cmd_bind_descriptor_sets(
                                command_buffer,
                                PipelineBindPoint::GRAPHICS,
                                pipeline_layout,
                                0,
                                &[descriptor_set],
                                &[],
                            );
                            push_constant.texture_index = primitive.texture_index;
                            let casted = bytemuck::cast::<PushConstant, [u8; 32]>(push_constant);
                            device.cmd_push_constants(
                                command_buffer,
                                pipeline_layout,
                                ShaderStageFlags::FRAGMENT | ShaderStageFlags::VERTEX,
                                0,
                                &casted[0..],
                            );
                            let vertex_buffers = [primitive.get_vertex_buffer()];
                            let index_buffer = primitive.get_index_buffer();
                            if let Some(ssbo) = mesh_lock.ssbo.as_ref() {
                                device.cmd_bind_descriptor_sets(
                                    command_buffer,
                                    PipelineBindPoint::GRAPHICS,
                                    pipeline_layout,
                                    1,
                                    &[ssbo.descriptor_set],
                                    &[],
                                );
                            }
                            device.cmd_bind_vertex_buffers(
                                command_buffer,
                                0,
                                &vertex_buffers[0..],
                                &[0],
                            );
                            device.cmd_bind_index_buffer(
                                command_buffer,
                                index_buffer,
                                0,
                                IndexType::UINT32,
                            );
                            device.cmd_draw_indexed(
                                command_buffer,
                                primitive.indices.len() as u32,
                                1,
                                0,
                                0,
                                0,
                            );
                            let result = device.end_command_buffer(command_buffer);
                            if let Err(e) = result {
                                log::error!("Error ending command buffer: {}", e.to_string());
                            }
                        }
                    })
                    .expect("Failed to push work into the worker thread.");
            }
        }
    }

    fn get_ssbo_index(&self) -> usize {
        self.ssbo_index
    }

    fn get_model_metadata(&self) -> ModelMetaData {
        self.model_metadata
    }

    fn get_position(&self) -> Vec3A {
        self.position
    }

    fn get_scale(&self) -> Vec3A {
        self.scale
    }

    fn get_rotation(&self) -> Vec3A {
        self.rotation
    }

    fn create_ssbo(&mut self) -> anyhow::Result<()> {
        let mut ssbo_handles = HashMap::new();
        let graphics = self
            .graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle.");
        for (index, _) in self.skinned_meshes.iter().enumerate() {
            let entry = ssbo_handles.entry(index).or_insert_with(Vec::new);
            let graphics_clone = graphics.clone();
            let (ssbo_send, ssbo_recv) = bounded(5);
            rayon::spawn(move || {
                let buffer = [Mat4::identity(); 500];
                ssbo_send
                    .send(SSBO::new(graphics_clone, &buffer))
                    .expect("Failed to send SSBO result.");
            });
            entry.push(ssbo_recv);
        }
        for (index, mesh) in self.skinned_meshes.iter().enumerate() {
            let ssbos = ssbo_handles
                .get_mut(&index)
                .expect("Failed to get SSBO handle.");
            let item = ssbos
                .get_mut(0)
                .expect("Failed to get ssbo result.")
                .recv()??;
            mesh.lock().ssbo = Some(item);
        }
        Ok(())
    }

    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandBuffer> {
        let buffers = self
            .skinned_meshes
            .iter()
            .map(|m| {
                m.lock()
                    .primitives
                    .iter()
                    .map(|p| {
                        p.command_data
                            .get(&frame_index)
                            .map(|(_, buffer)| *buffer)
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<Vec<_>>();
        buffers
    }

    fn set_position(&mut self, position: Vec3A) {
        self.position = position;
    }

    fn set_scale(&mut self, scale: Vec3A) {
        self.scale = scale;
    }

    fn set_rotation(&mut self, rotation: Vec3A) {
        self.rotation = rotation;
    }

    fn set_model_metadata(&mut self, model_metadata: ModelMetaData) {
        self.model_metadata = model_metadata;
    }

    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>) {
        for mesh in self.skinned_meshes.iter() {
            let mut mesh_lock = mesh.lock();
            mesh_lock.model_index = model_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn set_ssbo_index(&mut self, ssbo_index: usize) {
        self.ssbo_index = ssbo_index;
    }

    fn box_clone(&self) -> Box<dyn Renderable<Graphics, Buffer, CommandBuffer, Image> + Send> {
        Box::new(self.clone())
    }
}

/*impl CloneableRenderable<Graphics, Buffer, CommandBuffer, Image>
    for SkinnedModel<Graphics, Buffer, CommandBuffer, Image>
{
}*/

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped skinned model.");
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable
    for SkinnedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn dispose(&mut self) {
        log::info!(
            "Disposing skinned model...Skinned model: {}, Model index: {}",
            self.model_name.as_str(),
            self.ssbo_index
        );
        for mesh in self.skinned_meshes.iter_mut() {
            mesh.lock().dispose();
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
