//! The open note, snippet and diagram, and how each saves itself.
//!
//! Notes follow the API's revision model (the same rules the web and iOS apps use): the editor
//! remembers `base`, the revision it last agreed with the server, and every save sends the whole
//! draft on top of it. The server three-way merges anything saved elsewhere in the meantime.
//! After a save: if nothing was typed while it was in flight, adopt the server's document (its
//! head is the new base); if typing went on, keep the editor as it is and take the submitted
//! revision as the base, then save again.
use rune::{TextEdit, Tx};

use crate::api::models::{Diagram, Note, Snippet, join_tags, parse_tag_input, tags};

/// Seconds of quiet typing before an autosave.
pub const DEBOUNCE: f64 = 0.9;

/// Where a document's autosave stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Save {
    /// The editor matches the server.
    Clean,
    /// Edited; saves at `due` unless more typing pushes it back.
    Pending { due: f64 },
    /// A save is out. `draft` is what was sent, to tell whether typing went on.
    Saving { attempt: u64, draft: Draft },
    /// The last save failed; the draft is kept and retried on the next edit or sync.
    Failed { message: String },
}

impl Save {
    pub fn label(&self) -> &str {
        match self {
            Save::Clean => "Saved",
            Save::Pending { .. } => "Edited",
            Save::Saving { .. } => "Saving…",
            Save::Failed { .. } => "Not saved",
        }
    }
    pub fn unsaved(&self) -> bool { !matches!(self, Save::Clean) }
    pub fn in_flight(&self) -> bool { matches!(self, Save::Saving { .. }) }
}

/// A note's editable fields as text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub title: String,
    pub markdown: String,
    pub tags: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteMode {
    Write,
    Split,
    Read,
}

impl NoteMode {
    pub fn index(self) -> usize {
        match self {
            NoteMode::Write => 0,
            NoteMode::Split => 1,
            NoteMode::Read => 2,
        }
    }
    pub fn from_index(i: usize) -> NoteMode {
        match i {
            0 => NoteMode::Write,
            2 => NoteMode::Read,
            _ => NoteMode::Split,
        }
    }
}

pub struct NoteEditor {
    pub id: String,
    pub title: TextEdit,
    pub body: TextEdit,
    pub tags: TextEdit,
    /// The revision this editor last agreed with the server.
    pub base: Option<String>,
    /// The server version the editor last adopted, to tell a newer remote edit.
    pub version: i64,
    pub save: Save,
    /// The note isn't on the server yet (created here; the create is out).
    pub creating: bool,
}

impl NoteEditor {
    pub fn open(n: &Note) -> NoteEditor {
        NoteEditor {
            id: n.id.clone(),
            title: TextEdit::new(n.title.clone()),
            body: TextEdit::new(n.markdown.clone()),
            tags: TextEdit::new(tags(n.tags.as_deref()).join(", ")),
            base: n.head_revision_id.clone(),
            version: n.version,
            save: Save::Clean,
            creating: false,
        }
    }

    pub fn draft(&self) -> Draft { Draft { title: self.title.text.clone(), markdown: self.body.text.clone(), tags: self.tags.text.clone() } }

    pub fn stored_tags(&self) -> Option<String> { join_tags(&parse_tag_input(&self.tags.text)) }

    /// An edit happened: push the autosave back.
    pub fn touch(&mut self, tx: &mut Tx) {
        if self.save.in_flight() {
            return; // the answer reschedules
        }
        let due = tx.clock() + DEBOUNCE;
        self.save = Save::Pending { due };
        tx.wake_at(due);
    }

    /// Loads a newer version saved elsewhere, when nothing here is unsaved.
    pub fn adopt_remote(&mut self, n: &Note) -> bool {
        if self.save.unsaved() || n.version <= self.version || n.head_revision_id == self.base {
            return false;
        }
        let (title_focus, body_focus) = (self.title.focused, self.body.focused);
        let caret = self.body.focus;
        *self = NoteEditor::open(n);
        self.title.focused = title_focus;
        self.body.focused = body_focus;
        if body_focus {
            let mut at = caret.min(self.body.text.len());
            while !self.body.text.is_char_boundary(at) {
                at -= 1;
            }
            self.body.set_caret(at);
        }
        true
    }
}

pub struct SnippetEditor {
    pub id: String,
    pub title: TextEdit,
    pub code: TextEdit,
    pub language: String,
    pub editing: bool,
    pub save: Save,
}

impl SnippetEditor {
    pub fn open(s: &Snippet) -> SnippetEditor {
        SnippetEditor {
            id: s.id.clone(),
            title: TextEdit::new(s.title.clone().unwrap_or_default()),
            code: TextEdit::new(s.code.clone()),
            language: if s.language.is_empty() { "javascript".into() } else { s.language.clone() },
            editing: false,
            save: Save::Clean,
        }
    }
    pub fn touch(&mut self, tx: &mut Tx) {
        let due = tx.clock() + DEBOUNCE;
        if !self.save.in_flight() {
            self.save = Save::Pending { due };
        }
        tx.wake_at(due);
    }
}

pub struct DiagramEditor {
    pub id: String,
    pub title: TextEdit,
    pub source: TextEdit,
    pub save: Save,
}

impl DiagramEditor {
    pub fn open(d: &Diagram) -> DiagramEditor {
        DiagramEditor {
            id: d.id.clone(),
            title: TextEdit::new(d.title.clone()),
            source: TextEdit::new(d.mermaid_source.clone().unwrap_or_default()),
            save: Save::Clean,
        }
    }
    pub fn touch(&mut self, tx: &mut Tx) {
        let due = tx.clock() + DEBOUNCE;
        if !self.save.in_flight() {
            self.save = Save::Pending { due };
        }
        tx.wake_at(due);
    }
}
