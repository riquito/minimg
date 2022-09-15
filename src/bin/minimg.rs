use anyhow::{anyhow, Result};
use image::DynamicImage;
use log::debug;
use minimg::fs_utils::{start_file_reader, Direction};
use minimg::window::generate_window;
use show_image::event;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

struct ImagePair(PathBuf, Option<DynamicImage>);

impl ImagePair {
    fn path_str(&self) -> std::borrow::Cow<str> {
        self.0.to_string_lossy()
    }

    fn image_clone(&self) -> Option<DynamicImage> {
        self.1.clone()
    }
}

struct ImagesBag {
    tx_d: Sender<Direction>,
    rx_f: Receiver<Result<DynamicImage, String>>,
    _thread_handle: JoinHandle<()>,
}
impl ImagesBag {
    pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
        if paths.is_empty() {
            return Err(anyhow!("The list of images to read cannot be empty"));
        }

        let (tx_f, rx_f) = channel::<Result<DynamicImage, String>>();
        let (tx_d, rx_d) = channel::<Direction>();
        let _thread_handle = std::thread::spawn(move || start_file_reader(paths, 0, 5, rx_d, tx_f));

        Ok(ImagesBag {
            tx_d,
            rx_f,
            _thread_handle,
        })
    }

    pub fn get(&mut self, d: Direction) -> Option<ImagePair> {
        debug!("Request next image");
        self.tx_d.send(d).unwrap();
        debug!("Wait for next image");
        let maybe_img = self.rx_f.recv().unwrap();
        debug!("Received next image");

        if let Ok(image) = maybe_img {
            debug!("Start building ImagePair");

            let x = Some(ImagePair(PathBuf::from("/"), Some(image)));

            debug!("Done building ImagePair");
            return x;
        }

        None
    }

    pub fn next(&mut self) -> Option<ImagePair> {
        self.get(Direction::Right)
    }

    pub fn prev(&mut self) -> Option<ImagePair> {
        self.get(Direction::Left)
    }

    pub fn current(&mut self) -> Option<ImagePair> {
        self.get(Direction::Stay)
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
    let first_image_pair = images_bag.current().unwrap(); // there is at least one image

    let window = generate_window()?;

    // let's start by displaying something
    window.set_image(
        &first_image_pair.path_str(),
        first_image_pair.image_clone().unwrap(),
    )?;

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
                if let Some(image_pair) = images_bag.next() {
                    window.set_image(&image_pair.path_str(), image_pair.image_clone().unwrap())?;
                }
            } else if !event.is_synthetic
                && event.input.key_code == Some(event::VirtualKeyCode::Left)
                && event.input.state.is_pressed()
            {
                if let Some(image_pair) = images_bag.prev() {
                    window.set_image(&image_pair.path_str(), image_pair.image_clone().unwrap())?;
                }
            } else if !event.is_synthetic
                && event.input.key_code == Some(event::VirtualKeyCode::Key0)
                && event.input.state.is_pressed()
            {
                window.reset_image();
            }
        }
    }

    Ok(())
}
