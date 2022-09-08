use anyhow::{anyhow, Result};
use image::DynamicImage;

pub struct Window {
    window: show_image::WindowProxy,
}

impl Window {
    pub fn set_image(&self, name: &str, image: DynamicImage) -> Result<()> {
        self.window
            .set_image(name, image)
            .map_err(|_| anyhow!("Cannot apply the image"))
    }

    pub fn event_channel(
        &self,
    ) -> Result<
        std::sync::mpsc::Receiver<show_image::event::WindowEvent>,
        show_image::error::InvalidWindowId,
    > {
        self.window.event_channel()
    }
}

pub fn generate_window() -> Result<Window> {
    let window = show_image::create_window(
        "image",
        show_image::WindowOptions {
            preserve_aspect_ratio: true,
            ..Default::default()
        },
    )?;

    Ok(Window { window })
}
