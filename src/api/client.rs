//! The Turbodoc REST API (`api.turbodoc.ai`), one function per call. Each blocks, so callers
//! run them on `net::pool`; every one goes through [`Auth::with_token`].
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::auth::Auth;
use super::models::{Bookmark, Diagram, DigestPrefs, Note, Revision, Snippet, Stats};
use crate::net::http::{Method, NetError, Request, encode};

pub const PRODUCTION: &str = "https://api.turbodoc.ai";

/// `TURBODOC_API_URL` points the app at another deployment (`https://api-dev.turbodoc.ai`, or a
/// local worker on `http://localhost:8787`).
pub fn base() -> String {
    std::env::var("TURBODOC_API_URL").ok().filter(|s| s.starts_with("http")).map_or_else(|| PRODUCTION.to_owned(), |s| s.trim_end_matches('/').to_owned())
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

fn call<T: DeserializeOwned>(auth: &Auth, method: Method, path: &str, body: Option<Value>) -> Result<T, NetError> {
    let url = format!("{}{path}", base());
    auth.with_token(|token| {
        let mut r = Request::new(method, &url).bearer(token);
        if let Some(b) = &body {
            r = r.json(b.clone());
        }
        r.send::<T>()
    })
}

fn data<T: DeserializeOwned>(auth: &Auth, method: Method, path: &str, body: Option<Value>) -> Result<T, NetError> {
    call::<Data<T>>(auth, method, path, body).map(|d| d.data)
}

fn done(auth: &Auth, method: Method, path: &str) -> Result<(), NetError> { call::<Value>(auth, method, path, None).map(|_| ()) }

// Bookmarks

pub fn bookmarks(auth: &Auth) -> Result<Vec<Bookmark>, NetError> { data(auth, Method::Get, "/v1/bookmarks?sort=date_newest", None) }

pub fn create_bookmark(auth: &Auth, title: &str, url: &str, tags: Option<String>) -> Result<Bookmark, NetError> {
    let mut body = json!({ "title": if title.trim().is_empty() { url } else { title }, "url": url, "status": "unread" });
    if let Some(t) = tags {
        body["tags"] = json!(t);
    }
    data(auth, Method::Post, "/v1/bookmarks", Some(body))
}

/// A partial update. Leave `url` out unless it changed: sending it re-crawls and re-summarises.
pub fn update_bookmark(auth: &Auth, id: &str, patch: Value) -> Result<Bookmark, NetError> {
    data(auth, Method::Put, &format!("/v1/bookmarks/{id}"), Some(patch))
}

pub fn delete_bookmark(auth: &Auth, id: &str) -> Result<(), NetError> { done(auth, Method::Delete, &format!("/v1/bookmarks/{id}")) }

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OgImage {
    #[serde(rename = "ogImage")]
    pub og_image: Option<String>,
    pub title: Option<String>,
}

pub fn og_image(auth: &Auth, url: &str) -> Result<OgImage, NetError> {
    call(auth, Method::Get, &format!("/v1/bookmarks/og-image?url={}", encode(url)), None)
}

// Notes (v2 documents: revisions and three-way merges)

pub fn notes(auth: &Auth) -> Result<Vec<Note>, NetError> { data(auth, Method::Get, "/v2/documents", None) }

pub fn note(auth: &Auth, id: &str) -> Result<Note, NetError> { data(auth, Method::Get, &format!("/v2/documents/{id}"), None) }

/// Creates a note with an id made here, so a retried create can't make two.
pub fn create_note(auth: &Auth, id: &str, title: &str, markdown: &str, tags: Option<String>, device: &str) -> Result<Note, NetError> {
    let body = json!({ "id": id, "title": title, "markdown": markdown, "tags": tags, "device_id": device });
    data(auth, Method::Post, "/v2/documents", Some(body))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub data: Note,
    pub merged: bool,
    pub conflicts: i64,
    pub submitted_revision_id: Option<String>,
}

/// Saves a draft made on top of `base` (the revision the editor last agreed with). The server
/// merges it with anything saved elsewhere since.
pub fn save_note(auth: &Auth, id: &str, base: Option<&str>, title: &str, markdown: &str, tags: Option<String>, device: &str) -> Result<Saved, NetError> {
    let body = json!({ "title": title, "markdown": markdown, "tags": tags, "base_revision_id": base, "device_id": device });
    call(auth, Method::Put, &format!("/v2/documents/{id}"), Some(body))
}

pub fn patch_note(auth: &Auth, id: &str, patch: Value) -> Result<Saved, NetError> { call(auth, Method::Put, &format!("/v2/documents/{id}"), Some(patch)) }

pub fn delete_note(auth: &Auth, id: &str) -> Result<(), NetError> { done(auth, Method::Delete, &format!("/v2/documents/{id}")) }

pub fn revisions(auth: &Auth, id: &str) -> Result<Vec<Revision>, NetError> { data(auth, Method::Get, &format!("/v2/documents/{id}/revisions"), None) }

pub fn restore_revision(auth: &Auth, id: &str, revision: &str) -> Result<Note, NetError> {
    data(auth, Method::Post, &format!("/v2/documents/{id}/revisions/{revision}/restore"), None)
}

// Code snippets

pub fn snippets(auth: &Auth) -> Result<Vec<Snippet>, NetError> { data(auth, Method::Get, "/v1/code-snippets", None) }

pub fn create_snippet(auth: &Auth, title: &str, code: &str, language: &str) -> Result<Snippet, NetError> {
    data(auth, Method::Post, "/v1/code-snippets", Some(json!({ "title": title, "code": code, "language": language })))
}

pub fn update_snippet(auth: &Auth, id: &str, patch: Value) -> Result<Snippet, NetError> {
    data(auth, Method::Put, &format!("/v1/code-snippets/{id}"), Some(patch))
}

pub fn delete_snippet(auth: &Auth, id: &str) -> Result<(), NetError> { done(auth, Method::Delete, &format!("/v1/code-snippets/{id}")) }

// Diagrams

pub fn diagrams(auth: &Auth) -> Result<Vec<Diagram>, NetError> { data(auth, Method::Get, "/v1/diagrams", None) }

pub fn create_diagram(auth: &Auth, title: &str, mermaid: Option<&str>) -> Result<Diagram, NetError> {
    let body = match mermaid {
        Some(src) => json!({ "title": title, "diagram_type": "mermaid", "mermaid_source": src, "shapes": [], "connections": [] }),
        None => json!({ "title": title, "diagram_type": "canvas", "shapes": [], "connections": [] }),
    };
    data(auth, Method::Post, "/v1/diagrams", Some(body))
}

pub fn update_diagram(auth: &Auth, id: &str, patch: Value) -> Result<Diagram, NetError> {
    data(auth, Method::Put, &format!("/v1/diagrams/{id}"), Some(patch))
}

pub fn duplicate_diagram(auth: &Auth, id: &str) -> Result<Diagram, NetError> { data(auth, Method::Post, &format!("/v1/diagrams/{id}/duplicate"), None) }

pub fn delete_diagram(auth: &Auth, id: &str) -> Result<(), NetError> { done(auth, Method::Delete, &format!("/v1/diagrams/{id}")) }

// Account

pub fn stats(auth: &Auth) -> Result<Stats, NetError> { data(auth, Method::Get, "/v1/users/stats", None) }

pub fn digest(auth: &Auth) -> Result<DigestPrefs, NetError> { data(auth, Method::Get, "/v1/digest/preferences", None) }

pub fn set_digest(auth: &Auth, patch: Value) -> Result<DigestPrefs, NetError> { data(auth, Method::Put, "/v1/digest/preferences", Some(patch)) }

/// A random (v4) UUID, for ids made on this device.
pub fn uuid() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

#[cfg(test)]
mod tests {
    #[test]
    fn uuids_are_v4() {
        let u = super::uuid();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
        assert_ne!(u, super::uuid());
    }
}
