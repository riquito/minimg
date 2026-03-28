use anyhow::{anyhow, Result};
use log::debug;
use minimg::fs_utils::{start_file_reader, Direction, FileStatus, ImagePair};
use minimg::window::{generate_window, Rotation};
use show_image::event;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, RwLock};

fn get_next_idx(idx: usize, len: usize, d: Direction) -> usize {
    match d {
        Direction::Stay => idx,
        Direction::Left if idx > 0 => idx - 1,
        Direction::Right if idx < len - 1 => idx + 1,
        Direction::First => 0,
        Direction::Last => len - 1,
        Direction::Left | Direction::Right => {
            debug!("Trying to move out of bounds, equivalent to Stay");
            idx
        }
    }
}

fn try_get_image(
    rx: &Receiver<Result<Option<usize>, String>>,
    cache: Arc<RwLock<Vec<FileStatus<ImagePair>>>>,
) -> Result<Option<ImagePair>> {
    match rx.try_recv() {
        Ok(maybe_img) => {
            debug!("Received next image_pair idx {:?}", maybe_img);

            if let Ok(Some(idx)) = maybe_img {
                debug!("Load image_pair from cache at idx {:?}", idx);

                if let Some(FileStatus::Read(image_pair)) = cache.read().unwrap().get(idx) {
                    debug!("Got imagPair");
                    return Ok(Some(image_pair.clone()));
                } // XXX else???
            }
        }
        Err(TryRecvError::Empty) => {
            // nothing to read
        }
        Err(TryRecvError::Disconnected) => {
            return Err(anyhow!(
                "Found disconnected channel while checking if any image was available"
            ));
        }
    }

    Ok(None)
}

#[show_image::main]
fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 2 {
        return Err(anyhow!("usage: {} [IMAGE or DIR]...", args[0]));
    }

    let mut paths = Vec::new();

    for file_or_dir in args.iter().skip(1) {
        let arg_path = std::path::Path::new(file_or_dir);

        if arg_path.is_file() {
            paths.push(arg_path.canonicalize()?)
        } else if arg_path.is_dir() {
            for entry in arg_path
                .read_dir()?
                .filter_map(|x| x.ok())
                .filter(|e| e.path().is_file())
            {
                let path = entry.path();
                if image::ImageFormat::from_path(&path).is_ok() {
                    paths.push(path);
                }
            }
        }
    }

    paths.sort();

    if paths.is_empty() {
        return Err(anyhow!("Could not find any image"));
    }

    let num_images = paths.len();

    let cache: Arc<RwLock<Vec<FileStatus<ImagePair>>>> =
        Arc::new(RwLock::new(vec![FileStatus::Unread; paths.len()]));
    let _cache = cache.clone();

    let (tx_img_idx_to_load, rx_img_idx_to_load) = channel::<Option<usize>>();
    let (tx_img_idx_ready, rx_img_idx_ready) = channel::<Result<Option<usize>, String>>();

    let try_get_next_image = || try_get_image(&rx_img_idx_ready, cache.clone());

    let window = generate_window()?;

    let cp = window.window.context_proxy();
    let w2 = window.window.clone();

    debug!("Start background thread to load images");
    cp.run_background_task(move || {
        start_file_reader(
            _cache,
            paths,
            0,
            5,
            rx_img_idx_to_load,
            tx_img_idx_ready,
            w2,
            /* XXX TODO why the wakeup function does not work to emit a
            user event to wake up the eventloop? We'd use it instead of
            sending a windowproxy and rendering the image as we're doing now */
            // || {
            //    w2.run_function(|_| { // could use windowproxy or contextproxy
            //      debug!("WAKE UP!");
            //    })
            // };
        );
    });

    debug!("Request initial image");
    // let's start by displaying something
    tx_img_idx_to_load
        .send(Some(0))
        .expect("Failed to send image request to internal thread");
    let mut current_path;
    loop {
        if let Some(image_pair) = try_get_next_image()? {
            debug!("Display initial image");
            current_path = image_pair.path_str().to_string();
            window.set_image(image_pair)?;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(14));
    }

    let mut prev_idx = 0;
    let mut idx = 0;

    // Wait for the window to be closed or Escape to be pressed.
    for event in window.event_channel()? {
        if prev_idx != idx {
            debug!("Request image at idx {}", idx);
            tx_img_idx_to_load
                .send(Some(idx))
                .expect("Failed to send image request to internal thread");
            prev_idx = idx;
        }

        if let Some(image_pair) = try_get_next_image()? {
            current_path = image_pair.path_str().to_string();
            debug!("Display image {:?}", current_path);
            window.set_image(image_pair)?;
        }

        if let event::WindowEvent::KeyboardInput(event) = event {
            if !event.is_synthetic && event.input.state.is_pressed() {
                use event::{Key, NamedKey};
                let key = &event.input.logical_key;
                let ctrl = event.modifiers.contains(event::ModifiersState::CONTROL);
                debug!("Keyboard event {:?}", key);
                match key {
                    Key::Named(NamedKey::Escape) => break,
                    Key::Character(c) if c == "q" => break,
                    Key::Named(NamedKey::ArrowUp) => {
                        window.pan(0.0, 0.05);
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        window.pan(0.0, -0.05);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        window.pan(-0.05, 0.0);
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        window.pan(0.05, 0.0);
                    }
                    Key::Named(NamedKey::Space) if event.modifiers.contains(event::ModifiersState::SHIFT) => {
                        idx = get_next_idx(idx, num_images, Direction::Left);
                    }
                    Key::Named(NamedKey::Space) => {
                        idx = get_next_idx(idx, num_images, Direction::Right);
                    }
                    Key::Character(c) if c == "l" || c == "n" => {
                        idx = get_next_idx(idx, num_images, Direction::Right);
                    }
                    Key::Named(NamedKey::Backspace) => {
                        idx = get_next_idx(idx, num_images, Direction::Left);
                    }
                    Key::Character(c) if c == "h" || c == "p" || c == "N" => {
                        idx = get_next_idx(idx, num_images, Direction::Left);
                    }
                    Key::Named(NamedKey::Home) => {
                        idx = get_next_idx(idx, num_images, Direction::First);
                    }
                    Key::Named(NamedKey::End) => {
                        idx = get_next_idx(idx, num_images, Direction::Last);
                    }
                    Key::Character(c) if c == "0" => {
                        window.reset_image();
                    }
                    Key::Character(c) if c == "-" && ctrl => {
                        window.scale_down();
                    }
                    Key::Character(c) if c == "=" && ctrl => {
                        window.scale_up();
                    }
                    Key::Character(c) if c == "R" => {
                        window.rotate(Rotation::Left);
                    }
                    Key::Character(c) if c == "r" => {
                        window.rotate(Rotation::Right);
                    }
                    Key::Character(c) if c == "f" => {
                        window.toggle_fullscreen();
                    }
                    Key::Character(c) if c == "c" => {
                        println!("{}", current_path);
                    }
                    _ => (),
                }
            }
        }
    }

    debug!("Exiting. Wait for threads to close");
    tx_img_idx_to_load.send(None).unwrap();

    Ok(())
}
