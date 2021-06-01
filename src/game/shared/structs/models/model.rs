use ash::version::DeviceV1_0;
use ash::vk::{
    CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferUsageFlags,
    CommandPool, DescriptorSet, IndexType, PipelineBindPoint, Rect2D, ShaderStageFlags, Viewport,
};
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
use gltf::{Node, Scene};
use parking_lot::{Mutex, RwLock};
use std::convert::TryFrom;
use std::mem::ManuallyDrop;
use std::sync::{
    atomic::{AtomicPtr, AtomicUsize, Ordering},
    Arc, Weak,
};

use crate::game::graphics::vk::{Buffer, Graphics, Image, Pipeline, ThreadPool};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{
    Mesh, ModelMetaData, PositionInfo, Primitive, PushConstant, Vertex,
};
use crate::game::shared::traits::disposable::Disposable;
use crate::game::shared::traits::Renderable;
use crate::game::traits::GraphicsBase;
use crate::game::util::read_raw_data;
use ash::Device;
use slotmap::DefaultKey;
use std::collections::HashMap;

/// 最も一般的なモデル。<br />
/// GLTFのサポートは少ないため、この構造体の中にはモデルの読み込みコードも含めています。<br />
/// 詳しくはGLTFの仕様書を参照。<br />
/// The most common models.<br />
/// Since the support for GLTF format is lacking, this struct also contains codes for reading models.<br />
/// Please refer to GLTF's documentation to learn more.
pub struct Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub position_info: PositionInfo,
    pub model_metadata: ModelMetaData,
    pub meshes: Vec<Arc<Mutex<Mesh<BufferType, CommandType, TextureType>>>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
    pub ssbo_index: usize,
    pub entity: DefaultKey,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    /// モデルを作成するのエントリーポイント。<br />
    /// The entry point for creating a model.
    fn create_model(
        file_name: &str,
        model_index: Arc<AtomicUsize>,
        document: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        images: Vec<Arc<ShardedLock<TextureType>>>,
        graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
        position_info: PositionInfo,
        color: Vec4,
        texture_index_offset: usize,
        ssbo_index: usize,
        entity: DefaultKey,
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

        Model {
            position_info,
            model_metadata: ModelMetaData {
                world_matrix: Mat4::IDENTITY,
                object_color: color,
                reflectivity: 1.0,
                shine_damper: 10.0,
            },
            graphics,
            meshes,
            is_disposed: false,
            model_name: file_name.to_string(),
            ssbo_index,
            entity,
        }
    }

    fn process_model(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: Vec<Arc<ShardedLock<TextureType>>>,
        texture_index_offset: usize,
        model_index: Arc<AtomicUsize>,
    ) -> Vec<Mesh<BufferType, CommandType, TextureType>> {
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
        meshes
    }

    fn process_root_nodes(
        scene: Scene,
        buffers: &[gltf::buffer::Data],
        images: Vec<Arc<ShardedLock<TextureType>>>,
        texture_index_offset: usize,
        model_index: Arc<AtomicUsize>,
    ) -> Vec<Mesh<BufferType, CommandType, TextureType>> {
        let mut meshes = Vec::with_capacity(150);
        for node in scene.nodes() {
            let mut submeshes = Self::process_node(
                node,
                buffers,
                &images,
                Mat4::IDENTITY,
                texture_index_offset,
                model_index.clone(),
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
        model_index: Arc<AtomicUsize>,
    ) -> Vec<Mesh<BufferType, CommandType, TextureType>> {
        let mut meshes = Vec::with_capacity(10);
        let (t, r, s) = node.transform().decomposed();
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from_array(r),
            Vec3::from(t),
        );
        let transform = local_transform * transform;
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_mesh(
                mesh,
                buffers,
                transform,
                images,
                texture_index_offset,
                model_index.clone(),
            ));
        }
        for _node in node.children() {
            let mut submeshes = Self::process_node(
                _node,
                buffers,
                images,
                transform,
                texture_index_offset,
                model_index.clone(),
            );
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
        model_index: Arc<AtomicUsize>,
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

        let shader_type = if textures.is_empty() {
            ShaderType::BasicShaderWithoutTexture
        } else {
            ShaderType::BasicShader
        };
        Mesh {
            primitives,
            vertex_buffer: None,
            index_buffer: None,
            texture: textures,
            is_disposed: false,
            command_data: std::collections::HashMap::new(),
            shader_type,
            model_index: model_index.fetch_add(1, Ordering::SeqCst),
        }
    }
}

