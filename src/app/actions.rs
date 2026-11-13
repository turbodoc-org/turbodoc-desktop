//! What people do: navigate, select, create, edit, delete. Changes show at once (optimistic) and
//! roll back with a message if the server refuses them. Deletes wait out an Undo window first.
use rune::{TextEdit, Tx};
use serde_json::json;

use super::editors::{Draft, Save};
use super::{App, BookmarkForm, Dialog, DiagramEditor, Item, NoteEditor, PendingDelete, Section, SnippetEditor, ToastKind, Toast, UNDO_SECONDS};
use crate::api::client;
use crate::api::models::{Note, join_tags, parse_tag_input, tags};
use crate::format;
use crate::store::images::{self, Preview};

impl App {
    // Navigation

    pub fn go(&mut self, section: Section, tx: &mut Tx) {
        self.settings = false;
        if self.section == section {
            return;
        }
        self.leave_editors(tx);
        self.section = section;
        self.selected = None;
        self.search.clear();
        if section != Section::Bookmarks && section != Section::Notes {
            self.tag = None;
        }
        self.set_pref("section", &section.id());
        match section {
            Section::Notes => {
                if let Some(n) = self.visible_notes().first().map(|n| n.id.clone()) {
                    self.open_note(&n, tx);
                }
            }
            Section::Snippets => {
                if let Some(id) = self.lib.snippets.first().map(|s| s.id.clone()) {
                    self.open_snippet(&id);
                }
            }
            Section::Diagrams => {
                if let Some(id) = self.lib.diagrams.first().map(|d| d.id.clone()) {
                    self.open_diagram(&id);
                }
            }
            _ => {}
        }
    }

    /// Saves whatever is pending before an editor closes.
    fn leave_editors(&mut self, tx: &mut Tx) {
        if self.note.as_ref().is_some_and(|e| matches!(e.save, Save::Pending { .. } | Save::Failed { .. })) {
            self.save_note(tx);
        }
        if self.snippet.as_ref().is_some_and(|e| matches!(e.save, Save::Pending { .. })) {
            self.save_snippet(tx);
        }
        if self.diagram.as_ref().is_some_and(|e| matches!(e.save, Save::Pending { .. })) {
            self.save_diagram(tx);
        }
    }

    pub fn filter_tag(&mut self, tag: Option<String>, tx: &mut Tx) {
        if !matches!(self.section, Section::Bookmarks | Section::Notes) {
            self.go(Section::Bookmarks, tx);
        }
        self.settings = false;
        self.tag = if self.tag == tag { None } else { tag };
    }

    pub fn visible_notes(&self) -> Vec<&Note> {
        let q = self.search.text.trim();
        let fold = rune_kit::search::Fold::default();
        let mut v: Vec<&Note> = self
            .lib
            .notes
            .iter()
            .filter(|n| self.tag.as_ref().is_none_or(|t| n.tag_list().iter().any(|x| x.eq_ignore_ascii_case(t))))
            .filter(|n| q.is_empty() || fold.contains(&n.title, q) || fold.contains(&n.markdown, q))
            .collect();
        v.sort_by(|a, b| b.favorite().cmp(&a.favorite()).then_with(|| b.updated_at.cmp(&a.updated_at)));
        v
    }

    // Bookmarks

    pub fn select_bookmark(&mut self, id: &str) {
        self.selected = Some(id.to_owned());
        if !self.inspector.is_open() {
            self.inspector.toggle();
        }
    }

    pub fn open_bookmark(&mut self, id: &str, tx: &mut Tx) {
        let Some(b) = self.lib.bookmarks.iter().find(|b| b.id == id).cloned() else { return };
        let _ = tx.open_url(&b.url);
        if b.unread() {
            self.patch_bookmark(id, json!({ "status": "read" }), tx);
        }
    }

    /// Applies `patch` here at once and on the server, reverting if it fails.
    pub fn patch_bookmark(&mut self, id: &str, patch: serde_json::Value, tx: &mut Tx) {
        let Some(i) = self.lib.bookmarks.iter().position(|b| b.id == id) else { return };
        let before = self.lib.bookmarks[i].clone();
        let mut v = serde_json::to_value(&before).unwrap_or_default();
        if let (Some(obj), Some(p)) = (v.as_object_mut(), patch.as_object()) {
            for (k, val) in p {
                obj.insert(k.clone(), val.clone());
            }
        }
        if let Ok(after) = serde_json::from_value(v) {
            self.lib.bookmarks[i] = after;
        }
        let id = id.to_owned();
        let _ = tx;
        self.run(
            move |a| client::update_bookmark(a, &id, patch),
            move |s, r, tx| match r {
                Ok(b) => {
                    if let Some(slot) = s.lib.bookmarks.iter_mut().find(|x| x.id == b.id) {
                        *slot = b;
                    }
                    s.save_snapshot();
                }
                Err(e) => {
                    if let Some(slot) = s.lib.bookmarks.iter_mut().find(|x| x.id == before.id) {
                        *slot = before;
                    }
                    s.fail(&e, "Couldn't update the bookmark", tx);
                }
            },
        );
    }

