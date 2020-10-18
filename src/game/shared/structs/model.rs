use ash::version::DeviceV1_0;
use ash::vk::{
    CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferUsageFlags,
    CommandPool, DescriptorSet, IndexType, PipelineBindPoint, ShaderStageFlags,
};
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
use gltf::{Node, Scene};
use parking_lot::{Mutex, RwLock};
use std::convert::TryFrom;
use std::mem::ManuallyDrop;
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc, Weak,
};

use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{Mesh, ModelMetaData, Primitive, PushConstant, Vertex};
use crate::game::shared::traits::disposable::Disposable;
use crate::game::traits::GraphicsBase;
use crate::game::util::read_raw_data;
use ash::Device;
use std::collections::HashMap;

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
    pub model_metadata: ModelMetaData,
    pub meshes: Vec<Mesh<BufferType, CommandType, TextureType>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub model_index: usize,
    pub graphics: Weak<RwLock<GraphicsType>>,
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
        graphics: Weak<RwLock<GraphicsType>>,
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

        let mut model = Model {
            position,
            scale,
            rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
            model_metadata: ModelMetaData {
                world_matrix: Mat4::identity(),
                object_color: color,
                reflectivity: 1.0,
                shine_damper: 10.0,
            },
            graphics,
            meshes,
            is_disposed: false,
            model_name: file_name.to_string(),
            model_index: 0,
        };
        model.model_metadata.world_matrix = model.get_world_matrix();
        model
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
        graphics: Weak<RwLock<Graphics>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        model_index: usize,
    ) -> anyhow::Result<Receiver<Self>> {
        log::info!("Loading model {}...", file_name);
        let graphics_arc = graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle for model.");
        let (model_send, model_recv) = bounded(5);
        rayon::spawn(move || {
            let graphics_arc = graphics_arc;
            let command_pool: Arc<Mutex<CommandPool>>;
            {
                let graphics_ref = &*graphics_arc.read();
                command_pool = Graphics::get_command_pool(graphics_ref, model_index);
            }
            log::info!(
                "Model index: {}, Command pool: {:?}",
                model_index,
                command_pool
            );
            let (document, buffers, images) =
                read_raw_data(file_name).expect("Failed to read raw data from glTF.");
            let (textures, texture_index_offset) =
                Graphics::create_gltf_textures(images, graphics_arc.clone(), command_pool.clone())
                    .expect("Failed to create glTF textures.");
            let mut loaded_model = Self::create_model(
                file_name,
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
            {
                let graphics_lock = graphics_arc.read();
                let device = graphics_lock.logical_device.as_ref();
                let pool = *command_pool.lock();
                for mesh in loaded_model.meshes.iter_mut() {
                    let command_buffer = Graphics::create_secondary_command_buffer(device, pool);
                    mesh.command_pool = Some(command_pool.clone());
                    mesh.command_buffer = Some(command_buffer);
                }
                drop(graphics_lock);
            }
            loaded_model
                .create_buffers(graphics_arc)
                .expect("Failed to create buffers for model.");
            model_send
                .send(loaded_model)
                .expect("Failed to send model result.");
        });
        Ok(model_recv)
    }

    fn create_buffers(&mut self, graphics: Arc<RwLock<Graphics>>) -> anyhow::Result<()> {
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
            let (buffer_send, buffer_recv) = bounded(5);
            rayon::spawn(move || {
                let result = Graphics::create_buffer(g, vertices, indices, pool)
                    .expect("Failed to create buffers for model.");
                buffer_send
                    .send(result)
                    .expect("Failed to send buffer result.");
            });
            handles.insert(index, buffer_recv);
        }
        for (index, mesh) in self.meshes.iter_mut().enumerate() {
            if let Some(result) = handles.get_mut(&index) {
                let (vertex_buffer, index_buffer) = result.recv()?;
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
        device: Arc<Device>,
        pipeline: Arc<ShardedLock<ManuallyDrop<crate::game::graphics::vk::Pipeline>>>,
        descriptor_set: DescriptorSet,
        textured_shader_type: Option<ShaderType>,
    ) {
        let mut push_constant = push_constant;
        push_constant.model_index = self.model_index;
        unsafe {
            let inheritance = inheritance_info.load(Ordering::SeqCst).as_ref().unwrap();
            for mesh in self.meshes.iter() {
                let shader_type = if !mesh.texture.is_empty() {
                    textured_shader_type.unwrap_or(ShaderType::BasicShader)
                } else {
                    ShaderType::BasicShaderWithoutTexture
                };
                let pipeline_layout = pipeline
                    .read()
                    .expect("Failed to lock pipeline when acquiring pipeline layout.")
                    .get_pipeline_layout(shader_type);
                let pipeline = pipeline
                    .read()
                    .expect("Failed to lock pipeline when getting the graphics pipeline.")
                    .get_pipeline(shader_type, 0);
                let command_buffer_begin_info = CommandBufferBeginInfo::builder()
                    .inheritance_info(inheritance)
                    .flags(CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                    .build();
                let command_buffer = mesh.command_buffer.unwrap();
                let result =
                    device.begin_command_buffer(command_buffer, &command_buffer_begin_info);
                if let Err(e) = result {
                    log::error!(
                        "Error beginning secondary command buffer: {}",
                        e.to_string()
                    );
                }
                device.cmd_set_viewport(command_buffer, 0, &[viewport]);
                device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                device.cmd_bind_pipeline(command_buffer, PipelineBindPoint::GRAPHICS, pipeline);
                device.cmd_bind_descriptor_sets(
                    command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    0,
                    &[descriptor_set],
                    &[],
                );
                let vertex_buffers = [mesh.get_vertex_buffer()];
                let index_buffer = mesh.get_index_buffer();

                let mut vertex_offset_index = 0;
                let mut index_offset_index = 0;
                for primitive in mesh.primitives.iter() {
                    push_constant.texture_index = primitive.texture_index.unwrap_or_default();
                    let casted = bytemuck::cast::<PushConstant, [u8; 32]>(push_constant);
                    device.cmd_push_constants(
                        command_buffer,
                        pipeline_layout,
                        ShaderStageFlags::FRAGMENT | ShaderStageFlags::VERTEX,
                        0,
                        &casted[0..],
                    );
                    device.cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers[0..], &[0]);
                    device.cmd_bind_index_buffer(
                        command_buffer,
                        index_buffer,
                        0,
                        IndexType::UINT32,
                    );
                    device.cmd_draw_indexed(
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
                let result = device.end_command_buffer(command_buffer);
                if let Err(e) = result {
                    log::error!("Error ending command buffer: {}", e.to_string());
                }
            }
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
            model_metadata: model.model_metadata,
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
