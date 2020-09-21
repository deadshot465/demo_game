use ash::{
    Device,
    vk::*
};
use ash::version::DeviceV1_0;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Arc, Mutex};

use crate::game::enums::ShaderType;
use crate::game::graphics::vk::Shader;
use crate::game::structs::{BlendMode, Vertex, PushConstant};

pub struct Pipeline {
    pub render_pass: RenderPass,
    logical_device: Arc<Device>,
    pub pipeline_layouts: HashMap<ShaderType, PipelineLayout>,
    pub graphic_pipelines: HashMap<ShaderType, Vec<ash::vk::Pipeline>>,
    owned_renderpass: bool,
}

impl Pipeline {
    pub fn new(device: Arc<Device>) -> Self {
        Pipeline {
            logical_device: device,
            render_pass: RenderPass::null(),
            pipeline_layouts: HashMap::new(),
            graphic_pipelines: HashMap::new(),
            owned_renderpass: false,
        }
    }

    pub fn create_renderpass(&mut self, graphics_format: Format, depth_format: Format, sample_count: SampleCountFlags) {
        let mut attachment_descriptions = vec![];
        attachment_descriptions.push(AttachmentDescription::builder()
            .format(graphics_format)
            .samples(sample_count)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(AttachmentLoadOp::CLEAR)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .store_op(AttachmentStoreOp::STORE)
            .build());

        attachment_descriptions.push(AttachmentDescription::builder()
            .format(depth_format)
            .samples(sample_count)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(AttachmentLoadOp::CLEAR)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .store_op(AttachmentStoreOp::STORE)
            .build());

        attachment_descriptions.push(AttachmentDescription::builder()
            .format(graphics_format)
            .samples(SampleCountFlags::TYPE_1)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::PRESENT_SRC_KHR)
            .load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .store_op(AttachmentStoreOp::STORE)
            .build());

        let subpass_dependency = vec![SubpassDependency::builder()
            .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE)
            .src_access_mask(AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_subpass(0)
            .src_subpass(SUBPASS_EXTERNAL)
            .build()];