    pub fn toggle_favorite_bookmark(&mut self, id: &str, tx: &mut Tx) {
        let fav = self.lib.bookmarks.iter().find(|b| b.id == id).is_some_and(|b| b.favorite());
        self.patch_bookmark(id, json!({ "is_favorite": !fav }), tx);
    }

    pub fn toggle_read(&mut self, id: &str, tx: &mut Tx) {
        let unread = self.lib.bookmarks.iter().find(|b| b.id == id).is_some_and(|b| b.unread());
        self.patch_bookmark(id, json!({ "status": if unread { "read" } else { "unread" } }), tx);
    }

    pub fn toggle_archive(&mut self, id: &str, tx: &mut Tx) {
        let archived = self.lib.bookmarks.iter().find(|b| b.id == id).is_some_and(|b| b.archived());
        self.patch_bookmark(id, json!({ "status": if archived { "unread" } else { "archived" } }), tx);
        if !archived {
            self.toast(ToastKind::Info, "Archived", tx);
        }
    }

    pub fn new_bookmark_dialog(&mut self, url: Option<String>) {
        let mut form = BookmarkForm { id: None, url: TextEdit::new(url.clone().unwrap_or_default()), title: TextEdit::new(""), tags: TextEdit::new(""), busy: false, error: None };
        if url.is_some() { form.title.focused = true } else { form.url.focused = true }
        self.palette = None;
        self.dialog = Some(Dialog::Bookmark(form));
    }

    pub fn edit_bookmark_dialog(&mut self, id: &str) {
        let Some(b) = self.lib.bookmarks.iter().find(|b| b.id == id) else { return };
        let mut title = TextEdit::new(b.title.clone());
        title.focused = true;
        self.dialog = Some(Dialog::Bookmark(BookmarkForm {
            id: Some(b.id.clone()),
            url: TextEdit::new(b.url.clone()),
            title,
            tags: TextEdit::new(b.tag_list().join(", ")),
            busy: false,
            error: None,
        }));
    }

    pub fn submit_bookmark(&mut self, tx: &mut Tx) {
        let Some(Dialog::Bookmark(form)) = &mut self.dialog else { return };
        let raw = form.url.text.trim().to_owned();
        if !format::looks_like_url(&raw) {
            form.error = Some("Enter a web address, like turbodoc.ai/docs.".into());
            return;
        }
        let url = format::normalize_url(&raw);
        let title = form.title.text.trim().to_owned();
        let tags = join_tags(&parse_tag_input(&form.tags.text));
        match form.id.clone() {
            Some(id) => {
                let original = self.lib.bookmarks.iter().find(|b| b.id == id).cloned().unwrap_or_default();
                let mut patch = json!({ "title": if title.is_empty() { url.clone() } else { title }, "tags": tags });
                if original.url != url {
                    patch["url"] = json!(url); // only when it changed: a new URL is crawled again
                }
                self.dialog = None;
                self.patch_bookmark(&id, patch, tx);
            }
            None => {
                if let Some(existing) = self.lib.bookmarks.iter().find(|b| b.url.trim_end_matches('/') == url.trim_end_matches('/')) {
                    form.error = Some(format!("Already saved as “{}”.", existing.display_title()));
                    return;
                }
                form.busy = true;
                form.error = None;
                self.create_bookmark(url, title, tags, true, tx);
            }
        }
    }

    /// Saves a new bookmark. `from_dialog` closes the dialog on success.
    pub fn create_bookmark(&mut self, url: String, title: String, tags: Option<String>, from_dialog: bool, _tx: &mut Tx) {
        self.run(
            move |a| {
                let title = if title.is_empty() { client::og_image(a, &url).ok().and_then(|o| o.title).unwrap_or_default() } else { title };
                client::create_bookmark(a, &title, &url, tags)
            },
            move |s, r, tx| match r {
                Ok(b) => {
                    let name = b.display_title().to_owned();
                    let id = b.id.clone();
                    s.lib.bookmarks.insert(0, b);
                    if from_dialog {
                        s.dialog = None;
                    }
                    if s.section == Section::Bookmarks {
                        s.selected = Some(id);
                    }
                    s.save_snapshot();
                    s.notify_saved(&name, tx);
                }
                Err(e) => {
                    if let Some(Dialog::Bookmark(f)) = &mut s.dialog {
                        f.busy = false;
                        f.error = Some(e.to_string());
                    } else {
                        s.fail(&e, "Couldn't save the bookmark", tx);
                    }
                }
            },
        );
    }

