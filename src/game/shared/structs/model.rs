use ash::version::DeviceV1_0;
use ash::vk::{
    CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferUsageFlags,
    CommandPool, IndexType, PipelineBindPoint, ShaderStageFlags,
};
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
use gltf::{Node, Scene};
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::mem::ManuallyDrop;
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc, Weak,
};
use tokio::task::JoinHandle;

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{Mesh, Primitive, PushConstant, Vertex};
use crate::game::shared::traits::disposable::Disposable;
use crate::game::traits::GraphicsBase;
use crate::game::util::read_raw_data;
use downcast_rs::__std::collections::HashMap;

pub struct Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
    pub color: Vec4,
    pub meshes: Vec<Mesh<BufferType, CommandType, TextureType>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub model_index: usize,
    graphics: Weak<ShardedLock<GraphicsType>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn create_model(
        file_name: &str,
        document: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        images: Vec<Arc<ShardedLock<TextureType>>>,
        graphics: Weak<ShardedLock<GraphicsType>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        texture_index_offset: usize,
    ) -> Self {
        let meshes = Self::process_model(&document, &buffers, images, texture_index_offset);

        let x: f32 = rotation.x();
        let y: f32 = rotation.y();
        let z: f32 = rotation.z();

        Model {
            position,
            scale,
            rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
            color,
            graphics,
            meshes,
            is_disposed: false,
            model_name: file_name.to_string(),
            model_index: 0,
        }
    }

    fn process_model(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: Vec<Arc<ShardedLock<TextureType>>>,
        texture_index_offset: usize,
    ) -> Vec<Mesh<BufferType, CommandType, TextureType>> {
        let meshes = if let Some(scene) = document.default_scene() {
            Self::process_root_nodes(scene, buffers, images, texture_index_offset)
        } else {
            Self::process_root_nodes(
                document.scenes().next().unwrap(),
                buffers,
                images,
                texture_index_offset,
            )
        };
        meshes
    }

    fn process_root_nodes(
        scene: Scene,
        buffers: &[gltf::buffer::Data],
        images: Vec<Arc<ShardedLock<TextureType>>>,
        texture_index_offset: usize,
    ) -> Vec<Mesh<BufferType, CommandType, TextureType>> {
        let mut meshes = Vec::with_capacity(150);
        for node in scene.nodes() {
            let mut submeshes = Self::process_node(
                node,
                buffers,
                &images,
                Mat4::identity(),
                texture_index_offset,
            );
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_node(
        node: Node,
        buffers: &[gltf::buffer::Data],
        images: &[Arc<ShardedLock<TextureType>>],
        local_transform: Mat4,
        texture_index_offset: usize,
    ) -> Vec<Mesh<BufferType, CommandType, TextureType>> {
        let mut meshes = Vec::with_capacity(10);
        let (t, r, s) = node.transform().decomposed();
        let transform =
            Mat4::from_scale_rotation_translation(Vec3::from(s), Quat::from(r), Vec3::from(t));
        let transform = local_transform * transform;
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_mesh(
                mesh,
                buffers,
                transform,
                images,
                texture_index_offset,
            ));
        }
        for _node in node.children() {
            let mut submeshes =
                Self::process_node(_node, buffers, images, transform, texture_index_offset);
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_mesh(
        mesh: gltf::Mesh,
        buffers: &[gltf::buffer::Data],
        local_transform: Mat4,
        images: &[Arc<ShardedLock<TextureType>>],
        texture_index_offset: usize,
    ) -> Mesh<BufferType, CommandType, TextureType> {
        let mut primitives = Vec::with_capacity(5);
        let mut textures = Vec::with_capacity(5);
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions = reader.read_positions();
            let normals = reader.read_normals();
            let uvs = reader.read_tex_coords(0);
            let indices = reader
                .read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<_>>();

            let mut vertices = match (positions, normals, uvs) {
                (Some(positions), Some(normals), Some(uvs)) => {
                    let _vertices = positions
                        .zip(normals)
                        .zip(uvs.into_f32())
                        .map(|((pos, normal), uv)| Vertex {
                            position: Vec3A::from(pos),
                            normal: Vec3A::from(normal),
                            uv: Vec2::from(uv),
                        })
                        .collect::<Vec<_>>();
                    _vertices
                }
                (Some(positions), Some(normals), None) => positions
                    .zip(normals)
                    .map(|(pos, normal)| Vertex {
                        position: Vec3A::from(pos),
                        normal: Vec3A::from(normal),
                        uv: Vec2::new(0.0, 0.0),
                    })
                    .collect::<Vec<_>>(),
                (positions, normals, uvs) => {
                    unimplemented!(
                        "Unsupported combination of values. Positions: {}, Normals: {}, UVs: {}",
                        positions.is_some(),
                        normals.is_some(),
                        uvs.is_some()
                    );
                }
            };

            let texture_index = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|x| x.texture().index());
            let texture = texture_index.and_then(|index| images.get(index).cloned());
            if let Some(t) = texture {
                textures.push(t);
            }
            let texture_index = texture_index.map(|index| index + texture_index_offset);

            for vertex in vertices.iter_mut() {
                vertex.position =
                    Vec3A::from(local_transform.transform_point3(Vec3::from(vertex.position)));
            }

            primitives.push(Primitive {
                vertices,
                indices,
                texture_index,
                is_disposed: false,
            });
        }

        Mesh {
            primitives,
            vertex_buffer: None,
            index_buffer: None,
            texture: textures,
            is_disposed: false,
            sampler_resource: None,
            command_buffer: None,
            command_pool: None,
        }
    }

    pub fn get_world_matrix(&self) -> Mat4 {
        let world = Mat4::identity();
        let scale = Mat4::from_scale(glam::Vec3::from(self.scale));
        let translation = Mat4::from_translation(glam::Vec3::from(self.position));
        let rotate =
            Mat4::from_rotation_ypr(self.rotation.y(), self.rotation.x(), self.rotation.z());
        world * translation * rotate * scale
    }
}

