use crate::game::graphics::vk::{Buffer, Graphics, Image, Pipeline, ThreadPool};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{Mesh, PositionInfo, Primitive, PushConstant, Vertex};
use crate::game::shared::traits::Renderable;
use crate::game::shared::util::get_random_string;
use crate::game::structs::{Model, ModelMetaData};
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::CommandData;
use ash::vk::{
    CommandBuffer, CommandBufferInheritanceInfo, DescriptorSet, Rect2D, SamplerAddressMode,
    Viewport,
};
use ash::Device;
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Vec2, Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use slotmap::DefaultKey;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicPtr, AtomicUsize};
use std::sync::{Arc, Weak};

#[derive(Copy, Clone, Debug)]
pub enum PrimitiveType {
    Rect,
}

pub struct GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    pub is_disposed: bool,
    pub model: Option<Model<GraphicsType, BufferType, CommandType, TextureType>>,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    pub fn create_primitive(
        primitive_type: PrimitiveType,
        model_index: usize,
        ssbo_index: usize,
        texture_data: Option<(Arc<ShardedLock<TextureType>>, usize)>,
        graphics: Weak<RwLock<ManuallyDrop<GraphicsType>>>,
        command_data: CommandData<CommandType>,
        position_info: PositionInfo,
        color: Vec4,
        shader_type: Option<ShaderType>,
        entity: DefaultKey,
    ) -> Self {
        let mesh = match primitive_type {
            PrimitiveType::Rect => {
                Self::create_rect(texture_data, command_data, shader_type, model_index)
            }
        };
        GeometricPrimitive {
            is_disposed: false,
            model: Some(Model {
                position_info,
                model_metadata: ModelMetaData {
                    world_matrix: Mat4::identity(),
                    object_color: color,
                    reflectivity: 0.0,
                    shine_damper: 0.0,
                },
                meshes: vec![Arc::new(Mutex::new(mesh))],
                is_disposed: false,
                model_name: get_random_string(3),
                graphics,
                ssbo_index,
                entity,
            }),
        }
    }

    fn create_rect(
        texture_data: Option<(Arc<ShardedLock<TextureType>>, usize)>,
        command_data: CommandData<CommandType>,
        shader_type: Option<ShaderType>,
        model_index: usize,
    ) -> Mesh<BufferType, CommandType, TextureType> {
        let mut vertices = vec![Vertex::default(); 4];
        let mut indices = vec![u32::default(); 3 * 2];
        let normal: Vec3A = Vec3A::new(0.0, 1.0, 0.0);
        vertices[0] = Vertex {
            position: Vec3A::new(-1.0, 0.0, 1.0),
            normal,
            uv: Vec2::new(0.0, 0.0),
        };
        vertices[1] = Vertex {
            position: Vec3A::new(1.0, 0.0, 1.0),
            normal,
            uv: Vec2::new(1.0, 0.0),
        };
        vertices[2] = Vertex {
            position: Vec3A::new(1.0, 0.0, -1.0),
            normal,
            uv: Vec2::new(1.0, 1.0),
        };
        vertices[3] = Vertex {
            position: Vec3A::new(-1.0, 0.0, -1.0),
            normal,
            uv: Vec2::new(0.0, 1.0),
        };
        indices[0] = 0;
        indices[1] = 1;
        indices[2] = 2;
        indices[3] = 2;
        indices[4] = 3;
        indices[5] = 0;
        let (texture, texture_index) = match texture_data {
            Some(t) => (vec![t.0], Some(t.1)),
            None => (vec![], None),
        };
        let primitive = Primitive {
            vertices,
            indices,
            texture_index,
            is_disposed: false,
        };
        let final_shader_type = if texture.is_empty() {
            shader_type.unwrap_or(ShaderType::BasicShaderWithoutTexture)
        } else {
            shader_type.unwrap_or(ShaderType::BasicShader)
        };
        Mesh {
            primitives: vec![primitive],
            vertex_buffer: None,
            index_buffer: None,
            texture,
            is_disposed: false,
            command_data,
            shader_type: final_shader_type,
            model_index,
        }
    }
}

