use egui::{CursorIcon, TextureId, Vec2};
use fltk::{
    app,
    enums::{self, Cursor},
    prelude::{FltkError, ImageExt, WidgetExt, WindowExt},
};

pub use egui;
use egui_wgpu_backend::{epi::RepaintSignal, wgpu, RenderPass, ScreenDescriptor};
pub use fltk;
use std::{iter, num::NonZeroU32, sync::Arc, time::Instant};
mod clipboard;
use clipboard::Clipboard;
/// Construct the frontend.
///
/// DpiScaling can be Default or Custom(f32)
pub fn begin_with(
    window: &mut fltk::window::Window,
    render_pass: RenderPass,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    scale: DpiScaling,
) -> (Painter, EguiState) {
    app::set_screen_scale(window.screen_num(), 1.0);
    let scale = match scale {
        DpiScaling::Default => window.pixels_per_unit(),
        DpiScaling::Custom(custom) => custom,
    };
    let x = window.width();
    let y = window.height();
    let rect = egui::vec2(x as f32, y as f32) / scale;
    let screen_rect = egui::Rect::from_min_size(egui::Pos2::new(0f32, 0f32), rect);

    let painter = Painter {
        render_pass,
        surface,
        surface_config,
    };
    let state = EguiState {
        _window_resized: false,
        fuse_cursor: FusedCursor::new(),
        pointer_pos: egui::Pos2::new(0f32, 0f32),
        input: egui::RawInput {
            screen_rect: Some(screen_rect),
            pixels_per_point: Some(scale),
            ..Default::default()
        },
        modifiers: egui::Modifiers::default(),
        physical_width: x as u32,
        physical_height: y as u32,
        pixels_per_point: scale,
        screen_rect,
        clipboard: clipboard::Clipboard::default(),
    };
    (painter, state)
}

pub struct Signal;

impl Default for Signal {
    fn default() -> Self {
        Self {}
    }
}

impl RepaintSignal for Signal {
    fn request_repaint(&self) {}
}

pub struct Painter {
    pub render_pass: RenderPass,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl Painter {
    pub fn paint_jobs(
        &mut self,
        device: &mut wgpu::Device,
        queue: &mut wgpu::Queue,
        state: &mut EguiState,
        clipped_mesh: Vec<egui::ClippedMesh>,
        texture: Arc<egui::Texture>,
    ) {
        // Upload all resources for the GPU.
        let screen_descriptor;
        {
            let surface_config = &mut self.surface_config;
            let width = state.physical_width;
            let height = state.physical_height;
            surface_config.width = width;
            surface_config.height = height;
            self.surface.configure(&device, surface_config);
            screen_descriptor = ScreenDescriptor {
                physical_width: width,
                physical_height: height,
                scale_factor: state.pixels_per_point,
            }
        };

        // Record all render passes.
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return;
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });
        let render_pass = &mut self.render_pass;
        render_pass.update_buffers(device, queue, &clipped_mesh, &screen_descriptor);
        render_pass.update_texture(&device, &queue, &texture);
        render_pass.update_user_textures(&device, &queue);
        render_pass
            .execute(
                &mut encoder,
                &output_view,
                &clipped_mesh,
                &screen_descriptor,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();
        // Submit the commands.
        queue.submit(iter::once(encoder.finish()));
        output_frame.present();
    }
}

/// Frame time for CPU usage.
pub fn get_frame_time(start_time: Instant) -> f32 {
    (Instant::now() - start_time).as_secs_f64() as f32
}

/// The scaling factors of the app
pub enum DpiScaling {
    /// Default DPI Scale by fltk, usually 1.0
    Default,
    /// Custome DPI scaling, e.g: 0.8, 1.5, 2.0 and so fort.
    Custom(f32),
}

/// The default cursor
pub struct FusedCursor {
    pub cursor_icon: Cursor,
}

const ARROW: enums::Cursor = enums::Cursor::Arrow;

impl FusedCursor {
    /// Construct a new cursor
    pub fn new() -> Self {
        Self { cursor_icon: ARROW }
    }
}

impl Default for FusedCursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Shuttles FLTK's input and events to Egui
pub struct EguiState {
    _window_resized: bool,
    pub fuse_cursor: FusedCursor,
    pub pointer_pos: egui::Pos2,
    pub input: egui::RawInput,
    pub modifiers: egui::Modifiers,
    pub physical_width: u32,
    pub physical_height: u32,
    pub pixels_per_point: f32,
    pub screen_rect: egui::Rect,
    pub clipboard: Clipboard,
}

impl EguiState {
    /// Conveniece method bundling the necessary components for input/event handling
    pub fn fuse_input(&mut self, win: &mut fltk::window::Window, event: enums::Event) {
        input_to_egui(win, event, self);
    }

