use anyhow::{anyhow, Result};
use minimg::window::generate_window;
use show_image::event;

#[show_image::main]
fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(anyhow!("usage: {} IMAGE", args[0]));
    }

    let path = std::path::Path::new(&args[1]);
    let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("image");

    let image =
        image::open(path).map_err(|e| anyhow!("Failed to read image from {:?}: {}", path, e))?;
    /*
       let image_rgba8 = image.into_rgba8();

       let img_view = show_image::ImageView::new(
           show_image::ImageInfo::rgba8(image_rgba8.width(), image_rgba8.height()),
           image_rgba8.as_bytes(),
       );
    */

    let image_info = show_image::image_info(&image)?;
    println!("{:#?}", image_info);

    let path = std::path::Path::new(&args[2]);
    let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("image");

    let image2 =
        image::open(path).map_err(|e| anyhow!("Failed to read image from {:?}: {}", path, e))?;

    let window = generate_window()?;
    window.set_image(name, image.clone())?;

    // Wait for the window to be closed or Escape to be pressed.
    for event in window.event_channel()? {
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
                println!("Right pressed!");
                window.set_image(name, image2.clone())?;
            } else if !event.is_synthetic
                && event.input.key_code == Some(event::VirtualKeyCode::Left)
                && event.input.state.is_pressed()
            {
                println!("Right pressed!");
                window.set_image(name, image.clone())?;
            }
        }
    }

    Ok(())
}
