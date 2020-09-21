use glam::{Mat4};
use parking_lot::Mutex;
use std::mem::ManuallyDrop;
use std::sync::Arc;

use crate::game::shared::structs::{Joint, SkinnedVertex};
use crate::game::traits::Disposable;

#[derive(Clone, Debug)]
pub struct SkinnedPrimitive<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: Option<ManuallyDrop<BufferType>>,
    pub index_buffer: Option<ManuallyDrop<BufferType>>,
    pub texture: Option<ManuallyDrop<TextureType>>,
    pub is_disposed: bool,
    pub command_pool: Option<Arc<Mutex<ash::vk::CommandPool>>>,
    pub command_buffer: Option<CommandType>,
}

unsafe impl<BufferType, CommandType, TextureType> Send for SkinnedPrimitive<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {}
unsafe impl<BufferType, CommandType, TextureType> Sync for SkinnedPrimitive<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {}

impl<BufferType, CommandType, TextureType> Drop for SkinnedPrimitive<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<BufferType, CommandType, TextureType> Disposable for SkinnedPrimitive<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    fn dispose(&mut self) {
        unsafe {
            if let Some(buffer) = self.vertex_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.index_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(texture) = self.texture.as_mut() {
                ManuallyDrop::drop(texture);
            }
        }
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

#[derive(Clone, Debug)]
pub struct SkinnedMesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    pub primitives: Vec<SkinnedPrimitive<BufferType, CommandType, TextureType>>,
    pub is_disposed: bool,
    pub transform: Mat4,
    pub root_joint: Option<Joint>
}

impl<BufferType, CommandType, TextureType> Drop for SkinnedMesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped skinned mesh.");
        }
        else {
            log::warn!("Skinned mesh is already dropped.");
        }
    }
}

impl<BufferType, CommandType, TextureType> Disposable for SkinnedMesh<BufferType, CommandType, TextureType>
    where BufferType: 'static + Clone + Disposable,
          CommandType: 'static,
          TextureType: 'static + Clone + Disposable {
    fn dispose(&mut self) {
        for primitive in self.primitives.iter_mut() {
            primitive.dispose();
        }
        self.is_disposed = true;
        log::info!("Successfully disposed skinned mesh.");
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