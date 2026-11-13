//! Bookmark preview images. The API finds a page's `og:image` on request and stores nothing, so
//! the app remembers each answer (in the library snapshot) and keeps the image bytes in the
//! cache folder, bounded in count and bytes. Fetches run on the media lane; decoding runs on
//! Rune's image worker. Only cards that scroll into view ask.
use std::collections::HashMap;
use std::path::PathBuf;

use rune::{Image, ImageCache};
use sha2::{Digest, Sha256};

/// The most bytes one image may have (encoded).
const MAX_BYTES: u64 = 6 * 1024 * 1024;
/// The disk cache stays under this; the oldest files go first.
const DISK_BUDGET: u64 = 256 * 1024 * 1024;
/// Decoded images held in memory at once (each card's thumbnail is small once mipped).
const MEMORY_ENTRIES: usize = 400;

/// Where a bookmark's preview stands.
#[derive(Clone)]
pub enum Preview {
    /// Asked for; the answer hasn't come.
    Loading,
    /// The page has no preview image (or it failed): the card draws its own.
    None,
    Ready(Image),
}

#[derive(Default)]
pub struct Previews {
    pub cache: Option<ImageCache>,
    /// By bookmark URL.
    pub state: HashMap<String, Preview>,
    /// The `og:image` URL for each bookmark URL, as the API answered.
    pub known: HashMap<String, Option<String>>,
    order: Vec<String>,
}

impl Previews {
    pub fn get(&self, url: &str) -> Option<&Preview> { self.state.get(url) }

    /// Marks `url` as asked for; false when it already was.
    pub fn begin(&mut self, url: &str) -> bool {
        if self.state.contains_key(url) {
            return false;
        }
        self.state.insert(url.to_owned(), Preview::Loading);
        true
    }

    pub fn finish(&mut self, url: String, bytes: Option<Vec<u8>>) {
        let preview = match (bytes, &self.cache) {
            (Some(b), Some(cache)) => Preview::Ready(cache.load(b)),
            _ => Preview::None,
        };
        self.order.retain(|u| u != &url);
        self.order.push(url.clone());
        self.state.insert(url, preview);
        while self.order.len() > MEMORY_ENTRIES {
            let old = self.order.remove(0);
            self.state.remove(&old);
        }
    }
}

fn dir() -> Option<PathBuf> { Some(rune::dirs::AppDirs::new(crate::APP_ID)?.cache().join("previews")) }

fn file_for(image_url: &str) -> Option<PathBuf> {
    let hash = Sha256::digest(image_url.as_bytes());
    let name: String = hash.iter().take(16).map(|b| format!("{b:02x}")).collect();
    Some(dir()?.join(name))
}

/// The image bytes for `image_url`: from disk if fetched before, else from the network (and
/// then saved). Runs on a worker.
pub fn bytes(image_url: &str) -> Option<Vec<u8>> {
    let file = file_for(image_url);
    if let Some(f) = &file
        && let Ok(b) = std::fs::read(f)
    {
        return (!b.is_empty()).then_some(b);
    }
    if !image_url.starts_with("http") {
        return None;
    }
    let b = crate::net::http::get_bytes(image_url, MAX_BYTES).ok()?;
    if let Some(f) = file {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&f, &b);
    }
    Some(b)
}

/// Trims the disk cache to its budget, oldest first. Runs once a launch, on a worker.
pub fn trim() {
    crate::net::pool::spawn_bulk(|| {
        let Some(d) = dir() else { return };
        let Ok(entries) = std::fs::read_dir(&d) else { return };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((m.modified().ok()?, m.len(), e.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.0);
        for (_, len, path) in files {
            if total <= DISK_BUDGET {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total -= len;
            }
        }
    });
}
