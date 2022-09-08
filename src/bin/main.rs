use anyhow::{anyhow, Result};
use minimg::window::generate_window;
use show_image::event;

#[show_image::main]
fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        return Err(anyhow!("usage: {} IMAGE", args[0]));
    }

    let arg_path = std::path::Path::new(&args[1]);

    let mut paths = Vec::new();
    if arg_path.is_file() {
        paths.push(arg_path.canonicalize()?)
    } else if arg_path.is_dir() {
        for entry in arg_path
            .read_dir()?
            .filter_map(|x| x.ok())
            .filter(|e| e.path().is_file())
        {
            let path = entry.path();
            if let Ok(_) = image::ImageFormat::from_path(&path) {
                paths.push(path);
            }
        }
    }

    if paths.is_empty() {
        return Err(anyhow!(
            "Could not find images in the provided directory: {}",
            arg_path.display()
        ));
    }

    let mut images = Vec::new();

    // load all the images at once! (step 1, make it work...)
    for path in paths.iter() {
        let image = image::open(path)
            .map_err(|e| anyhow!("Failed to read image from {:?}: {}", path, e))?;
        images.push((path, image));
    }

    let window = generate_window()?;

    // let's start by displaying something
    window.set_image(
        &images.first().unwrap().0.to_string_lossy(),
        images.first().unwrap().1.clone(),
    )?;

    let mut img_idx = 0;

    // Wait for the window to be closed or Escape to be pressed.
    for event in window.event_channel()? {
        dbg!("have", img_idx);
        if let event::WindowEvent::KeyboardInput(event) = event {
            if !event.is_synthetic
                && event.input.key_code == Some(event::VirtualKeyCode::Escape)
                && event.input.state.is_pressed()
            {
                println!("Escape pressed!");
                break;
            } else if !event.is_synthetic
                && event.input.key_code == Some(event::VirtualKeyCode::Right)
                && event.input.state.is_pressed()
            {
                img_idx = (img_idx + 1) % images.len();
                let image = &images[img_idx];
                window.set_image(&image.0.to_string_lossy(), image.1.clone())?;
            } else if !event.is_synthetic
                && event.input.key_code == Some(event::VirtualKeyCode::Left)
                && event.input.state.is_pressed()
            {
                img_idx = if img_idx == 0 {
                    images.len() - 1
                } else {
                    (img_idx - 1) % images.len()
                };
                let image = &images[img_idx];
                window.set_image(&image.0.to_string_lossy(), image.1.clone())?;
            }
        }
    }

    Ok(())
}
