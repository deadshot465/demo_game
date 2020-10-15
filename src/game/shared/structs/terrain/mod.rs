use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{Mesh, Model, Primitive, Vertex};
use crate::game::shared::traits::{Disposable, GraphicsBase};
use crate::game::shared::util::get_random_string;
use crate::game::ResourceManager;
use ash::vk::{CommandBuffer, CommandPool, SamplerAddressMode};
use crossbeam::sync::ShardedLock;
use glam::{Vec2, Vec3A, Vec4};
use parking_lot::Mutex;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use crate::game::shared::util::height_generator::HeightGenerator;

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
    pub const SIZE: f32 = 1600.0;
    pub const VERTEX_COUNT: u32 = 256;
}

impl Terrain<Graphics, Buffer, CommandBuffer, Image> {
    pub async fn new(
        grid_x: i32,
        grid_z: i32,
        model_index: usize,
        resource_manager: Weak<RwLock<ResourceManager<Graphics, Buffer, CommandBuffer, Image>>>,
        graphics: Weak<RwLock<Graphics>>,
        height_generator: Arc<RwLock<HeightGenerator>>,
    ) -> anyhow::Result<JoinHandle<Self>> {
        log::info!("Generating terrain...");
        let graphics_arc = graphics.upgrade().unwrap();
        let command_pool = graphics_arc
            .read()
            .await
            .thread_pool
            .get_idle_command_pool();
        let image = Graphics::create_image_from_file(
            "textures/TexturesCom_Grass0150_1_seamless_S.jpg",
            graphics_arc.clone(),
            command_pool,
            SamplerAddressMode::REPEAT,
        )
        .await?;
        log::info!("Terrain texture successfully created.");
        let rm_arc = resource_manager
            .upgrade()
            .expect("Failed to upgrade Weak of resource manager for creating terrain.");
        let texture_index: usize;
        {
            let resource_lock = rm_arc.read().await;
            texture_index = resource_lock.get_texture_count() - 1;
        }
        let thread_count: usize;
        let command_pool: Arc<Mutex<CommandPool>>;
        let command_buffer: CommandBuffer;
        {
            let graphics_lock = graphics_arc.read().await;
            thread_count = graphics_lock.thread_pool.thread_count;
            command_pool = graphics_lock.thread_pool.threads[model_index % thread_count]
                .command_pool
                .clone();
            command_buffer = graphics_lock.create_secondary_command_buffer(command_pool.clone());
        }
        let terrain = tokio::spawn(async move {
            let mut generated_terrain = Terrain::create_terrain(
                grid_x,
                grid_z,
                texture_index,
                model_index,
                image,
                graphics.clone(),
                command_pool,
                command_buffer,
                height_generator,
            ).await;
            log::info!("Terrain successfully generated.");
            generated_terrain
                .create_buffers(graphics_arc.clone())
                .await
                .unwrap();
            generated_terrain
        });
        Ok(terrain)
    }

    async fn create_buffers(&mut self, graphics: Arc<RwLock<Graphics>>) -> anyhow::Result<()> {
        let vertices = self.model.meshes[0].primitives[0].vertices.to_vec();
        let indices = self.model.meshes[0].primitives[0].indices.to_vec();
        let command_pool = self.model.meshes[0].command_pool.clone().unwrap();

        let (vertex_buffer, index_buffer) =
            Graphics::create_buffer(graphics, vertices, indices, command_pool).await?;
        let mesh = &mut self.model.meshes[0];
        mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
        mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
        Ok(())
    }

    async fn create_terrain(
        grid_x: i32,
        grid_z: i32,
        texture_index: usize,
        model_index: usize,
        texture: Arc<ShardedLock<Image>>,
        graphics: Weak<RwLock<Graphics>>,
        command_pool: Arc<Mutex<CommandPool>>,
        command_buffer: CommandBuffer,
        height_generator: Arc<RwLock<HeightGenerator>>,
    ) -> Self {
        let x = grid_x as f32 * Self::SIZE;
        let z = grid_z as f32 * Self::SIZE;
        let model = Self::generate_terrain(
            model_index,
            texture_index,
            texture,
            graphics,
            Vec3A::new(x, 0.0, z),
            command_pool,
            command_buffer,
            height_generator,
        ).await;
        Terrain {
            x,
            z,
            model,
            is_disposed: false,
        }
    }

    async fn generate_terrain(
        model_index: usize,
        texture_index: usize,
        texture: Arc<ShardedLock<Image>>,
        graphics: Weak<RwLock<Graphics>>,
        position: Vec3A,
        command_pool: Arc<Mutex<CommandPool>>,
        command_buffer: CommandBuffer,
        height_generator: Arc<RwLock<HeightGenerator>>,
    ) -> Model<Graphics, Buffer, CommandBuffer, Image> {
        let count = Self::VERTEX_COUNT * Self::VERTEX_COUNT;
        let mut vertices: Vec<Vertex> = vec![];
        vertices.reserve(count as usize);
        let indices_count = 6 * (Self::VERTEX_COUNT - 1) * (Self::VERTEX_COUNT - 1);
        let mut indices: Vec<u32> = vec![0; indices_count as usize];
        let generator = height_generator.read().await;
        for i in 0..Self::VERTEX_COUNT {
            for j in 0..Self::VERTEX_COUNT {
                let vertex = Vertex {
                    position: Vec3A::new(
                        (j as f32 / (Self::VERTEX_COUNT - 1) as f32) * Self::SIZE,
                        generator.generate_height(j as f32, i as f32),
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
            command_pool: Some(command_pool),
            command_buffer: Some(command_buffer),
        };
        Model {
            position,
            scale: Vec3A::one(),
            rotation: Vec3A::zero(),
            color: Vec4::one(),
            meshes: vec![mesh],
            is_disposed: false,
            model_name: get_random_string(7),
            model_index,
            graphics,
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for Terrain<GraphicsType, BufferType, CommandType, TextureType>
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
    for Terrain<GraphicsType, BufferType, CommandType, TextureType>
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