impl Model<Graphics, Buffer, CommandBuffer, Image> {
    /// モデルの読み込みと全てのデータを作成します。<br />
    /// Read from the model file and create all necessary data.
    pub fn new(
        file_name: &'static str,
        graphics: Weak<RwLock<ManuallyDrop<Graphics>>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        model_index: Arc<AtomicUsize>,
        ssbo_index: usize,
        create_buffers: bool,
        entity: DefaultKey,
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
                let graphics = graphics_arc.read();
                command_pool = graphics.get_idle_command_pool();
            }
            log::info!("Model index: {}", ssbo_index);
            let (document, buffers, images) =
                read_raw_data(file_name).expect("Failed to read raw data from glTF.");
            let (textures, texture_index_offset) =
                Graphics::create_gltf_textures(images, graphics_arc.clone(), command_pool)
                    .expect("Failed to create glTF textures.");
            let x: f32 = rotation.x;
            let y: f32 = rotation.y;
            let z: f32 = rotation.z;
            let mut loaded_model = Self::create_model(
                file_name,
                model_index,
                document,
                buffers,
                textures,
                graphics,
                PositionInfo {
                    position,
                    scale,
                    rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
                },
                color,
                texture_index_offset,
                ssbo_index,
                entity,
            );
            loaded_model.model_metadata.world_matrix = loaded_model.get_world_matrix();
            {
                let graphics_lock = graphics_arc.read();
                let inflight_frame_count = std::env::var("INFLIGHT_BUFFER_COUNT")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                for mesh in loaded_model.meshes.iter_mut() {
                    let mut mesh_lock = mesh.lock();
                    for i in 0..inflight_frame_count {
                        let (pool, command_buffer) =
                            Graphics::get_command_pool_and_secondary_command_buffer(
                                &*graphics_lock,
                                mesh_lock.model_index,
                                i,
                            );
                        let entry = mesh_lock
                            .command_data
                            .entry(i)
                            .or_insert((None, CommandBuffer::null()));
                        *entry = (Some(pool), command_buffer);
                    }
                }
                drop(graphics_lock);
            }
            if create_buffers {
                loaded_model
                    .create_buffers(graphics_arc)
                    .expect("Failed to create buffers for model.");
            }
            model_send
                .send(loaded_model)
                .expect("Failed to send model result.");
        });
        Ok(model_recv)
    }

    /// モデルのバッファを作成する。<br />
    /// Create buffers for the model.
    fn create_buffers(
        &mut self,
        graphics: Arc<RwLock<ManuallyDrop<Graphics>>>,
    ) -> anyhow::Result<()> {
        let mut handles = HashMap::new();
        for (index, mesh) in self.meshes.iter().enumerate() {
            log::info!("Creating buffer for mesh {}...", index);
            let mesh_lock = mesh.lock();
            let vertices = mesh_lock
                .primitives
                .iter()
                .map(|p| &p.vertices)
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let indices = mesh_lock
                .primitives
                .iter()
                .map(|p| &p.indices)
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let pool = mesh_lock
                .command_data
                .get(&0)
                .map(|(pool, _)| pool.clone().unwrap())
                .unwrap();
            let g = graphics.clone();
            let (buffer_send, buffer_recv) = bounded(5);
            rayon::spawn(move || {
                let result = Graphics::create_vertex_and_index_buffer(g, vertices, indices, pool)
                    .expect("Failed to create buffers for model.");
                buffer_send
                    .send(result)
                    .expect("Failed to send buffer result.");
            });
            handles.insert(index, buffer_recv);
        }
        for (index, mesh) in self.meshes.iter_mut().enumerate() {
            if let Some(result) = handles.get_mut(&index) {
                let mut mesh_lock = mesh.lock();
                let (vertex_buffer, index_buffer) = result.recv()?;
                mesh_lock.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                mesh_lock.index_buffer = Some(ManuallyDrop::new(index_buffer));
            }
        }
        Ok(())
    }
}

/*impl From<&Model<Graphics, Buffer, CommandBuffer, Image>>
    for Model<Graphics, Buffer, CommandBuffer, Image>
{
    fn from(model: &Self) -> Self {
        loop {
            let is_buffer_completed = model.meshes.iter().all(|mesh| {
                let mesh_lock = mesh.lock();
                mesh_lock.vertex_buffer.is_some() && mesh_lock.index_buffer.is_some()
            });
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
            ssbo_index: model.ssbo_index,
        };

        _model
            .meshes
            .iter_mut()
            .for_each(|mesh| mesh.lock().is_disposed = true);
        _model
    }
}*/

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

impl<GraphicsType, BufferType, CommandType, TextureType> Clone
    for Model<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn clone(&self) -> Self {
        loop {
            let is_buffer_completed = self.meshes.iter().all(|m| {
                let mesh_lock = m.lock();
                mesh_lock.vertex_buffer.is_some() && mesh_lock.index_buffer.is_some()
            });
            if is_buffer_completed {
                break;
            }
        }
        Model {
            position_info: self.position_info,
            model_metadata: self.model_metadata,
            meshes: self.meshes.clone(),
            is_disposed: true,
            model_name: self.model_name.clone(),
            graphics: self.graphics.clone(),
            ssbo_index: 0,
            entity: self.entity,
        }
    }
}

