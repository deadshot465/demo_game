use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::Drawer;
use ash::version::DeviceV1_0;
use ash::vk::{
    CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferUsageFlags,
    CommandPool, Framebuffer, Semaphore, Viewport,
};
use nuklear::{
    font_cyrillic_glyph_ranges, AntiAliasing, Context, ConvertConfig, DrawNullTexture, Flags,
    FontAtlas, FontAtlasFormat, FontConfig, FontID, LayoutFormat, PanelFlags, TextAlignment,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

const MAX_VERTEX_MEMORY: usize = 512 * 1024;
const MAX_INDEX_MEMORY: usize = 128 * 1024;
const MAX_COMMANDS_MEMORY: usize = 64 * 1024;
const RATIO_W: [f32; 2] = [0.15, 0.85];
const RATIO_WC: [f32; 3] = [0.15, 0.50, 0.35];

struct Media {
    font_14: FontID,
    font_18: FontID,
    font_20: FontID,
    font_22: FontID,
    font_atlas: FontAtlas,
    font_tex: nuklear::Handle,
}

pub struct UIManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    font_bytes: Vec<u8>,
    phantom_1: PhantomData<&'static GraphicsType>,
    phantom_2: PhantomData<&'static BufferType>,
    phantom_3: PhantomData<&'static CommandType>,
    phantom_4: PhantomData<&'static TextureType>,
    context: Context,
    convert_config: ConvertConfig,
    drawer: Drawer,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    UIManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub fn clear(&mut self) {
        self.context.clear();
    }

    pub fn end_input(&mut self) {
        self.context.input_end();
    }

    pub fn start_input(&mut self) {
        self.context.input_begin();
    }

    pub fn prerender(&mut self) {
        let ctx = &mut self.context;
        let drawer = &mut self.drawer;
        drawer.set_font_size(ctx, 20);
        let flags =
            PanelFlags::Border as Flags | PanelFlags::Movable as Flags | PanelFlags::Title as Flags;
        ctx.begin(
            nuklear::nk_string!("Basic User Interface"),
            nuklear::Rect {
                x: 320.0,
                y: 50.0,
                w: 275.0,
                h: 610.0,
            },
            flags,
        );
        Self::set_ui_header(drawer, ctx, "Basic Image");
        Self::set_ui_widget(drawer, ctx, 35.0, false);
        ctx.button_text("Push me.");
        drawer.set_font_size(ctx, 14);
        ctx.end();
    }

    fn set_ui_header(drawer: &mut Drawer, ctx: &mut Context, title: &str) {
        drawer.set_font_size(ctx, 18);
        ctx.layout_row_dynamic(20.0, 1);
        ctx.text(title, TextAlignment::Left as Flags);
    }

    fn set_ui_widget(drawer: &mut Drawer, ctx: &mut Context, height: f32, centered: bool) {
        drawer.set_font_size(ctx, 22);
        ctx.layout_row(
            LayoutFormat::Dynamic,
            height,
            if centered { &RATIO_WC } else { &RATIO_W },
        );
        ctx.spacing(1);
    }
}

impl UIManager<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(graphics: &Graphics) -> Self {
        let font_bytes = std::fs::read("resource/Roboto-Regular.ttf")
            .expect("Failed to read bytes from the font file.");

        let mut drawer = unsafe {
            Drawer::new(
                graphics.logical_device.as_ref().clone(),
                graphics.instance.clone(),
                graphics.physical_device.physical_device,
                *graphics.graphics_queue.lock(),
                graphics
                    .physical_device
                    .queue_indices
                    .graphics_family
                    .expect("Failed to get graphics queue family index."),
                graphics.swapchain.format.format,
                graphics.depth_format,
                graphics.sample_count,
                MAX_VERTEX_MEMORY as u64,
                MAX_INDEX_MEMORY as u64,
                MAX_COMMANDS_MEMORY,
                font_bytes.as_slice(),
            )
        };

        let ctx = drawer.create_context(14);

        let mut convert_config = ConvertConfig::default();
        convert_config.set_null(drawer.draw_null_texture.clone());
        convert_config.set_circle_segment_count(22);
        convert_config.set_curve_segment_count(22);
        convert_config.set_arc_segment_count(22);
        convert_config.set_global_alpha(1.0);
        convert_config.set_shape_aa(AntiAliasing::On);
        convert_config.set_line_aa(AntiAliasing::On);

        UIManager {
            font_bytes,
            phantom_1: PhantomData,
            phantom_2: PhantomData,
            phantom_3: PhantomData,
            phantom_4: PhantomData,
            context: ctx,
            convert_config,
            drawer,
        }
    }

    pub fn render(
        &mut self,
        framebuffer: Framebuffer,
        viewport: Viewport,
        scale: nuklear::Vec2,
        wait_semaphore: Semaphore,
    ) -> Semaphore {
        let context = &mut self.context;
        let convert_config = &mut self.convert_config;
        self.drawer.draw(
            framebuffer,
            viewport,
            scale,
            context,
            convert_config,
            wait_semaphore,
        )
    }
}
