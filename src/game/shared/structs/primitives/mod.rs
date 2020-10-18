use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{Mesh, Primitive, Vertex};
use crate::game::shared::util::get_random_string;
use crate::game::structs::{Model, ModelMetaData};
use crate::game::traits::{Disposable, GraphicsBase};
use ash::vk::{CommandBuffer, CommandPool, SamplerAddressMode};
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Vec2, Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use std::mem::ManuallyDrop;
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
        texture_data: Option<(Arc<ShardedLock<TextureType>>, usize)>,
        graphics: Weak<RwLock<GraphicsType>>,
        command_pool: Arc<Mutex<CommandPool>>,
        command_buffer: CommandType,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        shader_type: Option<ShaderType>,
    ) -> Self {
        let mesh = match primitive_type {
            PrimitiveType::Rect => {
                Self::create_rect(texture_data, command_pool, command_buffer, shader_type)
            }
        };
        let mut result = GeometricPrimitive {
            is_disposed: false,
            model: Some(Model {
                position,
                scale,
                rotation,
                model_metadata: ModelMetaData {
                    world_matrix: Mat4::identity(),
                    object_color: color,
                    reflectivity: 0.0,
                    shine_damper: 0.0,
                },
                meshes: vec![mesh],
                is_disposed: false,
                model_name: get_random_string(3),
                model_index,
                graphics,
            }),
        };
        let mutable = result
            .model
            .as_mut()
            .expect("Failed to get mutable reference to the geometric primitive.");
        let world_matrix = mutable.get_world_matrix();
        mutable.model_metadata.world_matrix = world_matrix;
        result
    }

    fn create_rect(
        texture_data: Option<(Arc<ShardedLock<TextureType>>, usize)>,
        command_pool: Arc<Mutex<CommandPool>>,
        command_buffer: CommandType,
        shader_type: Option<ShaderType>,
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
            command_pool: Some(command_pool),
            command_buffer: Some(command_buffer),
            shader_type: final_shader_type,
        }
    }
}

impl GeometricPrimitive<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        graphics: Weak<RwLock<Graphics>>,
        primitive_type: PrimitiveType,
        texture_name: Option<&'static str>,
        model_index: usize,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        shader_type: Option<ShaderType>,
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
            let (command_pool, command_buffer) =
                Graphics::get_command_pool_and_secondary_command_buffer(
                    &*graphics_arc.read(),
                    model_index,
                );
            let texture_data = match texture_name {
                None => None,
                Some(file_name) => Some(
                    Graphics::create_image_from_file(
                        file_name,
                        graphics_arc.clone(),
                        command_pool.clone(),
                        SamplerAddressMode::REPEAT,
                    )
                    .expect("Failed to create texture for geometric primitive."),
                ),
            };
            let mut generated_mesh = Self::create_primitive(
                primitive_type,
                model_index,
                texture_data,
                graphics,
                command_pool,
                command_buffer,
                position,
                scale,
                rotation,
                color,
                shader_type,
            );
            generated_mesh
                .create_buffer(graphics_arc)
                .expect("Failed to create buffer for geometric primitive.");
            primitive_send
                .send(generated_mesh)
                .expect("Failed to send geometric primitive.");
        });
        Ok(primitive_recv)
    }

    fn create_buffer(&mut self, graphics: Arc<RwLock<Graphics>>) -> anyhow::Result<()> {
        let mutable_model = self
            .model
            .as_mut()
            .expect("Failed to get mutable reference to the geometric primitive.");
        let vertices = mutable_model.meshes[0].primitives[0].vertices.to_vec();
        let indices = mutable_model.meshes[0].primitives[0].indices.to_vec();
        let command_pool = mutable_model.meshes[0].command_pool.clone().unwrap();

        let (vertex_buffer, index_buffer) =
            Graphics::create_buffer(graphics, vertices, indices, command_pool)?;
        let mesh = &mut mutable_model.meshes[0];
        mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
        mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
        Ok(())
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
