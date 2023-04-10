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
    loop {
        if let Some(image_pair) = try_get_next_image()? {
            debug!("Display initial image");
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
            debug!("Display image {:?}", image_pair.path_str().to_string());
            window.set_image(image_pair)?;
        }

        if let event::WindowEvent::KeyboardInput(event) = event {
            if !event.is_synthetic && event.input.state.is_pressed() {
                debug!("Keyboard event {:?}", event.input.key_code);
                match event.input.key_code {
                    Some(event::VirtualKeyCode::Escape) | Some(event::VirtualKeyCode::Q) => break,
                    Some(event::VirtualKeyCode::Right)
                    | Some(event::VirtualKeyCode::L)
                    | Some(event::VirtualKeyCode::N)
                    | Some(event::VirtualKeyCode::Space) => {
                        idx = get_next_idx(idx, num_images, Direction::Right);
                    }
                    Some(event::VirtualKeyCode::Left)
                    | Some(event::VirtualKeyCode::H)
                    | Some(event::VirtualKeyCode::P)
                    | Some(event::VirtualKeyCode::Back) => {
                        idx = get_next_idx(idx, num_images, Direction::Left);
                    }
                    Some(event::VirtualKeyCode::Home) => {
                        idx = get_next_idx(idx, num_images, Direction::First);
                    }
                    Some(event::VirtualKeyCode::End) => {
                        idx = get_next_idx(idx, num_images, Direction::Last);
                    }
                    Some(event::VirtualKeyCode::Key0) => {
                        window.reset_image();
                    }
                    Some(event::VirtualKeyCode::Minus)
                        if event.input.modifiers == event::ModifiersState::CTRL =>
                    {
                        window.scale_down();
                    }
                    Some(event::VirtualKeyCode::Equals)
                        if event.input.modifiers == event::ModifiersState::CTRL =>
                    {
                        window.scale_up();
                    }
                    Some(event::VirtualKeyCode::R)
                        if event.input.modifiers == event::ModifiersState::SHIFT =>
                    {
                        window.rotate(Rotation::Left);
                    }
                    Some(event::VirtualKeyCode::R) => {
                        window.rotate(Rotation::Right);
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
