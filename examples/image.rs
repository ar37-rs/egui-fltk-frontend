use egui_fltk_frontend as frontend;
use egui_fltk_frontend::{
    egui::{self, Label},
    fltk::{
        app,
        enums::Event,
        prelude::{GroupExt, WidgetBase, WidgetExt, WindowExt},
        window,
    },
    pollster, wgpu, EguiImageConvertible, RWHandleExt, RenderPass, RetainedEguiImage, Timer,
};
use fltk::image::{JpegImage, SvgImage};
use std::{cell::RefCell, rc::Rc, time::Instant};

fn main() {
    let fltk_app = app::App::default();

    // Initialize fltk windows with minimal size:
    let mut window = window::GlWindow::default()
        .with_size(1000, 720)
        .center_screen();
    window.set_label("Image Demo Window");
    window.make_resizable(true);
    window.end();
    window.show();
    window.make_current();

    // wgpu::Backends::PRIMARY can be changed accordingly, .e.g: (wgpu::Backends::VULKAN, wgpu::Backends::GL .etc)
    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    // let surface = unsafe { instance.create_surface(&window) };
    // window.use_compat() for raw-window-handle 4.x compatible
    let surface = unsafe { instance.create_surface(&window.use_compat()) };

    // WGPU 0.11+ support force fallback (if HW implementation not supported), set it to true or false (optional).
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::default(),
            limits: wgpu::Limits::default(),
            label: None,
        },
        None,
    ))
    .unwrap();

    let texture_format = surface.get_supported_formats(&adapter)[0];
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: texture_format,
        width: window.width() as u32,
        height: window.height() as u32,
        present_mode: wgpu::PresentMode::Fifo,
    };

    surface.configure(&device, &surface_config);

    // Prepare back and front.
    let render_pass = RenderPass::new(&device, texture_format, 1);
    let (mut painter, state) =
        frontend::begin_with(&mut window, render_pass, surface, surface_config);

    // // Set visual scale if needed, e.g: 0.8, 1.5, 2.0 .etc (default is 1.0)
    // // (might cause blurry image if the image not scaled properly)
    // let mut state = state;
    // state.set_visual_scale(1.5);

    // Create egui state
    let state = Rc::new(RefCell::new(state));

    // Handle window events
    window.handle({
        let state = state.clone();
        move |win, ev| match ev {
            Event::Push
            | Event::Released
            | Event::KeyDown
            | Event::KeyUp
            | Event::MouseWheel
            | Event::Resize
            | Event::Move
            | Event::Drag
            | Event::Focus => {
                // Using "if let ..." for safety.
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.fuse_input(win, ev);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    });

    let egui_ctx = egui::Context::default();
    let start_time = Instant::now();

    // egui image from fltk svg
    let fltk_svg_image = SvgImage::from_data(include_str!("resources/fingerprint.svg")).unwrap();
    let retained_egui_svg_image =
        RetainedEguiImage::from_fltk_svg_image("fingerprint.svg", fltk_svg_image).unwrap();

    // fltk image to egui image
    let retained_egui_image = JpegImage::from_data(include_bytes!("resources/nature.jpg"))
        .unwrap()
        .egui_image("nature.jpg")
        .unwrap();

    // Use Timer for auto repaint if the app is idle.
    let mut timer = Timer::new(1);

    let mut quit = false;

    while fltk_app.wait() {
        let mut state = state.borrow_mut();
        state.start_time(start_time.elapsed().as_secs_f64());

        let app_output = egui_ctx.run(state.take_input(), |ctx| {
            egui::CentralPanel::default().show(&ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(Label::new("this is fingerprint.svg"));
                        retained_egui_svg_image.show(ui);
                        ui.add(Label::new("this is nature.jpg"));
                        retained_egui_image.show(ui);
                        if ui
                            .button("Quit?")
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            quit = true;
                        }
                    });
            });
        });

        let window_resized = state.window_resized();

        // Make sure to put timer.elapsed() on the last order.
        if app_output.repaint_after.is_zero()
            || window_resized
            || state.mouse_btn_pressed()
            || timer.elapsed()
        {
            state.fuse_output(&mut window, app_output.platform_output);
            let clipped_primitive = egui_ctx.tessellate(app_output.shapes);
            let texture = app_output.textures_delta;
            painter.paint_jobs(&device, &queue, &mut state, clipped_primitive, texture);
            window.redraw();
            window.flush();
            app::awake();
        }

        if quit {
            app::quit();
        }
    }
}
