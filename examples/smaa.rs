// Enable the "fltk-enable-glwindow" features for more windowing compatibility.

use egui_fltk_frontend as frontend;
use egui_fltk_frontend::{
    egui,
    fltk::{
        app,
        enums::Event,
        prelude::{GroupExt, WidgetBase, WidgetExt, WindowExt},
        window,
    },
    pollster, wgpu, RWHandleExt, RenderPass, Timer,
};

use std::borrow::Cow;
use std::{cell::RefCell, rc::Rc, time::Instant};
use wgpu::{ColorTargetState, ColorWrites};

fn main() {
    let fltk_app = app::App::default();

    // Initialize fltk windows with minimal size:
    let mut window = window::GlWindow::default()
        .with_size(960, 540)
        .center_screen();
    window.set_label("SMAA Demo Window");
    window.make_resizable(true);
    window.end();
    window.show();
    window.make_current();

    // SMAA support on vulkan and gl backends only.
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

    // Set visual scale if needed, e.g: 0.8, 1.5, 2.0 .etc (default is 1.0)
    // state.set_visual_scale(1.25);

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

    // Prepare scene
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("resources/shader.wgsl"))),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format: texture_format,
                blend: None,
                write_mask: ColorWrites::all(),
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // Create SMAA target
    let mut smaa_target = smaa::SmaaTarget::new(
        &device,
        &queue,
        window.width() as u32,
        window.height() as u32,
        texture_format,
        smaa::SmaaMode::Disabled,
    );

    let egui_ctx = egui::Context::default();
    let start_time = Instant::now();

    // Use Timer for auto repaint if the app is idle.
    let mut timer = Timer::new(1);

    let mut quit = false;

    let mut disable_smaa = true;

    window.draw(move |window| {
        let mut state = state.borrow_mut();
        state.start_time(start_time.elapsed().as_secs_f64());

        let app_output = egui_ctx.run(state.take_input(), |ctx| {
            egui::Window::new("Config").show(&ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                        if ui
                            .button(if !disable_smaa {
                                "Disable SMAA?"
                            } else {
                                "Enable SMAA?"
                            })
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            if !disable_smaa {
                                smaa_target = smaa::SmaaTarget::new(
                                    &device,
                                    &queue,
                                    window.width() as u32,
                                    window.height() as u32,
                                    texture_format,
                                    smaa::SmaaMode::Disabled,
                                );
                                disable_smaa = true;
                            } else {
                                smaa_target = smaa::SmaaTarget::new(
                                    &device,
                                    &queue,
                                    window.width() as u32,
                                    window.height() as u32,
                                    texture_format,
                                    smaa::SmaaMode::Smaa1X,
                                );
                                disable_smaa = false;
                            }
                        }
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
            state.fuse_output(window, app_output.platform_output);
            let clipped_primitive = egui_ctx.tessellate(app_output.shapes);
            let texture = app_output.textures_delta;

            // Get calculated ScreenDescriptor
            let screen_descriptor = &state.screen_descriptor;

            // Resize surface according to window screen_descriptor
            if window_resized {
                let size = screen_descriptor.size_in_pixels;
                painter.surface_config.width = size[0];
                painter.surface_config.height = size[1];
                painter.surface.configure(&device, &painter.surface_config);
                smaa_target.resize(&device, size[0], size[1]);
            }

            // Record all render passes.
            match painter.surface.get_current_texture() {
                Ok(frame) => {
                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("encoder"),
                        });

                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let smaa_view = smaa_target.start_frame(&device, &queue, &view);
                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &smaa_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                    store: true,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });

                        // Draw Triangle Texture
                        rpass.set_pipeline(&render_pipeline);
                        rpass.draw(0..3, 0..1);

                        // Draw Egui Texture
                        painter.paint_with_rpass(
                            &mut rpass,
                            &device,
                            &queue,
                            screen_descriptor,
                            clipped_primitive,
                            texture,
                        );
                    }

                    // Submit command buffer
                    let cm_buffer = encoder.finish();
                    queue.submit(Some(cm_buffer));
                    smaa_view.resolve();
                    frame.present();
                }
                Err(e) => return eprintln!("Dropped frame with error: {}", e),
            };
            app::awake();
        }

        if quit {
            app::quit();
        }
    });

    let mut init = true;
    while fltk_app.wait() {
        if init {
            fltk_app.redraw();
            init = false;
        }
        window.flush();
    }
}
