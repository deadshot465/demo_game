pub mod buffer;
pub mod texture;
pub use buffer::Buffer;
pub use texture::Texture;

use ash::version::DeviceV1_0;
use ash::vk::*;
use ash::Device;
use image::GenericImageView;
use nuklear::{
    font_cyrillic_glyph_ranges, Buffer as NkBuffer, Context, ConvertConfig, DrawNullTexture,
    DrawVertexLayoutAttribute, DrawVertexLayoutElements, DrawVertexLayoutFormat, FontAtlas,
    FontAtlasFormat, FontConfig, FontID, Handle, Size, UserFont, Vec2,
};
use std::collections::HashMap;
use std::sync::Arc;

struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

type Ortho = [[f32; 4]; 4];

pub struct Drawer {
    pub allocator: nuklear::Allocator,
    pub draw_null_texture: DrawNullTexture,
    nuklear_buffer: NkBuffer,
    logical_device: Arc<Device>,
    instance: Arc<ash::Instance>,
    physical_device: PhysicalDevice,
    graphics_queue: Queue,
    graphics_queue_index: u32,
    color_format: Format,
    depth_format: Format,
    sample_count: SampleCountFlags,
    render_completed: Semaphore,
    command_finished: Fence,
    font_sampler: Sampler,
    font_image: Texture,
    pipeline_layout: PipelineLayout,
    pipeline: Pipeline,
    renderpass: RenderPass,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    uniform_buffer: Buffer,
    command_pool: CommandPool,
    command_buffer: CommandBuffer,
    descriptor_pool: DescriptorPool,
    descriptor_set_layout: DescriptorSetLayout,
    descriptor_set: DescriptorSet,
    layout_elements: DrawVertexLayoutElements,
    font_config: FontConfig,
    font_atlas: FontAtlas,
    fonts: HashMap<u8, FontID>,
    textures: Vec<Texture>,
    texture_ids: Vec<Handle>,
}

impl Drawer {
    pub unsafe fn new(
        device: Arc<ash::Device>,
        instance: Arc<ash::Instance>,
        physical_device: PhysicalDevice,
        graphics_queue: Queue,
        graphics_queue_index: u32,
        color_format: Format,
        depth_format: Format,
        sample_count: SampleCountFlags,
        vertex_buffer_size: u64,
        index_buffer_size: u64,
        nk_command_buffer_size: usize,
        font_bytes: &[u8],
    ) -> Self {
        let semaphore = Self::create_semaphore(&*device);
        let fence = Self::create_fence(&*device);
        let renderpass =
            Self::create_renderpass(&*device, color_format, depth_format, sample_count);
        let vertex_buffer = Buffer::new(
            &*device,
            vertex_buffer_size,
            &*instance,
            physical_device,
            BufferUsageFlags::VERTEX_BUFFER,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
        );
        let index_buffer = Buffer::new(
            &*device,
            index_buffer_size,
            &*instance,
            physical_device,
            BufferUsageFlags::INDEX_BUFFER,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
        );
        let uniform_buffer = Buffer::new(
            &*device,
            std::mem::size_of::<glam::Mat4>() as u64,
            &*instance,
            physical_device,
            BufferUsageFlags::UNIFORM_BUFFER,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
        );
        let descriptor_pool = Self::create_descriptor_pool(&*device);
        let descriptor_set_layout = Self::create_descriptor_set_layout(&*device);
        let layouts = [descriptor_set_layout];
        let descriptor_set = Self::create_descriptor_set(&*device, descriptor_pool, &layouts[0..]);
        let pipeline_layout = Self::create_pipeline_layout(&*device, &layouts[0..]);
        let vertex_shader = Self::create_shader_module(&*device, "shaders/ui_vert.spv");
        let fragment_shader = Self::create_shader_module(&*device, "shaders/ui_frag.spv");

        let name = std::ffi::CString::new("main").expect("Failed to create CString for shader.");
        let mut shader_stage_info = vec![PipelineShaderStageCreateInfo::builder()
            .stage(ShaderStageFlags::VERTEX)
            .name(name.as_c_str())
            .module(vertex_shader)
            .build()];
        shader_stage_info.push(
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::FRAGMENT)
                .name(name.as_c_str())
                .module(fragment_shader)
                .build(),
        );
        let pipeline = Self::create_pipeline(
            &*device,
            sample_count,
            pipeline_layout,
            renderpass,
            shader_stage_info.as_slice(),
        );

