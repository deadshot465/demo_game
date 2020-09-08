use crate::game::shared::traits::disposable::Disposable;
use crate::game::structs::Vertex;
use std::mem::ManuallyDrop;

#[derive(Clone)]
pub struct Mesh<BufferType: 'static + Disposable> {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub is_disposed: bool,
}

impl Mesh<crate::game::graphics::vk::Buffer> {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Mesh {
            vertices,
            indices,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false
        }
    }
}

impl<BufferType: 'static + Disposable> Drop for Mesh<BufferType> {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<BufferType: 'static + Disposable> Disposable for Mesh<BufferType> {
    fn dispose(&mut self) {
        unsafe {
            ManuallyDrop::drop(self.index_buffer.as_mut().unwrap());
            ManuallyDrop::drop(self.vertex_buffer.as_mut().unwrap());
        }
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        unimplemented!()
    }

    fn set_name(&mut self, name: String) -> &str {
        unimplemented!()
    }
}