impl Model<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        file_name: &'static str,
        graphics: Weak<ShardedLock<Graphics>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        model_index: usize,
    ) -> anyhow::Result<JoinHandle<Self>> {
        log::info!("Loading model {}...", file_name);
        let graphics_arc = graphics.upgrade().unwrap();
        let model = tokio::spawn(async move {
            let graphics_clone = graphics_arc;
            let thread_count: usize;
            let command_pool: Arc<Mutex<CommandPool>>;
            {
                let graphics_lock = graphics_clone.read().unwrap();
                thread_count = graphics_lock.thread_pool.thread_count;
                command_pool = graphics_lock.thread_pool.threads[model_index % thread_count]
                    .command_pool
                    .clone();
            }
            log::info!(
                "Model index: {}, Command pool: {:?}",
                model_index,
                command_pool
            );
            let (document, buffers, images) = read_raw_data(file_name).unwrap();
            let (textures, texture_index_offset) = Graphics::create_gltf_textures(
                images,
                graphics_clone.clone(),
                command_pool.clone(),
            )
            .await
            .unwrap();
            let mut loaded_model = Self::create_model(
                file_name,
                document,
                buffers,
                textures,
                graphics.clone(),
                position,
                scale,
                rotation,
                color,
                texture_index_offset,
            );
            {
                let graphics_lock = graphics_clone.read().unwrap();
                for mesh in loaded_model.meshes.iter_mut() {
                    let command_buffer =
                        graphics_lock.create_secondary_command_buffer(command_pool.clone());
                    mesh.command_pool = Some(command_pool.clone());
                    mesh.command_buffer = Some(command_buffer);
                }
                drop(graphics_lock);
            }
            loaded_model.create_buffers(graphics_clone).await.unwrap();
            loaded_model
        });
        Ok(model)
    }

    async fn create_buffers(&mut self, graphics: Arc<ShardedLock<Graphics>>) -> anyhow::Result<()> {
        let mut handles = HashMap::new();
        for (index, mesh) in self.meshes.iter().enumerate() {
            log::info!("Creating buffer for mesh {}...", index);
            let vertices = mesh
                .primitives
                .iter()
                .map(|p| &p.vertices)
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let indices = mesh
                .primitives
                .iter()
                .map(|p| &p.indices)
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let cmd_pool = mesh.command_pool.clone().unwrap();
            let pool = cmd_pool.clone();
            let g = graphics.clone();
            let buffer_result =
                tokio::spawn(
                    async move { Graphics::create_buffer(g, vertices, indices, pool).await },
                );
            handles.insert(index, buffer_result);
        }
        for (index, mesh) in self.meshes.iter_mut().enumerate() {
            if let Some(result) = handles.get_mut(&index) {
                let (vertex_buffer, index_buffer) = result.await??;
                mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
            }
        }
        Ok(())
    }

    pub fn update(&mut self, _delta_time: f64) {}

    pub fn render(
        &self,
        inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
        push_constant: PushConstant,
        viewport: ash::vk::Viewport,
        scissor: ash::vk::Rect2D,
    ) {
        let graphics_ptr = self.graphics.upgrade().unwrap();
        let graphics_lock = graphics_ptr.read().unwrap();
        let mut push_constant = push_constant;
        push_constant.object_color = self.color;
        push_constant.model_index = self.model_index;
        unsafe {
            let inheritance = inheritance_info.load(Ordering::SeqCst).as_ref().unwrap();
            for mesh in self.meshes.iter() {
                let shader_type = if !mesh.texture.is_empty() {
                    ShaderType::BasicShader
                } else {
                    ShaderType::BasicShaderWithoutTexture
                };
                let pipeline_layout = graphics_lock.pipeline.get_pipeline_layout(shader_type);
                let pipeline = graphics_lock.pipeline.get_pipeline(shader_type, 0);
                let command_buffer_begin_info = CommandBufferBeginInfo::builder()
                    .inheritance_info(inheritance)
                    .flags(CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                    .build();
                let command_buffer = mesh.command_buffer.unwrap();
                let result = graphics_lock
                    .logical_device
                    .begin_command_buffer(command_buffer, &command_buffer_begin_info);
                if let Err(e) = result {
                    log::error!(
                        "Error beginning secondary command buffer: {}",
                        e.to_string()
                    );
                }
                graphics_lock
                    .logical_device
                    .cmd_set_viewport(command_buffer, 0, &[viewport]);
                graphics_lock
                    .logical_device
                    .cmd_set_scissor(command_buffer, 0, &[scissor]);
                graphics_lock.logical_device.cmd_bind_pipeline(
                    command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    pipeline,
                );
                graphics_lock.logical_device.cmd_bind_descriptor_sets(
                    command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    0,
                    &[graphics_lock.descriptor_sets[0]],
                    &[],
                );
                let vertex_buffers = [mesh.get_vertex_buffer()];
                let index_buffer = mesh.get_index_buffer();
                /*if !mesh.texture.is_empty() {
                    match mesh.sampler_resource.as_ref() {
                        Some(res) => {
                            match res {
                                SamplerResource::DescriptorSet(set) => {
                                    graphics_lock.logical_device
                                        .cmd_bind_descriptor_sets(
                                            command_buffer, PipelineBindPoint::GRAPHICS,
                                            pipeline_layout, 1, &[*set], &[]
                                        );
                                }
                            }
                        },
                        None => (),
                    }
                }*/

                let mut vertex_offset_index = 0;
                let mut index_offset_index = 0;
                for primitive in mesh.primitives.iter() {
                    push_constant.texture_index = primitive.texture_index.unwrap_or_default();
                    let casted = bytemuck::cast::<PushConstant, [u8; 48]>(push_constant);
                    graphics_lock.logical_device.cmd_push_constants(
                        command_buffer,
                        pipeline_layout,
                        ShaderStageFlags::FRAGMENT | ShaderStageFlags::VERTEX,
                        0,
                        &casted[0..],
                    );
                    graphics_lock.logical_device.cmd_bind_vertex_buffers(
                        command_buffer,
                        0,
                        &vertex_buffers[0..],
                        &[0],
                    );
                    graphics_lock.logical_device.cmd_bind_index_buffer(
                        command_buffer,
                        index_buffer,
                        0,
                        IndexType::UINT32,
                    );
                    graphics_lock.logical_device.cmd_draw_indexed(
                        command_buffer,
                        u32::try_from(primitive.indices.len()).unwrap(),
                        1,
                        index_offset_index,
                        vertex_offset_index,
                        0,
                    );
                    vertex_offset_index += primitive.vertices.len() as i32;
                    index_offset_index += primitive.indices.len() as u32;
                }
                let result = graphics_lock
                    .logical_device
                    .end_command_buffer(command_buffer);
                if let Err(e) = result {
                    log::error!("Error ending command buffer: {}", e.to_string());
                }
            }
            drop(graphics_lock);
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    From<&Model<GraphicsType, BufferType, CommandType, TextureType>>
    for Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn from(model: &Self) -> Self {
        loop {
            let is_buffer_completed = model
                .meshes
                .iter()
                .all(|mesh| mesh.vertex_buffer.is_some() && mesh.index_buffer.is_some());
            if is_buffer_completed {
                break;
            }
        }
        let mut _model = Model {
            position: model.position,
            scale: model.scale,
            rotation: model.rotation,
            color: model.color,
            graphics: model.graphics.clone(),
            meshes: model.meshes.to_vec(),
            is_disposed: true,
            model_name: model.model_name.clone(),
            model_index: 0,
        };

        _model
            .meshes
            .iter_mut()
            .for_each(|mesh| mesh.is_disposed = true);
        _model
    }
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Send
    for Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}
unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Sync
    for Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        log::info!(
            "Dropping model...Model: {}, Model Index: {}",
            self.model_name.as_str(),
            self.model_index
        );
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped model.");
        } else {
            log::warn!("Model is already dropped.");
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable
    for Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn dispose(&mut self) {
        log::info!(
            "Disposing model...Model: {}, Model index: {}",
            self.model_name.as_str(),
            self.model_index
        );
        for mesh in self.meshes.iter_mut() {
            mesh.dispose();
        }
        self.is_disposed = true;
        log::info!("Successfully disposed model.");
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
