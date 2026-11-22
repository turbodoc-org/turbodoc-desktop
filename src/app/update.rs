//! Updating the app: a quiet check soon after launch and every few hours, the download in the
//! background, and a button in the toolbar once the new version is ready. Pressing it saves
//! what's open, sends pending deletes, puts the update in place and starts the new version.
//! `crate::updater` checks and installs; this decides when, and holds the phase.
use rune::Tx;

use super::{App, Save, ToastKind};
use crate::updater::{self, Channel, Found};

/// Seconds after launch before the first check, so it never competes with the first sync.
const FIRST_CHECK_AFTER: f64 = 15.0;
/// Seconds between checks while the app runs.
const CHECK_EVERY: f64 = 6.0 * 3600.0;
/// Seconds a restart waits for open documents to save before it gives up.
const SAVE_WAIT: f64 = 15.0;

/// Where an update stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// Nothing newer is known.
    Idle,
    /// A check is out.
    Checking,
    /// A newer version is downloading; its signature is checked once it lands.
    Downloading { version: String },
    /// Downloaded and verified: the toolbar offers to restart into it.
    Ready { version: String },
    /// A newer version this copy can't install itself (the .deb, the .msi): the toolbar links
    /// to the releases page.
    Available { version: String },
    /// Restarting into `version`: waiting for open documents to save (`since`), then installing
    /// (`started`).
    Installing { version: String, since: f64, started: bool },
}

pub struct Updates {
    pub phase: Update,
    pub channel: Channel,
    /// The new version's release notes, for the button's tooltip.
    pub notes: String,
    /// The user asked for this check (menu, Settings), so its answer is shown even when it's
    /// "up to date".
    asked: bool,
    /// Clock time of the next background check; 0 until the first tick.
    next_check: f64,
    /// The verified download, from `Ready` until it's installed.
    ready: Option<(Found, Vec<u8>)>,
}

impl Updates {
    /// `live`: a real window with a network (not tests or headless renders).
    pub fn new(live: bool) -> Updates {
        let channel = if live { updater::channel() } else { Channel::Off };
        Updates { phase: Update::Idle, channel, notes: String::new(), asked: false, next_check: 0.0, ready: None }
    }
}

impl App {
    /// From `tick`: the background check when it falls due, and a restart waiting on saves.
    pub(super) fn update_due(&mut self, clock: f64, tx: &mut Tx) -> bool {
        if matches!(self.updates.phase, Update::Installing { .. }) {
            return self.install_when_saved(clock, tx);
        }
        if self.updates.channel == Channel::Off {
            return false;
        }
        if self.updates.next_check == 0.0 {
            self.updates.next_check = clock + FIRST_CHECK_AFTER;
        }
        if clock >= self.updates.next_check && matches!(self.updates.phase, Update::Idle | Update::Available { .. }) {
            self.check_for_update(false, tx);
            return true;
        }
        tx.wake_at(self.updates.next_check);
        false
    }

    /// Checks for a new version now, and downloads it when this copy can install it. `asked`:
    /// the user asked (menu, Settings), so the answer is shown either way.
    pub fn check_for_update(&mut self, asked: bool, tx: &mut Tx) {
        match &self.updates.phase {
            Update::Checking | Update::Downloading { .. } => {
                self.updates.asked |= asked;
                return;
            }
            Update::Ready { version } | Update::Installing { version, .. } => {
                if asked {
                    let text = format!("Turbodoc {version} is ready. Restart to update.");
                    self.toast(ToastKind::Info, text, tx);
                }
                return;
            }
            Update::Idle | Update::Available { .. } => {}
        }
        if self.updates.channel == Channel::Off {
            if asked {
                self.toast(ToastKind::Info, "This build of Turbodoc doesn't update itself.", tx);
            }
            return;
        }
        let Some(handle) = self.handle.clone() else { return };
        self.updates.phase = Update::Checking;
        self.updates.asked = asked;
        self.updates.next_check = tx.clock() + CHECK_EVERY;
        let channel = self.updates.channel;
        // The media lane: a download can take a while, and API calls shouldn't wait behind it.
        crate::net::pool::spawn_bulk(move || {
            let result = match updater::check(channel) {
                Ok(Some(found)) if channel == Channel::InPlace => {
                    let version = found.version.clone();
                    handle.update(move |s, _| s.updates.phase = Update::Downloading { version });
                    updater::download(&found).map(|bytes| Some((found, Some(bytes))))
                }
                Ok(Some(found)) => Ok(Some((found, None))),
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            };
            handle.update(move |s, tx| s.update_checked(result, tx));
        });
    }

