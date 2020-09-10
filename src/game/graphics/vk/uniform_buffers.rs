use crate::game::shared::traits::Disposable;
use std::mem::ManuallyDrop;

pub struct UniformBuffers {
    pub is_disposed: bool,
    pub view_projection: ManuallyDrop<super::Buffer>,
    pub directional_light: ManuallyDrop<super::Buffer>,
    pub model_buffer: Option<ManuallyDrop<super::Buffer>>,
    pub mesh_buffer: Option<ManuallyDrop<super::Buffer>>,
}

impl UniformBuffers {
    pub fn new(view_projection: super::Buffer, directional_light: super::Buffer) -> Self {
        UniformBuffers {
            is_disposed: false,
            view_projection: ManuallyDrop::new(view_projection),
            directional_light: ManuallyDrop::new(directional_light),
            model_buffer: None,
            mesh_buffer: None
        }
    }
}

impl Drop for UniformBuffers {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl Disposable for UniformBuffers {
    fn dispose(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.view_projection);
            ManuallyDrop::drop(&mut self.directional_light);
            if let Some(buffer) = self.model_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
            }
            if let Some(buffer) = self.mesh_buffer.as_mut() {
                ManuallyDrop::drop(buffer);
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