    /// Fetches a card's preview image the first time the card shows.
    pub fn want_preview(&mut self, url: &str) {
        if self.handle.is_none() || !self.previews.begin(url) {
            return;
        }
        let known = self.previews.known.get(url).cloned();
        let url = url.to_owned();
        let Some(handle) = self.handle.clone() else { return };
        let auth = self.auth.clone();
        crate::net::pool::spawn_bulk(move || {
            let (image_url, learned) = match known {
                Some(answer) => (answer, false),
                None => match client::og_image(&auth, &url) {
                    Ok(o) => (o.og_image.filter(|u| u.starts_with("http")), true),
                    Err(_) => (None, false),
                },
            };
            let bytes = image_url.as_deref().and_then(images::bytes);
            handle.update(move |s, _| {
                if learned {
                    s.previews.known.insert(url.clone(), image_url);
                }
                s.previews.finish(url, bytes);
            });
        });
    }

    pub fn preview(&self, url: &str) -> Option<&rune::Image> {
        match self.previews.get(url) {
            Some(Preview::Ready(img)) if img.error().is_none() => Some(img),
            _ => None,
        }
    }

    // Notes

    pub fn open_note(&mut self, id: &str, tx: &mut Tx) {
        if self.note.as_ref().is_some_and(|e| e.id == id) {
            return;
        }
        self.leave_editors(tx);
        let Some(n) = self.lib.notes.iter().find(|n| n.id == id) else { return };
        self.note = Some(NoteEditor::open(n));
        self.selected = Some(id.to_owned());
        // Catch edits made on other devices since the list was fetched.
        let id = id.to_owned();
        self.run(
            move |a| client::note(a, &id),
            |s, r, _| {
                if let Ok(n) = r {
                    if let Some(ed) = &mut s.note
                        && ed.id == n.id
                    {
                        ed.adopt_remote(&n);
                    }
                    if let Some(slot) = s.lib.notes.iter_mut().find(|x| x.id == n.id) {
                        *slot = n;
                    }
                }
            },
        );
    }

    pub fn new_note(&mut self, markdown: String, tx: &mut Tx) {
        self.leave_editors(tx);
        self.settings = false;
        self.palette = None;
        let id = client::uuid();
        let title = String::new();
        let now = chrono::Utc::now().to_rfc3339();
        let note = Note { id: id.clone(), title: title.clone(), markdown: markdown.clone(), version: 0, created_at: Some(now.clone()), updated_at: Some(now), ..Default::default() };
        self.lib.notes.insert(0, note.clone());
        let mut ed = NoteEditor::open(&note);
        ed.creating = true;
        ed.title.focused = true;
        ed.save = Save::Saving { attempt: self.next(), draft: ed.draft() };
        self.note = Some(ed);
        self.section = Section::Notes;
        self.search.clear();
        self.tag = None;
        self.selected = Some(id.clone());
        let device = self.device.clone();
        self.run(
            move |a| client::create_note(a, &id, &title, &markdown, None, &device),
            |s, r, tx| match r {
                Ok(n) => s.note_saved(n, None, 0, tx),
                Err(e) => {
                    if let Some(ed) = &mut s.note {
                        ed.save = Save::Failed { message: e.to_string() };
                    }
                    s.fail(&e, "Couldn't create the note", tx);
                }
            },
        );
    }

