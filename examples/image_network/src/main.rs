mod framework;
use asynchron::{Futurize, Futurized, InnerTaskHandle, Progress};
use backend::epi;
use egui_fltk_frontend as frontend;
use egui_wgpu_backend as backend;
use frontend::{egui, fltk, ImgWidget, ImgWidgetExt};
use std::{borrow::Cow, time::Duration};
const INTEGRATION_NAME: &str = "egui + fltk + wgpu-backend";

struct ImageDemo<'a> {
    image_widget: Option<ImgWidget>,
    tokio_rt: tokio::runtime::Runtime,
    task: Option<Futurized<(), Vec<u8>>>,
    fetch_btn_label: Cow<'a, str>,
    err_label: Option<Cow<'a, str>>,
    seed: usize,
}

impl<'a> Default for ImageDemo<'a> {
    fn default() -> Self {
        let tokio_rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        Self {
            image_widget: None,
            tokio_rt,
            task: None,
            fetch_btn_label: "fetch image".into(),
            err_label: None,
            seed: 1,
        }
    }
}

impl<'a> epi::App for ImageDemo<'a> {
    fn name(&self) -> &str {
        "world"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        let Self {
            image_widget,
            tokio_rt,
            task,
            fetch_btn_label,
            err_label,
            seed,
        } = self;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Image Newtork Demo");
            if ui.button(&fetch_btn_label).clicked() {
                // Create new task if only task if None or freed.
                if let None = task {
                    let current_seed = *seed;
                    let rt = tokio_rt.handle().clone();
                    let new_task =
                        Futurize::task(0, move |_task: InnerTaskHandle| -> Progress<(), Vec<u8>> {
                            rt.block_on(async {
                                let timeout = Duration::from_secs(3);
                                let client = match reqwest::Client::builder()
                                    .connect_timeout(timeout)
                                    .build()
                                {
                                    Ok(client) => client,
                                    Err(e) => return Progress::Error(e.to_string().into()),
                                };

                                // Url to .jpg image
                                let url = format!("https://picsum.photos/seed/{}/640/480", current_seed);

                                let request = match client
                                    .get(url)
                                    .build()
                                {
                                    Ok(request) => request,
                                    Err(e) => return Progress::Error(e.to_string().into()),
                                };

                                let response = match client.execute(request).await {
                                    Ok(response) => response,
                                    Err(e) => return Progress::Error(e.to_string().into()),
                                };

                                println!("Status: {}\n", response.status());

                                for value in response.headers().values() {
                                    println!("{}\n", value.to_str().unwrap());
                                }

                                let content = match response.bytes().await {
                                    Ok(r) => r,
                                    Err(e) => return Progress::Error(e.to_string().into()),
                                };
                                return Progress::Completed(content.to_vec());
                            })
                        });
                    new_task.try_do();
                    *task = Some(new_task);
                    *seed += 1;
                }
            }

            // Only resolve if task is some.
            if let Some(this) = task {
                let mut task_should_free = false;
                this.try_resolve(|progress, done| {
                    match progress {
                        Progress::Completed(jpg) => {
                            *image_widget = fltk::image::JpegImage::from_data(&jpg)
                                .unwrap()
                                .into_img_widget(frame);
                            *err_label = None;
                        }
                        Progress::Error(e) => {
                            *err_label = Some(e.to_string().into());
                            *image_widget = None;
                        }
                        Progress::Current(_) => *fetch_btn_label = "fetching...".into(),
                        _ => (),
                    }
                    if done {
                        task_should_free = true;
                        *fetch_btn_label = "refetch the image".into();
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
    framework::run(ImageDemo::default(), (656, 800), "hello", INTEGRATION_NAME)
}
