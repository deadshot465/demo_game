use crate::game::graphics::vk::{Buffer, Graphics, Image, Pipeline, ThreadPool};
use crate::game::shared::enums::ShaderType;
use crate::game::shared::structs::{InstanceData, Model, ModelMetaData, PushConstant};
use crate::game::structs::Vertex;
use crate::game::traits::{Disposable, GraphicsBase, Mappable, Renderable};
use ash::version::DeviceV1_0;
use ash::vk::{
    BufferUsageFlags, CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo,
    CommandBufferUsageFlags, CommandPool, DescriptorSet, IndexType, MemoryPropertyFlags,
    PipelineBindPoint, Rect2D, ShaderStageFlags, Viewport,
};
use ash::Device;
use crossbeam::channel::*;
use crossbeam::sync::ShardedLock;
use glam::{Vec3A, Vec4};
use parking_lot::{Mutex, RwLock};
use std::mem::ManuallyDrop;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

pub struct InstancedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub model: Model<GraphicsType, BufferType, CommandType, TextureType>,
    pub instance_data: Vec<InstanceData>,
    pub instance_buffer: ManuallyDrop<BufferType>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub is_disposed: bool,
    pub ssbo_index: usize,
    pub model_index: usize,
    pub command_data:
        std::collections::HashMap<usize, (Option<Arc<Mutex<CommandPool>>>, CommandType)>,
}

impl InstancedModel<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(
        file_name: &'static str,
        graphics: Weak<RwLock<Graphics>>,
        position: Vec3A,
        scale: Vec3A,
        rotation: Vec3A,
        color: Vec4,
        model_index: Arc<AtomicUsize>,
        ssbo_index: usize,
        instance_data: Vec<InstanceData>,
    ) -> anyhow::Result<Receiver<Self>> {
        log::info!("Loading instanced model: {}...", file_name);
        let graphics_arc = graphics
            .upgrade()
            .expect("Failed to upgrade graphics handle for model.");
        let (model_send, model_recv) = bounded(0);
        rayon::spawn(move || {
            let loaded_model = Model::new(
                file_name,
                graphics,
                position,
                scale,
                rotation,
                color,
                model_index.clone(),
                ssbo_index,
                true,
            )
            .expect("Failed to load instanced model data.")
            .recv()
            .expect("Failed to receive instanced model data.");
            let model_index = model_index.fetch_add(1, Ordering::SeqCst);
            let inflight_frame_count = std::env::var("INFLIGHT_BUFFER_COUNT")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let graphics_lock = graphics_arc.read();
            let mut command_data = std::collections::HashMap::new();
            for i in 0..inflight_frame_count {
                let (command_pool, command_buffer) =
                    Graphics::get_command_pool_and_secondary_command_buffer(
                        &*graphics_lock,
                        model_index,
                        i,
                    );
                let entry = command_data
                    .entry(i)
                    .or_insert((None, CommandBuffer::null()));
                *entry = (Some(command_pool), command_buffer);
            }
            drop(graphics_lock);
            let instance_buffer =
                Self::create_instance_buffer(graphics_arc.clone(), instance_data.as_slice())
                    .expect("Failed to create instance buffer.");
            let loaded_instance = InstancedModel {
                model: loaded_model,
                instance_data,
                instance_buffer: ManuallyDrop::new(instance_buffer),
                vertex_buffer: None,
                index_buffer: None,
                vertices: vec![],
                indices: vec![],
                is_disposed: false,
                ssbo_index,
                model_index,
                command_data: std::collections::HashMap::new(),
            };
            /*loaded_instance
            .create_vertex_and_index_buffer(graphics_arc)
            .expect("Failed to create vertex and index buffer for instance.");*/
            model_send
                .send(loaded_instance)
                .expect("Failed to send instanced model.");
        });
        Ok(model_recv)
    }

    fn create_vertex_and_index_buffer(
        &mut self,
        graphics: Arc<RwLock<Graphics>>,
    ) -> anyhow::Result<()> {
        let mut vertices = vec![];
        let mut indices = vec![];
        for mesh in self.model.meshes.iter() {
            let mesh_lock = mesh.lock();
            for primitive in mesh_lock.primitives.iter() {
                vertices.push(primitive.vertices.to_vec());
                indices.push(primitive.indices.to_vec());
            }
        }
        let vertices = vertices.iter().flatten().copied().collect::<Vec<_>>();
        let indices = indices.iter().flatten().copied().collect::<Vec<_>>();
        let command_pool = self
            .command_data
            .get(&0)
            .map(|(pool, _)| pool.clone().unwrap())
            .unwrap();
        let (vertex_buffer, index_buffer) = Graphics::create_vertex_and_index_buffer(
            graphics,
            vertices.clone(),
            indices.clone(),
            command_pool,
        )?;
        self.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
        self.index_buffer = Some(ManuallyDrop::new(index_buffer));
        self.vertices = vertices;
        self.indices = indices;
        Ok(())
    }

    fn create_instance_buffer(
        graphics: Arc<RwLock<Graphics>>,
        instance_data: &[InstanceData],
    ) -> anyhow::Result<Buffer> {
        let buffer_size = (std::mem::size_of::<InstanceData>() * instance_data.len()) as u64;
        let graphics_lock = graphics.read();
        let mut staging_buffer = Buffer::new(
            Arc::downgrade(&graphics_lock.logical_device),
            buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
            Arc::downgrade(&graphics_lock.allocator),
        );
        unsafe {
            let mapped = staging_buffer.map_memory(buffer_size, 0);
            std::ptr::copy_nonoverlapping(
                instance_data.as_ptr() as *const std::ffi::c_void,
                mapped,
                buffer_size as usize,
            );
        }
        let instance_buffer = Buffer::new(
            Arc::downgrade(&graphics_lock.logical_device),
            buffer_size,
            BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
            MemoryPropertyFlags::DEVICE_LOCAL,
            Arc::downgrade(&graphics_lock.allocator),
        );
        let cmd_pool = graphics_lock.get_idle_command_pool();
        instance_buffer.copy_buffer(
            &staging_buffer,
            buffer_size,
            *cmd_pool.lock(),
            *graphics_lock.graphics_queue.lock(),
            None,
        );
        Ok(instance_buffer)
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Clone
    for InstancedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn clone(&self) -> Self {
        InstancedModel {
            model: self.model.clone(),
            instance_data: self.instance_data.to_vec(),
            instance_buffer: self.instance_buffer.clone(),
            vertex_buffer: self.vertex_buffer.clone(),
            index_buffer: self.index_buffer.clone(),
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            is_disposed: true,
            ssbo_index: 0,
            model_index: 0,
            command_data: self.command_data.clone(),
        }
    }
}