    fn update_checked(&mut self, result: Result<Option<(Found, Option<Vec<u8>>)>, String>, tx: &mut Tx) {
        let asked = std::mem::take(&mut self.updates.asked);
        match result {
            Ok(Some((found, Some(bytes)))) => {
                let version = found.version.clone();
                self.updates.notes = found.notes.clone();
                self.updates.ready = Some((found, bytes));
                self.updates.phase = Update::Ready { version: version.clone() };
                if asked {
                    self.toast(ToastKind::Success, format!("Turbodoc {version} is ready. Restart to update."), tx);
                }
            }
            Ok(Some((found, None))) => {
                self.updates.notes = found.notes;
                self.updates.phase = Update::Available { version: found.version.clone() };
                if asked {
                    self.toast(ToastKind::Info, format!("Turbodoc {} is out.", found.version), tx);
                }
            }
            Ok(None) => {
                self.updates.phase = Update::Idle;
                if asked {
                    self.toast(ToastKind::Success, "Turbodoc is up to date.", tx);
                }
            }
            Err(e) => {
                // A background check that fails says nothing: the next one is in a few hours.
                self.updates.phase = Update::Idle;
                if asked {
                    self.toast(ToastKind::Error, format!("Couldn't check for updates: {e}"), tx);
                } else {
                    eprintln!("turbodoc: update check failed: {e}");
                }
            }
        }
    }

    /// The toolbar's button: save what's open now, then install and restart once it's saved.
    pub fn restart_to_update(&mut self, tx: &mut Tx) {
        let Update::Ready { version } = &self.updates.phase else { return };
        let version = version.clone();
        // Now rather than after the typing pause.
        if matches!(self.note.as_ref().map(|e| &e.save), Some(Save::Pending { .. })) {
            self.save_note(tx);
        }
        if matches!(self.snippet.as_ref().map(|e| &e.save), Some(Save::Pending { .. })) {
            self.save_snippet(tx);
        }
        if matches!(self.diagram.as_ref().map(|e| &e.save), Some(Save::Pending { .. })) {
            self.save_diagram(tx);
        }
        self.updates.phase = Update::Installing { version, since: tx.clock(), started: false };
        tx.wake_at(tx.clock());
    }

    fn open_saves(&self) -> [Option<&Save>; 3] {
        [self.note.as_ref().map(|e| &e.save), self.snippet.as_ref().map(|e| &e.save), self.diagram.as_ref().map(|e| &e.save)]
    }

    fn install_when_saved(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let Update::Installing { version, since, started } = &self.updates.phase else { return false };
        if *started {
            return false;
        }
        let (version, since) = (version.clone(), *since);
        let saves = self.open_saves();
        let failed = saves.iter().flatten().any(|s| matches!(s, Save::Failed { .. }));
        let unsaved = saves.iter().flatten().any(|s| s.unsaved());
        if failed || (unsaved && clock - since > SAVE_WAIT) {
            // Never restart over unsaved work. The download is kept; the button comes back.
            self.updates.phase = Update::Ready { version };
            self.toast(ToastKind::Error, "Something open couldn't be saved, so Turbodoc didn't restart. Try again once it's saved.", tx);
            return true;
        }
        if unsaved {
            tx.wake_at(clock + 0.2);
            return false;
        }
        self.updates.phase = Update::Installing { version, since, started: true };
        // The Windows installer ends this process as it starts, before `on_quit` would run.
        self.flush_deletes();
        let (Some((found, bytes)), Some(handle)) = (self.updates.ready.take(), self.handle.clone()) else { return true };
        crate::net::pool::spawn_bulk(move || {
            let result = updater::install(found, bytes);
            handle.update(move |s, tx| match result {
                Ok(()) => {
                    if updater::relaunch_appimage() {
                        tx.quit();
                    } else {
                        tx.relaunch();
                    }
                }
                Err(e) => {
                    s.updates.phase = Update::Idle;
                    s.toast(ToastKind::Error, format!("Couldn't update Turbodoc: {e}"), tx);
                }
            });
        });
        true
    }
}
