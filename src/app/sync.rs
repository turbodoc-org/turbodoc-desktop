//! Signing in and out, keeping the library current, and the per-frame bookkeeping: autosaves
//! falling due, toasts expiring, deferred deletes going out.
use rune::Tx;

use super::{App, AuthMode, Phase, SignIn, Sync, ToastKind};
use crate::api::{auth, client};
use crate::net::http::NetError;
use crate::store::cache::{self, Library};

/// Refresh the library at most this often when the window comes back to the front.
const FOCUS_SYNC_EVERY: f64 = 45.0;
/// And this often while the window shows.
const BACKGROUND_SYNC_EVERY: f64 = 300.0;

fn join<T>(h: std::thread::ScopedJoinHandle<'_, Result<T, NetError>>) -> Result<T, NetError> {
    h.join().unwrap_or(Err(NetError::Offline("worker stopped".into())))
}

impl App {
    pub fn submit_sign_in(&mut self, tx: &mut Tx) {
        let Phase::SignedOut(form) = &mut self.phase else { return };
        let email = form.email.text.trim().to_owned();
        let password = form.password.text.clone();
        if email.is_empty() || !email.contains('@') {
            form.error = Some("Enter the email address you use for Turbodoc.".into());
            return;
        }
        if form.mode != AuthMode::Reset && password.len() < 6 {
            form.error = Some("Passwords are at least 6 characters.".into());
            return;
        }
        form.busy = true;
        form.error = None;
        form.info = None;
        let mode = form.mode;
        let Some(handle) = self.handle.clone() else { return };
        crate::net::pool::spawn(move || match mode {
            AuthMode::SignIn => {
                let r = auth::sign_in(&email, &password);
                handle.update(move |s, tx| s.signed_in(r, tx));
            }
            AuthMode::SignUp => {
                let r = auth::sign_up(&email, &password);
                handle.update(move |s, tx| match r {
                    Ok(Some(session)) => s.signed_in(Ok(session), tx),
                    Ok(None) => s.form_info("Check your inbox to confirm your email, then sign in.", AuthMode::SignIn),
                    Err(e) => s.form_error(e),
                });
            }
            AuthMode::Reset => {
                let r = auth::recover(&email);
                handle.update(move |s, _| match r {
                    Ok(()) => s.form_info("We sent you a link to choose a new password.", AuthMode::SignIn),
                    Err(e) => s.form_error(e),
                });
            }
        });
        let _ = tx;
    }

    fn form_info(&mut self, text: &str, mode: AuthMode) {
        if let Phase::SignedOut(f) = &mut self.phase {
            f.busy = false;
            f.mode = mode;
            f.info = Some(text.into());
            f.password.clear();
        }
    }

    fn form_error(&mut self, e: NetError) {
        if let Phase::SignedOut(f) = &mut self.phase {
            f.busy = false;
            f.error = Some(match e {
                NetError::Status(_, m) if m.to_lowercase().contains("invalid login") => "That email and password don't match an account.".into(),
                NetError::Status(_, m) if m.to_lowercase().contains("not confirmed") => "Confirm your email first: the link is in your inbox.".into(),
                other => other.to_string(),
            });
        }
    }

    /// After the first frame: reads the saved session on a worker (the keychain may ask the
    /// person first), then syncs. Requests made meanwhile wait for it.
    pub fn restore_session(&mut self) {
        let Some(handle) = self.handle.clone() else { return };
        let auth = self.auth.clone();
        crate::net::pool::spawn(move || {
            let saved = auth.restore_saved();
            handle.update(move |s, tx| s.restored(saved, tx));
        });
    }