    pub fn window_resized(&mut self) -> bool {
        let tmp = self._window_resized;
        self._window_resized = false;
        tmp
    }

    /// Convenience method for outputting what egui emits each frame
    pub fn fuse_output(&mut self, win: &mut fltk::window::Window, egui_output: &egui::Output) {
        let copied_text = &egui_output.copied_text;
        if !copied_text.is_empty() {
            self.clipboard.set(copied_text.into());
        }
        translate_cursor(win, &mut self.fuse_cursor, egui_output.cursor_icon);
    }

    /// Updates the screen rect
    pub fn update_screen_rect(&mut self, x: i32, y: i32) {
        let rect = egui::vec2(x as f32, y as f32) / self.pixels_per_point;
        self.screen_rect = egui::Rect::from_min_size(Default::default(), rect);
    }

    pub fn update_screen_rect_size(&mut self, size: egui::Vec2) {
        self.screen_rect =
            egui::Rect::from_min_size(Default::default(), size * self.pixels_per_point);
    }
}

/// Handles input/events from FLTK
pub fn input_to_egui(win: &mut fltk::window::Window, event: enums::Event, state: &mut EguiState) {
    let (x, y) = app::event_coords();
    let pixels_per_point = state.pixels_per_point;
    match event {
        enums::Event::Resize => {
            let width = win.width();
            let height = win.height();
            state.physical_width = width as u32;
            state.physical_height = height as u32;
            state.update_screen_rect(width, height);
            state.input.screen_rect = Some(state.screen_rect);
            state._window_resized = true;
        }
        //MouseButonLeft pressed is the only one needed by egui
        enums::Event::Push => {
            let mouse_btn = match app::event_mouse_button() {
                app::MouseButton::Left => Some(egui::PointerButton::Primary),
                app::MouseButton::Middle => Some(egui::PointerButton::Middle),
                app::MouseButton::Right => Some(egui::PointerButton::Secondary),
                _ => None,
            };
            if let Some(pressed) = mouse_btn {
                state.input.events.push(egui::Event::PointerButton {
                    pos: state.pointer_pos,
                    button: pressed,
                    pressed: true,
                    modifiers: state.modifiers,
                });
            }
        }

        //MouseButonLeft pressed is the only one needed by egui
        enums::Event::Released => {
            // fix unreachable, we can use Option.
            let mouse_btn = match app::event_mouse_button() {
                app::MouseButton::Left => Some(egui::PointerButton::Primary),
                app::MouseButton::Middle => Some(egui::PointerButton::Middle),
                app::MouseButton::Right => Some(egui::PointerButton::Secondary),
                _ => None,
            };
            if let Some(released) = mouse_btn {
                state.input.events.push(egui::Event::PointerButton {
                    pos: state.pointer_pos,
                    button: released,
                    pressed: false,
                    modifiers: state.modifiers,
                });
            }
        }

        enums::Event::Move | enums::Event::Drag => {
            state.pointer_pos =
                egui::pos2(x as f32 / pixels_per_point, y as f32 / pixels_per_point);
            state
                .input
                .events
                .push(egui::Event::PointerMoved(state.pointer_pos));
        }

        enums::Event::KeyUp => {
            if let Some(key) = translate_virtual_key_code(app::event_key()) {
                let keymod = app::event_state();
                state.modifiers = egui::Modifiers {
                    alt: (keymod & enums::EventState::Alt == enums::EventState::Alt),
                    ctrl: (keymod & enums::EventState::Ctrl == enums::EventState::Ctrl),
                    shift: (keymod & enums::EventState::Shift == enums::EventState::Shift),
                    mac_cmd: keymod & enums::EventState::Meta == enums::EventState::Meta,

                    //TOD: Test on both windows and mac
                    command: (keymod & enums::EventState::Command == enums::EventState::Command),
                };
                state.input.events.push(egui::Event::Key {
                    key,
                    pressed: false,
                    modifiers: state.modifiers,
                });
            }
        }

        enums::Event::KeyDown => {
            if let Some(c) = app::event_text().chars().next() {
                if let Some(del) = app::compose() {
                    state.input.events.push(egui::Event::Text(c.to_string()));
                    if del != 0 {
                        app::compose_reset();
                    }
                }
            }
            if let Some(key) = translate_virtual_key_code(app::event_key()) {
                let keymod = app::event_state();
                state.modifiers = egui::Modifiers {
                    alt: (keymod & enums::EventState::Alt == enums::EventState::Alt),
                    ctrl: (keymod & enums::EventState::Ctrl == enums::EventState::Ctrl),
                    shift: (keymod & enums::EventState::Shift == enums::EventState::Shift),
                    mac_cmd: keymod & enums::EventState::Meta == enums::EventState::Meta,

                    //TOD: Test on both windows and mac
                    command: (keymod & enums::EventState::Command == enums::EventState::Command),
                };

                if state.modifiers.command && key == egui::Key::C {
                    // println!("copy event");
                    state.input.events.push(egui::Event::Copy);
                } else if state.modifiers.command && key == egui::Key::X {
                    // println!("cut event");
                    state.input.events.push(egui::Event::Cut);
                } else if state.modifiers.command && key == egui::Key::V {
                    if let Some(value) = state.clipboard.get() {
                        state.input.events.push(egui::Event::Text(value));
                    }
                } else {
                    state.input.events.push(egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers: state.modifiers,
                    });
                }
            }
        }

        enums::Event::MouseWheel => {
            if app::is_event_command() {
                let zoom_factor = 1.2;
                match app::event_dy() {
                    app::MouseWheel::Up => {
                        state.input.zoom_delta /= zoom_factor;
                    }
                    app::MouseWheel::Down => {
                        state.input.zoom_delta *= zoom_factor;
                    }
                    _ => (),
                }
            } else {
                let scroll_factor = 15.0;
                match app::event_dy() {
                    app::MouseWheel::Up => {
                        state.input.scroll_delta.y -= scroll_factor;
                    }
                    app::MouseWheel::Down => {
                        state.input.scroll_delta.y += scroll_factor;
                    }
                    _ => (),
                }
            }
        }

        _ => {
            //dbg!(event);
        }
    }
}

