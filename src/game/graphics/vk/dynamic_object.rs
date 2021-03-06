use ash::vk::DeviceSize;
use glam::Mat4;

/// 動的ユニーフォームバッファを使うときのワールド行列データ<br />
/// World matrix data when using dynamic uniform buffer
pub struct DynamicModel {
    pub model_matrices: Vec<Mat4>,
    pub buffer: *mut Mat4,
}

/// 動的ユニーフォームバッファのワールド行列データ、とメモリーの中の最低限の間隔<br />
/// World matrix data when using dynamic uniform buffer and the minimum alignment information in the memory
pub struct DynamicBufferObject {
    pub models: DynamicModel,
    pub meshes: DynamicModel,
    pub min_alignment: DeviceSize,
    pub dynamic_alignment: DeviceSize,
}

impl Default for DynamicBufferObject {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DynamicModel {
    fn default() -> Self {
        Self::new()
    }
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
            dynamic_alignment: 0,
        }
    }
}

impl Drop for DynamicModel {
    fn drop(&mut self) {
        unsafe {
            if self.buffer.is_null() {
                return;
            }
            aligned_alloc::aligned_free(self.buffer as *mut ());
        }
    }
}
