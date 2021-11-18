mod framework;
use asynchron::{Futurize, Futurized, InnerTaskHandle, Progress};
use backend::epi;
use egui_fltk_frontend as frontend;
use egui_wgpu_backend as backend;
use frontend::{egui, fltk, ImgWidget, ImgWidgetExt};
use std::{borrow::Cow, time::Duration};
const INTEGRATION_NAME: &str = "egui + fltk + wgpu-backend";
use std::io::Read;
use ureq::{Agent, AgentBuilder};

struct ImageDemoUreq<'a> {
    image_widget: Option<ImgWidget>,
    task: Option<Futurized<(), Vec<u8>>>,
    fetch_btn_label: Cow<'a, str>,
    err_label: Option<Cow<'a, str>>,
    seed: usize,
}

impl<'a> Default for ImageDemoUreq<'a> {
    fn default() -> Self {
        Self {
            image_widget: None,
            task: None,
            fetch_btn_label: "fetch image".into(),
            err_label: None,
            seed: 1,
        }
    }
}

impl<'a> epi::App for ImageDemoUreq<'a> {
    fn name(&self) -> &str {
        "world"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        let Self {
            image_widget,
            task,
            fetch_btn_label,
            err_label,
            seed,
        } = self;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Image Newtork Demo Ureq");
            if ui.button(&fetch_btn_label).clicked() {
                // Create new task if only task is None or freed.
                if task.is_none() {
                    let current_seed = *seed;
                    let new_task = Futurize::task(
                        current_seed,
                        move |_task: InnerTaskHandle| -> Progress<(), Vec<u8>> {
                            let timeout = Duration::from_secs(3);
                            let agent: Agent = AgentBuilder::new()
                                .timeout_read(timeout)
                                .timeout_write(timeout)
                                .build();

                            // Url to .jpg image
                            let url =
                                format!("https://picsum.photos/seed/{}/640/480", current_seed);

                            let response = if let Ok(response) = agent.get(&url).call() {
                                response
                            } else {
                                return Progress::Error(
                                    format!("Network problem, unable to request url: {}", &url)
                                        .into(),
                                );
                            };

                            println!("Status: {}\n", response.status());
                            println!("Url: {}\n", response.get_url());
                            println!("HTTP Version: {}\n", response.http_version());
                            println!("Content-Type: {}\n", response.content_type());
                            println!("Charset: {}\n", response.charset());

                            let mut buf: Vec<u8> = Vec::new();
                            if let Err(e) = response.into_reader().read_to_end(&mut buf) {
                                return Progress::Error(e.to_string().into());
                            }

                            Progress::Completed(buf)
                        },
                    );
                    new_task.try_do();
                    *task = Some(new_task);
                    *seed += 1;
                } else {
                    println!("fecth button clicked, \nUreq is busy, please wait until it done.\n");
                }
            }

            // Only resolve if task is some.
            if let Some(this) = task {
                let mut task_should_free = false;
                this.try_resolve(|progress, done| {
                    match progress {
                        Progress::Completed(jpg) => {
                            // Just to make sure, free unused texture id.
                            if let Some(this) = image_widget {
                                frame.tex_allocator().free(this.texture_id());
                            }
                            *image_widget = fltk::image::JpegImage::from_data(&jpg)
                                .unwrap()
                                .into_img_widget(frame);
                            *err_label = None;
                        }
                        Progress::Error(e) => {
                            *err_label = Some(e.to_string().into());
                            // And here.
                            if let Some(this) = image_widget {
                                frame.tex_allocator().free(this.texture_id());
                            }
                            *image_widget = None;
                        }
                        Progress::Current(_) => *fetch_btn_label = "fetching...".into(),
                        _ => (),
                    }
                    if done {
                        // Redraw
                        frame.repaint_signal();
                        task_should_free = true;
                        *fetch_btn_label = "fetch next image".into();
                    }
                });

                // Free task.
                if task_should_free {
                    *task = None;
                }
            }

            // Only show label if reqwest is error.
            if let Some(this) = err_label {
                ui.label(this);
            }

            // Only show image if reqwest is succeed.
            if let Some(this) = image_widget {
                ui.add(this.widget());
            }
        });

        // Resize the native window to be just the size we need it to be:
        frame.set_window_size(ctx.used_size());
    }
}

fn main() {
    framework::run_boxed(
        Box::new(ImageDemoUreq::default()),
        (656, 800),
        "hello",
        INTEGRATION_NAME,
    )
}