    fn restored(&mut self, saved: Option<auth::Session>, tx: &mut Tx) {
        self.restoring = false;
        match (saved, &self.phase) {
            (Some(session), Phase::Ready(u)) if u.id == session.user.id => {
                self.phase = Phase::Ready(session.user.clone());
                self.refresh(tx);
            }
            (Some(session), _) => {
                // Signed in by a build that didn't remember the account: open its library now.
                tx.instant();
                self.open_account(session.user, tx);
            }
            (None, Phase::Ready(_)) => self.reset_signed_out(Some("Sign in again to keep syncing."), tx),
            (None, Phase::SignedOut(_)) => {}
        }
    }

    fn open_account(&mut self, user: auth::User, tx: &mut Tx) {
        self.set_pref("account", &user);
        self.lib = cache::load(&user.id).unwrap_or_default();
        self.previews.known = self.lib.og.iter().cloned().collect();
        self.loaded = self.lib.synced_at > 0;
        self.phase = Phase::Ready(user);
        self.refresh(tx);
    }

    fn signed_in(&mut self, r: Result<auth::Session, NetError>, tx: &mut Tx) {
        match r {
            Ok(session) => {
                let user = session.user.clone();
                self.auth.adopt(session);
                self.set_pref("account", &user);
                self.lib = cache::load(&user.id).unwrap_or_default();
                self.previews.known = self.lib.og.iter().cloned().collect();
                self.loaded = self.lib.synced_at > 0;
                self.phase = Phase::Ready(user);
                tx.motion(rune::tok::motion::REFLOW);
                self.refresh(tx);
            }
            Err(e) => self.form_error(e),
        }
    }

    /// Signing out on purpose also removes this account's offline copy from the Mac.
    pub fn sign_out(&mut self, tx: &mut Tx) {
        self.flush_deletes();
        if let Some(u) = self.user() {
            cache::forget(&u.id);
        }
        let _ = self.prefs.remove("account");
        self.auth.sign_out();
        self.reset_signed_out(None, tx);
    }

    /// The server refused the session (revoked, or signed out elsewhere).
    pub fn session_ended(&mut self, tx: &mut Tx) {
        if matches!(self.phase, Phase::Ready(_)) && !self.restoring {
            self.reset_signed_out(Some("Your session ended. Sign in again to keep syncing."), tx);
        }
    }

    fn reset_signed_out(&mut self, info: Option<&str>, tx: &mut Tx) {
        let mut form = SignIn::new();
        if let Phase::Ready(u) = &self.phase
            && let Some(email) = &u.email
        {
            form.email = rune::TextEdit::new(email.clone());
            form.email.focused = false;
            form.password.focused = true;
        }
        form.info = info.map(str::to_owned);
        self.phase = Phase::SignedOut(form);
        self.lib = Library::default();
        self.previews = Default::default();
        self.note = None;
        self.snippet = None;
        self.diagram = None;
        self.dialog = None;
        self.palette = None;
        self.settings = false;
        self.selected = None;
        self.loaded = false;
        self.sync = Sync::Idle;
        tx.motion(rune::tok::motion::REFLOW);
    }