    /// Sends the open note's draft (see `editors` for the rules).
    pub fn save_note(&mut self, tx: &mut Tx) {
        let attempt = self.next();
        let device = self.device.clone();
        let Some(ed) = &mut self.note else { return };
        if ed.save.in_flight() {
            return;
        }
        let draft = ed.draft();
        let (id, base, tags, creating) = (ed.id.clone(), ed.base.clone(), ed.stored_tags(), ed.creating);
        ed.save = Save::Saving { attempt, draft: draft.clone() };
        // The list shows the edit at once.
        if let Some(n) = self.lib.notes.iter_mut().find(|n| n.id == id) {
            n.title = draft.title.clone();
            n.markdown = draft.markdown.clone();
            n.tags = tags.clone();
            n.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        let _ = tx;
        self.run(
            move |a| {
                if creating {
                    return client::create_note(a, &id, &draft.title, &draft.markdown, tags, &device).map(|n| client::Saved { data: n, ..Default::default() });
                }
                client::save_note(a, &id, base.as_deref(), &draft.title, &draft.markdown, tags, &device)
            },
            move |s, r, tx| {
                let Some(ed) = &mut s.note else { return };
                if !matches!(&ed.save, Save::Saving { attempt: a, .. } if *a == attempt) {
                    return;
                }
                match r {
                    Ok(saved) => s.note_saved(saved.data, saved.submitted_revision_id, saved.conflicts, tx),
                    Err(e) => {
                        ed.save = Save::Failed { message: e.to_string() };
                        if !e.is_offline() {
                            s.fail(&e, "Couldn't save the note", tx);
                        } else {
                            s.sync = super::Sync::Offline { since: tx.clock() };
                        }
                    }
                }
            },
        );
    }

    fn note_saved(&mut self, n: Note, submitted: Option<String>, conflicts: i64, tx: &mut Tx) {
        let Some(ed) = &mut self.note else { return };
        if ed.id != n.id {
            return;
        }
        let sent: Option<Draft> = match &ed.save {
            Save::Saving { draft, .. } => Some(draft.clone()),
            _ => None,
        };
        ed.creating = false;
        ed.version = n.version;
        let typed_since = sent.as_ref() != Some(&ed.draft());
        if typed_since {
            // Keep what's on screen; it was made on top of what we sent.
            ed.base = submitted.or_else(|| n.head_revision_id.clone());
            ed.touch_now(tx);
        } else {
            ed.base = n.head_revision_id.clone();
            let merged = n.title != ed.title.text || n.markdown != ed.body.text;
            if merged {
                // The server merged in edits from elsewhere: show them.
                let (tf, bf) = (ed.title.focused, ed.body.focused);
                let caret = ed.body.focus;
                ed.title.replace_text(n.title.clone());
                ed.body.replace_text(n.markdown.clone());
                ed.title.focused = tf;
                ed.body.focused = bf;
                let mut at = caret.min(ed.body.text.len());
                while !ed.body.text.is_char_boundary(at) {
                    at -= 1;
                }
                ed.body.set_caret(at);
            }
            ed.save = Save::Clean;
        }
        if let Some(slot) = self.lib.notes.iter_mut().find(|x| x.id == n.id) {
            *slot = n;
        }
        if conflicts > 0 {
            self.toast(ToastKind::Info, "This note was also edited on another device. Both versions are kept.", tx);
        }
        self.save_snapshot();
    }

    pub fn toggle_favorite_note(&mut self, id: &str, tx: &mut Tx) {
        let Some(n) = self.lib.notes.iter_mut().find(|n| n.id == id) else { return };
        let fav = !n.favorite();
        n.is_favorite = Some(fav);
        let id = id.to_owned();
        let back = id.clone();
        let _ = tx;
        self.run(
            move |a| client::patch_note(a, &id, json!({ "is_favorite": fav })),
            move |s, r, tx| match r {
                Ok(saved) => {
                    let n = saved.data;
                    if let Some(ed) = &mut s.note
                        && ed.id == n.id
                        && !ed.save.unsaved()
                    {
                        ed.base = n.head_revision_id.clone();
                        ed.version = n.version;
                    }
                    if let Some(slot) = s.lib.notes.iter_mut().find(|x| x.id == n.id) {
                        slot.is_favorite = n.is_favorite;
                        slot.version = n.version;
                        slot.head_revision_id = n.head_revision_id;
                    }
                }
                Err(e) => {
                    if let Some(n) = s.lib.notes.iter_mut().find(|n| n.id == back) {
                        n.is_favorite = Some(!fav);
                    }
                    s.fail(&e, "Couldn't update the note", tx);
                }
            },
        );
    }

    /// Lists the note's saved revisions in a menu; picking one restores it as a new revision
    /// (history is never rewritten, so a restore can itself be undone from here).
    pub fn show_history(&mut self, id: &str) {
        let id = id.to_owned();
        let note = id.clone();
        self.run(
            move |a| client::revisions(a, &id),
            move |s, r, tx| match r {
                Ok(revs) if !revs.is_empty() => {
                    let mut menu = rune::PopupMenu::new();
                    for (i, r) in revs.iter().take(25).enumerate() {
                        let when = format::ago(r.created_at.as_deref().unwrap_or(""));
                        let what = match r.kind.as_str() {
                            "merge" => "Merged",
                            "restore" => "Restored",
                            _ => "Edited",
                        };
                        let from = r.device_id.as_deref().map_or("", |d| if d.starts_with("desktop") { " on desktop" } else if d.starts_with("ios") { " on iPhone" } else if d.starts_with("web") { " on the web" } else { "" });
                        let name = r.name.clone().map(|n| format!("“{n}” · ")).unwrap_or_default();
                        let title = format!("{name}{what}{from} · {when}");
                        menu = menu.item(rune::PopupItem::new(r.id.clone(), title).checked(i == 0).enabled(i != 0));
                    }
                    let note = note.clone();
                    tx.popup_menu(menu, "note-more", move |s: &mut App, picked, tx| {
                        if let Some(rev) = picked {
                            s.restore_revision(&note, rev, tx);
                        }
                    });
                }
                Ok(_) => s.toast(ToastKind::Info, "No earlier versions yet", tx),
                Err(e) => s.fail(&e, "Couldn't load the history", tx),
            },
        );
    }

    fn restore_revision(&mut self, id: &str, rev: &str, _tx: &mut Tx) {
        let (id, rev) = (id.to_owned(), rev.to_owned());
        self.run(
            move |a| client::restore_revision(a, &id, &rev),
            |s, r, tx| match r {
                Ok(n) => {
                    if let Some(ed) = &mut s.note
                        && ed.id == n.id
                    {
                        *ed = NoteEditor::open(&n);
                    }
                    if let Some(slot) = s.lib.notes.iter_mut().find(|x| x.id == n.id) {
                        *slot = n;
                    }
                    s.toast(ToastKind::Success, "Restored. The version before is still in the history.", tx);
                    s.save_snapshot();
                }
                Err(e) => s.fail(&e, "Couldn't restore that version", tx),
            },
        );
    }

    // Snippets

    pub fn open_snippet(&mut self, id: &str) {
        let Some(sn) = self.lib.snippets.iter().find(|s| s.id == id) else { return };
        self.snippet = Some(SnippetEditor::open(sn));
        self.selected = Some(id.to_owned());
    }

    pub fn new_snippet(&mut self, code: String, language: Option<String>, tx: &mut Tx) {
        self.leave_editors(tx);
        self.section = Section::Snippets;
        self.settings = false;
        self.palette = None;
        let language = language.unwrap_or_else(|| guess_language(&code).to_owned());
        let title = "Untitled snippet".to_owned();
        let code = if code.is_empty() { "// Paste or write code here\n".to_owned() } else { code };
        self.run(
            move |a| client::create_snippet(a, &title, &code, &language),
            |s, r, tx| match r {
                Ok(sn) => {
                    let id = sn.id.clone();
                    s.lib.snippets.insert(0, sn);
                    s.open_snippet(&id);
                    if let Some(ed) = &mut s.snippet {
                        ed.editing = true;
                        ed.title.focused = true;
                        ed.title.anchor = 0;
                        ed.title.focus = ed.title.text.len();
                    }
                    s.save_snapshot();
                }
                Err(e) => s.fail(&e, "Couldn't create the snippet", tx),
            },
        );
    }

    pub fn save_snippet(&mut self, _tx: &mut Tx) {
        let attempt = self.next();
        let Some(ed) = &mut self.snippet else { return };
        if ed.save.in_flight() {
            return;
        }
        let (id, title, code, language) = (ed.id.clone(), ed.title.text.clone(), ed.code.text.clone(), ed.language.clone());
        ed.save = Save::Saving { attempt, draft: Draft { title: title.clone(), markdown: code.clone(), tags: language.clone() } };
        if let Some(sn) = self.lib.snippets.iter_mut().find(|s| s.id == id) {
            sn.title = Some(title.clone());
            sn.code = code.clone();
            sn.language = language.clone();
        }
        self.run(
            move |a| client::update_snippet(a, &id, json!({ "title": title, "code": code, "language": language })),
            move |s, r, tx| {
                let Some(ed) = &mut s.snippet else { return };
                let Save::Saving { attempt: a, draft } = &ed.save else { return };
                if *a != attempt {
                    return;
                }
                let same = draft.title == ed.title.text && draft.markdown == ed.code.text && draft.tags == ed.language;
                match r {
                    Ok(sn) => {
                        ed.save = Save::Clean;
                        if !same {
                            ed.touch(tx);
                        }
                        if let Some(slot) = s.lib.snippets.iter_mut().find(|x| x.id == sn.id) {
                            slot.updated_at = sn.updated_at;
                        }
                        s.save_snapshot();
                    }
                    Err(e) => {
                        ed.save = Save::Failed { message: e.to_string() };
                        s.fail(&e, "Couldn't save the snippet", tx);
                    }
                }
            },
        );
    }

    // Diagrams

    pub fn open_diagram(&mut self, id: &str) {
        let Some(d) = self.lib.diagrams.iter().find(|d| d.id == id) else { return };
        self.diagram = Some(DiagramEditor::open(d));
        self.selected = Some(id.to_owned());
    }

    pub fn submit_new_diagram(&mut self, tx: &mut Tx) {
        let Some(Dialog::NewDiagram { title, mermaid, busy }) = &mut self.dialog else { return };
        let name = if title.text.trim().is_empty() { "Untitled Diagram".to_owned() } else { title.text.trim().to_owned() };
        *busy = true;
        let source = mermaid.then_some("flowchart LR\n  Idea --> Draft\n  Draft --> Review\n  Review -->|ship| Done\n");
        let _ = tx;
        self.run(
            move |a| client::create_diagram(a, &name, source),
            |s, r, tx| match r {
                Ok(d) => {
                    let id = d.id.clone();
                    s.lib.diagrams.insert(0, d);
                    s.dialog = None;
                    s.section = Section::Diagrams;
                    s.open_diagram(&id);
                    if let Some(ed) = &mut s.diagram {
                        ed.source.focused = true;
                    }
                    s.save_snapshot();
                }
                Err(e) => {
                    if let Some(Dialog::NewDiagram { busy, .. }) = &mut s.dialog {
                        *busy = false;
                    }
                    s.fail(&e, "Couldn't create the diagram", tx);
                }
            },
        );
    }

    pub fn save_diagram(&mut self, _tx: &mut Tx) {
        let attempt = self.next();
        let Some(ed) = &mut self.diagram else { return };
        if ed.save.in_flight() {
            return;
        }
        let (id, title, source) = (ed.id.clone(), ed.title.text.clone(), ed.source.text.clone());
        let mermaid = self.lib.diagrams.iter().find(|d| d.id == id).is_some_and(|d| d.is_mermaid());
        ed.save = Save::Saving { attempt, draft: Draft { title: title.clone(), markdown: source.clone(), tags: String::new() } };
        if let Some(d) = self.lib.diagrams.iter_mut().find(|d| d.id == id) {
            d.title = title.clone();
            if mermaid {
                d.mermaid_source = Some(source.clone());
            }
        }
        let patch = if mermaid { json!({ "title": title, "mermaid_source": source, "diagram_type": "mermaid" }) } else { json!({ "title": title }) };
        self.run(
            move |a| client::update_diagram(a, &id, patch),
            move |s, r, tx| {
                let Some(ed) = &mut s.diagram else { return };
                let Save::Saving { attempt: a, draft } = &ed.save else { return };
                if *a != attempt {
                    return;
                }
                let same = draft.title == ed.title.text && draft.markdown == ed.source.text;
                match r {
                    Ok(d) => {
                        ed.save = Save::Clean;
                        if !same {
                            ed.touch(tx);
                        }
                        if let Some(slot) = s.lib.diagrams.iter_mut().find(|x| x.id == d.id) {
                            slot.updated_at = d.updated_at;
                        }
                        s.save_snapshot();
                    }
                    Err(e) => {
                        ed.save = Save::Failed { message: e.to_string() };
                        s.fail(&e, "Couldn't save the diagram", tx);
                    }
                }
            },
        );
    }

    pub fn duplicate_diagram(&mut self, id: &str, _tx: &mut Tx) {
        let id = id.to_owned();
        self.run(
            move |a| client::duplicate_diagram(a, &id),
            |s, r, tx| match r {
                Ok(d) => {
                    let id = d.id.clone();
                    s.lib.diagrams.insert(0, d);
                    s.open_diagram(&id);
                    s.toast(ToastKind::Success, "Duplicated", tx);
                }
                Err(e) => s.fail(&e, "Couldn't duplicate the diagram", tx),
            },
        );
    }

    // Deleting, with Undo

    pub fn delete(&mut self, id: &str, tx: &mut Tx) {
        let found = if let Some(i) = self.lib.bookmarks.iter().position(|b| b.id == id) {
            Some((Item::Bookmark(self.lib.bookmarks.remove(i)), i))
        } else if let Some(i) = self.lib.notes.iter().position(|n| n.id == id) {
            if self.note.as_ref().is_some_and(|e| e.id == id) {
                self.note = None;
            }
            Some((Item::Note(self.lib.notes.remove(i)), i))
        } else if let Some(i) = self.lib.snippets.iter().position(|s| s.id == id) {
            if self.snippet.as_ref().is_some_and(|e| e.id == id) {
                self.snippet = None;
            }
            Some((Item::Snippet(self.lib.snippets.remove(i)), i))
        } else if let Some(i) = self.lib.diagrams.iter().position(|d| d.id == id) {
            if self.diagram.as_ref().is_some_and(|e| e.id == id) {
                self.diagram = None;
            }
            Some((Item::Diagram(self.lib.diagrams.remove(i)), i))
        } else {
            None
        };
        let Some((item, index)) = found else { return };
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
            self.select_neighbour(index, tx);
        }
        let due = tx.clock() + UNDO_SECONDS;
        let toast_id = self.next();
        self.toasts.retain(|t| t.undo.is_some());
        self.toasts.push(Toast { id: toast_id, kind: ToastKind::Info, text: format!("{} deleted", item.noun()), undo: Some(id.to_owned()), due });
        self.deletes.push(PendingDelete { item, index, due });
        tx.wake_at(due);
    }

