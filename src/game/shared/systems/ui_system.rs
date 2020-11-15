use crate::game::graphics::vk::{Buffer, Graphics, Image};
use crate::game::traits::{Disposable, GraphicsBase};
use crate::game::{Drawer, NetworkSystem};
use ash::vk::{CommandBuffer, Framebuffer, Semaphore, Viewport};
use nuklear::{
    AntiAliasing, Context, ConvertConfig, EditType, Flags, FontAtlas, FontID, LayoutFormat,
    PanelFlags, TextAlignment, TextEdit,
};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use winit::event::{ElementState, MouseScrollDelta, VirtualKeyCode};

const MAX_VERTEX_MEMORY: usize = 512 * 1024;
const MAX_INDEX_MEMORY: usize = 128 * 1024;
const MAX_COMMANDS_MEMORY: usize = 64 * 1024;
const RATIO_W: [f32; 2] = [0.15, 0.85];
const RATIO_WC: [f32; 3] = [0.15, 0.7, 0.15];
const MOUSE_SENSITIVITY: f64 = 22.0;

struct Media {
    font_14: FontID,
    font_18: FontID,
    font_20: FontID,
    font_22: FontID,
    font_atlas: FontAtlas,
    font_tex: nuklear::Handle,
}

#[derive(Copy, Clone, Debug)]
pub struct ButtonState {
    pub game_started: bool,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonState {
    pub fn new() -> Self {
        ButtonState {
            game_started: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TitleSceneInputs {
    pub username_input: [u8; 64],
    pub username_length: i32,
    pub nickname_input: [u8; 64],
    pub nickname_length: i32,
    pub email_input: [u8; 64],
    pub email_length: i32,
    pub password_input: [u8; 64],
    pub password_length: i32,
}

impl TitleSceneInputs {
    pub fn new() -> Self {
        TitleSceneInputs {
            username_input: [0; 64],
            username_length: 0,
            nickname_input: [0; 64],
            nickname_length: 0,
            email_input: [0; 64],
            email_length: 0,
            password_input: [0; 64],
            password_length: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UIState {
    pub show_login_box: bool,
    pub show_register_box: bool,
    pub show_login_form: bool,
    pub title_scene_inputs: TitleSceneInputs,
}

impl Default for UIState {
    fn default() -> Self {
        Self::new()
    }
}

impl UIState {
    pub fn new() -> Self {
        UIState {
            show_login_box: false,
            show_register_box: false,
            show_login_form: false,
            title_scene_inputs: TitleSceneInputs::new(),
        }
    }
}

pub struct UISystem<GraphicsType, BufferType, CommandType, TextureType>
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
    button_state: ButtonState,
    ui_state: UIState,
}

impl<GraphicsType, BufferType, CommandType, TextureType>
    UISystem<GraphicsType, BufferType, CommandType, TextureType>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    pub fn clear(&mut self) {
        self.context.clear();
    }

    pub async fn draw_title_ui(
        &mut self,
        network_system: &mut NetworkSystem,
    ) -> anyhow::Result<ButtonState> {
        if !self.is_initialized {
            return Ok(self.button_state);
        }
        let ctx = &mut self.context;
        let drawer = &mut self.drawer;
        drawer.set_font_size(ctx, 24);
        let flags = PanelFlags::Border as Flags | PanelFlags::NoScrollbar as Flags;
        ctx.begin(
            nuklear::nk_string!("Basic User Interface"),
            nuklear::Rect {
                x: 0.0,
                y: 0.0,
                w: 300.0,
                h: 900.0,
            },
            flags,
        );
        Self::set_ui_header(drawer, ctx, "Game Menu", TextAlignment::Centered);
        Self::set_ui_widget(drawer, ctx, 50.0, true);
        if ctx.button_text("Start")
            && !self.ui_state.show_login_box
            && !self.ui_state.show_register_box
            && !self.ui_state.show_login_form
            && !network_system.is_player_login
        {
            self.ui_state.show_login_box = true;
        }
        drawer.set_font_size(ctx, 24);
        ctx.end();

        if self.ui_state.show_login_box {
            self.draw_login_box(flags);
        }

        if self.ui_state.show_register_box {
            self.draw_register_box(flags, network_system).await?;
        }

        Ok(self.button_state)
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

    pub fn set_disposing(&mut self) {
        self.is_initialized = false;
    }

    pub fn set_initialized(&mut self) {
        self.is_initialized = true;
    }

    pub fn toggle_login_box(&mut self) {
        self.ui_state.show_login_box = !self.ui_state.show_login_box;
    }

    pub fn start_input(&mut self) {
        self.context.input_begin();
    }

    pub fn wait_idle(&self) {
        self.drawer.wait_idle();
    }

    fn draw_login_box(&mut self, flags: Flags) {
        let mut ui_state = self.ui_state.clone();
        {
            let ctx = &mut self.context;
            let drawer = &mut self.drawer;
            drawer.set_font_size(ctx, 28);
            ctx.begin(
                nuklear::nk_string!("Login"),
                nuklear::Rect {
                    x: 500.0,
                    y: 350.0,
                    w: 600.0,
                    h: 200.0,
                },
                flags,
            );
            Self::set_ui_header(drawer, ctx, "Login", TextAlignment::Centered);
            ctx.text_wrap("You haven't logged in. Please login or register first!");
            drawer.set_font_size(ctx, 16);
            let ratio = [0.03, 0.28, 0.03, 0.32, 0.03, 0.28, 0.03];
            ctx.layout_row(LayoutFormat::Dynamic, 50.0, &ratio[..]);
            ctx.spacing(1);
            if ctx.button_text("Login") {
                ui_state.show_login_box = false;
                ui_state.show_login_form = true;
            }
            ctx.spacing(1);
            if ctx.button_text("Register") {
                ui_state.show_login_box = false;
                ui_state.show_register_box = true;
            }
            ctx.spacing(1);
            if ctx.button_text("Cancel") {
                ui_state.show_login_box = false;
            }
            ctx.spacing(1);
            drawer.set_font_size(ctx, 24);
            ctx.end();
        }
        self.ui_state = ui_state;
    }

    async fn draw_register_box(
        &mut self,
        flags: Flags,
        network_system: &mut NetworkSystem,
    ) -> anyhow::Result<()> {
        let mut ui_state = self.ui_state.clone();
        {
            let ctx = &mut self.context;
            let drawer = &mut self.drawer;
            drawer.set_font_size(ctx, 28);
            ctx.begin(
                nuklear::nk_string!("Register"),
                nuklear::Rect {
                    x: 500.0,
                    y: 300.0,
                    w: 600.0,
                    h: 400.0,
                },
                flags,
            );
            //Self::set_ui_header(drawer, ctx, "Register", TextAlignment::Centered);
            drawer.set_font_size(ctx, 36);
            ctx.layout_row_dynamic(50.0, 1);
            ctx.text("Register", TextAlignment::Centered as Flags);
            drawer.set_font_size(ctx, 16);
            let ratio = [0.4, 0.6];
            ctx.layout_row(LayoutFormat::Dynamic, 50.0, &ratio[..]);
            ctx.text("Username: ", TextAlignment::Right as Flags);
            ctx.edit_string_custom_filter(
                EditType::Field as Flags,
                ui_state.title_scene_inputs.username_input.as_mut(),
                &mut ui_state.title_scene_inputs.username_length,
                Self::free_type_filter,
            );
            ctx.text("Nickname: ", TextAlignment::Right as Flags);
            ctx.edit_string_custom_filter(
                EditType::Field as Flags,
                ui_state.title_scene_inputs.nickname_input.as_mut(),
                &mut ui_state.title_scene_inputs.nickname_length,
                Self::free_type_filter,
            );
            ctx.text("Email: ", TextAlignment::Right as Flags);
            ctx.edit_string_custom_filter(
                EditType::Field as Flags,
                ui_state.title_scene_inputs.email_input.as_mut(),
                &mut ui_state.title_scene_inputs.email_length,
                Self::email_filter,
            );
            ctx.text("Password: ", TextAlignment::Right as Flags);
            ctx.edit_string_custom_filter(
                EditType::Field as Flags,
                ui_state.title_scene_inputs.password_input.as_mut(),
                &mut ui_state.title_scene_inputs.password_length,
                Self::free_type_filter,
            );
            ctx.layout_row_dynamic(50.0, 2);
            if ctx.button_text("Register") {
                let username = std::str::from_utf8(
                    &self.ui_state.title_scene_inputs.username_input
                        [0..(self.ui_state.title_scene_inputs.username_length as usize)],
                )?;
                let nickname = std::str::from_utf8(
                    &self.ui_state.title_scene_inputs.nickname_input
                        [0..(self.ui_state.title_scene_inputs.nickname_length as usize)],
                )?;
                let email = std::str::from_utf8(
                    &self.ui_state.title_scene_inputs.email_input
                        [0..(self.ui_state.title_scene_inputs.email_length as usize)],
                )?;
                let password = std::str::from_utf8(
                    &self.ui_state.title_scene_inputs.password_input
                        [0..(self.ui_state.title_scene_inputs.password_length as usize)],
                )?;
                let result = network_system
                    .register(username, nickname, email, password)
                    .await;
            }
            if ctx.button_text("Cancel") {
                ui_state.show_register_box = false;
            }
            drawer.set_font_size(ctx, 24);
            ctx.end();
        }
        self.ui_state = ui_state;
        Ok(())
    }

    fn free_type_filter(_: &TextEdit, c: char) -> bool {
        c >= '\u{0020}'
    }

    fn email_filter(_: &TextEdit, c: char) -> bool {
        c == '\u{002E}'
            || (c >= '\u{0030}' && c <= '\u{0039}')
            || (c >= '\u{0040}' && c <= '\u{005A}')
            || c == '\u{005F}'
            || (c >= '\u{0061}' && c <= '\u{007A}')
    }

    fn set_ui_header(
        drawer: &mut Drawer,
        ctx: &mut Context,
        title: &str,
        text_alignment: TextAlignment,
    ) {
        drawer.set_font_size(ctx, 36);
        ctx.layout_row_dynamic(50.0, 1);
        ctx.text(title, text_alignment as Flags);
    }

    fn set_ui_widget(drawer: &mut Drawer, ctx: &mut Context, height: f32, centered: bool) {
        drawer.set_font_size(ctx, 20);
        ctx.layout_row(
            LayoutFormat::Dynamic,
            height,
            if centered { &RATIO_WC } else { &RATIO_W },
        );
        ctx.spacing(1);
    }
}

impl UISystem<Graphics, Buffer, CommandBuffer, Image> {
    pub fn new(graphics: &Graphics) -> Self {
        let font_bytes = std::fs::read("resource/Comfortaa-Regular.ttf")
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

        let ctx = drawer.create_context(16);

        let mut convert_config = ConvertConfig::default();
        convert_config.set_null(drawer.draw_null_texture.clone());
        convert_config.set_circle_segment_count(22);
        convert_config.set_curve_segment_count(22);
        convert_config.set_arc_segment_count(22);
        convert_config.set_global_alpha(1.0);
        convert_config.set_shape_aa(AntiAliasing::On);
        convert_config.set_line_aa(AntiAliasing::On);

        UISystem {
            font_bytes,
            phantom_1: PhantomData,
            phantom_2: PhantomData,
            phantom_3: PhantomData,
            phantom_4: PhantomData,
            context: ctx,
            convert_config,
            drawer: ManuallyDrop::new(drawer),
            is_initialized: true,
            button_state: ButtonState::new(),
            ui_state: UIState::new(),
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
    for UISystem<GraphicsType, BufferType, CommandType, TextureType>
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
