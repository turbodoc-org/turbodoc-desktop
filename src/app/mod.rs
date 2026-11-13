//! The app's state. Views read it; handlers change it; workers answer into it through the
//! `AppHandle`. Behaviour with phases (signing in, syncing, autosaving, a deferred delete) is an
//! explicit enum, so no combination of flags can say two things at once.
pub mod actions;
pub mod editors;
pub mod sync;

use rune::{AppHandle, TextEdit, Tx};
use rune_kit::PaneState;

use crate::api::auth::{Auth, User};
use crate::api::models::{Bookmark, Diagram, Note, Snippet};
use crate::net::http::NetError;
use crate::store::cache::Library;
use crate::store::images::Previews;
pub use editors::{DiagramEditor, NoteEditor, NoteMode, Save, SnippetEditor};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Home,
    Bookmarks,
    Notes,
    Snippets,
    Diagrams,
}

impl Section {
    pub const ALL: [Section; 5] = [Section::Home, Section::Bookmarks, Section::Notes, Section::Snippets, Section::Diagrams];
    pub fn title(self) -> &'static str {
        match self {
            Section::Home => "Home",
            Section::Bookmarks => "Bookmarks",
            Section::Notes => "Notes",
            Section::Snippets => "Snippets",
            Section::Diagrams => "Diagrams",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Section::Home => "home",
            Section::Bookmarks => "bookmarks",
            Section::Notes => "notes",
            Section::Snippets => "snippets",
            Section::Diagrams => "diagrams",
        }
    }
    pub fn from_id(id: &str) -> Option<Section> { Section::ALL.into_iter().find(|s| s.id() == id) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookmarkFilter {
    All,
    Unread,
    Favorites,
    Archived,
    Broken,
}

impl BookmarkFilter {
    pub fn title(self) -> &'static str {
        match self {
            BookmarkFilter::All => "All bookmarks",
            BookmarkFilter::Unread => "Reading list",
            BookmarkFilter::Favorites => "Favorites",
            BookmarkFilter::Archived => "Archive",
            BookmarkFilter::Broken => "Broken links",
        }
    }
    pub fn keeps(self, b: &Bookmark) -> bool {
        match self {
            BookmarkFilter::All => !b.archived(),
            BookmarkFilter::Unread => b.unread(),
            BookmarkFilter::Favorites => b.favorite(),
            BookmarkFilter::Archived => b.archived(),
            BookmarkFilter::Broken => b.broken(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn id(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }
    pub fn from_id(s: &str) -> Appearance {
        match s {
            "light" => Appearance::Light,
            "dark" => Appearance::Dark,
            _ => Appearance::System,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    SignIn,
    SignUp,
    Reset,
}

/// The sign-in form.
pub struct SignIn {
    pub mode: AuthMode,
    pub email: TextEdit,
    pub password: TextEdit,
    pub busy: bool,
    pub error: Option<String>,
    pub info: Option<String>,
}

impl SignIn {
    pub fn new() -> SignIn {
        let mut email = TextEdit::new("");
        email.focused = true;
        SignIn { mode: AuthMode::SignIn, email, password: TextEdit::new("").secure(), busy: false, error: None, info: None }
    }
}

/// Where the account stands.
pub enum Phase {
    SignedOut(SignIn),
    /// Signed in; `user` is who.
    Ready(User),
}

/// Where the library's sync stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Sync {
    /// Showing what the snapshot or the last sync gave.
    Idle,
    /// A full refresh is out; `attempt` fences its answer.
    Running { attempt: u64 },
    /// The last refresh couldn't reach the server.
    Offline { since: f64 },
}

/// A modal form over the window.
pub enum Dialog {
    Bookmark(BookmarkForm),
    NewDiagram { title: TextEdit, mermaid: bool, busy: bool },
    Shortcuts,
}

pub struct BookmarkForm {
    /// `Some` when editing an existing bookmark.
    pub id: Option<String>,
    pub url: TextEdit,
    pub title: TextEdit,
    pub tags: TextEdit,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Bookmark(Bookmark),
    Note(Note),
    Snippet(Snippet),
    Diagram(Diagram),
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Bookmark(b) => &b.id,
            Item::Note(n) => &n.id,
            Item::Snippet(s) => &s.id,
            Item::Diagram(d) => &d.id,
        }
    }
    pub fn noun(&self) -> &'static str {
        match self {
            Item::Bookmark(_) => "Bookmark",
            Item::Note(_) => "Note",
            Item::Snippet(_) => "Snippet",
            Item::Diagram(_) => "Diagram",
        }
    }
}

/// A delete the user can still undo: the item is gone from the screen and goes from the server
/// when `due` passes (or the app quits).
pub struct PendingDelete {
    pub item: Item,
    pub index: usize,
    pub due: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub text: String,
    /// An Undo button for this deleted item's id.
    pub undo: Option<String>,
    pub due: f64,
}

/// The command palette (⌘K).
pub struct Palette {
    pub query: TextEdit,
    pub active: usize,
}

pub struct App {
    /// Answers from workers come back through this. `None` in headless renders and tests,
    /// which never touch the network.
    pub handle: Option<AppHandle<App>>,
    pub auth: Auth,
    pub phase: Phase,
    /// The saved session is still being read from the keychain.
    pub restoring: bool,
    /// This install's id in the revision history (`desktop-<uuid>`).
    pub device: String,
    pub prefs: rune::prefs::Prefs,
    pub appearance: Appearance,

    pub lib: Library,
    pub sync: Sync,
    pub seq: u64,
    pub loaded: bool,
    pub previews: Previews,

    pub section: Section,
    pub sidebar: PaneState,
    pub inspector: PaneState,
    pub tags_open: bool,
    pub search: TextEdit,
    pub bm_filter: BookmarkFilter,
    pub tag: Option<String>,
    pub layout: Layout,
    pub selected: Option<String>,

    pub note: Option<NoteEditor>,
    pub snippet: Option<SnippetEditor>,
    pub diagram: Option<DiagramEditor>,

    pub dialog: Option<Dialog>,
    pub palette: Option<Palette>,
    pub settings: bool,
    pub digest: Option<crate::api::models::DigestPrefs>,
    pub toasts: Vec<Toast>,
    pub deletes: Vec<PendingDelete>,
    pub drop_hover: bool,
    pub framed: bool,
    pub focused: bool,
    pub last_focus_sync: f64,
}

pub const UNDO_SECONDS: f64 = 6.0;

impl App {
    pub fn new(handle: Option<AppHandle<App>>) -> App {
        let mut prefs = if handle.is_some() { rune::prefs::Prefs::open(crate::APP_ID) } else { rune::prefs::Prefs::memory() };
        let device = match prefs.get::<String>("device_id") {
            Some(d) => d,
            None => {
                let d = format!("desktop-{}", crate::api::client::uuid());
                let _ = prefs.set("device_id", &d);
                d
            }
        };
        // The account last signed in is remembered in preferences (it isn't secret), so the
        // window opens on its library at once; the session itself is read from the keychain on a
        // worker after the first frame (`App::restore_session`).
        let auth = if handle.is_some() { Auth::pending() } else { Auth::default() };
        let restoring = handle.is_some();
        let phase = match prefs.get::<User>("account").filter(|_| restoring) {
            Some(u) => Phase::Ready(u),
            None => Phase::SignedOut(SignIn::new()),
        };
        let lib = match &phase {
            Phase::Ready(u) => crate::store::cache::load(&u.id).unwrap_or_default(),
            Phase::SignedOut(_) => Library::default(),
        };
        let sidebar_w = prefs.get::<f32>("sidebar_width").unwrap_or(248.0);
        let mut app = App {
            handle,
            restoring,
            auth,
            phase,
            device,
            appearance: Appearance::from_id(&prefs.get::<String>("appearance").unwrap_or_default()),
            section: prefs.get::<String>("section").and_then(|s| Section::from_id(&s)).unwrap_or(Section::Home),
            layout: if prefs.get::<String>("layout").as_deref() == Some("list") { Layout::List } else { Layout::Grid },
            prefs,
            loaded: !lib.bookmarks.is_empty() || !lib.notes.is_empty() || lib.synced_at > 0,
            lib,
            sync: Sync::Idle,
            seq: 0,
            previews: Previews::default(),
            sidebar: PaneState::new(sidebar_w).range(200.0, 360.0),
            inspector: PaneState::new(320.0).range(280.0, 440.0),
            tags_open: true,
            search: TextEdit::new(""),
            bm_filter: BookmarkFilter::All,
            tag: None,
            selected: None,
            note: None,
            snippet: None,
            diagram: None,
            dialog: None,
            palette: None,
            settings: false,
            digest: None,
            toasts: Vec::new(),
            deletes: Vec::new(),
            drop_hover: false,
            framed: false,
            focused: true,
            last_focus_sync: 0.0,
        };
        app.previews.known = app.lib.og.iter().cloned().collect();
        app
    }

    pub fn user(&self) -> Option<&User> {
        match &self.phase {
            Phase::Ready(u) => Some(u),
            Phase::SignedOut(_) => None,
        }
    }

    pub fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// Runs `work` on a worker with the session, and `done` back on the UI thread with its
    /// answer. A 401 that outlived a refresh ends the session first.
    pub fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Auth) -> Result<T, NetError> + Send + 'static,
        done: impl FnOnce(&mut App, Result<T, NetError>, &mut Tx) + Send + 'static,
    ) {
        let Some(handle) = self.handle.clone() else { return };
        let auth = self.auth.clone();
        crate::net::pool::spawn(move || {
            let r = work(&auth);
            handle.update(move |s, tx| {
                let ended = matches!(r, Err(NetError::Unauthorized)) && !s.auth.signed_in();
                done(s, r, tx);
                if ended {
                    s.session_ended(tx);
                }
            });
        });
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>, tx: &mut Tx) {
        let id = self.next();
        let due = tx.clock() + if kind == ToastKind::Error { 6.0 } else { 3.2 };
        self.toasts.retain(|t| t.undo.is_some());
        self.toasts.push(Toast { id, kind, text: text.into(), undo: None, due });
        tx.wake_at(due);
    }

    pub fn fail(&mut self, e: &NetError, what: &str, tx: &mut Tx) {
        if e.is_offline() {
            self.sync = Sync::Offline { since: tx.clock() };
        }
        if matches!(e, NetError::Unauthorized) {
            return;
        }
        self.toast(ToastKind::Error, format!("{what}: {e}"), tx);
    }

    pub fn set_pref<T: serde::Serialize>(&mut self, key: &str, value: &T) { let _ = self.prefs.set(key, value); }

    pub fn dark(&self, system: bool) -> bool {
        match self.appearance {
            Appearance::System => system,
            Appearance::Light => false,
            Appearance::Dark => true,
        }
    }

    /// Every distinct tag on bookmarks and notes, most used first.
    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        let lists = self.lib.bookmarks.iter().map(|b| b.tags.as_deref()).chain(self.lib.notes.iter().map(|n| n.tags.as_deref()));
        for raw in lists {
            for t in crate::api::models::tags(raw) {
                match counts.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&t)) {
                    Some(c) => c.1 += 1,
                    None => counts.push((t, 1)),
                }
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        counts
    }
}
