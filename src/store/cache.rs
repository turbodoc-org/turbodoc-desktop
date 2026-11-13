//! The library as it was last synced, kept per account in the app's data folder, so the window
//! opens with everything on screen before the network answers, and keeps working offline.
//! Reads happen once at sign-in; writes go to a worker, never the UI thread.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::api::models::{Bookmark, Diagram, Note, Snippet, Stats};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub bookmarks: Vec<Bookmark>,
    pub notes: Vec<Note>,
    pub snippets: Vec<Snippet>,
    pub diagrams: Vec<Diagram>,
    pub stats: Option<Stats>,
    /// Preview image URL per bookmark URL (`None`: the page has none).
    pub og: Vec<(String, Option<String>)>,
    /// Unix seconds of the last complete sync.
    pub synced_at: i64,
}

fn path(user: &str) -> Option<PathBuf> {
    let safe: String = user.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    if safe.is_empty() {
        return None;
    }
    Some(rune::dirs::AppDirs::new(crate::APP_ID)?.data().join("accounts").join(safe).join("library.json"))
}

pub fn load(user: &str) -> Option<Library> {
    let bytes = std::fs::read(path(user)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Writes the snapshot through a temporary file and a rename, on a worker.
pub fn save(user: &str, lib: Library) {
    let Some(path) = path(user) else { return };
    crate::net::pool::spawn_bulk(move || {
        let Some(dir) = path.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let Ok(json) = serde_json::to_vec(&lib) else { return };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    });
}

pub fn forget(user: &str) {
    if let Some(dir) = path(user).and_then(|p| p.parent().map(PathBuf::from)) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