    fn select_neighbour(&mut self, index: usize, tx: &mut Tx) {
        match self.section {
            Section::Notes => {
                let ids: Vec<String> = self.visible_notes().iter().map(|n| n.id.clone()).collect();
                if let Some(id) = ids.get(index.min(ids.len().saturating_sub(1))) {
                    self.open_note(id, tx);
                }
            }
            Section::Snippets => {
                if let Some(id) = self.lib.snippets.get(index.min(self.lib.snippets.len().saturating_sub(1))).map(|s| s.id.clone()) {
                    self.open_snippet(&id);
                }
            }
            Section::Diagrams => {
                if let Some(id) = self.lib.diagrams.get(index.min(self.lib.diagrams.len().saturating_sub(1))).map(|d| d.id.clone()) {
                    self.open_diagram(&id);
                }
            }
            _ => {}
        }
    }

    pub fn undo_delete(&mut self, id: &str, tx: &mut Tx) {
        let Some(i) = self.deletes.iter().position(|d| d.item.id() == id) else { return };
        let d = self.deletes.remove(i);
        self.toasts.retain(|t| t.undo.as_deref() != Some(id));
        match d.item {
            Item::Bookmark(b) => self.lib.bookmarks.insert(d.index.min(self.lib.bookmarks.len()), b),
            Item::Note(n) => {
                self.lib.notes.insert(d.index.min(self.lib.notes.len()), n);
                if self.section == Section::Notes {
                    self.open_note(id, tx);
                }
            }
            Item::Snippet(s) => {
                self.lib.snippets.insert(d.index.min(self.lib.snippets.len()), s);
                self.open_snippet(id);
            }
            Item::Diagram(dg) => {
                self.lib.diagrams.insert(d.index.min(self.lib.diagrams.len()), dg);
                self.open_diagram(id);
            }
        }
        self.selected = Some(id.to_owned());
    }

