use image::{DynamicImage, ImageError};
use log::{debug, error};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use threadpool::ThreadPool;

#[derive(Clone, PartialEq)]
pub struct ImagePair(pub PathBuf, pub Option<DynamicImage>);

impl ImagePair {
    pub fn path_str(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }

    pub fn image_clone(&self) -> Option<DynamicImage> {
        self.1.clone()
    }

    pub fn image(self) -> Option<DynamicImage> {
        self.1
    }
}

pub fn read_image(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut f = File::open(path)?;
        f.read_to_end(&mut buffer)?;
    }

    Ok(buffer)
}

#[derive(Debug)]
pub enum Direction {
    Left,
    Right,
    Stay,
    First,
    Last,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileStatus<T, E = String> {
    Unread,
    Reading,
    Thumbnail(T),
    Read(T),
    Err(E),
}

impl<T> From<std::io::Result<T>> for FileStatus<T> {
    fn from(res: std::io::Result<T>) -> Self {
        match res {
            Ok(x) => FileStatus::Read(x),
            Err(x) => FileStatus::Err(x.to_string()),
        }
    }
}

impl From<std::result::Result<DynamicImage, ImageError>> for FileStatus<DynamicImage> {
    fn from(res: std::result::Result<DynamicImage, ImageError>) -> Self {
        match res {
            Ok(x) => FileStatus::Read(x),
            Err(x) => FileStatus::Err(x.to_string()),
        }
    }
}

impl From<(PathBuf, std::result::Result<DynamicImage, ImageError>)> for FileStatus<ImagePair> {
    fn from((p, res): (PathBuf, std::result::Result<DynamicImage, ImageError>)) -> Self {
        match res {
            Ok(x) => FileStatus::Read(ImagePair(p, Some(clamp_image_size(x)))),
            Err(x) => FileStatus::Err(x.to_string()),
        }
    }
}

/// Generate a small thumbnail for fast preview display.
/// Returns None if the image is already small enough to serve as its own thumbnail.
fn generate_thumbnail(img: &DynamicImage, max_dim: u32) -> Option<DynamicImage> {
    let (w, h) = (img.width(), img.height());
    if w <= max_dim && h <= max_dim {
        return None;
    }
    Some(img.thumbnail(max_dim, max_dim))
}

/// Downscale an image if its raw buffer would exceed the wgpu max_storage_buffer_binding_size (128 MiB).
fn clamp_image_size(img: DynamicImage) -> DynamicImage {
    // wgpu max_storage_buffer_binding_size is 128 MiB. Each pixel is at most 4 bytes (RGBA8).
    const MAX_PIXELS: u32 = 134_217_728 / 4;
    let (w, h) = (img.width(), img.height());
    let pixels = w as u64 * h as u64;
    if pixels <= MAX_PIXELS as u64 {
        return img;
    }
    let scale = ((MAX_PIXELS as f64) / (pixels as f64)).sqrt();
    let new_w = (w as f64 * scale) as u32;
    let new_h = (h as f64 * scale) as u32;
    eprintln!(
        "Image {}x{} too large for GPU, downscaling to {}x{}",
        w, h, new_w, new_h
    );
    img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
}

impl<T, E> FileStatus<T, E> {
    pub const fn as_ref(&self) -> FileStatus<&T, &E> {
        match self {
            FileStatus::Read(x) => FileStatus::Read(x),
            FileStatus::Thumbnail(x) => FileStatus::Thumbnail(x),
            FileStatus::Unread => FileStatus::Unread,
            FileStatus::Reading => FileStatus::Reading,
            FileStatus::Err(x) => FileStatus::Err(x),
        }
    }
}

pub fn start_file_reader(
    cache: Arc<RwLock<Vec<FileStatus<ImagePair>>>>,
    paths: Vec<PathBuf>,
    start_idx: usize,
    cache_side_max_length: usize,
    rx: std::sync::mpsc::Receiver<Option<usize>>,
    tx: std::sync::mpsc::Sender<Result<Option<usize>, String>>,
    //wakeup: impl Fn() -> (),
    w: show_image::WindowProxy,
) {
    // TODO let's start by storing every loaded image, we'll later find a way
    // to drop some of them

    let n_workers = 4;
    let pool = ThreadPool::new(n_workers);

    debug!("start_file_reader");

    // immediately load the first image
    {
        let maybe_image = image::open(&paths[start_idx]);
        let mut c = cache.write().unwrap();
        c[start_idx] = FileStatus::from((paths[start_idx].clone(), maybe_image));
    }

    let mut idx: usize;
    let mut pending_idx: Option<usize> = None;

    'outer: loop {
        // Get latest idx. Use pending_idx if we have one from a previous
        // iteration (user moved on while we were showing a thumbnail).
        if let Some(pending) = pending_idx.take() {
            idx = pending;
        } else {
            // Blocking recv to not starve the CPU.
            match rx.recv() {
                Ok(Some(next_idx)) => idx = next_idx,
                Ok(None) => break 'outer,
                Err(e) => {
                    error!("Failed to read what image (idx) to load. Error was: {}", e);
                    break 'outer;
                }
            }
        }
        // Drain channel to keep only the most recent request.
        loop {
            match rx.try_recv() {
                Ok(Some(next_idx)) => idx = next_idx,
                Ok(None) => break 'outer,
                Err(_) => break,
            }
        }

        debug!("Got a request to load idx {}", idx);

        // Helper: decode an image, store thumbnail first, then full quality.
        // Returns the decoded image for display.
        let display_image = |idx: usize, cache: &Arc<RwLock<Vec<FileStatus<ImagePair>>>>| {
            let status = cache.read().unwrap()[idx].clone();
            match status {
                FileStatus::Read(ref k) | FileStatus::Thumbnail(ref k) => {
                    Some(k.image_clone().unwrap())
                }
                FileStatus::Err(_) => None,
                FileStatus::Reading => {
                    // Wait for pool to finish
                    loop {
                        let s = cache.read().unwrap()[idx].clone();
                        match s {
                            FileStatus::Reading => {
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            FileStatus::Read(k) | FileStatus::Thumbnail(k) => {
                                return Some(k.image_clone().unwrap());
                            }
                            _ => return None,
                        }
                    }
                }
                FileStatus::Unread => None,
            }
        };

        // Try to display from cache first
        if let Some(img) = display_image(idx, &cache) {
            let _ = w.set_image("", img);
            tx.send(Ok(Some(idx))).unwrap();
        } else {
            // Not cached at all — decode now
            debug!(
                "Image not cached, loading {}: {}",
                idx,
                &paths[idx].to_string_lossy()
            );
            cache.write().unwrap()[idx] = FileStatus::Reading;
            match image::open(&paths[idx]) {
                Ok(img) => {
                    // For large images, show a fast thumbnail first
                    const THUMB_MAX: u32 = 512;
                    if let Some(thumb) = generate_thumbnail(&img, THUMB_MAX) {
                        cache.write().unwrap()[idx] = FileStatus::Thumbnail(ImagePair(
                            paths[idx].clone(),
                            Some(thumb.clone()),
                        ));
                        let _ = w.set_image("", thumb);
                        tx.send(Ok(Some(idx))).unwrap();

                        // Before the expensive Lanczos3 clamp, check if user moved on
                        if let Ok(msg) = rx.try_recv() {
                            // Offload full-quality processing to pool
                            let c = cache.clone();
                            let p = paths[idx].clone();
                            let captured_idx = idx;
                            pool.execute(move || {
                                let clamped = clamp_image_size(img);
                                c.write().unwrap()[captured_idx] =
                                    FileStatus::Read(ImagePair(p, Some(clamped)));
                            });

                            match msg {
                                Some(next_idx) => {
                                    pending_idx = Some(next_idx);
                                    continue 'outer;
                                }
                                None => break 'outer,
                            }
                        }
                    }

                    // Small image or user stayed — do full clamp and display
                    let clamped = clamp_image_size(img);
                    cache.write().unwrap()[idx] =
                        FileStatus::Read(ImagePair(paths[idx].clone(), Some(clamped.clone())));
                    let _ = w.set_image("", clamped);
                    tx.send(Ok(Some(idx))).unwrap();
                }
                Err(e) => {
                    cache.write().unwrap()[idx] = FileStatus::Err(e.to_string());
                    tx.send(Err(e.to_string())).unwrap();
                }
            }
        }

        // If the displayed image was only a thumbnail, upgrade to full quality
        if matches!(cache.read().unwrap()[idx], FileStatus::Thumbnail(_)) {
            match image::open(&paths[idx]) {
                Ok(img) => {
                    let clamped = clamp_image_size(img);
                    cache.write().unwrap()[idx] =
                        FileStatus::Read(ImagePair(paths[idx].clone(), Some(clamped.clone())));
                    let _ = w.set_image("", clamped);
                    tx.send(Ok(Some(idx))).unwrap();
                }
                Err(e) => {
                    cache.write().unwrap()[idx] = FileStatus::Err(e.to_string());
                }
            }
        }

        let tmp_range = suggested_items_to_cache(idx, paths.len(), cache_side_max_length);

        for some_idx in tmp_range {
            let c = cache.clone();

            if c.read().unwrap()[some_idx] == FileStatus::Unread {
                let f_path = paths[some_idx].clone();

                debug!("preload img {:?}", f_path);

                pool.execute(move || {
                    {
                        let mut rw_lock = c.write().unwrap();
                        let c_rw = &mut rw_lock[some_idx];

                        if *c_rw != FileStatus::Unread {
                            return;
                        }

                        *c_rw = FileStatus::Reading;
                    }

                    let maybe_image = image::open(&f_path);
                    c.write().unwrap()[some_idx] = FileStatus::from((f_path.clone(), maybe_image));
                });
            }
        }
    }
}

pub fn suggested_items_to_cache(
    idx: usize,
    len: usize,
    cache_side_max_length: usize,
) -> std::ops::Range<usize> {
    if len <= cache_side_max_length * 2 {
        // if the len is less than twice the cache side, we can return
        // it as the region to cache
        0..len
    } else if idx + cache_side_max_length > len - 1 {
        // ask too much on right side
        idx - cache_side_max_length - (cache_side_max_length - (len - 1 - idx))..len
    } else if cache_side_max_length > idx {
        // ask too much on left side
        0..cache_side_max_length + 1 + cache_side_max_length
    } else {
        // contained
        idx - cache_side_max_length..idx + cache_side_max_length + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggested_items_to_cache() {
        let len = 6;
        let side_cache_length = 2;

        // 0123456
        // x-|--x- e.g. if idx (|) is 2 and len is 2, we expect bounds (x) to be [0, 5)

        assert_eq!(0..1, suggested_items_to_cache(0, 1, side_cache_length));
        assert_eq!(0..5, suggested_items_to_cache(0, len, side_cache_length));
        assert_eq!(0..5, suggested_items_to_cache(1, len, side_cache_length));
        assert_eq!(0..5, suggested_items_to_cache(2, len, side_cache_length));
        assert_eq!(1..6, suggested_items_to_cache(3, len, side_cache_length));
        assert_eq!(1..6, suggested_items_to_cache(4, len, side_cache_length));
        assert_eq!(1..6, suggested_items_to_cache(5, len, side_cache_length));
        assert_eq!(3..14, suggested_items_to_cache(9, 14, 5));
        assert_eq!(0..10, suggested_items_to_cache(0, 10, 5));
    }
}
