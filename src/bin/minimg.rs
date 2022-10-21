use anyhow::{anyhow, Result};
use log::debug;
use minimg::fs_utils::{start_file_reader, Direction, FileStatus, ImagePair};
use minimg::window::{generate_window, Rotation};
use show_image::event;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;

struct ImagesBag {
    tx_d: Sender<Direction>,
    rx_f: Receiver<Result<Option<usize>, String>>,
    _thread_handle: JoinHandle<()>,
    _cache: Arc<RwLock<Vec<FileStatus<ImagePair>>>>,
}
impl ImagesBag {
    pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
        if paths.is_empty() {
            return Err(anyhow!("The list of images to read cannot be empty"));
        }

        let (tx_f, rx_f) = channel::<Result<Option<usize>, String>>();
        let (tx_d, rx_d) = channel::<Direction>();

        let cache: Arc<RwLock<Vec<FileStatus<ImagePair>>>> =
            Arc::new(RwLock::new(vec![FileStatus::Unread; paths.len()]));
        let _cache = cache.clone();

        let _thread_handle =
            std::thread::spawn(move || start_file_reader(cache, paths, 0, 5, rx_d, tx_f));

        Ok(ImagesBag {
            tx_d,
            rx_f,
            _thread_handle,
            _cache,
        })
    }

    pub fn get(&mut self, d: Direction) -> Option<ImagePair> {
        debug!("Request next image");
        self.tx_d.send(d).unwrap();
        debug!("Wait for next image");
        let maybe_img = self.rx_f.recv().unwrap();
        debug!("Received next image_pair idx");

        if let Ok(Some(idx)) = maybe_img {
            debug!("Search for image_pair at idx");

            if let Some(FileStatus::Read(image_pair)) = self._cache.read().unwrap().get(idx) {
                debug!("Got imagPair");
                return Some(image_pair.clone());
            }
        }

        // TODO handle error

        None
    }

    pub fn stop(self) -> Result<(), Box<dyn std::any::Any + Send>> {
        self.tx_d.send(Direction::Exit).unwrap();
        self._thread_handle.join()
    }
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

    let mut images_bag = ImagesBag::new(paths)?;
    let first_image_pair = images_bag.get(Direction::Stay).unwrap(); // there is at least one image

    let window = generate_window()?;

    // let's start by displaying something
    window.set_image(first_image_pair)?;

    // Wait for the window to be closed or Escape to be pressed.
    for event in window.event_channel()? {
        if let event::WindowEvent::KeyboardInput(event) = event {
            if !event.is_synthetic && event.input.state.is_pressed() {
                match event.input.key_code {
                    Some(event::VirtualKeyCode::Escape) | Some(event::VirtualKeyCode::Q) => break,
                    Some(event::VirtualKeyCode::Right)
                    | Some(event::VirtualKeyCode::L)
                    | Some(event::VirtualKeyCode::N)
                    | Some(event::VirtualKeyCode::Space) => {
                        if let Some(image_pair) = images_bag.get(Direction::Right) {
                            window.set_image(image_pair)?;
                        }
                    }
                    Some(event::VirtualKeyCode::Left)
                    | Some(event::VirtualKeyCode::H)
                    | Some(event::VirtualKeyCode::P)
                    | Some(event::VirtualKeyCode::Back) => {
                        if let Some(image_pair) = images_bag.get(Direction::Left) {
                            window.set_image(image_pair)?;
                        }
                    }
                    Some(event::VirtualKeyCode::Home) => {
                        if let Some(image_pair) = images_bag.get(Direction::First) {
                            window.set_image(image_pair)?;
                        }
                    }
                    Some(event::VirtualKeyCode::End) => {
                        if let Some(image_pair) = images_bag.get(Direction::Last) {
                            window.set_image(image_pair)?;
                        }
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

    images_bag
        .stop()
        .expect("Could not stop cleanly the image's loader thread");

    Ok(())
}
