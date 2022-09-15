use image::{DynamicImage, ImageError};
use log::debug;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use threadpool::ThreadPool;

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
    Exit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileStatus<T, E = String> {
    Unread,
    Reading,
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

impl<T, E> FileStatus<T, E> {
    pub const fn as_ref(&self) -> FileStatus<&T, &E> {
        match self {
            FileStatus::Read(x) => FileStatus::Read(x),
            FileStatus::Unread => FileStatus::Unread,
            FileStatus::Reading => FileStatus::Reading,
            FileStatus::Err(x) => FileStatus::Err(x),
        }
    }
}

pub fn start_file_reader(
    paths: Vec<PathBuf>,
    start_idx: usize,
    cache_side_max_length: usize,
    rx: std::sync::mpsc::Receiver<Direction>,
    tx: std::sync::mpsc::Sender<Result<Option<DynamicImage>, String>>,
) {
    // TODO let's start by storing every loaded image, we'll later find a way
    // to drop some of them
    let cache: Arc<RwLock<Vec<FileStatus<DynamicImage>>>> =
        Arc::new(RwLock::new(vec![FileStatus::Unread; paths.len()]));

    let n_workers = 4;
    let pool = ThreadPool::new(n_workers);

    debug!("start_file_reader");

    // immediately load the first image
    {
        //let maybe_image_bytes = read_to_end(&paths[start_idx]);
        let maybe_image = image::open(&paths[start_idx]);
        let mut c = cache.write().unwrap();
        c[start_idx] = FileStatus::from(maybe_image);
    }

    let mut idx = start_idx;

    loop {
        idx = match rx.recv().unwrap() {
            Direction::Stay => idx,
            Direction::Left if idx > 0 => idx - 1,
            Direction::Right if idx < paths.len() - 1 => idx + 1,
            Direction::Left | Direction::Right => {
                tx.send(Ok(None)).unwrap();
                continue;
            }
            Direction::Exit => break,
        };

        debug!("Got a request to load idx {}", idx);

        if cache.read().unwrap()[idx] == FileStatus::Unread {
            debug!(
                "FILE NOT FOUND, load it now {}: {}",
                idx,
                &paths[idx].to_string_lossy()
            );
            //let maybe_image_bytes = read_to_end(&paths[idx]);
            let maybe_image = image::open(&paths[start_idx]);
            cache.write().unwrap()[idx] = FileStatus::from(maybe_image);
        } else {
            while cache.read().unwrap()[idx] == FileStatus::Reading {
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        {
            // now the file is either Read or Err
            let c = cache.read().unwrap();
            if let FileStatus::Read(v) = &c[idx] {
                debug!("I have the file, cloning");
                let some_clone = v.clone();
                debug!("Cloned. Sending it back to main thread");
                tx.send(Ok(Some(some_clone))).unwrap();
            } else if let FileStatus::Err(e) = &c[idx] {
                tx.send(Err(e.to_string())).unwrap();
            } else {
                panic!("Invariant error. File not loaded");
            }
        }

        let paths_ref = &paths;
        let tmp_range = suggested_items_to_cache(idx, paths.len(), cache_side_max_length);
        dbg!(&tmp_range);
        for some_idx in tmp_range {
            let c = cache.clone(); // Arc

            debug!(
                "should preload idx? {}: {}",
                some_idx,
                &paths[some_idx].to_string_lossy()
            );

            if c.read().unwrap()[some_idx] == FileStatus::Unread {
                let f_path = paths_ref[some_idx].clone();

                debug!("YES! spawn thread for idx {}", some_idx);

                pool.execute(move || {
                    {
                        let mut rw_lock = c.write().unwrap();
                        let c_rw = &mut rw_lock[some_idx];

                        // After the RW lock, is it still unread or are we too late?
                        if *c_rw != FileStatus::Unread {
                            return;
                        }

                        *c_rw = FileStatus::Reading;
                    }

                    //let maybe_image_bytes = read_to_end(&f_path);
                    let maybe_image = image::open(&f_path);
                    c.write().unwrap()[some_idx] = FileStatus::from(maybe_image);
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
    if len < cache_side_max_length * 2 {
        // ff the len is less than twice the cache side, we can return
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
        // x-|--x- e.g. if idx (|) is 2, we expect bounds (x) to be [0, 5)

        assert_eq!(0..1, suggested_items_to_cache(0, 1, side_cache_length));
        assert_eq!(0..5, suggested_items_to_cache(0, len, side_cache_length));
        assert_eq!(0..5, suggested_items_to_cache(1, len, side_cache_length));
        assert_eq!(0..5, suggested_items_to_cache(2, len, side_cache_length));
        assert_eq!(1..6, suggested_items_to_cache(3, len, side_cache_length));
        assert_eq!(1..6, suggested_items_to_cache(4, len, side_cache_length));
        assert_eq!(1..6, suggested_items_to_cache(5, len, side_cache_length));
        assert_eq!(3..14, suggested_items_to_cache(9, 14, 5));
    }
}
