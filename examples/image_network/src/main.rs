mod framework;
use backend::epi;
use egui_fltk_frontend as frontend;
use egui_wgpu_backend as backend;
use flowync::Leaper;
use frontend::{egui, fltk, ImgWidget, ImgWidgetExt};
use std::{thread, time::Duration};
const INTEGRATION_NAME: &str = "egui + fltk + wgpu-backend";

struct ImageDemoReqwest {
    image_widget: Option<ImgWidget>,
    tokio_rt: tokio::runtime::Runtime,
    task: Option<Leaper<Vec<u8>>>,
    fetch_btn_label: String,
    err_label: Option<String>,
    seed: usize,
}

impl Default for ImageDemoReqwest {
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

async fn fecth(url: &str, timeout: Duration) -> Result<Vec<u8>, reqwest::Error> {
    let client = reqwest::Client::builder()
        .connect_timeout(timeout)
        .build()?;
    let request = client.get(url).build()?;
    let response = client.execute(request).await?;

    println!("Status: {}\n", response.status());
    for value in response.headers().values() {
        println!("{}\n", value.to_str().unwrap());
    }

    let content = response.bytes().await?.to_vec();
    Ok(content)
}

impl epi::App for ImageDemoReqwest {
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
            ui.heading("Image Newtork Demo Reqwest");
            if ui.button(&fetch_btn_label).clicked() {
                // Create new task if only task is None or freed.
                if task.is_none() {
                    let current_seed = *seed;
                    let rt = tokio_rt.handle().clone();
                    let new_task = Leaper::new(current_seed);
                    thread::spawn({
                        let handle = new_task.handle();
                        move || {
                            rt.block_on(async {
                                // Url to .jpg image
                                let url =
                                    format!("https://picsum.photos/seed/{}/640/480", current_seed);
                                let timeout = Duration::from_secs(3);
                                let content = match fecth(&url, timeout).await {
                                    Ok(content) => content,
                                    Err(e) => return handle.err(e.to_string()),
                                };
                                handle.ok(content);
                            });
                        }
                    });
                    *task = Some(new_task);
                    *seed += 1;
                } else {
                    println!(
                        "fecth button clicked, \nReqwest is busy, please wait until it done.\n"
                    );
                }
            }

            // Only resolve if task is some.
            if let Some(this) = task {
                *fetch_btn_label = "fetching...".into();
                let mut task_should_free = false;
                this.try_catch(|result| {
                    match result {
                        Ok(jpg) => {
                            if let Some(this) = image_widget {
                                frame.tex_allocator().free(this.texture_id());
                            }
                            *image_widget = fltk::image::JpegImage::from_data(&jpg)
                                .unwrap()
                                .into_img_widget(frame);
                            *err_label = None;
                        }
                        Err(e) => {
                            *err_label = Some(e);
                            // And here.
                            if let Some(this) = image_widget {
                                frame.tex_allocator().free(this.texture_id());
                            }
                            *image_widget = None;
                        }
                    }

                    // Redraw
                    frame.repaint_signal();
                    *fetch_btn_label = "fetch next image".into();

                    // Free task.
                    task_should_free = true;
                });

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
        Box::new(ImageDemoReqwest::default()),
        (656, 800),
        "hello",
        INTEGRATION_NAME,
    )
}