        let color_reference = vec![AttachmentReference::builder()
            .attachment(0)
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let depth_reference = AttachmentReference::builder()
            .attachment(1)
            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let resolve_reference = vec![AttachmentReference::builder()
            .attachment(2)
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let subpass_description = vec![SubpassDescription::builder()
            .color_attachments(color_reference.as_slice())
            .depth_stencil_attachment(&depth_reference)
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .resolve_attachments(resolve_reference.as_slice())
            .build()];

        let renderpass_info = RenderPassCreateInfo::builder()
            .attachments(attachment_descriptions.as_slice())
            .dependencies(subpass_dependency.as_slice())
            .subpasses(subpass_description.as_slice());
        unsafe {
            self.render_pass = self.logical_device
                .create_render_pass(&renderpass_info, None)
                .expect("Failed to create renderpass.");
            self.owned_renderpass = true;
        }
    }

    pub async fn create_graphic_pipelines(&mut self, descriptor_set_layout: &[DescriptorSetLayout],
                                          sample_count: SampleCountFlags,
                                          shaders: Vec<Shader>,
                                          _pipeline_cache: Option<PipelineCache>,
                                          shader_type: ShaderType) {
        let push_constant_range = vec![PushConstantRange::builder()
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<PushConstant>() as u32)
            .build()];
        let layout_info = PipelineLayoutCreateInfo::builder()
            .set_layouts(descriptor_set_layout)
            .push_constant_ranges(push_constant_range.as_slice());
        unsafe {
            let pipeline_layout = self.logical_device
                .create_pipeline_layout(&layout_info, None)
                .expect("Failed to create pipeline layout.");
            self.pipeline_layouts.insert(shader_type, pipeline_layout);
        }

        let alpha_blend_op = [
            BlendOp::ADD, BlendOp::ADD, BlendOp::ADD,
            BlendOp::ADD, BlendOp::ADD, BlendOp::ADD,
            BlendOp::MAX, BlendOp::MIN, BlendOp::ADD,
        ];

        let blend_enable = [
            false, true, true,
            true, true, true,
            true, true, true
        ];

        let color_blend_op = [
            BlendOp::ADD, BlendOp::ADD, BlendOp::ADD,
            BlendOp::ADD, BlendOp::ADD, BlendOp::ADD,
            BlendOp::MAX, BlendOp::MIN, BlendOp::ADD,
        ];

        let color_write_mask = ColorComponentFlags::R | ColorComponentFlags::G |
            ColorComponentFlags::B | ColorComponentFlags::A;

        let write_masks = [color_write_mask; 9];

        let dst_alpha_blend_factor = [
            BlendFactor::ZERO, BlendFactor::ONE_MINUS_SRC_ALPHA, BlendFactor::ONE,
            BlendFactor::ONE, BlendFactor::ZERO, BlendFactor::ZERO,
            BlendFactor::ONE, BlendFactor::ONE, BlendFactor::ONE_MINUS_SRC_ALPHA
        ];

        let dst_color_blend_factor = [
            BlendFactor::ZERO, BlendFactor::ONE_MINUS_SRC_ALPHA, BlendFactor::ONE,
            BlendFactor::ONE_MINUS_SRC_COLOR, BlendFactor::ZERO, BlendFactor::ZERO,
            BlendFactor::ONE, BlendFactor::ONE, BlendFactor::ONE_MINUS_SRC_COLOR
        ];

        let src_alpha_blend_factor = [
            BlendFactor::ONE, BlendFactor::ONE, BlendFactor::ZERO,
            BlendFactor::ZERO, BlendFactor::ONE, BlendFactor::DST_ALPHA,
            BlendFactor::ONE, BlendFactor::ONE, BlendFactor::ONE
        ];

        let src_color_blend_factor = [
            BlendFactor::ONE, BlendFactor::SRC_ALPHA, BlendFactor::SRC_ALPHA,
            BlendFactor::SRC_ALPHA, BlendFactor::SRC_ALPHA, BlendFactor::DST_COLOR,
            BlendFactor::ONE, BlendFactor::ONE, BlendFactor::SRC_ALPHA
        ];

        let mut worker_threads = vec![];
        let _shaders = Arc::new(Mutex::new(shaders));
        unsafe {
            for i in 0..BlendMode::END.0 {
                let color_attachment = vec![PipelineColorBlendAttachmentState::builder()
                    .color_write_mask(write_masks[i])
                    .alpha_blend_op(alpha_blend_op[i])
                    .blend_enable(blend_enable[i])
                    .color_blend_op(color_blend_op[i])
                    .dst_alpha_blend_factor(dst_alpha_blend_factor[i])
                    .dst_color_blend_factor(dst_color_blend_factor[i])
                    .src_alpha_blend_factor(src_alpha_blend_factor[i])
                    .src_color_blend_factor(src_color_blend_factor[i])
                    .build()];
                let ptr_shaders = _shaders.clone();
                let pipeline_layout = self.pipeline_layouts
                    .get(&shader_type).unwrap().clone();
                let render_pass = self.render_pass.clone();
                let device = self.logical_device.clone();
                worker_threads.push(
                    tokio::spawn(async move {
                        let attr_desc = Vertex::get_attribute_description(0);
                        let binding_desc = vec![Vertex::get_binding_description(0, VertexInputRate::VERTEX)];
                        let vi_info = PipelineVertexInputStateCreateInfo::builder()
                            .vertex_attribute_descriptions(attr_desc.as_slice())
                            .vertex_binding_descriptions(binding_desc.as_slice());
                        let ia_info = PipelineInputAssemblyStateCreateInfo::builder()
                            .primitive_restart_enable(false)
                            .topology(PrimitiveTopology::TRIANGLE_LIST);
                        let rs_info = PipelineRasterizationStateCreateInfo::builder()
                            .cull_mode(CullModeFlags::BACK)
                            .depth_bias_clamp(0.0)
                            .depth_bias_constant_factor(0.0)
                            .depth_bias_enable(false)
                            .depth_bias_slope_factor(0.0)
                            .depth_clamp_enable(false)
                            .front_face(FrontFace::CLOCKWISE)
                            .line_width(1.0)
                            .polygon_mode(PolygonMode::FILL)
                            .rasterizer_discard_enable(false);
                        let vp_info = PipelineViewportStateCreateInfo::builder()
                            .scissor_count(1)
                            .viewport_count(1);
                        let color_blend_info = PipelineColorBlendStateCreateInfo::builder()
                            .logic_op(LogicOp::COPY)
                            .attachments(color_attachment.as_slice())
                            .logic_op_enable(false);
                        let depth_info = PipelineDepthStencilStateCreateInfo::builder()
                            .depth_bounds_test_enable(false)
                            .depth_compare_op(CompareOp::LESS)
                            .depth_test_enable(true)
                            .depth_write_enable(true)
                            .stencil_test_enable(false);
                        let dynamic_states = vec![
                            DynamicState::SCISSOR, DynamicState::VIEWPORT
                        ];
                        let dynamic_info = PipelineDynamicStateCreateInfo::builder()
                            .dynamic_states(dynamic_states.as_slice());
                        let msaa_info = PipelineMultisampleStateCreateInfo::builder()
                            .alpha_to_coverage_enable(false)
                            .alpha_to_one_enable(false)
                            .min_sample_shading(0.25)
                            .rasterization_samples(sample_count)
                            .sample_shading_enable(true);
                        let shader_vector = ptr_shaders.lock().unwrap();
                        let mut stage_infos = shader_vector.iter()
                            .map(|s| s.shader_stage_info)
                            .collect::<Vec<_>>();
                        let name = CString::new("main").unwrap();
                        stage_infos.iter_mut().for_each(|s| {
                            s.p_name = name.as_ptr();
                        });
                        let pipeline_info = vec![GraphicsPipelineCreateInfo::builder()
                            .layout(pipeline_layout)
                            .base_pipeline_index(-1)
                            .base_pipeline_handle(ash::vk::Pipeline::null())
                            .color_blend_state(&color_blend_info)
                            .depth_stencil_state(&depth_info)
                            .dynamic_state(&dynamic_info)
                            .input_assembly_state(&ia_info)
                            .multisample_state(&msaa_info)
                            .rasterization_state(&rs_info)
                            .render_pass(render_pass)
                            .subpass(0)
                            .vertex_input_state(&vi_info)
                            .viewport_state(&vp_info)
                            .stages(stage_infos.as_slice())
                            .build()];
                        let pipeline = device
                            .create_graphics_pipelines(
                                PipelineCache::null(),
                                pipeline_info.as_slice(),
                                None
                            ).expect("Failed to create graphics pipeline.");
                        log::info!("Graphics pipeline successfully created.");
                        pipeline[0]
                    }));
            }
        }

        let mut pipelines = vec![];
        for worker in worker_threads.into_iter() {
            let pipeline = worker.await.unwrap();
            pipelines.push(pipeline);
        }
        self.graphic_pipelines.insert(shader_type, pipelines);
        log::info!("Graphic pipelines successfully created.");
    }

    pub fn get_pipeline(&self, shader_type: ShaderType, index: usize) -> ash::vk::Pipeline {
        let pipelines = self.graphic_pipelines.get(&shader_type).unwrap();
        pipelines.get(index).unwrap().clone()
    }

    pub fn get_pipeline_layout(&self, shader_type: ShaderType) -> ash::vk::PipelineLayout {
        self.pipeline_layouts.get(&shader_type).unwrap().clone()
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            for (_, pipelines) in self.graphic_pipelines.iter() {
                for pipeline in pipelines.iter() {
                    self.logical_device.destroy_pipeline(*pipeline, None);
                }
            }

            for (_, layout) in self.pipeline_layouts.iter() {
                self.logical_device.destroy_pipeline_layout(*layout, None);
            }

            if self.owned_renderpass {
                self.logical_device.destroy_render_pass(self.render_pass, None);
            }

            log::info!("Graphic pipelines successfully destroyed.");
        }
    }
}