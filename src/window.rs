use anyhow::{anyhow, Result};
use show_image::glam;

use crate::fs_utils::ImagePair;

pub struct Window {
    pub window: show_image::WindowProxy,
}

pub enum Rotation {
    Right,
    Left,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RotationState {
    UP,
    RIGHT,
    DOWN,
    LEFT,
}
impl RotationState {
    fn clockwise(&self) -> RotationState {
        match self {
            RotationState::UP => RotationState::RIGHT,
            RotationState::RIGHT => RotationState::DOWN,
            RotationState::DOWN => RotationState::LEFT,
            RotationState::LEFT => RotationState::UP,
        }
    }

    fn counter_clockwise(&self) -> RotationState {
        match self {
            RotationState::UP => RotationState::LEFT,
            RotationState::RIGHT => RotationState::UP,
            RotationState::DOWN => RotationState::RIGHT,
            RotationState::LEFT => RotationState::DOWN,
        }
    }

    fn rotate(&self, direction: Rotation) -> RotationState {
        match direction {
            Rotation::Right => self.clockwise(),
            Rotation::Left => self.counter_clockwise(),
        }
    }
}

impl Window {
    pub fn set_image(&self, image_pair: ImagePair) -> Result<()> {
        let name = image_pair.path_str().to_string();
        let image = image_pair.image().unwrap();
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
    pub fn rotate(&self, direction: Rotation) {
        self.window
            .run_function_wait(move |mut window_handle| {
                window_handle.set_preserve_aspect_ratio(false);
                let cur_transform = window_handle.transform();

                // x-axis y-axis (assuming we rotate clockwise)
                //  1/ 0  0/ 1   # ⮝
                //  0/ 1 -1/ 0   # ⮞
                // -1/ 0  0/-1   # ⮟
                //  0/-1  1/ 0   # ⮜
                // As usual with floats, they're hard to compare for equality

                // (if I could remember more of algebra, I wouldn't need to detect the current rotation... )
                let r_state = match cur_transform.matrix2.to_cols_array() {
                    k if k[0] > k[1] && k[2] < k[3] => RotationState::UP,
                    k if k[0] < k[1] && k[2] < k[3] => RotationState::RIGHT,
                    k if k[0] < k[1] && k[2] > k[3] => RotationState::DOWN,
                    k if k[0] > k[1] && k[2] > k[3] => RotationState::LEFT,
                    _ => RotationState::UP,
                };

                let r_state = r_state.rotate(direction);

                let angle = std::f32::consts::PI / 2.0
                    * match r_state {
                        RotationState::UP => 0.0,
                        RotationState::RIGHT => 1.0,
                        RotationState::DOWN => 2.0,
                        RotationState::LEFT => 3.0,
                    };

                let rotate = glam::Affine2::from_angle(angle);

                let image_size = window_handle.image_info().unwrap().size.as_vec2();
                let mut inner_size = window_handle.inner_size().as_vec2();

                // is it going to be rotated 90 or 180 degree? Invert x with y
                if r_state == RotationState::RIGHT || r_state == RotationState::LEFT {
                    inner_size = glam::Vec2::new(inner_size.y, inner_size.x);
                }

                let (fit_transform, _) = fit(inner_size, image_size);

                let position =
                    glam::Affine2::from_translation(glam::Vec2::from_slice(match r_state {
                        RotationState::UP => &[0.0, 0.0],
                        RotationState::RIGHT => &[1.0, 0.0],
                        RotationState::DOWN => &[1.0, 1.0],
                        RotationState::LEFT => &[0.0, 1.0],
                    }));

                window_handle.set_transform(position * rotate * fit_transform);
            })
            .expect("XXX TODO rotate failed");
    }

    pub fn exit(&self) {}
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

fn fit(window_size: glam::Vec2, image_size: glam::Vec2) -> (glam::Affine2, glam::Vec2) {
    let ratios = image_size / window_size;

    let w;
    let h;
    if ratios.x >= ratios.y {
        w = 1.0;
        h = ratios.y / ratios.x;
    } else {
        w = ratios.x / ratios.y;
        h = 1.0;
    }

    let transform = glam::Affine2::from_scale_angle_translation(
        glam::Vec2::new(w, h),
        0.0,
        0.5 * glam::Vec2::new(1.0 - w, 1.0 - h),
    );

    (transform, image_size)
}