impl Renderable<Graphics, Buffer, CommandBuffer, Image>
    for Model<Graphics, Buffer, CommandBuffer, Image>
{
    fn box_clone(&self) -> Box<dyn Renderable<Graphics, Buffer, CommandBuffer, Image> + Send> {
        Box::new(self.clone())
    }

    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandBuffer> {
        let buffers = self
            .meshes
            .iter()
            .map(|m| {
                m.lock()
                    .command_data
                    .get(&frame_index)
                    .map(|(_, buffer)| *buffer)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        buffers
    }

    fn get_entity(&self) -> DefaultKey {
        self.entity
    }

    fn get_model_metadata(&self) -> ModelMetaData {
        self.model_metadata
    }

    fn get_position_info(&self) -> PositionInfo {
        self.position_info
    }

    fn get_ssbo_index(&self) -> usize {
        self.ssbo_index
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
        let mut push_constant = push_constant;
        push_constant.model_index = self.ssbo_index;
        unsafe {
            for mesh in self.meshes.iter() {
                let mesh_clone = mesh.clone();
                let mesh_lock = mesh_clone.lock();
                let model_index = mesh_lock.model_index;
                let shader_type = mesh_lock.shader_type;
                drop(mesh_lock);
                let pipeline_layout = pipeline
                    .read()
                    .expect("Failed to lock pipeline when acquiring pipeline layout.")
                    .get_pipeline_layout(shader_type);
                let pipeline = pipeline
                    .read()
                    .expect("Failed to lock pipeline when getting the graphics pipeline.")
                    .get_pipeline(shader_type, 0);
                let inheritance_clone = inheritance_info.clone();
                let device_clone = device.clone();
                thread_pool.threads[model_index % thread_count]
                    .add_job(move || {
                        let device_clone = device_clone;
                        let inheritance =
                            inheritance_clone.load(Ordering::SeqCst).as_ref().unwrap();
                        let mesh = mesh_clone;
                        let mesh_lock = mesh.lock();
                        let command_buffer_begin_info = CommandBufferBeginInfo::builder()
                            .inheritance_info(inheritance)
                            .flags(CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                            .build();
                        let (_, command_buffer) = mesh_lock.command_data.get(&frame_index).unwrap();
                        let command_buffer = *command_buffer;
                        let result = device_clone
                            .begin_command_buffer(command_buffer, &command_buffer_begin_info);
                        if let Err(e) = result {
                            log::error!(
                                "Error beginning secondary command buffer: {}",
                                e.to_string()
                            );
                        }
                        device_clone.cmd_set_viewport(command_buffer, 0, &[viewport]);
                        device_clone.cmd_set_scissor(command_buffer, 0, &[scissor]);
                        device_clone.cmd_bind_pipeline(
                            command_buffer,
                            PipelineBindPoint::GRAPHICS,
                            pipeline,
                        );
                        device_clone.cmd_bind_descriptor_sets(
                            command_buffer,
                            PipelineBindPoint::GRAPHICS,
                            pipeline_layout,
                            0,
                            &[descriptor_set],
                            &[],
                        );
                        let vertex_buffers = [mesh_lock.get_vertex_buffer()];
                        let index_buffer = mesh_lock.get_index_buffer();
                        let mut vertex_offset_index = 0;
                        let mut index_offset_index = 0;
                        for primitive in mesh_lock.primitives.iter() {
                            push_constant.texture_index =
                                primitive.texture_index.unwrap_or_default();
                            let casted = bytemuck::cast::<PushConstant, [u8; 32]>(push_constant);
                            device_clone.cmd_push_constants(
                                command_buffer,
                                pipeline_layout,
                                ShaderStageFlags::FRAGMENT | ShaderStageFlags::VERTEX,
                                0,
                                &casted[0..],
                            );
                            device_clone.cmd_bind_vertex_buffers(
                                command_buffer,
                                0,
                                &vertex_buffers[0..],
                                &[0],
                            );
                            device_clone.cmd_bind_index_buffer(
                                command_buffer,
                                index_buffer,
                                0,
                                IndexType::UINT32,
                            );
                            device_clone.cmd_draw_indexed(
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
                        let result = device_clone.end_command_buffer(command_buffer);
                        if let Err(e) = result {
                            log::error!("Error ending command buffer: {}", e.to_string());
                        }
                    })
                    .expect("Failed to push work into the worker thread.");
            }
        }
    }

    fn set_model_metadata(&mut self, model_metadata: ModelMetaData) {
        self.model_metadata = model_metadata;
    }

    fn set_position_info(&mut self, position_info: PositionInfo) {
        self.position_info = position_info;
    }

    fn set_ssbo_index(&mut self, ssbo_index: usize) {
        self.ssbo_index = ssbo_index;
    }

    fn update(&mut self, _delta_time: f64) {
        self.model_metadata.world_matrix = self.get_world_matrix();
    }

    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>) {
        for mesh in self.meshes.iter() {
            mesh.lock().model_index = model_count.fetch_add(1, Ordering::SeqCst);
        }
    }
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
            self.ssbo_index
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
            self.ssbo_index
        );
        for mesh in self.meshes.iter_mut() {
            mesh.lock().dispose();
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
