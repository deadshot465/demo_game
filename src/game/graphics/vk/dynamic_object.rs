use ash::vk::DeviceSize;
use glam::Mat4;

pub struct DynamicModel {
    pub model_matrices: Vec<Mat4>,
    pub buffer: *mut Mat4,
}

pub struct DynamicBufferObject {
    pub models: DynamicModel,
    pub meshes: DynamicModel,
    pub min_alignment: DeviceSize,
    pub dynamic_alignment: DeviceSize,
}

impl DynamicModel {
    pub fn new() -> Self {
        DynamicModel {
            model_matrices: vec![],
            buffer: std::ptr::null_mut(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.model_matrices.is_empty()
    }
}

impl DynamicBufferObject {
    pub fn new() -> Self {
        DynamicBufferObject {
            models: DynamicModel::new(),
            meshes: DynamicModel::new(),
            min_alignment: 0,
            dynamic_alignment: 0
        }
    }
}

impl Drop for DynamicModel {
    fn drop(&mut self) {
        unsafe {
            if self.buffer == std::ptr::null_mut() {
                return;
            }
            aligned_alloc::aligned_free(self.buffer as *mut ());
        }
    }
}