        device.destroy_shader_module(vertex_shader, None);
        device.destroy_shader_module(fragment_shader, None);

        let command_pool = Self::create_command_pool(&*device, graphics_queue_index);
        let command_buffer = Self::allocate_command_buffers(&*device, command_pool);
        let mut nk_allocator = nuklear::Allocator::new_vec();
        let mut font_config = Self::create_font_config(font_bytes);
        let (mut atlas, fonts) = Self::setup_font_atlas(&mut nk_allocator, &mut font_config);
        let mut draw_null_texture = DrawNullTexture::default();
        let (font_image, font_sampler) = Self::bake_font(
            &mut atlas,
            &*device,
            &*instance,
            physical_device,
            command_pool,
            graphics_queue,
            &mut draw_null_texture,
        );
        Self::update_write_descriptor_set(
            &uniform_buffer,
            &font_image,
            font_sampler,
            descriptor_set,
            &*device,
        );

        Drawer {
            nuklear_buffer: nuklear::Buffer::with_size(&mut nk_allocator, nk_command_buffer_size),
            draw_null_texture,
            logical_device: device,
            instance,
            physical_device,
            graphics_queue,
            graphics_queue_index,
            color_format,
            depth_format,
            render_completed: semaphore,
            command_finished: fence,
            font_sampler,
            font_image,
            pipeline_layout,
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            command_pool,
            command_buffer,
            descriptor_pool,
            descriptor_set_layout,
            descriptor_set,
            layout_elements: DrawVertexLayoutElements::new(&[
                (
                    DrawVertexLayoutAttribute::Position,
                    DrawVertexLayoutFormat::Float,
                    memoffset::offset_of!(Vertex, position),
                ),
                (
                    DrawVertexLayoutAttribute::TexCoord,
                    DrawVertexLayoutFormat::Float,
                    memoffset::offset_of!(Vertex, uv),
                ),
                (
                    DrawVertexLayoutAttribute::Color,
                    DrawVertexLayoutFormat::R8G8B8A8,
                    memoffset::offset_of!(Vertex, color),
                ),
                (
                    DrawVertexLayoutAttribute::AttributeCount,
                    DrawVertexLayoutFormat::Count,
                    0,
                ),
            ]),
            font_config,
            renderpass,
            sample_count,
            font_atlas: atlas,
            fonts,
            allocator: nk_allocator,
            textures: vec![],
            texture_ids: vec![],
        }
    }

    pub fn add_texture_from_file(&mut self, file_name: &str) {
        let raw_bytes = std::fs::read(file_name).expect("Failed to open texture file for Nuklear.");
        let texture = Self::create_texture(
            &*self.logical_device,
            &*self.instance,
            self.physical_device,
            self.command_pool,
            self.graphics_queue,
            raw_bytes.as_slice(),
            self.color_format,
        );
        self.textures.push(texture);
        let handle = Handle::from_id(self.textures.len() as i32);
        self.texture_ids.push(handle);
    }

    pub fn add_texture_from_image(&mut self, image: crate::game::Image) {
        self.textures.push(Texture {
            image: image.image,
            image_view: image.image_view,
            device_memory: image.device_memory,
        });
        let handle = Handle::from_id(self.textures.len() as i32);
        self.texture_ids.push(handle);
    }

    pub fn create_context(&mut self, font_size: u8) -> Context {
        let font = self.get_font(font_size).clone();
        Context::new(&mut self.allocator, &font)
    }

    pub fn draw(
        &mut self,
        framebuffer: Framebuffer,
        viewport: Viewport,
        scale: Vec2,
        context: &mut Context,
        convert_config: &mut ConvertConfig,
        wait_semaphore: Semaphore,
    ) -> Semaphore {
        let cmd_begin_info =
            CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        let renderpass_begin_info = RenderPassBeginInfo::builder()
            .framebuffer(framebuffer)
            .render_area(Rect2D {
                offset: Offset2D { x: 0, y: 0 },
                extent: Extent2D {
                    width: viewport.width as u32,
                    height: viewport.height as u32,
                },
            })
            .render_pass(self.renderpass);
        unsafe {
            let cmd_buffer = self.command_buffer;
            let fences = [self.command_finished];
            {
                let device = &self.logical_device;
                device
                    .wait_for_fences(&fences[0..], true, u64::MAX)
                    .expect("Failed to wait for fences for Nuklear.");
                device
                    .reset_fences(&fences[0..])
                    .expect("Failed to reset fences for Nuklear.");
                device
                    .reset_command_pool(self.command_pool, CommandPoolResetFlags::empty())
                    .expect("Failed to reset command pool for Nuklear.");
                device
                    .begin_command_buffer(cmd_buffer, &cmd_begin_info)
                    .expect("Failed to begin command buffer for Nuklear.");
                device.cmd_begin_render_pass(
                    cmd_buffer,
                    &renderpass_begin_info,
                    SubpassContents::INLINE,
                );
                let viewports = [viewport];
                device.cmd_set_viewport(cmd_buffer, 0, &viewports[0..]);
                device.cmd_bind_pipeline(cmd_buffer, PipelineBindPoint::GRAPHICS, self.pipeline);
                let descriptor_sets = [self.descriptor_set];
                device.cmd_bind_descriptor_sets(
                    cmd_buffer,
                    PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &descriptor_sets[0..],
                    &[],
                );
            }
            self.update(
                viewport.width as u32,
                viewport.height as u32,
                context,
                convert_config,
            );
            let device = &self.logical_device;
            let vertex_buffers = [self.vertex_buffer.buffer];
            let offsets = [0];
            device.cmd_bind_vertex_buffers(cmd_buffer, 0, &vertex_buffers[0..], &offsets[0..]);
            device.cmd_bind_index_buffer(
                cmd_buffer,
                self.index_buffer.buffer,
                0,
                IndexType::UINT16,
            );

            let mut index_offset = 0;
            for cmd in context.draw_command_iterator(&self.nuklear_buffer) {
                if cmd.elem_count() < 1 {
                    continue;
                }
                let scissors = vec![Rect2D {
                    offset: Offset2D {
                        x: ((cmd.clip_rect().x * scale.x) as i32).max(0),
                        y: ((cmd.clip_rect().y * scale.y) as i32).max(0),
                    },
                    extent: Extent2D {
                        width: (cmd.clip_rect().w * scale.x) as u32,
                        height: (cmd.clip_rect().h * scale.y) as u32,
                    },
                }];
                device.cmd_set_scissor(cmd_buffer, 0, scissors.as_slice());
                device.cmd_draw_indexed(cmd_buffer, cmd.elem_count(), 1, index_offset, 0, 0);
                index_offset += cmd.elem_count();
            }
            context.clear();
            device.cmd_end_render_pass(cmd_buffer);
            device
                .end_command_buffer(cmd_buffer)
                .expect("Failed to end command buffer for Nuklear.");

            let wait_stages = [PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cmd_buffers = [cmd_buffer];
            let signal_semaphore = [self.render_completed];
            let wait_semaphores = [wait_semaphore];
            let submit_info = vec![SubmitInfo::builder()
                .command_buffers(&cmd_buffers[0..])
                .signal_semaphores(&signal_semaphore[0..])
                .wait_dst_stage_mask(&wait_stages[0..])
                .wait_semaphores(&wait_semaphores[0..])
                .build()];

            device
                .queue_submit(
                    self.graphics_queue,
                    submit_info.as_slice(),
                    self.command_finished,
                )
                .expect("Failed to submit queue for Nuklear.");
            self.render_completed
        }
    }

    pub fn get_font(&self, font_size: u8) -> &UserFont {
        let font_id = self
            .fonts
            .get(&font_size)
            .expect("Failed to get requested font size in Nuklear.");
        self.font_atlas
            .font(*font_id)
            .expect("Failed to get font in the font atlas.")
            .handle()
    }

    pub fn set_font_size(&mut self, context: &mut Context, font_size: u8) {
        let font_id = self
            .fonts
            .get(&font_size)
            .expect("Failed to get requested font size in Nuklear.");
        let atlas = &mut self.font_atlas;
        context.style_set_font(
            atlas
                .font(*font_id)
                .expect("Failed to get font in the font atlas.")
                .handle(),
        );
    }

    pub fn update(
        &mut self,
        width: u32,
        height: u32,
        context: &mut Context,
        convert_config: &mut ConvertConfig,
    ) {
        let ortho: Ortho = [
            [2.0 / width as f32, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height as f32, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ];
        {
            let mapped = self.uniform_buffer.get_mapped_memory(&self.logical_device);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &ortho as *const _ as *const std::ffi::c_void,
                    mapped,
                    std::mem::size_of::<Ortho>(),
                );
            }
        }
        convert_config.set_vertex_size(std::mem::size_of::<Vertex>() as Size);
        convert_config.set_vertex_layout(&self.layout_elements);
        {
            let vertex_mapped = self.vertex_buffer.get_mapped_memory(&self.logical_device);
            let index_mapped = self.index_buffer.get_mapped_memory(&self.logical_device);
            let vertex_slice = unsafe {
                std::slice::from_raw_parts_mut(
                    vertex_mapped as *mut u8,
                    self.vertex_buffer.buffer_size as usize,
                )
            };
            let mut v_buffer = NkBuffer::with_fixed(vertex_slice);

            let index_slice = unsafe {
                std::slice::from_raw_parts_mut(
                    index_mapped as *mut u8,
                    self.index_buffer.buffer_size as usize,
                )
            };
            let mut i_buffer = NkBuffer::with_fixed(index_slice);
            context.convert(
                &mut self.nuklear_buffer,
                &mut v_buffer,
                &mut i_buffer,
                convert_config,
            );
        }
    }

    pub fn wait_idle(&self) {
        unsafe {
            let fences = [self.command_finished];
            self.logical_device
                .wait_for_fences(&fences[0..], true, u64::MAX)
                .expect("Failed to wait for fences for Nuklear.");
        }
    }

    fn allocate_command_buffers(device: &ash::Device, command_pool: CommandPool) -> CommandBuffer {
        let allocate_info = CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(1)
            .level(CommandBufferLevel::PRIMARY);
        unsafe {
            let command_buffers = device
                .allocate_command_buffers(&allocate_info)
                .expect("Failed to allocate command buffers for Nuklear.");
            command_buffers[0]
        }
    }

    fn bake_font(
        atlas: &mut FontAtlas,
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        command_pool: CommandPool,
        queue: Queue,
        null_texture: &mut DrawNullTexture,
    ) -> (Texture, Sampler) {
        let (bytes, width, height) = atlas.bake(FontAtlasFormat::Rgba32);
        let (font_texture, sampler) = Self::upload_atlas(
            bytes,
            width,
            height,
            device,
            instance,
            physical_device,
            command_pool,
            queue,
        );
        let id = Handle::from_id(0);
        atlas.end(id, Some(null_texture));
        (font_texture, sampler)
    }

    fn create_command_pool(device: &ash::Device, queue_index: u32) -> CommandPool {
        let pool_info = CommandPoolCreateInfo::builder().queue_family_index(queue_index);
        unsafe {
            device
                .create_command_pool(&pool_info, None)
                .expect("Failed to create command pool for Nuklear.")
        }
    }

    fn create_descriptor_pool(device: &ash::Device) -> DescriptorPool {
        let mut pool_sizes = vec![DescriptorPoolSize::builder()
            .descriptor_count(1)
            .ty(DescriptorType::UNIFORM_BUFFER)
            .build()];
        pool_sizes.push(
            DescriptorPoolSize::builder()
                .descriptor_count(1)
                .ty(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .build(),
        );
        let pool_info = DescriptorPoolCreateInfo::builder()
            .pool_sizes(pool_sizes.as_slice())
            .max_sets(1);
        unsafe {
            device
                .create_descriptor_pool(&pool_info, None)
                .expect("Failed to create descriptor pool for Nuklear.")
        }
    }

    fn create_descriptor_set(
        device: &ash::Device,
        descriptor_pool: DescriptorPool,
        descriptor_set_layout: &[DescriptorSetLayout],
    ) -> DescriptorSet {
        let allocate_info = DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(descriptor_set_layout);
        unsafe {
            let sets = device
                .allocate_descriptor_sets(&allocate_info)
                .expect("Failed to allocate descriptor set for Nuklear.");
            sets[0]
        }
    }

    fn create_descriptor_set_layout(device: &ash::Device) -> DescriptorSetLayout {
        let mut layout_bindings = vec![DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::UNIFORM_BUFFER)
            .stage_flags(ShaderStageFlags::VERTEX)
            .build()];
        layout_bindings.push(
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .descriptor_count(1)
                .build(),
        );
        let layout_info =
            DescriptorSetLayoutCreateInfo::builder().bindings(layout_bindings.as_slice());
        unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .expect("Failed to create descriptor set layout for Nuklear.")
        }
    }

    fn create_fence(device: &ash::Device) -> Fence {
        let fence_info = FenceCreateInfo::builder().flags(FenceCreateFlags::SIGNALED);
        unsafe {
            device
                .create_fence(&fence_info, None)
                .expect("Failed to create fence for Nuklear.")
        }
    }

    fn create_font_config(font_bytes: &[u8]) -> FontConfig {
        let mut font_config = FontConfig::with_size(0.0);
        font_config.set_oversample_h(3);
        font_config.set_oversample_v(2);
        font_config.set_glyph_range(font_cyrillic_glyph_ranges());
        font_config.set_ttf(font_bytes);
        font_config
    }

    fn create_texture(
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        command_pool: CommandPool,
        graphics_queue: Queue,
        raw_data: &[u8],
        color_format: ash::vk::Format,
    ) -> Texture {
        let img = image::load_from_memory(raw_data).expect("Failed to read texture from memory.");
        let (width, height) = img.dimensions();
        let mut texture = Texture::new(
            width,
            height,
            device,
            instance,
            physical_device,
            MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let image_memory_requirements =
            unsafe { device.get_image_memory_requirements(texture.image) };
        let staging_buffer = Buffer::new(
            device,
            image_memory_requirements.size,
            instance,
            physical_device,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
        );
        let buffer_memory_requirements =
            unsafe { device.get_buffer_memory_requirements(staging_buffer.buffer) };
        let mapped = unsafe {
            device
                .map_memory(
                    staging_buffer.device_memory,
                    0,
                    buffer_memory_requirements.size,
                    MemoryMapFlags::empty(),
                )
                .expect("Failed to map memory for staging buffer.")
        };
        let rgba_raw_data = img.to_rgba8();
        let bgra_raw_data = img.to_bgra8();
        let raw_bytes = match color_format {
            Format::B8G8R8A8_UNORM => bgra_raw_data.as_ptr(),
            Format::R8G8B8A8_UNORM => rgba_raw_data.as_ptr(),
            _ => panic!("The color format of the texture has to either be RGBA or BGRA."),
        };
        unsafe {
            std::ptr::copy_nonoverlapping(
                raw_bytes as *const std::ffi::c_void,
                mapped,
                (width * height * 4) as usize,
            );
            device.unmap_memory(staging_buffer.device_memory);
        }
        texture.copy_buffer_to_image(
            device,
            staging_buffer.buffer,
            command_pool,
            graphics_queue,
            width,
            height,
        );
        texture.create_image_view(device);

        unsafe {
            device.free_memory(staging_buffer.device_memory, None);
            device.destroy_buffer(staging_buffer.buffer, None);
        }
        texture
    }

    fn create_pipeline(
        device: &ash::Device,
        sample_count: SampleCountFlags,
        pipeline_layout: PipelineLayout,
        renderpass: RenderPass,
        shader_stages: &[PipelineShaderStageCreateInfo],
    ) -> Pipeline {
        let mut vertex_attribute_descriptions = vec![];
        vertex_attribute_descriptions.push(
            VertexInputAttributeDescription::builder()
                .format(Format::R32G32_SFLOAT)
                .binding(0)
                .offset(memoffset::offset_of!(Vertex, position) as u32)
                .location(0)
                .build(),
        );
        vertex_attribute_descriptions.push(
            VertexInputAttributeDescription::builder()
                .format(Format::R32G32_SFLOAT)
                .binding(0)
                .offset(memoffset::offset_of!(Vertex, uv) as u32)
                .location(1)
                .build(),
        );
        vertex_attribute_descriptions.push(
            VertexInputAttributeDescription::builder()
                .format(Format::R8G8B8A8_UINT)
                .binding(0)
                .offset(memoffset::offset_of!(Vertex, color) as u32)
                .location(2)
                .build(),
        );

        let vertex_binding_description = vec![VertexInputBindingDescription::builder()
            .binding(0)
            .input_rate(VertexInputRate::VERTEX)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .build()];

        let vi_info = PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(vertex_attribute_descriptions.as_slice())
            .vertex_binding_descriptions(vertex_binding_description.as_slice());

        let ia_info = PipelineInputAssemblyStateCreateInfo::builder()
            .primitive_restart_enable(false)
            .topology(PrimitiveTopology::TRIANGLE_LIST);

        let vp_info = PipelineViewportStateCreateInfo::builder()
            .scissor_count(1)
            .viewport_count(1);

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

        let attachment_state = vec![PipelineColorBlendAttachmentState::builder()
            .alpha_blend_op(BlendOp::ADD)
            .blend_enable(true)
            .color_blend_op(BlendOp::ADD)
            .color_write_mask(ColorComponentFlags::all())
            .dst_alpha_blend_factor(BlendFactor::ZERO)
            .src_alpha_blend_factor(BlendFactor::ONE)
            .dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
            .src_color_blend_factor(BlendFactor::SRC_ALPHA)
            .build()];

        let color_info = PipelineColorBlendStateCreateInfo::builder()
            .attachments(attachment_state.as_slice())
            .logic_op_enable(false);

        let depth_info = PipelineDepthStencilStateCreateInfo::builder()
            .depth_bounds_test_enable(false)
            .depth_compare_op(CompareOp::LESS_OR_EQUAL)
            .depth_test_enable(true)
            .depth_write_enable(true)
            .stencil_test_enable(false);

        let msaa_info = PipelineMultisampleStateCreateInfo::builder()
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false)
            .min_sample_shading(0.25)
            .rasterization_samples(sample_count)
            .sample_shading_enable(true);

        let dynamic_states = [DynamicState::SCISSOR, DynamicState::VIEWPORT];

        let dynamic_state_info =
            PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_states[0..]);

        unsafe {
            let pipeline_info = vec![GraphicsPipelineCreateInfo::builder()
                .render_pass(renderpass)
                .layout(pipeline_layout)
                .base_pipeline_index(-1)
                .color_blend_state(&color_info)
                .dynamic_state(&dynamic_state_info)
                .input_assembly_state(&ia_info)
                .rasterization_state(&rs_info)
                .stages(shader_stages)
                .subpass(0)
                .vertex_input_state(&vi_info)
                .viewport_state(&vp_info)
                .multisample_state(&msaa_info)
                .depth_stencil_state(&depth_info)
                .build()];

            let pipeline = device
                .create_graphics_pipelines(PipelineCache::null(), pipeline_info.as_slice(), None)
                .expect("Failed to create graphics pipeline for Nuklear.");
            pipeline[0]
        }
    }

    fn create_pipeline_layout(
        device: &ash::Device,
        descriptor_set_layouts: &[DescriptorSetLayout],
    ) -> PipelineLayout {
        let layout_info = PipelineLayoutCreateInfo::builder().set_layouts(descriptor_set_layouts);
        unsafe {
            device
                .create_pipeline_layout(&layout_info, None)
                .expect("Failed to create pipeline layout for Nuklear.")
        }
    }

    fn create_renderpass(
        device: &ash::Device,
        color_format: Format,
        depth_format: Format,
        sample_count: SampleCountFlags,
    ) -> RenderPass {
        let mut attachments = vec![AttachmentDescription::builder()
            .format(color_format)
            .initial_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .samples(sample_count)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .load_op(AttachmentLoadOp::LOAD)
            .final_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        attachments.push(
            AttachmentDescription::builder()
                .format(depth_format)
                .initial_layout(ImageLayout::UNDEFINED)
                .samples(sample_count)
                .store_op(AttachmentStoreOp::STORE)
                .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                .load_op(AttachmentLoadOp::DONT_CARE)
                .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .build(),
        );

        attachments.push(
            AttachmentDescription::builder()
                .format(color_format)
                .initial_layout(ImageLayout::UNDEFINED)
                .samples(SampleCountFlags::TYPE_1)
                .store_op(AttachmentStoreOp::STORE)
                .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                .load_op(AttachmentLoadOp::DONT_CARE)
                .final_layout(ImageLayout::PRESENT_SRC_KHR)
                .build(),
        );

        let color_reference = vec![AttachmentReference::builder()
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .attachment(0)
            .build()];

        let depth_reference = AttachmentReference::builder()
            .attachment(1)
            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let resolve_reference = vec![AttachmentReference::builder()
            .attachment(2)
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let subpass_description = vec![SubpassDescription::builder()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .color_attachments(color_reference.as_slice())
            .depth_stencil_attachment(&depth_reference)
            .resolve_attachments(resolve_reference.as_slice())
            .build()];

        let mut subpass_dependencies = vec![SubpassDependency::builder()
            .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
            .src_access_mask(AccessFlags::MEMORY_READ)
            .src_subpass(SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(PipelineStageFlags::FRAGMENT_SHADER)
            .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dependency_flags(DependencyFlags::BY_REGION)
            .build()];

        subpass_dependencies.push(
            SubpassDependency::builder()
                .dst_access_mask(AccessFlags::MEMORY_READ)
                .src_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                .src_subpass(0)
                .dst_subpass(SUBPASS_EXTERNAL)
                .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(PipelineStageFlags::FRAGMENT_SHADER)
                .dependency_flags(DependencyFlags::BY_REGION)
                .build(),
        );

        let renderpass_info = RenderPassCreateInfo::builder()
            .attachments(attachments.as_slice())
            .subpasses(subpass_description.as_slice())
            .dependencies(subpass_dependencies.as_slice());

        unsafe {
            device
                .create_render_pass(&renderpass_info, None)
                .expect("Failed to create renderpass for Nuklear.")
        }
    }

    fn create_semaphore(device: &ash::Device) -> Semaphore {
        let semaphore_info = SemaphoreCreateInfo::builder();
        unsafe {
            device
                .create_semaphore(&semaphore_info, None)
                .expect("Failed to create semaphore for Nuklear.")
        }
    }

    fn create_shader_module(device: &ash::Device, file_name: &str) -> ShaderModule {
        let mut file = std::fs::File::open(file_name)
            .unwrap_or_else(|_| panic!("Failed to open shader file: {}", file_name));
        let byte_code = ash::util::read_spv(&mut file)
            .unwrap_or_else(|_| panic!("Failed to read shader byte code: {}", file_name));
        let shader_module_info = ShaderModuleCreateInfo::builder().code(byte_code.as_slice());
        unsafe {
            device
                .create_shader_module(&shader_module_info, None)
                .expect("Failed to create shader module.")
        }
    }

    fn setup_font_atlas(
        allocator: &mut nuklear::Allocator,
        font_config: &mut FontConfig,
    ) -> (FontAtlas, HashMap<u8, FontID>) {
        let mut fonts = HashMap::new();
        let mut atlas = FontAtlas::new(allocator);
        atlas.begin();

        for i in (12..48).step_by(4) {
            font_config.set_ttf_data_owned_by_atlas(false);
            font_config.set_size(i as f32);
            let font = atlas
                .add_font_with_config(&font_config)
                .expect("Failed to load font into Nuklear runtime.");
            fonts.insert(i, font);
        }

        (atlas, fonts)
    }

    fn update_write_descriptor_set(
        uniform_buffer: &Buffer,
        font_texture: &Texture,
        sampler: Sampler,
        descriptor_set: DescriptorSet,
        device: &ash::Device,
    ) {
        let buffer_info = vec![DescriptorBufferInfo::builder()
            .buffer(uniform_buffer.buffer)
            .offset(0)
            .range(std::mem::size_of::<Ortho>() as u64)
            .build()];

        let image_info = vec![DescriptorImageInfo::builder()
            .sampler(sampler)
            .image_view(font_texture.image_view)
            .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .build()];

        let mut write_descriptors = vec![WriteDescriptorSet::builder()
            .descriptor_type(DescriptorType::UNIFORM_BUFFER)
            .buffer_info(buffer_info.as_slice())
            .dst_array_element(0)
            .dst_binding(0)
            .dst_set(descriptor_set)
            .build()];

        write_descriptors.push(
            WriteDescriptorSet::builder()
                .image_info(image_info.as_slice())
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .dst_set(descriptor_set)
                .dst_binding(1)
                .dst_array_element(0)
                .build(),
        );

        unsafe {
            device.update_descriptor_sets(write_descriptors.as_slice(), &[]);
        }
    }

    fn upload_atlas(
        image: &[u8],
        width: u32,
        height: u32,
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        command_pool: CommandPool,
        graphics_queue: Queue,
    ) -> (Texture, Sampler) {
        let mut texture = Texture::new(
            width,
            height,
            device,
            instance,
            physical_device,
            MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let image_memory_requirements =
            unsafe { device.get_image_memory_requirements(texture.image) };
        let staging_buffer = Buffer::new(
            device,
            image_memory_requirements.size,
            instance,
            physical_device,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
        );
        let buffer_memory_requirements =
            unsafe { device.get_buffer_memory_requirements(staging_buffer.buffer) };
        let mapped = unsafe {
            device
                .map_memory(
                    staging_buffer.device_memory,
                    0,
                    buffer_memory_requirements.size,
                    MemoryMapFlags::empty(),
                )
                .expect("Failed to map memory for staging buffer.")
        };
        unsafe {
            std::ptr::copy_nonoverlapping(
                image.as_ptr() as *const std::ffi::c_void,
                mapped,
                (width * height * 4) as usize,
            );
            device.unmap_memory(staging_buffer.device_memory);
        }
        texture.copy_buffer_to_image(
            device,
            staging_buffer.buffer,
            command_pool,
            graphics_queue,
            width,
            height,
        );
        texture.create_image_view(device);

        unsafe {
            device.free_memory(staging_buffer.device_memory, None);
            device.destroy_buffer(staging_buffer.buffer, None);
        }

        let sampler_info = SamplerCreateInfo::builder()
            .unnormalized_coordinates(false)
            .mipmap_mode(SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .min_filter(Filter::LINEAR)
            .max_lod(0.0)
            .max_anisotropy(1.0)
            .mag_filter(Filter::LINEAR)
            .compare_op(CompareOp::ALWAYS)
            .compare_enable(false)
            .border_color(BorderColor::FLOAT_OPAQUE_WHITE)
            .anisotropy_enable(false)
            .address_mode_u(SamplerAddressMode::REPEAT)
            .address_mode_v(SamplerAddressMode::REPEAT)
            .address_mode_w(SamplerAddressMode::REPEAT);

        let sampler = unsafe {
            device
                .create_sampler(&sampler_info, None)
                .expect("Failed to create sampler for Nuklear texture.")
        };
        (texture, sampler)
    }
}

impl Drop for Drawer {
    fn drop(&mut self) {
        let device = &self.logical_device;
        let fences = [self.command_finished];
        unsafe {
            device
                .wait_for_fences(&fences[0..], true, u64::MAX)
                .expect("Failed to wait for fences for Nuklear.");
            device.destroy_semaphore(self.render_completed, None);
            device.destroy_fence(self.command_finished, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_render_pass(self.renderpass, None);
            device.destroy_command_pool(self.command_pool, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            device.destroy_sampler(self.font_sampler, None);
            device.free_memory(self.font_image.device_memory, None);
            device.destroy_image_view(self.font_image.image_view, None);
            device.destroy_image(self.font_image.image, None);
            device.free_memory(self.uniform_buffer.device_memory, None);
            device.destroy_buffer(self.uniform_buffer.buffer, None);
            device.free_memory(self.index_buffer.device_memory, None);
            device.destroy_buffer(self.index_buffer.buffer, None);
            device.free_memory(self.vertex_buffer.device_memory, None);
            device.destroy_buffer(self.vertex_buffer.buffer, None);
        }
    }
}
