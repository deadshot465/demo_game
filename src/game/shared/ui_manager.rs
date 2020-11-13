use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::Drawer;
use ash::vk::{CommandBuffer, Framebuffer, Semaphore, Viewport};
use nuklear::{
    AntiAliasing, Context, ConvertConfig, Flags, FontAtlas, FontID, LayoutFormat, PanelFlags,
    TextAlignment,
};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use winit::event::{ElementState, MouseScrollDelta, VirtualKeyCode};

const MAX_VERTEX_MEMORY: usize = 512 * 1024;
const MAX_INDEX_MEMORY: usize = 128 * 1024;
const MAX_COMMANDS_MEMORY: usize = 64 * 1024;
const RATIO_W: [f32; 2] = [0.15, 0.85];
const RATIO_WC: [f32; 3] = [0.15, 0.50, 0.35];
const MOUSE_SENSITIVITY: f64 = 22.0;

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
    drawer: ManuallyDrop<Drawer>,
    is_initialized: bool,
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

    pub fn input_button(
        &mut self,
        button: winit::event::MouseButton,
        x: f64,
        y: f64,
        element_state: ElementState,
    ) {
        use winit::event::MouseButton;
        self.context.input_button(
            match button {
                MouseButton::Right => nuklear::Button::Right,
                MouseButton::Left => nuklear::Button::Left,
                MouseButton::Middle => nuklear::Button::Middle,
                _ => nuklear::Button::Max,
            },
            x as i32,
            y as i32,
            element_state == ElementState::Pressed,
        );
    }

    pub fn input_key(&mut self, key: VirtualKeyCode, element_state: ElementState) {
        use nuklear::Key;
        self.context.input_key(
            match key {
                VirtualKeyCode::Up => Key::Up,
                VirtualKeyCode::Down => Key::Down,
                VirtualKeyCode::Left => Key::Left,
                VirtualKeyCode::Right => Key::Right,
                VirtualKeyCode::Delete => Key::Del,
                VirtualKeyCode::Back => Key::Backspace,
                _ => Key::None,
            },
            element_state == ElementState::Pressed,
        );
    }

    pub fn input_motion(&mut self, x: f64, y: f64) {
        self.context.input_motion(x as i32, y as i32);
    }

    pub fn input_scroll(&mut self, mouse_scroll_delta: MouseScrollDelta) {
        self.context.input_scroll(match mouse_scroll_delta {
            MouseScrollDelta::LineDelta(x, y) => {
                let altered_x = (x as f64) * MOUSE_SENSITIVITY;
                let altered_y = (y as f64) * MOUSE_SENSITIVITY;
                nuklear::Vec2 {
                    x: altered_x as f32,
                    y: altered_y as f32,
                }
            }
            MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition { x, y }) => nuklear::Vec2 {
                x: (x * MOUSE_SENSITIVITY) as f32,
                y: (y * MOUSE_SENSITIVITY) as f32,
            },
        });
    }

    pub fn input_unicode(&mut self, c: char) {
        self.context.input_unicode(c);
    }

    pub fn prerender(&mut self) {
        if !self.is_initialized {
            return;
        }
        let ctx = &mut self.context;
        let drawer = &mut self.drawer;
        drawer.set_font_size(ctx, 20);
        let flags =
            PanelFlags::Border as Flags | PanelFlags::Movable as Flags | PanelFlags::Title as Flags;
        ctx.begin(
            nuklear::nk_string!("Basic User Interface"),
            nuklear::Rect {
                x: 50.0,
                y: 50.0,
                w: 300.0,
                h: 300.0,
            },
            flags,
        );
        Self::set_ui_header(drawer, ctx, "Basic Image");
        Self::set_ui_widget(drawer, ctx, 100.0, true);
        ctx.image(nuklear::Image::with_id(0));
        drawer.set_font_size(ctx, 14);
        ctx.end();
    }

    pub fn set_disposing(&mut self) {
        self.is_initialized = false;
    }

    pub fn set_initialized(&mut self) {
        self.is_initialized = true;
    }

    pub fn start_input(&mut self) {
        self.context.input_begin();
    }

    pub fn wait_idle(&self) {
        self.drawer.wait_idle();
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
                graphics.logical_device.clone(),
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
            drawer: ManuallyDrop::new(drawer),
            is_initialized: true,
        }
    }

    pub fn render(
        &mut self,
        framebuffer: Framebuffer,
        viewport: Viewport,
        scale: nuklear::Vec2,
        wait_semaphore: Semaphore,
    ) -> Semaphore {
        if !self.is_initialized {
            return Semaphore::null();
        }
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

impl<GraphicsType, BufferType, CommandType, TextureType> Drop
    for UIManager<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn drop(&mut self) {
        unsafe {
            log::info!("Dropping UI manager...");
            ManuallyDrop::drop(&mut self.drawer);
            log::info!("Successfully dropped UI manager.");
        }
    }
}
