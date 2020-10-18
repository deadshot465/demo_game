use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::shared::structs::{Mesh, Model, ModelMetaData, Primitive, Vertex};
use crate::game::shared::traits::{Disposable, GraphicsBase};
use crate::game::shared::util::get_random_string;
use crate::game::shared::util::height_generator::HeightGenerator;
use ash::vk::{CommandBuffer, CommandPool, SamplerAddressMode};
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Vec2, Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};

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
    pub const SIZE: f32 = 800.0;
    pub const VERTEX_COUNT: u32 = 128;

    fn calculate_normal(x: f32, z: f32, height_generator: &HeightGenerator) -> Vec3A {
        let height_l = height_generator.generate_height(x - 1.0, z);
        let height_r = height_generator.generate_height(x + 1.0, z);
        let height_d = height_generator.generate_height(x, z - 1.0);
        let height_u = height_generator.generate_height(x, z + 1.0);
        let normal: Vec3A = Vec3A::new(height_l - height_r, 2.0, height_d - height_u);
        normal.normalize()
    }
}

impl Terrain<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        grid_x: i32,
        grid_z: i32,
        model_index: usize,
        graphics: Weak<RwLock<Graphics>>,
        height_generator: Arc<ShardedLock<HeightGenerator>>,
        size_ratio_x: f32,
        size_ratio_z: f32,
        vertex_count_ratio: f32,
    ) -> anyhow::Result<Receiver<Self>> {
        log::info!("Generating terrain...");
        let graphics_arc = graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle.");
        let (terrain_send, terrain_recv) = bounded(5);
        rayon::spawn(move || {
            let graphics_arc = graphics_arc;
            let (command_pool, command_buffer) =
                Graphics::get_command_pool_and_secondary_command_buffer(
                    &*graphics_arc.read(),
                    model_index,
                );
            let (image, texture_index) = Graphics::create_image_from_file(
                "textures/TexturesCom_Grass0150_1_seamless_S.jpg",
                graphics_arc.clone(),
                command_pool.clone(),
                SamplerAddressMode::REPEAT,
            )
            .expect("Failed to create image from file.");
            log::info!("Terrain texture successfully created.");
            let mut generated_terrain = Terrain::create_terrain(
                grid_x,
                grid_z,
                texture_index,
                model_index,
                image,
                graphics,
                command_pool,
                command_buffer,
                height_generator,
                size_ratio_x,
                size_ratio_z,
                vertex_count_ratio,
            )
            .expect("Failed to create terrain.");
            log::info!("Terrain successfully generated.");
            generated_terrain
                .create_buffers(graphics_arc)
                .expect("Failed to create buffer for terrain.");
            terrain_send
                .send(generated_terrain)
                .expect("Failed to send terrain.");
        });
        Ok(terrain_recv)
    }

    fn create_buffers(&mut self, graphics: Arc<RwLock<Graphics>>) -> anyhow::Result<()> {
        let vertices = self.model.meshes[0].primitives[0].vertices.to_vec();
        let indices = self.model.meshes[0].primitives[0].indices.to_vec();
        let command_pool = self.model.meshes[0].command_pool.clone().unwrap();

        let (vertex_buffer, index_buffer) =
            Graphics::create_buffer(graphics, vertices, indices, command_pool)?;
        let mesh = &mut self.model.meshes[0];
        mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
        mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
        Ok(())
    }

    fn create_terrain(
        grid_x: i32,
        grid_z: i32,
        texture_index: usize,
        model_index: usize,
        texture: Arc<ShardedLock<Image>>,
        graphics: Weak<RwLock<Graphics>>,
        command_pool: Arc<Mutex<CommandPool>>,
        command_buffer: CommandBuffer,
        height_generator: Arc<ShardedLock<HeightGenerator>>,
        size_ratio_x: f32,
        size_ratio_z: f32,
        vertex_count_ratio: f32,
    ) -> anyhow::Result<Self> {
        let x = grid_x as f32 * Self::SIZE * size_ratio_x;
        let z = grid_z as f32 * Self::SIZE * size_ratio_z;
        let model = Self::generate_terrain(
            model_index,
            texture_index,
            texture,
            graphics,
            Vec3A::new(x, 0.0, z),
            command_pool,
            command_buffer,
            height_generator,
            size_ratio_x,
            size_ratio_z,
            vertex_count_ratio,
        )?;
        Ok(Terrain {
            x,
            z,
            model,
            is_disposed: false,
        })
    }

    fn generate_terrain(
        model_index: usize,
        texture_index: usize,
        texture: Arc<ShardedLock<Image>>,
        graphics: Weak<RwLock<Graphics>>,
        position: Vec3A,
        command_pool: Arc<Mutex<CommandPool>>,
        command_buffer: CommandBuffer,
        height_generator: Arc<ShardedLock<HeightGenerator>>,
        size_ratio_x: f32,
        size_ratio_z: f32,
        vertex_count_ratio: f32,
    ) -> anyhow::Result<Model<Graphics, Buffer, CommandBuffer, Image>> {
        let vertex_count = (Self::VERTEX_COUNT as f32 * vertex_count_ratio) as u32;
        let count = vertex_count * vertex_count;
        let mut vertices: Vec<Vertex> = vec![];
        vertices.reserve(count as usize);
        let indices_count = 6 * (vertex_count - 1) * (vertex_count - 1);
        let mut indices: Vec<u32> = vec![0; indices_count as usize];
        let generator = height_generator
            .read()
            .expect("Failed to lock height generator.");
        for i in 0..vertex_count {
            for j in 0..vertex_count {
                let vertex = Vertex {
                    position: Vec3A::new(
                        (j as f32 / (vertex_count - 1) as f32) * Self::SIZE * size_ratio_x,
                        generator.generate_height(j as f32, i as f32),
                        (i as f32 / (vertex_count - 1) as f32) * Self::SIZE * size_ratio_z,
                    ),
                    normal: Self::calculate_normal(j as f32, i as f32, &*generator),
                    uv: Vec2::new(
                        j as f32 / (vertex_count - 1) as f32,
                        i as f32 / (vertex_count - 1) as f32,
                    ),
                };
                vertices.push(vertex);
            }
        }
        let mut pointer = 0;
        for gz in 0..vertex_count - 1 {
            for gx in 0..vertex_count - 1 {
                let top_left = (gz * vertex_count) + gx;
                let top_right = top_left + 1;
                let bottom_left = ((gz + 1) * vertex_count) + gx;
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
        let mut model = Model {
            position,
            scale: Vec3A::one(),
            rotation: Vec3A::zero(),
            model_metadata: ModelMetaData {
                world_matrix: Mat4::identity(),
                object_color: Vec4::one(),
                reflectivity: 0.0,
                shine_damper: 0.0,
            },
            meshes: vec![mesh],
            is_disposed: false,
            model_name: get_random_string(7),
            model_index,
            graphics,
        };
        model.model_metadata.world_matrix = model.get_world_matrix();
        Ok(model)
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
        self.model.get_name()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.model.set_name(name)
    }
}
