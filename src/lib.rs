pub use egui;
use egui::{pos2, vec2, CursorIcon, Event, Rect, Vec2};
pub use egui_image::RetainedEguiImage;
mod backend;
pub use backend::{RenderPass, ScreenDescriptor};
pub use fltk;
use fltk::{
    app,
    enums::{self, Cursor},
    prelude::{FltkError, ImageExt, WindowExt},
    window::SingleWindow,
};
pub use pollster;
use std::{iter, time::Instant};
pub use wgpu;
mod clipboard;
mod egui_image;
use clipboard::Clipboard;

#[cfg(feature = "fltk-enable-glwindow")]
use fltk::window::GlWindow;

/// Pixel per unit trait helper.
pub trait PPU {
    fn pixels_per_unit(&self) -> f32;
}

#[cfg(feature = "fltk-enable-glwindow")]
impl PPU for GlWindow {
    fn pixels_per_unit(&self) -> f32 {
        self.pixels_per_unit()
    }
}

impl PPU for SingleWindow {
    fn pixels_per_unit(&self) -> f32 {
        self.pixels_per_unit()
    }
}

impl PPU for fltk::window::Window {
    fn pixels_per_unit(&self) -> f32 {
        self.pixels_per_unit()
    }
}

/// Construct the frontend.
pub fn begin_with<W>(
    window: &mut W,
    render_pass: RenderPass,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
) -> (Painter, EguiState)
where
    W: WindowExt + PPU,
{
    app::set_screen_scale(window.screen_num(), 1.0);
    let ppu = window.pixels_per_unit();
    let x = window.width();
    let y = window.height();
    let rect = egui::vec2(x as _, y as _) / ppu;
    let screen_rect = egui::Rect::from_min_size(egui::Pos2::new(0f32, 0f32), rect);

    let painter = Painter {
        render_pass,
        surface,
        surface_config,
    };

    let state = EguiState {
        _window_resized: false,
        fuse_cursor: FusedCursor::new(),
        pointer_pos: egui::Pos2::new(0.0, 0.0),
        input: egui::RawInput {
            screen_rect: Some(screen_rect),
            pixels_per_point: Some(ppu),
            ..Default::default()
        },
        physical_width: x as _,
        physical_height: y as _,
        _pixels_per_point: ppu,
        clipboard: clipboard::Clipboard::default(),
        _mouse_btn_pressed: false,
        scroll_factor: 12.0,
        zoom_factor: 8.0,
    };
    (painter, state)
}

pub struct Painter {
    pub render_pass: RenderPass,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl Painter {
    /// Get the calculated WGPU ScreenDescriptor.
    pub fn get_screen_descriptor(
        &mut self,
        device: &wgpu::Device,
        state: &EguiState,
    ) -> ScreenDescriptor {
        self.surface_config.width = state.physical_width;
        self.surface_config.height = state.physical_height;
        self.surface.configure(device, &self.surface_config);
        ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: state.pixels_per_point(),
        }
    }

    /// Paint with egui renderpass
    pub fn paint_with_rpass<'rpass>(
        &'rpass mut self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &ScreenDescriptor,
        clipped_primitive: Vec<egui::ClippedPrimitive>,
        texture: egui::TexturesDelta,
    ) {
        if texture.free.len() > 0 {
            texture.free.into_iter().for_each(|id| {
                self.render_pass.free_texture(&id);
            });
        }

        for (id, img_del) in texture.set {
            self.render_pass.update_texture(device, queue, id, &img_del);
        }

        self.render_pass
            .update_buffers(device, queue, &clipped_primitive, screen_descriptor);
        self.render_pass
            .execute_with_renderpass(rpass, &clipped_primitive, screen_descriptor);
    }

    pub fn paint_jobs(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        state: &EguiState,
        clipped_primitive: Vec<egui::ClippedPrimitive>,
        texture: egui::TexturesDelta,
    ) {
        // Upload all resources for the GPU.
        let screen_descriptor = {
            self.surface_config.width = state.physical_width;
            self.surface_config.height = state.physical_height;
            self.surface.configure(device, &self.surface_config);
            ScreenDescriptor {
                size_in_pixels: [self.surface_config.width, self.surface_config.height],
                pixels_per_point: state.pixels_per_point(),
            }
        };

        // Record all render passes.
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

                if texture.free.len() > 0 {
                    texture.free.into_iter().for_each(|id| {
                        self.render_pass.free_texture(&id);
                    });
                }

                for (id, img_del) in texture.set {
                    self.render_pass.update_texture(device, queue, id, &img_del);
                }

                self.render_pass.update_buffers(
                    device,
                    queue,
                    &clipped_primitive,
                    &screen_descriptor,
                );

                self.render_pass.execute(
                    &mut encoder,
                    &frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                    &clipped_primitive,
                    &screen_descriptor,
                    Some(wgpu::Color::BLACK),
                );

                // Submit command buffer
                let cm_buffer = encoder.finish();
                queue.submit(iter::once(cm_buffer));
                frame
            }
            Err(e) => return eprintln!("Dropped frame with error: {}", e),
        };

        // Draw finalize frame
        output_frame.present();
    }
}

