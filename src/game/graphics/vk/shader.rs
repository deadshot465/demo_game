use ash::{
    Device,
    util::read_spv,
    vk::{
        PipelineShaderStageCreateInfo,
        ShaderModule,
        ShaderModuleCreateInfo,
        ShaderStageFlags,
    }
};
use ash::version::DeviceV1_0;
use std::ffi::CString;
use std::sync::Arc;

use crate::game::traits::disposable::Disposable;

pub struct Shader {
    logical_device: Arc<Device>,
    pub file_name: String,
    pub shader_module: ShaderModule,
    pub shader_stage_info: PipelineShaderStageCreateInfo,
    pub is_disposed: bool,
}

unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

impl Shader {
    pub fn new(device: Arc<Device>, file_name: &str, stage_flag: ShaderStageFlags) -> Self {
        let name = CString::new("main").unwrap();
        let mut file = std::fs::File::open(file_name).unwrap();
        let bytes = read_spv(&mut file).unwrap();
        let module_info = ShaderModuleCreateInfo::builder()
            .code(bytes.as_slice())
            .build();

        unsafe {
            let shader_module = device
                .create_shader_module(&module_info, None)
                .expect("Failed to create shader module.");
            let mut shader_stage_info = PipelineShaderStageCreateInfo::builder()
                .module(shader_module)
                .stage(stage_flag)
                .build();
            shader_stage_info.p_name = name.as_ptr();

            Shader {
                logical_device: device,
                file_name: file_name.to_string(),
                shader_module,
                shader_stage_info,
                is_disposed: false,
            }
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl Disposable for Shader {
    fn dispose(&mut self) {
        unsafe {
            self.logical_device
                .destroy_shader_module(self.shader_module, None);
            self.is_disposed = true;
        }
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        self.file_name.as_str()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.file_name = name;
        self.file_name.as_str()
    }
}