impl GeometricPrimitive<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        graphics: Weak<RwLock<ManuallyDrop<Graphics>>>,
        primitive_type: PrimitiveType,
        texture_name: Option<&'static str>,
        model_index: usize,
        ssbo_index: usize,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        shader_type: Option<ShaderType>,
        entity: DefaultKey,
    ) -> anyhow::Result<Receiver<Self>> {
        log::info!(
            "Generating geometric primitive...Model index: {}",
            model_index
        );
        let graphics_arc = graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle.");
        let (primitive_send, primitive_recv) = bounded(5);
        rayon::spawn(move || {
            let graphics_arc = graphics_arc;
            let inflight_frame_count = std::env::var("INFLIGHT_BUFFER_COUNT")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let mut command_data = HashMap::new();
            for i in 0..inflight_frame_count {
                let (command_pool, command_buffer) =
                    Graphics::get_command_pool_and_secondary_command_buffer(
                        &*graphics_arc.read(),
                        model_index,
                        i,
                    );
                let entry = command_data
                    .entry(i)
                    .or_insert((None, CommandBuffer::null()));
                *entry = (Some(command_pool), command_buffer);
            }
            let texture_data = match texture_name {
                None => None,
                Some(file_name) => Some(
                    Graphics::create_image_from_file(
                        file_name,
                        graphics_arc.clone(),
                        command_data
                            .get(&0)
                            .map(|(pool, _)| pool.clone().unwrap())
                            .unwrap(),
                        SamplerAddressMode::REPEAT,
                    )
                    .expect("Failed to create texture for geometric primitive."),
                ),
            };
            let x: f32 = rotation.x;
            let y: f32 = rotation.y;
            let z: f32 = rotation.z;
            let mut generated_mesh = Self::create_primitive(
                primitive_type,
                model_index,
                ssbo_index,
                texture_data,
                graphics,
                command_data,
                PositionInfo {
                    position,
                    scale,
                    rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
                },
                color,
                shader_type,
                entity,
            );
            generated_mesh
                .model
                .as_mut()
                .unwrap()
                .model_metadata
                .world_matrix = generated_mesh.get_world_matrix();
            generated_mesh
                .create_buffer(graphics_arc)
                .expect("Failed to create buffer for geometric primitive.");
            primitive_send
                .send(generated_mesh)
                .expect("Failed to send geometric primitive.");
        });
        Ok(primitive_recv)
    }

    fn create_buffer(
        &mut self,
        graphics: Arc<RwLock<ManuallyDrop<Graphics>>>,
    ) -> anyhow::Result<()> {
        let mutable_model = self
            .model
            .as_mut()
            .expect("Failed to get mutable reference to the geometric primitive.");
        let mut mesh = mutable_model.meshes[0].lock();
        let vertices = mesh.primitives[0].vertices.to_vec();
        let indices = mesh.primitives[0].indices.to_vec();
        let command_pool = mesh
            .command_data
            .get(&0)
            .map(|(pool, _)| pool.clone().unwrap())
            .unwrap();
        let (vertex_buffer, index_buffer) =
            Graphics::create_vertex_and_index_buffer(graphics, vertices, indices, command_pool)?;
        mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
        mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
        Ok(())
    }
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Send
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Sync
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

impl<GraphicsType, BufferType, CommandType, TextureType> Clone
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn clone(&self) -> Self {
        GeometricPrimitive {
            is_disposed: true,
            model: self.model.clone(),
        }
    }
}

impl Renderable<Graphics, Buffer, CommandBuffer, Image>
    for GeometricPrimitive<Graphics, Buffer, CommandBuffer, Image>
{
    fn update(&mut self, _delta_time: f64) {}

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
        let model = self.model.as_ref().unwrap();
        model.render(
            inheritance_info,
            push_constant,
            viewport,
            scissor,
            device,
            pipeline,
            descriptor_set,
            thread_pool,
            frame_index,
        );
    }

    fn get_ssbo_index(&self) -> usize {
        self.model.as_ref().unwrap().ssbo_index
    }

    fn get_model_metadata(&self) -> ModelMetaData {
        self.model.as_ref().unwrap().model_metadata
    }

    fn get_position_info(&self) -> PositionInfo {
        self.model.as_ref().unwrap().position_info
    }

    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandBuffer> {
        self.model
            .as_ref()
            .unwrap()
            .get_command_buffers(frame_index)
    }

    fn set_position_info(&mut self, position_info: PositionInfo) {
        self.model.as_mut().unwrap().position_info = position_info;
    }

    fn set_model_metadata(&mut self, model_metadata: ModelMetaData) {
        self.model
            .as_mut()
            .unwrap()
            .set_model_metadata(model_metadata);
    }

    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>) {
        self.model
            .as_mut()
            .unwrap()
            .update_model_indices(model_count);
    }

    fn set_ssbo_index(&mut self, ssbo_index: usize) {
        self.model.as_mut().unwrap().set_ssbo_index(ssbo_index);
    }

    fn box_clone(&self) -> Box<dyn Renderable<Graphics, Buffer, CommandBuffer, Image> + Send> {
        Box::new(self.clone())
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable
    for GeometricPrimitive<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    fn dispose(&mut self) {
        if self.is_disposed {
            return;
        }
        self.model
            .as_mut()
            .expect("Failed to get mutable reference to the geometric primitive.")
            .dispose();
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        self.model
            .as_ref()
            .expect("Failed to get reference to the geometric primitive.")
            .get_name()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.model
            .as_mut()
            .expect("Failed to get mutable reference to the geometric primitive.")
            .set_name(name)
    }
}