/// Frame time for CPU usage.
pub fn get_frame_time(start_time: Instant) -> f32 {
    (Instant::now() - start_time).as_secs_f64() as _
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
    input: egui::RawInput,
    pub physical_width: u32,
    pub physical_height: u32,
    pub _pixels_per_point: f32,
    pub clipboard: Clipboard,
    /// default value is 12.0
    pub scroll_factor: f32,
    /// default value is 8.0
    pub zoom_factor: f32,
    _mouse_btn_pressed: bool,
}

impl EguiState {
    /// Conveniece method bundling the necessary components for input/event handling
    pub fn fuse_input<W>(&mut self, win: &mut W, event: enums::Event)
    where
        W: WindowExt,
    {
        input_to_egui(win, event, self);
    }

    pub fn window_resized(&mut self) -> bool {
        let tmp = self._window_resized;
        self._window_resized = false;
        tmp
    }

    pub fn mouse_btn_pressed(&self) -> bool {
        self._mouse_btn_pressed
    }

    /// Convenience method for outputting what egui emits each frame
    pub fn fuse_output<W>(&mut self, win: &mut W, egui_output: egui::PlatformOutput)
    where
        W: WindowExt,
    {
        if win.damage() {
            win.clear_damage();
        }

        let copied_text = &egui_output.copied_text;
        if !copied_text.is_empty() {
            self.clipboard.set(copied_text.into());
        }
        translate_cursor(win, &mut self.fuse_cursor, egui_output.cursor_icon);
    }

    /// Set visual scale, e.g: 0.8, 1.5, 2.0 .etc (default is 1.0)
    pub fn set_visual_scale(&mut self, size: f32) {
        // have to be setted the pixels_per_point of both the inner (input) and the state.
        self.input.pixels_per_point = Some(size);
        self._pixels_per_point = size;

        // resize rect with physical dimention size.
        let rect = vec2(self.physical_width as _, self.physical_height as _) / size;
        self.input.screen_rect = Some(Rect::from_min_size(Default::default(), rect));
    }

    pub fn pixels_per_point(&self) -> f32 {
        self._pixels_per_point
    }

    pub fn take_input(&mut self) -> egui::RawInput {
        let pixels_per_point = self.input.pixels_per_point;
        let take = self.input.take();
        self.input.pixels_per_point = pixels_per_point;
        if let Some(ppp) = pixels_per_point {
            self._pixels_per_point = ppp;
        }
        take
    }

    /// Set start time for egui timer related activity.
    pub fn start_time(&mut self, elapsed: f64) {
        self.input.time = Some(elapsed);
    }
}

