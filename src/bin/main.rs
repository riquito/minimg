use anyhow::{anyhow, Result};
use image::DynamicImage;
use log::debug;
use minimg::fs_utils::{start_file_reader, Direction, FileStatus};
use minimg::window::generate_window;
use show_image::event;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

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
    data: Vec<ImagePair>,
    current: usize,
    _thread_handle: Option<JoinHandle<()>>,
}
impl ImagesBag {
    pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
        if paths.is_empty() {
            return Err(anyhow!("The list of images to read cannot be empty"));
        }

        Ok(ImagesBag {
            data: paths.into_iter().map(|x| ImagePair(x, None)).collect(),
            current: 0,
            _thread_handle: None,
        })
    }

    pub fn preload(&mut self, n: usize) -> Result<()> {
        let from = 0;
        // load all the images at once! (step 1-bis, make it work...)
        let data: &mut Vec<ImagePair> = self.data.as_mut();
        for ImagePair(path, wannabe_img) in data.iter_mut().skip(from).take(n) {
            let image = image::open(&path)
                .map_err(|e| anyhow!("Failed to read image from {:?}: {}", path, e))?;
            *wannabe_img = Some(image);
        }

        Ok(())
    }

    pub fn get(&mut self, idx: usize) -> Option<&ImagePair> {
        if idx + 5 > self.data.len() {
            let paths: Vec<(usize, PathBuf)> = self
                .data
                .iter()
                .map(|ip| ip.0.clone())
                .enumerate()
                .collect();
        }

        if let Some(image_pair) = self.data.get(idx) {
            self.current = idx;
            return Some(image_pair);
        }
        None
    }

    pub fn next(&mut self) -> Option<&ImagePair> {
        self.get(self.current + 1)
    }

    pub fn prev(&mut self) -> Option<&ImagePair> {
        if self.current == 0 {
            return None;
        }
        self.get(self.current - 1)
    }
}

struct ImagesBag2 {
    tx_d: Sender<Direction>,
    rx_f: Receiver<Result<DynamicImage, String>>,
    _thread_handle: JoinHandle<()>,
}
impl ImagesBag2 {
    pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
        if paths.is_empty() {
            return Err(anyhow!("The list of images to read cannot be empty"));
        }

        let (tx_f, rx_f) = channel::<Result<DynamicImage, String>>();
        let (tx_d, rx_d) = channel::<Direction>();
        let _thread_handle = std::thread::spawn(move || start_file_reader(paths, 0, 5, rx_d, tx_f));

        Ok(ImagesBag2 {
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

    let mut images_bag = ImagesBag2::new(paths)?;
    // images_bag.preload(10)?;
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
            }
        }
    }

    Ok(())
}