    /// Fetches everything (four lists and the stats, in parallel) and replaces the library.
    pub fn refresh(&mut self, tx: &mut Tx) {
        if self.user().is_none() || self.restoring || matches!(self.sync, Sync::Running { .. }) {
            return;
        }
        let attempt = self.next();
        self.sync = Sync::Running { attempt };
        self.last_focus_sync = tx.clock();
        tx.wake_after(std::time::Duration::from_secs_f64(BACKGROUND_SYNC_EVERY));
        self.run(
            |a| {
                let (b, n, s, d, st) = std::thread::scope(|scope| {
                    let b = scope.spawn(|| client::bookmarks(a));
                    let n = scope.spawn(|| client::notes(a));
                    let s = scope.spawn(|| client::snippets(a));
                    let d = scope.spawn(|| client::diagrams(a));
                    let st = client::stats(a).ok();
                    (join(b), join(n), join(s), join(d), st)
                });
                Ok((b?, n?, s?, d?, st))
            },
            move |s, r, tx| {
                if s.sync != (Sync::Running { attempt }) {
                    return;
                }
                match r {
                    Ok((bookmarks, notes, snippets, diagrams, stats)) => {
                        s.sync = Sync::Idle;
                        s.apply_library(bookmarks, notes, snippets, diagrams, stats, !s.loaded, tx);
                    }
                    Err(e) => {
                        s.sync = if e.is_offline() { Sync::Offline { since: tx.clock() } } else { Sync::Idle };
                        if !e.is_offline() {
                            s.fail(&e, "Couldn't sync", tx);
                        }
                    }
                }
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_library(
        &mut self,
        bookmarks: Vec<crate::api::models::Bookmark>,
        notes: Vec<crate::api::models::Note>,
        snippets: Vec<crate::api::models::Snippet>,
        diagrams: Vec<crate::api::models::Diagram>,
        stats: Option<crate::api::models::Stats>,
        first: bool,
        tx: &mut Tx,
    ) {
        if first {
            tx.instant(); // the first answer lands without animating everything in
        }
        // Items deleted here but not yet on the server stay hidden.
        let hidden: Vec<String> = self.deletes.iter().map(|d| d.item.id().to_owned()).collect();
        let keep = |id: &str| !hidden.iter().any(|h| h == id);
        self.lib.bookmarks = bookmarks.into_iter().filter(|b| keep(&b.id)).collect();
        // A note open with unsaved edits keeps the editor's text; the list shows the server's.
        self.lib.notes = notes.into_iter().filter(|n| keep(&n.id)).collect();
        if let Some(ed) = &mut self.note
            && let Some(n) = self.lib.notes.iter().find(|n| n.id == ed.id)
        {
            ed.adopt_remote(n);
        }
        self.lib.snippets = snippets.into_iter().filter(|s| keep(&s.id)).collect();
        self.lib.diagrams = diagrams.into_iter().filter(|d| keep(&d.id)).collect();
        if stats.is_some() {
            self.lib.stats = stats;
        }
        self.lib.synced_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        self.loaded = true;
        self.save_snapshot();
    }

    pub fn save_snapshot(&mut self) {
        let Some(u) = self.user() else { return };
        let id = u.id.clone();
        let mut lib = self.lib.clone();
        lib.og = self.previews.known.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        cache::save(&id, lib);
    }

    /// The window came to the front: catch up with edits made elsewhere.
    pub fn on_focus(&mut self, focused: bool, tx: &mut Tx) {
        self.focused = focused;
        if focused && tx.clock() - self.last_focus_sync > FOCUS_SYNC_EVERY {
            self.refresh(tx);
        }
    }

    /// Before every frame: timers that fell due. True when anything visible changed.
    pub fn tick(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if !self.framed && self.handle.is_some() {
            self.framed = rune_mac::remember_frame("Turbodoc");
        }
        let before = self.toasts.len();
        self.toasts.retain(|t| t.due > clock);
        changed |= before != self.toasts.len();
        if let Some(next) = self.toasts.iter().map(|t| t.due).reduce(f64::min) {
            tx.wake_at(next);
        }
        changed |= self.commit_due_deletes(clock, tx);
        changed |= self.autosave_due(clock, tx);
        if self.user().is_some() && !matches!(self.sync, Sync::Running { .. }) && clock - self.last_focus_sync > BACKGROUND_SYNC_EVERY && self.focused {
            self.refresh(tx);
            changed = true;
        }
        changed
    }

    pub fn offline(&self) -> bool { matches!(self.sync, Sync::Offline { .. }) }

    pub fn notify_saved(&mut self, what: &str, tx: &mut Tx) {
        if !self.focused && rune_notify::available() {
            let id = format!("saved-{}", self.next());
            let _ = rune_notify::post(&rune_notify::Notification::new(id, "Saved to Turbodoc").body(what));
        } else {
            self.toast(ToastKind::Success, format!("Saved {what}"), tx);
        }
    }
}