/// Handles input/events from FLTK
pub fn input_to_egui<W>(win: &mut W, event: enums::Event, state: &mut EguiState)
where
    W: WindowExt,
{
    match event {
        enums::Event::Resize => {
            state.physical_width = win.width() as _;
            state.physical_height = win.height() as _;
            state.set_visual_scale(state.pixels_per_point());
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
                state._mouse_btn_pressed = true;
                state.input.events.push(egui::Event::PointerButton {
                    pos: state.pointer_pos,
                    button: pressed,
                    pressed: true,
                    modifiers: state.input.modifiers,
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
                state._mouse_btn_pressed = false;
                state.input.events.push(egui::Event::PointerButton {
                    pos: state.pointer_pos,
                    button: released,
                    pressed: false,
                    modifiers: state.input.modifiers,
                });
            }
        }

        enums::Event::Move | enums::Event::Drag => {
            let (x, y) = app::event_coords();
            let ppp = state.pixels_per_point();
            state.pointer_pos = pos2(x as f32 / ppp, y as f32 / ppp);
            state
                .input
                .events
                .push(egui::Event::PointerMoved(state.pointer_pos))
        }

        enums::Event::KeyUp => {
            if let Some(key) = translate_virtual_key_code(app::event_key()) {
                let keymod = app::event_state();
                state.input.modifiers = egui::Modifiers {
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
                    modifiers: state.input.modifiers,
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
                state.input.modifiers = egui::Modifiers {
                    alt: (keymod & enums::EventState::Alt == enums::EventState::Alt),
                    ctrl: (keymod & enums::EventState::Ctrl == enums::EventState::Ctrl),
                    shift: (keymod & enums::EventState::Shift == enums::EventState::Shift),
                    mac_cmd: keymod & enums::EventState::Meta == enums::EventState::Meta,

                    //TOD: Test on both windows and mac
                    command: (keymod & enums::EventState::Command == enums::EventState::Command),
                };
                state.input.events.push(egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers: state.input.modifiers,
                });
                if state.input.modifiers.command && key == egui::Key::C {
                    // println!("copy event");
                    state.input.events.push(egui::Event::Copy);
                } else if state.input.modifiers.command && key == egui::Key::X {
                    // println!("cut event");
                    state.input.events.push(egui::Event::Cut);
                } else if state.input.modifiers.command && key == egui::Key::V {
                    if let Some(value) = state.clipboard.get() {
                        state.input.events.push(egui::Event::Text(value));
                    }
                }
            }
        }

        enums::Event::MouseWheel => {
            if app::is_event_ctrl() {
                let zoom_factor = state.zoom_factor;
                match app::event_dy() {
                    app::MouseWheel::Up => {
                        let delta = vec2(1., -1.) * zoom_factor;

                        // Treat as zoom in:
                        state
                            .input
                            .events
                            .push(Event::Zoom((delta.y / 200.0).exp()));
                    }
                    app::MouseWheel::Down => {
                        let delta = vec2(-1., 1.) * zoom_factor;

                        // Treat as zoom out:
                        state
                            .input
                            .events
                            .push(Event::Zoom((delta.y / 200.0).exp()));
                    }
                    _ => (),
                }
            } else {
                let scroll_factor = state.scroll_factor;
                match app::event_dy() {
                    app::MouseWheel::Up => {
                        state.input.events.push(Event::Scroll(Vec2 {
                            x: 0.,
                            y: -scroll_factor,
                        }));
                    }
                    app::MouseWheel::Down => {
                        state.input.events.push(Event::Scroll(Vec2 {
                            x: 0.,
                            y: scroll_factor,
                        }));
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
pub fn translate_cursor<W>(win: &mut W, fused: &mut FusedCursor, cursor_icon: CursorIcon)
where
    W: WindowExt,
{
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

/// Compat for epi::App impl trait
pub struct Compat {
    setup: bool,
}

impl Default for Compat {
    fn default() -> Self {
        Self { setup: true }
    }
}

impl Compat {
    /// Called once before the first frame.
    pub fn needs_setup(&mut self) -> bool {
        if self.setup {
            self.setup = false;
            return true;
        }
        self.setup
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
            elapse: elapse * 6,
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

pub trait EguiImageConvertible<I>
where
    I: ImageExt,
{
    fn egui_image(self, debug_name: &str) -> Result<RetainedEguiImage, FltkError>;
}

impl<I> EguiImageConvertible<I> for I
where
    I: ImageExt,
{
    /// Return (egui_extras::RetainedEguiImage)
    fn egui_image(self, debug_name: &str) -> Result<RetainedEguiImage, FltkError> {
        let size = [self.data_w() as _, self.data_h() as _];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            size,
            &self
                .to_rgb()?
                .convert(enums::ColorDepth::Rgba8)?
                .to_rgb_data(),
        );

        Ok(RetainedEguiImage::from_color_image(debug_name, color_image))
    }
}

pub trait EguiSvgConvertible {
    fn egui_svg_image(self, debug_name: &str) -> Result<RetainedEguiImage, FltkError>;
}

impl EguiSvgConvertible for fltk::image::SvgImage {
    /// Return (egui_extras::RetainedEguiImage)
    fn egui_svg_image(mut self, debug_name: &str) -> Result<RetainedEguiImage, FltkError> {
        self.normalize();
        let size = [self.data_w() as usize, self.data_h() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            size,
            &self
                .to_rgb()?
                .convert(enums::ColorDepth::Rgba8)?
                .to_rgb_data(),
        );

        Ok(RetainedEguiImage::from_color_image(debug_name, color_image))
    }
}
/// egui::ColorImage Extender.
pub trait ColorImageExt {
    fn from_vec_color32(size: [usize; 2], vec: Vec<egui::Color32>) -> Self;

    fn from_color32_slice(size: [usize; 2], slice: &[egui::Color32]) -> Self;
}

impl ColorImageExt for egui::ColorImage {
    fn from_vec_color32(size: [usize; 2], vec: Vec<egui::Color32>) -> Self {
        let mut pixels: Vec<u8> = Vec::with_capacity(vec.len() * 4);
        vec.into_iter().for_each(|x| {
            pixels.push(x[0]);
            pixels.push(x[1]);
            pixels.push(x[2]);
            pixels.push(x[3]);
        });
        egui::ColorImage::from_rgba_unmultiplied(size, &pixels)
    }

    fn from_color32_slice(size: [usize; 2], slice: &[egui::Color32]) -> Self {
        let mut pixels: Vec<u8> = Vec::with_capacity(slice.len() * 4);
        slice.into_iter().for_each(|x| {
            pixels.push(x[0]);
            pixels.push(x[1]);
            pixels.push(x[2]);
            pixels.push(x[3]);
        });
        egui::ColorImage::from_rgba_unmultiplied(size, &pixels)
    }
}

/// egui::TextureHandle Extender.
pub trait TextureHandleExt {
    /// egui::TextureHandle from Vec u8
    fn from_vec_u8(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        vec: Vec<u8>,
    ) -> egui::TextureHandle;

    fn from_u8_slice(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        slice: &[u8],
    ) -> egui::TextureHandle;

    fn from_vec_color32(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        vec: Vec<egui::Color32>,
    ) -> egui::TextureHandle;

    fn from_color32_slice(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        slice: &[egui::Color32],
    ) -> egui::TextureHandle;
}

impl TextureHandleExt for egui::TextureHandle {
    fn from_vec_u8(ctx: &egui::Context, debug_name: &str, size: [usize; 2], vec: Vec<u8>) -> Self {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &vec);
        drop(vec);
        ctx.load_texture(debug_name, color_image)
    }

    fn from_u8_slice(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        slice: &[u8],
    ) -> Self {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, slice);
        ctx.load_texture(debug_name, color_image)
    }

    fn from_vec_color32(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        vec: Vec<egui::Color32>,
    ) -> Self {
        let mut pixels: Vec<u8> = Vec::with_capacity(vec.len() * 4);
        vec.into_iter().for_each(|x| {
            pixels.push(x[0]);
            pixels.push(x[1]);
            pixels.push(x[2]);
            pixels.push(x[3]);
        });
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
        ctx.load_texture(debug_name, color_image)
    }

    fn from_color32_slice(
        ctx: &egui::Context,
        debug_name: &str,
        size: [usize; 2],
        slice: &[egui::Color32],
    ) -> Self {
        let mut pixels: Vec<u8> = Vec::with_capacity(slice.len() * 4);
        slice.into_iter().for_each(|x| {
            pixels.push(x[0]);
            pixels.push(x[1]);
            pixels.push(x[2]);
            pixels.push(x[3]);
        });
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
        ctx.load_texture(debug_name, color_image)
    }
}
