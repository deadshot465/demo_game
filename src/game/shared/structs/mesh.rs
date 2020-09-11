use crate::game::shared::traits::disposable::Disposable;
use crate::game::structs::Vertex;
use std::mem::ManuallyDrop;
use glam::Mat4;
use crate::game::shared::structs::Joint;

#[derive(Clone)]
pub struct Mesh<BufferType: 'static + Disposable, TextureType: 'static + Clone + Disposable> {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub is_disposed: bool,
    pub texture: Option<ManuallyDrop<TextureType>>,
    pub transform: Option<Mat4>,
    pub root_joint: Option<Joint>,
}

impl Mesh<crate::game::graphics::vk::Buffer, crate::game::graphics::vk::Image> {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Mesh {
            vertices,
            indices,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false,
            texture: None,
            transform: None,
            root_joint: None,
        }
    }

    pub fn get_vertex_buffer(&self) -> ash::vk::Buffer {
        if let Some(buffer) = self.vertex_buffer.as_ref() {
            buffer.buffer
        }
        else {
            panic!("Vertex buffer is not yet created.");
        }
    }

    pub fn get_index_buffer(&self) -> ash::vk::Buffer {
        if let Some(buffer) = self.index_buffer.as_ref() {
            buffer.buffer
        }
        else {
            panic!("Index buffer is not yet created.");
        }
    }
}

impl<BufferType: 'static + Disposable, TextureType: 'static + Clone + Disposable> Drop for Mesh<BufferType, TextureType> {
    fn drop(&mut self) {
        log::info!("Dropping mesh...");
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped mesh.");
        }
        else {
            log::warn!("Mesh is already dropped.");
        }
    }
}

impl<BufferType: 'static + Disposable, TextureType: 'static + Clone + Disposable> Disposable for Mesh<BufferType, TextureType> {
    fn dispose(&mut self) {
        log::info!("Disposing mesh...");
        unsafe {
            if let Some(buffer) = self.index_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.vertex_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
        }
        self.is_disposed = true;
        log::info!("Successfully disposed mesh.");
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