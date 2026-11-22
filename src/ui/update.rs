//! A new version, where it shows: a quiet pill in the toolbar once it's ready (or, for a copy
//! that can't update itself, once it's out), and the "Software update" line in Settings.
use rune::{El, icon, row, text};
use rune_kit::{Kit, Spinner};

use super::icons;
use crate::app::App;
use crate::app::update::Update;

/// The toolbar's pill, when there's something to say.
pub fn pill(s: &App, k: &Kit) -> Option<El> {
    match &s.updates.phase {
        Update::Ready { version } => {
            let help = match first_line(&s.updates.notes) {
                Some(n) => format!("{n}\nRestarts Turbodoc. Anything open is saved first."),
                None => "Restarts Turbodoc. Anything open is saved first.".to_owned(),
            };
            let label = format!("Update to {version}");
            Some(frame(k).help(help).label(label.clone()).child(icon(&icons::DOWNLOAD, 13.0).color(k.c.border_focus)).child(text(label).font(k.ty.small_medium).color(k.c.text_muted)).on_press_tx(|s: &mut App, tx| s.restart_to_update(tx)))
        }
        Update::Available { version } => {
            let label = format!("Turbodoc {version} is out");
            Some(
                frame(k)
                    .help("Opens the download page. This copy can't update itself.")
                    .label(label.clone())
                    .child(icon(&icons::EXTERNAL, 13.0).color(k.c.border_focus))
                    .child(text(label).font(k.ty.small_medium).color(k.c.text_muted))
                    .on_press_tx(|_: &mut App, tx| {
                        let _ = tx.open_url(crate::updater::RELEASES);
                    }),
            )
        }
        Update::Installing { .. } => Some(row().key("update").gap(6.0).center_y().child(Spinner::new().build(k)).child(text("Updating").font(k.ty.small).color(k.c.text_faint)).transition(rune::Transition::opacity())),
        Update::Idle | Update::Checking | Update::Downloading { .. } => None,
    }
}

/// The same pill as "Offline": a hairline capsule that lights on hover.
fn frame(k: &Kit) -> El {
    row()
        .key("update")
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .radius(14.0)
        .border(1.0, k.c.border_base)
        .center_y()
        .cursor(rune::Cursor::Pointer)
        .hover(k.c.hover)
        .role("button")
        .focusable(true)
        .transition(rune::Transition::opacity())
}

fn first_line(notes: &str) -> Option<&str> { notes.lines().map(str::trim).find(|l| !l.is_empty()) }

/// The control at the end of Settings' line.
pub enum Control {
    Check,
    Restart,
    Download,
    Busy,
    Nothing,
}

/// What Settings' line says, and its control.
pub fn status(s: &App) -> (String, Control) {
    use crate::updater::Channel;
    match &s.updates.phase {
        _ if s.updates.channel == Channel::Off => ("This build doesn't update itself.".into(), Control::Nothing),
        Update::Idle => ("Turbodoc checks for new versions every few hours and downloads them in the background.".into(), Control::Check),
        Update::Checking => ("Checking for a new version…".into(), Control::Busy),
        Update::Downloading { version } => (format!("Downloading Turbodoc {version}…"), Control::Busy),
        Update::Ready { version } => (format!("Turbodoc {version} is downloaded. Restarting saves anything open first."), Control::Restart),
        Update::Available { version } => (format!("Turbodoc {version} is out. This copy can't update itself: download it from the releases page."), Control::Download),
        Update::Installing { version, .. } => (format!("Updating to {version}…"), Control::Busy),
    }
}