impl Renderable<Graphics, Buffer, CommandBuffer, Image>
    for InstancedModel<Graphics, Buffer, CommandBuffer, Image>
{
    fn update(&mut self, delta_time: f64) {
        self.model.update(delta_time);
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
        let instance_buffer = self.instance_buffer.buffer;
        let instance_count = self.instance_data.len();
        unsafe {
            for mesh in self.model.meshes.iter() {
                let mesh_clone = mesh.clone();
                let mesh_lock = mesh_clone.lock();
                let model_index = mesh_lock.model_index;
                //let shader_type = mesh_lock.shader_type;
                drop(mesh_lock);
                let pipeline_layout = pipeline
                    .read()
                    .expect("Failed to lock pipeline when acquiring pipeline layout.")
                    .get_pipeline_layout(ShaderType::InstanceDraw);
                let pipeline = pipeline
                    .read()
                    .expect("Failed to lock pipeline when getting the graphics pipeline.")
                    .get_pipeline(ShaderType::InstanceDraw, 0);
                let inheritance_clone = inheritance_info.clone();
                let device_clone = device.clone();
                let vertex_buffer_offsets = vec![0, 0];
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
                        let vertex_buffers = [mesh_lock.get_vertex_buffer(), instance_buffer];
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
                                vertex_buffer_offsets.as_slice(),
                            );
                            device_clone.cmd_bind_index_buffer(
                                command_buffer,
                                index_buffer,
                                0,
                                IndexType::UINT32,
                            );
                            device_clone.cmd_draw_indexed(
                                command_buffer,
                                primitive.indices.len() as u32,
                                instance_count as u32,
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

    fn get_ssbo_index(&self) -> usize {
        self.ssbo_index
    }

    fn get_model_metadata(&self) -> ModelMetaData {
        self.model.get_model_metadata()
    }

    fn get_position(&self) -> Vec3A {
        self.model.get_position()
    }

    fn get_scale(&self) -> Vec3A {
        self.model.get_scale()
    }

    fn get_rotation(&self) -> Vec3A {
        self.model.get_rotation()
    }

    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandBuffer> {
        self.model.get_command_buffers(frame_index)
    }

    fn set_position(&mut self, position: Vec3A) {
        self.model.set_position(position)
    }

    fn set_scale(&mut self, scale: Vec3A) {
        self.model.set_scale(scale)
    }

    fn set_rotation(&mut self, rotation: Vec3A) {
        self.model.set_rotation(rotation)
    }

    fn set_model_metadata(&mut self, model_metadata: ModelMetaData) {
        self.model.set_model_metadata(model_metadata)
    }

    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>) {
        self.model.update_model_indices(model_count)
    }

    fn set_ssbo_index(&mut self, ssbo_index: usize) {
        self.ssbo_index = ssbo_index;
    }

    fn box_clone(&self) -> Box<dyn Renderable<Graphics, Buffer, CommandBuffer, Image> + Send> {
        Box::new((*self).clone())
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for InstancedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        if !self.is_disposed() {
            self.dispose();
        }
    }
}

impl<GraphicsType, BufferType, CommandType, TextureType> Disposable
    for InstancedModel<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn dispose(&mut self) {
        if self.is_disposed() {
            return;
        }
        self.model.dispose();
        unsafe {
            ManuallyDrop::drop(&mut self.instance_buffer);
            if let Some(buffer) = self.vertex_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.index_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
        }
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed && self.model.is_disposed()
    }

    fn get_name(&self) -> &str {
        self.model.get_name()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.model.set_name(name)
    }
}
