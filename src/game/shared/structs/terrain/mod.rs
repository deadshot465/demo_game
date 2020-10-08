use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{Mesh, Model, Primitive, Vertex};
use crate::game::shared::traits::{Disposable, GraphicsBase};
use crate::game::shared::util::get_random_string;
use ash::vk::CommandBuffer;
use crossbeam::sync::ShardedLock;
use glam::{Vec2, Vec3A, Vec4};
use std::mem::ManuallyDrop;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Terrain<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    pub is_disposed: bool,
    pub model: Model<GraphicsType, BufferType, CommandType, TextureType>,
    x: f32,
    z: f32,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    Terrain<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
{
    const SIZE: f32 = 800.0;
    const VERTEX_COUNT: u32 = 128;

    pub fn new(
        grid_x: u32,
        grid_z: u32,
        texture_index: usize,
        texture: Arc<ShardedLock<TextureType>>,
        graphics: Arc<RwLock<GraphicsType>>,
    ) -> Self {
        let model = Self::generate_terrain(texture_index, texture, graphics);
        Terrain {
            x: grid_x as f32 * Self::SIZE,
            z: grid_z as f32 * Self::SIZE,
            model,
            is_disposed: false,
        }
    }

    fn generate_terrain(
        texture_index: usize,
        texture: Arc<ShardedLock<TextureType>>,
        graphics: Arc<RwLock<GraphicsType>>,
    ) -> Model<GraphicsType, BufferType, CommandType, TextureType> {
        let count = Self::VERTEX_COUNT * Self::VERTEX_COUNT;
        let mut vertices: Vec<Vertex> = vec![];
        vertices.reserve(count as usize);
        let indices_count = 6 * (Self::VERTEX_COUNT - 1) * (Self::VERTEX_COUNT - 1);
        let mut indices: Vec<u32> = vec![0; indices_count as usize];
        for i in 0..Self::VERTEX_COUNT {
            for j in 0..Self::VERTEX_COUNT {
                let vertex = Vertex {
                    position: Vec3A::new(
                        (j as f32 / (Self::VERTEX_COUNT - 1) as f32) * Self::SIZE,
                        0.0,
                        (i as f32 / (Self::VERTEX_COUNT - 1) as f32) * Self::SIZE,
                    ),
                    normal: Vec3A::new(0.0, -1.0, 0.0),
                    uv: Vec2::new(
                        j as f32 / (Self::VERTEX_COUNT - 1) as f32,
                        i as f32 / (Self::VERTEX_COUNT - 1) as f32,
                    ),
                };
                vertices.push(vertex);
            }
        }
        let mut pointer = 0;
        for gz in 0..Self::VERTEX_COUNT - 1 {
            for gx in 0..Self::VERTEX_COUNT - 1 {
                let top_left = (gz * Self::VERTEX_COUNT) + gx;
                let top_right = top_left + 1;
                let bottom_left = ((gz + 1) * Self::VERTEX_COUNT) + gx;
                let bottom_right = bottom_left + 1;

                indices[pointer] = top_left;
                pointer += 1;
                indices[pointer] = bottom_left;
                pointer += 1;
                indices[pointer] = top_right;
                pointer += 1;
                indices[pointer] = top_right;
                pointer += 1;
                indices[pointer] = bottom_left;
                pointer += 1;
                indices[pointer] = bottom_right;
                pointer += 1;
            }
        }

        let primitive = Primitive {
            vertices,
            indices,
            texture_index: Some(texture_index),
            is_disposed: false,
        };
        let mesh = Mesh {
            primitives: vec![primitive],
            vertex_buffer: None,
            index_buffer: None,
            texture: vec![texture],
            is_disposed: false,
            command_pool: None,
            command_buffer: None,
        };
        let model = Model {
            position: Vec3A::zero(),
            scale: Vec3A::one(),
            rotation: Vec3A::zero(),
            color: Vec4::one(),
            meshes: vec![mesh],
            is_disposed: false,
            model_name: get_random_string(7),
            model_index: 0,
            graphics: Arc::downgrade(&graphics),
        };
        model
    }
}

impl Terrain<Graphics, Buffer, CommandBuffer, Image> {
    pub async fn create_buffers(&mut self) -> anyhow::Result<()> {
        let graphics = self.model.graphics.upgrade().unwrap();
        let vertices = self.model.meshes[0].primitives[0].vertices.to_vec();
        let indices = self.model.meshes[0].primitives[0].indices.to_vec();
        let command_pool = self.model.meshes[0].command_pool.clone().unwrap();

        let (vertex_buffer, index_buffer) = Graphics::create_buffer(
            graphics.clone(),
            vertices,
            indices,
            command_pool,
        )
        .await?;
        let mesh = &mut self.model.meshes[0];
        mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
        mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
        Ok(())
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop for Terrain<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Disposable + Clone {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable for Terrain<GraphicsType, BufferType, CommandType, TextureType>
    where GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
          BufferType: 'static + Disposable + Clone,
          CommandType: 'static + Clone,
          TextureType: 'static + Disposable + Clone {
    fn dispose(&mut self) {
        if self.is_disposed {
            return;
        }
        self.model.dispose();
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        unimplemented!()
    }

    fn set_name(&mut self, _name: String) -> &str {
        unimplemented!()
    }
}