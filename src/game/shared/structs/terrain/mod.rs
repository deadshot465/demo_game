use crate::game::graphics::vk::{Buffer, Graphics, Image, Pipeline, ThreadPool};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{Mesh, Model, ModelMetaData, Primitive, PushConstant, Vertex};
use crate::game::shared::traits::{Disposable, GraphicsBase, Renderable};
use crate::game::shared::util::get_random_string;
use crate::game::shared::util::height_generator::HeightGenerator;
use ash::vk::{
    CommandBuffer, CommandBufferInheritanceInfo, CommandPool, DescriptorSet, Rect2D,
    SamplerAddressMode, Viewport,
};
use ash::Device;
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Mat4, Vec2, Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicPtr, AtomicUsize};

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

    fn create_terrain(
        _grid_x: i32,
        _grid_z: i32,
        texture_data: (Arc<ShardedLock<TextureType>>, usize),
        model_index: usize,
        ssbo_index: usize,
        graphics: Weak<RwLock<GraphicsType>>,
        command_data: HashMap<usize, (Option<Arc<Mutex<CommandPool>>>, CommandType)>,
        height_generator: Arc<ShardedLock<HeightGenerator>>,
        size_ratio_x: f32,
        size_ratio_z: f32,
        vertex_count_ratio: f32,
    ) -> Self {
        let pos_x = std::env::var("POS_X").unwrap().parse::<f32>().unwrap();
        let pos_z = std::env::var("POS_Z").unwrap().parse::<f32>().unwrap();
        // let x = grid_x as f32 * Self::SIZE * size_ratio_x;
        // let z = grid_z as f32 * Self::SIZE * size_ratio_z;
        let x = pos_x * Self::SIZE * size_ratio_x;
        let z = pos_z * Self::SIZE * size_ratio_z;
        let model = Self::generate_terrain(
            model_index,
            ssbo_index,
            texture_data,
            graphics,
            Vec3A::new(x, 0.0, z),
            command_data,
            height_generator,
            size_ratio_x,
            size_ratio_z,
            vertex_count_ratio,
        );
        Terrain {
            x,
            z,
            model,
            is_disposed: false,
        }
    }

    fn generate_terrain(
        model_index: usize,
        ssbo_index: usize,
        texture_data: (Arc<ShardedLock<TextureType>>, usize),
        graphics: Weak<RwLock<GraphicsType>>,
        position: Vec3A,
        command_data: HashMap<usize, (Option<Arc<Mutex<CommandPool>>>, CommandType)>,
        height_generator: Arc<ShardedLock<HeightGenerator>>,
        size_ratio_x: f32,
        size_ratio_z: f32,
        vertex_count_ratio: f32,
    ) -> Model<GraphicsType, BufferType, CommandType, TextureType> {
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
        let (texture, texture_index) = texture_data;
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
            command_data,
            shader_type: ShaderType::Terrain,
            model_index,
        };
        Model {
            position,
            scale: Vec3A::one(),
            rotation: Vec3A::zero(),
            model_metadata: ModelMetaData {
                world_matrix: Mat4::identity(),
                object_color: Vec4::one(),
                reflectivity: 0.0,
                shine_damper: 0.0,
            },
            meshes: vec![Arc::new(Mutex::new(mesh))],
            is_disposed: false,
            model_name: get_random_string(7),
            graphics,
            ssbo_index,
        }
    }

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
        ssbo_index: usize,
        graphics: Weak<RwLock<Graphics>>,
        height_generator: Arc<ShardedLock<HeightGenerator>>,
        size_ratio_x: f32,
        size_ratio_z: f32,
        vertex_count_ratio: f32,
    ) -> anyhow::Result<Receiver<Self>> {
        log::info!("Generating terrain...Model index: {}", model_index);
        let graphics_arc = graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle.");
        let (terrain_send, terrain_recv) = bounded(5);
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
            let (image, texture_index) = Graphics::create_image_from_file(
                "textures/TexturesCom_Grass0150_1_seamless_S.jpg",
                graphics_arc.clone(),
                command_data
                    .get(&0)
                    .map(|(pool, _)| pool.clone().unwrap())
                    .unwrap(),
                SamplerAddressMode::REPEAT,
            )
            .expect("Failed to create image from file.");
            log::info!("Terrain texture successfully created.");
            let mut generated_terrain = Terrain::create_terrain(
                grid_x,
                grid_z,
                (image, texture_index),
                model_index,
                ssbo_index,
                graphics,
                command_data,
                height_generator,
                size_ratio_x,
                size_ratio_z,
                vertex_count_ratio,
            );
            generated_terrain.model.model_metadata.world_matrix =
                generated_terrain.get_world_matrix();
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
        let mut mesh = self.model.meshes[0].lock();
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
    for Terrain<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

unsafe impl<GraphicsType, BufferType, CommandType, TextureType> Sync
    for Terrain<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
}

impl<GraphicsType, BufferType, CommandType, TextureType> Clone
    for Terrain<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn clone(&self) -> Self {
        Terrain {
            is_disposed: true,
            model: self.model.clone(),
            x: self.x,
            z: self.z,
        }
    }
}

impl Renderable<Graphics, Buffer, CommandBuffer, Image>
    for Terrain<Graphics, Buffer, CommandBuffer, Image>
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
        self.model.render(
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
        self.model.ssbo_index
    }

    fn get_model_metadata(&self) -> ModelMetaData {
        self.model.model_metadata
    }

    fn get_position(&self) -> Vec3A {
        self.model.position
    }

    fn get_scale(&self) -> Vec3A {
        self.model.scale
    }

    fn get_rotation(&self) -> Vec3A {
        self.model.rotation
    }

    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandBuffer> {
        self.model.get_command_buffers(frame_index)
    }

    fn set_position(&mut self, position: Vec3A) {
        self.model.set_position(position);
    }

    fn set_scale(&mut self, scale: Vec3A) {
        self.model.set_scale(scale);
    }

    fn set_rotation(&mut self, rotation: Vec3A) {
        self.model.set_rotation(rotation);
    }

    fn set_model_metadata(&mut self, model_metadata: ModelMetaData) {
        self.model.set_model_metadata(model_metadata);
    }

    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>) {
        self.model.update_model_indices(model_count);
    }

    fn set_ssbo_index(&mut self, ssbo_index: usize) {
        self.model.set_ssbo_index(ssbo_index);
    }

    fn box_clone(&self) -> Box<dyn Renderable<Graphics, Buffer, CommandBuffer, Image> + Send> {
        Box::new(self.clone())
    }
}

/*impl CloneableRenderable<Graphics, Buffer, CommandBuffer, Image>
    for Terrain<Graphics, Buffer, CommandBuffer, Image>
{
}*/

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
