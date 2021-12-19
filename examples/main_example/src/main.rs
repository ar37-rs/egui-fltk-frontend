use backend::{
    epi::{
        backend::{AppOutput, FrameBuilder},
        App, IntegrationInfo,
    },
    wgpu,
};
use egui_demo_lib::WrapApp;
use egui_fltk_frontend as frontend;
use egui_wgpu_backend as backend;
use frontend::{
    egui::CtxRef,
    fltk::{
        app,
        enums::Event,
        prelude::{GroupExt, WidgetBase, WidgetExt, WindowExt},
        window,
    },
    get_frame_time, DpiScaling, Signal, Timer, RWHandleExt,
};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};
const INTEGRATION_NAME: &str = "egui + fltk + wgpu-backend";

fn main() {
    let a = app::App::default();

    // Initialize fltk windows with minimal size:
    let mut window = window::Window::default()
        .with_size(200, 200)
        .center_screen();
    window.set_label("Demo Window");
    window.make_resizable(true);
    window.end();
    window.show();

    // Fix window resizable on fltk 1.2.20+
    window.set_size(800, 600);
    window  = window.center_screen();

    window.make_current();

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
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

    let surface_format = surface.get_preferred_format(&adapter).unwrap();
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: window.width() as u32,
        height: window.height() as u32,
        present_mode: wgpu::PresentMode::Mailbox,
    };

    surface.configure(&device, &surface_config);

    // Prepare back and front.
    let render_pass = backend::RenderPass::new(&device, surface_format, 1);
    let (painter, state) = frontend::begin_with(
        &mut window,
        render_pass,
        surface,
        surface_config,
        DpiScaling::Default,
    );

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
                let mut handled = false;
                // Using "if let ..." for safety.
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.fuse_input(win, ev);
                    handled = true;
                }
                handled
            }
            _ => false,
        }
    });

    // We use the egui_wgpu_backend crate as the render backend.
    let device = Rc::new(RefCell::new(device));
    let queue = Rc::new(RefCell::new(queue));
    let painter = Rc::new(RefCell::new(painter));

    // Display the demo application that ships with egui.
    let demo_app = Rc::new(RefCell::new(WrapApp::default()));
    let egui_ctx = Rc::new(RefCell::new(CtxRef::default()));
    let repaint_signal = Arc::new(Signal::default());
    let start_time = Instant::now();
    let mut cpu_usage = get_frame_time(start_time);
    let app_output = Rc::new(RefCell::new(AppOutput::default()));

    // Redraw window while being resized (required on windows platform).
    window.draw({
        let repaint_signal = repaint_signal.clone();
        let state = state.clone();
        let painter = painter.clone();
        let egui_ctx = egui_ctx.clone();
        let device = device.clone();
        let queue = queue.clone();
        let demo_app = demo_app.clone();
        let app_output = app_output.clone();
        move |window| {
            // And here also using "if let ..." for safety.
            if let Ok(mut state) = state.try_borrow_mut() {
                if state.window_resized() {
                    window.clear_damage();
                    if let Ok(mut painter) = painter.try_borrow_mut() {
                        let egui_start = Instant::now();
                        let mut egui_ctx = egui_ctx.borrow_mut();
                        let mut device = device.borrow_mut();
                        let mut queue = queue.borrow_mut();
                        let mut app_output = app_output.borrow_mut();
                        {
                            // Begin frame
                            let mut frame = FrameBuilder {
                                info: IntegrationInfo {
                                    web_info: None,
                                    cpu_usage: Some(cpu_usage),
                                    native_pixels_per_point: Some(state.pixels_per_point),
                                    prefer_dark_mode: None,
                                    name: INTEGRATION_NAME,
                                },
                                tex_allocator: &mut painter.render_pass,
                                output: &mut app_output,
                                repaint_signal: repaint_signal.clone(),
                            }
                            .build();
                            let start_time = start_time.elapsed().as_secs_f64();
                            state.input.time = Some(start_time);
                            egui_ctx.begin_frame(state.input.take());

                            // Draw the demo application.
                            let mut demo_app = demo_app.borrow_mut();
                            demo_app.update(&egui_ctx, &mut frame);
                        }

                        // End the UI frame. We could now handle the output and draw the UI with the backend.
                        let (output, shapes) = egui_ctx.end_frame();
                        cpu_usage = (Instant::now() - egui_start).as_secs_f64() as f32;
                        state.fuse_output(window, &output);
                        let clipped_mesh = egui_ctx.tessellate(shapes);
                        let texture = egui_ctx.texture();
                        painter.paint_jobs(
                            &mut device,
                            &mut queue,
                            &mut state,
                            clipped_mesh,
                            texture,
                        );
                    }
                }
            }
        }
    });

    // Use Timer for auto repaint if the app is idle.
    let mut timer = Timer::new(1);

    while a.wait() {
        let egui_start = Instant::now();
        let mut state = state.borrow_mut();
        let mut painter = painter.borrow_mut();
        let mut egui_ctx = egui_ctx.borrow_mut();
        let mut device = device.borrow_mut();
        let mut queue = queue.borrow_mut();
        let mut app_output = app_output.borrow_mut();
        {
            let mut frame = FrameBuilder {
                info: IntegrationInfo {
                    web_info: None,
                    cpu_usage: Some(cpu_usage),
                    native_pixels_per_point: Some(state.pixels_per_point),
                    prefer_dark_mode: None,
                    name: INTEGRATION_NAME,
                },
                tex_allocator: &mut painter.render_pass,
                output: &mut app_output,
                repaint_signal: repaint_signal.clone(),
            }
            .build();
            let start_time = start_time.elapsed().as_secs_f64();
            state.input.time = Some(start_time);
            egui_ctx.begin_frame(state.input.take());

            // Draw the demo application.
            let mut demo_app = demo_app.borrow_mut();
            demo_app.update(&egui_ctx, &mut frame);
        }

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let (output, shapes) = egui_ctx.end_frame();
        cpu_usage = (Instant::now() - egui_start).as_secs_f64() as f32;

        if app_output.quit {
            break;
        }

        let window_resized = state.window_resized();
        if window_resized {
            window.clear_damage();
        }

        // Make sure to put timer.elapsed() on the last order.
        if output.needs_repaint || window_resized || state.mouse_btn_pressed() || timer.elapsed() {
            state.fuse_output(&mut window, &output);
            let clipped_mesh = egui_ctx.tessellate(shapes);
            let texture = egui_ctx.texture();
            painter.paint_jobs(&mut device, &mut queue, &mut state, clipped_mesh, texture);
        }
        app::awake();
    }
}