    pub(super) fn commit_due_deletes(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.deletes).into_iter().partition(|d| d.due <= clock);
        self.deletes = keep;
        if let Some(next) = self.deletes.iter().map(|d| d.due).reduce(f64::min) {
            tx.wake_at(next);
        }
        let any = !due.is_empty();
        for d in due {
            self.send_delete(d.item);
        }
        if any {
            self.save_snapshot();
        }
        any
    }

    fn send_delete(&self, item: Item) {
        self.run(
            move |a| match &item {
                Item::Bookmark(b) => client::delete_bookmark(a, &b.id),
                Item::Note(n) => client::delete_note(a, &n.id),
                Item::Snippet(s) => client::delete_snippet(a, &s.id),
                Item::Diagram(d) => client::delete_diagram(a, &d.id),
            },
            |s, r, tx| {
                if let Err(e) = r {
                    s.fail(&e, "Couldn't delete it on the server", tx);
                    s.refresh(tx);
                }
            },
        );
    }

    /// Sends every pending delete now (quitting, signing out). Blocks briefly: the process may
    /// be about to end.
    pub fn flush_deletes(&mut self) {
        let items: Vec<Item> = std::mem::take(&mut self.deletes).into_iter().map(|d| d.item).collect();
        if items.is_empty() || self.handle.is_none() {
            return;
        }
        let auth = self.auth.clone();
        let worker = std::thread::spawn(move || {
            for item in items {
                let _ = match &item {
                    Item::Bookmark(b) => client::delete_bookmark(&auth, &b.id),
                    Item::Note(n) => client::delete_note(&auth, &n.id),
                    Item::Snippet(s) => client::delete_snippet(&auth, &s.id),
                    Item::Diagram(d) => client::delete_diagram(&auth, &d.id),
                };
            }
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !worker.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    pub(super) fn autosave_due(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(Save::Pending { due }) = self.note.as_ref().map(|e| &e.save) {
            if *due <= clock { self.save_note(tx); changed = true } else { tx.wake_at(*due) }
        }
        if let Some(Save::Pending { due }) = self.snippet.as_ref().map(|e| &e.save) {
            if *due <= clock { self.save_snippet(tx); changed = true } else { tx.wake_at(*due) }
        }
        if let Some(Save::Pending { due }) = self.diagram.as_ref().map(|e| &e.save) {
            if *due <= clock { self.save_diagram(tx); changed = true } else { tx.wake_at(*due) }
        }
        changed
    }

    // Import and capture

    /// Files dropped on the window: Markdown and text become notes, code becomes snippets.
    pub fn import_files(&mut self, files: &[std::path::PathBuf], tx: &mut Tx) {
        let mut made = 0;
        for f in files.iter().take(20) {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            if text.len() > 2 * 1024 * 1024 {
                continue;
            }
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported").to_owned();
            if matches!(ext.as_str(), "md" | "markdown" | "txt" | "text" | "") {
                let body = if format::first_line(&text).is_empty() { text } else { text.clone() };
                let md = if body.trim_start().starts_with('#') { body } else { format!("# {stem}\n\n{body}") };
                self.new_note(md, tx);
            } else if let Some(lang) = language_for_extension(&ext) {
                self.new_snippet(text, Some(lang.into()), tx);
            } else {
                continue;
            }
            made += 1;
        }
        if made == 0 {
            self.toast(ToastKind::Info, "Drop Markdown, text or code files to import them.", tx);
        } else if made > 1 {
            self.toast(ToastKind::Success, format!("Imported {made} files"), tx);
        }
    }

    /// Text pasted with nothing focused: a link becomes a bookmark, anything else a note.
    pub fn paste_anywhere(&mut self, text: &str, tx: &mut Tx) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        if format::looks_like_url(t) {
            self.new_bookmark_dialog(Some(format::normalize_url(t)));
        } else if self.section == Section::Snippets {
            self.new_snippet(t.to_owned(), None, tx);
        } else {
            self.new_note(t.to_owned(), tx);
        }
    }

    pub fn copy_link(&mut self, id: &str, tx: &mut Tx) {
        if let Some(b) = self.lib.bookmarks.iter().find(|b| b.id == id) {
            tx.copy(b.url.clone());
            self.toast(ToastKind::Success, "Link copied", tx);
        }
    }

    pub fn tags_of(&self, id: &str) -> Vec<String> {
        self.lib.bookmarks.iter().find(|b| b.id == id).map(|b| tags(b.tags.as_deref())).unwrap_or_default()
    }
}

impl NoteEditor {
    /// Schedules a save right away (typing went on during the last one).
    fn touch_now(&mut self, tx: &mut Tx) {
        let due = tx.clock() + 0.2;
        self.save = Save::Pending { due };
        tx.wake_at(due);
    }
}

/// A best guess at a pasted snippet's language.
pub fn guess_language(code: &str) -> &'static str {
    let c = code.trim_start();
    let has = |s: &str| code.contains(s);
    if c.starts_with('{') || c.starts_with('[') && serde_json::from_str::<serde_json::Value>(c).is_ok() {
        "json"
    } else if has("fn ") && (has("let ") || has("->") || has("impl ")) {
        "rust"
    } else if has("func ") && has("package ") {
        "go"
    } else if has("def ") && has(":\n") || c.starts_with("import ") && !has(";") {
        "python"
    } else if c.starts_with("#!/") || c.starts_with("$ ") {
        "bash"
    } else if has("<div") || c.starts_with("<!") || c.starts_with("<html") {
        "html"
    } else if has("SELECT ") || has("select ") && has(" from ") {
        "sql"
    } else if has(": ") && has("interface ") || has(": string") || has(": number") {
        "typescript"
    } else if has("func ") || has("let ") && has("var ") {
        "swift"
    } else {
        "javascript"
    }
}

pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "tsx" | "mts" => "typescript",
        "py" => "python",
        "rs" => "rust",
        "go" => "go",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "java" => "java",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "html" | "htm" => "html",
        "css" | "scss" => "css",
        "sql" => "sql",
        "sh" | "bash" | "zsh" => "bash",
        "json" => "json",
        "yml" | "yaml" => "yaml",
        _ => return None,
    })
}