/// Translates key codes
pub fn translate_virtual_key_code(key: enums::Key) -> Option<egui::Key> {
    match key {
        enums::Key::Left => Some(egui::Key::ArrowLeft),
        enums::Key::Up => Some(egui::Key::ArrowUp),
        enums::Key::Right => Some(egui::Key::ArrowRight),
        enums::Key::Down => Some(egui::Key::ArrowDown),
        enums::Key::Escape => Some(egui::Key::Escape),
        enums::Key::Tab => Some(egui::Key::Tab),
        enums::Key::BackSpace => Some(egui::Key::Backspace),
        enums::Key::Insert => Some(egui::Key::Insert),
        enums::Key::Home => Some(egui::Key::Home),
        enums::Key::Delete => Some(egui::Key::Delete),
        enums::Key::End => Some(egui::Key::End),
        enums::Key::PageDown => Some(egui::Key::PageDown),
        enums::Key::PageUp => Some(egui::Key::PageUp),
        enums::Key::Enter => Some(egui::Key::Enter),
        _ => {
            if let Some(k) = key.to_char() {
                match k {
                    ' ' => Some(egui::Key::Space),
                    'a' => Some(egui::Key::A),
                    'b' => Some(egui::Key::B),
                    'c' => Some(egui::Key::C),
                    'd' => Some(egui::Key::D),
                    'e' => Some(egui::Key::E),
                    'f' => Some(egui::Key::F),
                    'g' => Some(egui::Key::G),
                    'h' => Some(egui::Key::H),
                    'i' => Some(egui::Key::I),
                    'j' => Some(egui::Key::J),
                    'k' => Some(egui::Key::K),
                    'l' => Some(egui::Key::L),
                    'm' => Some(egui::Key::M),
                    'n' => Some(egui::Key::N),
                    'o' => Some(egui::Key::O),
                    'p' => Some(egui::Key::P),
                    'q' => Some(egui::Key::Q),
                    'r' => Some(egui::Key::R),
                    's' => Some(egui::Key::S),
                    't' => Some(egui::Key::T),
                    'u' => Some(egui::Key::U),
                    'v' => Some(egui::Key::V),
                    'w' => Some(egui::Key::W),
                    'x' => Some(egui::Key::X),
                    'y' => Some(egui::Key::Y),
                    'z' => Some(egui::Key::Z),
                    '0' => Some(egui::Key::Num0),
                    '1' => Some(egui::Key::Num1),
                    '2' => Some(egui::Key::Num2),
                    '3' => Some(egui::Key::Num3),
                    '4' => Some(egui::Key::Num4),
                    '5' => Some(egui::Key::Num5),
                    '6' => Some(egui::Key::Num6),
                    '7' => Some(egui::Key::Num7),
                    '8' => Some(egui::Key::Num8),
                    '9' => Some(egui::Key::Num9),
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

/// Translates FLTK cursor to Egui cursors
pub fn translate_cursor(
    win: &mut fltk::window::Window,
    fused: &mut FusedCursor,
    cursor_icon: egui::CursorIcon,
) {
    let tmp_icon = match cursor_icon {
        CursorIcon::None => enums::Cursor::None,
        CursorIcon::Default => enums::Cursor::Arrow,
        CursorIcon::Help => enums::Cursor::Help,
        CursorIcon::PointingHand => enums::Cursor::Hand,
        CursorIcon::ResizeHorizontal => enums::Cursor::WE,
        CursorIcon::ResizeNeSw => enums::Cursor::NESW,
        CursorIcon::ResizeNwSe => enums::Cursor::NWSE,
        CursorIcon::ResizeVertical => enums::Cursor::NS,
        CursorIcon::Text => enums::Cursor::Insert,
        CursorIcon::Crosshair => enums::Cursor::Cross,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => enums::Cursor::Wait,
        CursorIcon::Wait => enums::Cursor::Wait,
        CursorIcon::Progress => enums::Cursor::Wait,
        CursorIcon::Grab => enums::Cursor::Hand,
        CursorIcon::Grabbing => enums::Cursor::Move,
        CursorIcon::Move => enums::Cursor::Move,

        _ => enums::Cursor::Arrow,
    };

    if tmp_icon != fused.cursor_icon {
        fused.cursor_icon = tmp_icon;
        win.set_cursor(tmp_icon);
    }
}

pub trait ImgWidgetConvert {
    /// Convert fltk Image to Egui Image Widget.
    ///
    /// label: Debug label of the texture. This will show up in graphics debuggers for easy identification.
    fn to_img_widget(
        self,
        painter: &mut Painter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<ImgWidget, FltkError>;
}

impl<I> ImgWidgetConvert for I
where
    I: ImageExt,
{
    fn to_img_widget(
        self,
        painter: &mut Painter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<ImgWidget, FltkError> {
        let (width, height) = (self.data_w() as usize, self.data_h() as usize);
        let render_pass = &mut painter.render_pass;
        let texture_id;
        {
            let size = wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            };

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(format!("{}_texture", label).as_str()),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            });
            texture_id = render_pass.egui_texture_from_wgpu_texture(
                device,
                &texture,
                wgpu::FilterMode::Linear,
            );
            let pixels = self
                .to_rgb()?
                .convert(enums::ColorDepth::Rgba8)?
                .to_rgb_data();
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels.as_slice(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new((pixels.len() / height) as u32),
                    rows_per_image: NonZeroU32::new(height as u32),
                },
                size,
            );
        }
        let image = ImgWidget::new(texture_id, egui::vec2(width as f32, height as f32));
        Ok(image)
    }
}

/// Egui Image Widget
pub struct ImgWidget {
    _texture_id: egui::TextureId,
    _size: Vec2,
}

impl Drop for ImgWidget {
    fn drop(&mut self) {}
}

impl ImgWidget {
    pub fn texture_id(&self) -> TextureId {
        self._texture_id
    }

    pub fn size(&self) -> Vec2 {
        self._size
    }

    pub fn widget(&self) -> egui::Image {
        egui::Image::new(self._texture_id, self._size)
    }

    pub fn resize(&mut self, x: f32, y: f32) {
        self._size.x = x;
        self._size.y = y;
    }

    pub fn set_size_x(&mut self, x: f32) {
        self._size.x = x;
    }

    pub fn set_size_y(&mut self, y: f32) {
        self._size.y = y;
    }

    pub fn get_size_x(&self) -> f32 {
        self._size.x
    }

    pub fn get_size_y(&self) -> f32 {
        self._size.y
    }

    pub fn new(texture_id: TextureId, size: Vec2) -> Self {
        Self {
            _texture_id: texture_id,
            _size: size,
        }
    }

    /// label: Debug label of the texture. This will show up in graphics debuggers for easy identification.
    pub fn from_rgba8(
        pixels: &[u8],
        width: u32,
        height: u32,
        painter: &mut Painter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<Self, FltkError> {
        let render_pass = &mut painter.render_pass;
        let texture_id;
        {
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(format!("{}_texture", label).as_str()),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            });
            texture_id = render_pass.egui_texture_from_wgpu_texture(
                device,
                &texture,
                wgpu::FilterMode::Linear,
            );
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new((pixels.len() as u32 / height) as u32),
                    rows_per_image: NonZeroU32::new(height as u32),
                },
                size,
            );
        }
        let image = ImgWidget::new(texture_id, egui::vec2(width as f32, height as f32));
        Ok(image)
    }
}

pub struct Timer {
    timer: u32,
    elapse: u32,
    duration: f32,
}

impl Timer {
    /// Elapse every, approximately in second(s).
    pub fn new(elapse: u32) -> Self {
		let _elapse = elapse * 180;
        let duration = _elapse as f32 / 1000.0;
        Self {
            timer: 0,
            elapse: elapse * 5,
            duration,
        }
    }

    /// Check if the timer is elapsed.
    pub fn elapsed(&mut self) -> bool {
        if self.timer >= self.elapse {
            self.timer = 0;
            return true;
        }
        self.timer += 1;
        app::sleep(self.duration.into());
        false
    }
}
