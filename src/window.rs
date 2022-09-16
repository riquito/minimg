use anyhow::{anyhow, Result};
use image::DynamicImage;
use show_image::glam;

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

    pub fn reset_image(&self) {
        self.window
            .run_function_wait(|mut window_handle| {
                let scale = 1f32;

                let transform = glam::Affine2::from_scale_angle_translation(
                    glam::Vec2::splat(scale),
                    0.0,
                    glam::Vec2::new(0.0, 0.0),
                );

                window_handle.set_transform(transform);
            })
            .expect("XXX TODO reset_scale failed");
    }

    pub fn scale_down(&self) {
        self.window
            .run_function_wait(|mut window_handle| {
                let transform = window_handle.transform();
                let scale_x = transform.x_axis.length();

                // TODO should use a curve, maybe exp, to smooth it out

                // never reach 0
                let scale = if scale_x > 1.0 {
                    scale_x / 1.25
                } else if scale_x > 0.2 {
                    scale_x - 0.1
                } else {
                    scale_x
                };

                let origin = glam::Vec2::splat((1.0 - scale) / 2.0);

                let transform = glam::Affine2::from_scale_angle_translation(
                    glam::Vec2::splat(scale),
                    0.0,
                    origin,
                );

                window_handle.set_transform(transform);
            })
            .expect("XXX TODO reset_scale failed");
    }

    pub fn scale_up(&self) {
        self.window
            .run_function_wait(|mut window_handle| {
                let transform = window_handle.transform();
                let scale_x = transform.x_axis.length();

                let scale = if scale_x < 1.0 {
                    scale_x + 0.1
                } else if scale_x < 4.0 {
                    scale_x * 1.25
                } else {
                    scale_x
                };

                let origin = glam::Vec2::splat((1.0 - scale) / 2.0);

                let transform = glam::Affine2::from_scale_angle_translation(
                    glam::Vec2::splat(scale),
                    0.0,
                    origin,
                );

                window_handle.set_transform(transform);
            })
            .expect("XXX TODO reset_scale failed");
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
