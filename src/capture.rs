//! Quick capture: a panel that drops over whatever app is in front (⌃⌥Space), takes a link or
//! a thought, saves it, and gets out of the way. It's a non-activating launcher panel, so the
//! app you were in keeps focus; a link on the clipboard is offered at once.
use rune::{Align, AppHandle, Cx, DisplayTarget, El, Key, OverlayPlacement, ScreenEdge, TextEdit, Tx, col, el, icon, row, text};
use rune_kit::{Kit, TextArea};

use crate::api::auth::Auth;
use crate::api::client;
use crate::app::App;
use crate::format;
use crate::ui::{icons, theme, widgets};

/// Where the capture stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Writing,
    Saving,
    Saved(String),
    Failed(String),
}

pub struct Capture {
    pub me: Option<AppHandle<Capture>>,
    pub main: Option<AppHandle<App>>,
    pub auth: Auth,
    pub field: TextEdit,
    pub phase: Phase,
    pub seq: u64,
}

impl Capture {
    pub fn new(me: Option<AppHandle<Capture>>, main: Option<AppHandle<App>>, auth: Auth) -> Capture {
        let mut field = TextEdit::new("");
        field.focused = true;
        Capture { me, main, auth, field, phase: Phase::Writing, seq: 0 }
    }

    fn kind(&self) -> Kind {
        let t = self.field.text.trim();
        if t.is_empty() {
            Kind::Empty
        } else if format::looks_like_url(t) {
            Kind::Link(format::normalize_url(t))
        } else {
            Kind::Note
        }
    }

    fn save(&mut self, tx: &mut Tx) {
        if matches!(self.phase, Phase::Saving) {
            return;
        }
        let kind = self.kind();
        if kind == Kind::Empty {
            return;
        }
        if !self.auth.signed_in() {
            self.phase = Phase::Failed("Sign in to Turbodoc first.".into());
            return;
        }
        self.phase = Phase::Saving;
        self.seq += 1;
        let attempt = self.seq;
        let text = self.field.text.trim().to_owned();
        let (Some(me), main, auth) = (self.me.clone(), self.main.clone(), self.auth.clone()) else { return };
        let device = "desktop-capture".to_owned();
        crate::net::pool::spawn(move || {
            let result: Result<String, String> = match kind {
                Kind::Link(url) => {
                    let title = client::og_image(&auth, &url).ok().and_then(|o| o.title).unwrap_or_default();
                    client::create_bookmark(&auth, &title, &url, None).map_err(|e| e.to_string()).map(|b| {
                        let name = b.display_title().to_owned();
                        if let Some(m) = &main {
                            m.update(move |s, _| {
                                if !s.lib.bookmarks.iter().any(|x| x.id == b.id) {
                                    s.lib.bookmarks.insert(0, b);
                                    s.save_snapshot();
                                }
                            });
                        }
                        name
                    })
                }
                _ => {
                    let id = client::uuid();
                    let (title, body) = split_note(&text);
                    client::create_note(&auth, &id, &title, &body, None, &device).map_err(|e| e.to_string()).map(|n| {
                        let name = n.display_title();
                        if let Some(m) = &main {
                            m.update(move |s, _| {
                                if !s.lib.notes.iter().any(|x| x.id == n.id) {
                                    s.lib.notes.insert(0, n);
                                    s.save_snapshot();
                                }
                            });
                        }
                        name
                    })
                }
            };
            me.update(move |s, tx| {
                if s.seq != attempt {
                    return;
                }
                match result {
                    Ok(name) => {
                        s.phase = Phase::Saved(name);
                        s.field.clear();
                        tx.wake_after(std::time::Duration::from_millis(900));
                    }
                    Err(e) => s.phase = Phase::Failed(e),
                }
            });
        });
        let _ = tx;
    }
}

/// A note's title is its first line when there's more than one.
fn split_note(text: &str) -> (String, String) {
    match text.split_once('\n') {
        Some((first, rest)) if first.chars().count() <= 80 && !rest.trim().is_empty() => (first.trim_start_matches('#').trim().to_owned(), rest.trim_start().to_owned()),
        _ => (String::new(), text.to_owned()),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Empty,
    Link(String),
    Note,
}

/// Shows the panel near the top of the screen, offering a link from the clipboard.
pub fn summon(h: &AppHandle<Capture>) {
    h.place(DisplayTarget::UnderPointer, OverlayPlacement::Edge { edge: ScreenEdge::Top, margin: 160.0, align: Align::Center, inset: 0.0 });
    h.update(|s, tx| {
        if matches!(s.phase, Phase::Saved(_) | Phase::Failed(_)) {
            s.phase = Phase::Writing;
        }
        s.field.focused = true;
        if s.field.text.is_empty() {
            tx.read_clipboard(|s: &mut Capture, content, _| {
                let t = content.text().trim();
                if s.field.text.is_empty() && format::looks_like_url(t) && t.len() < 2048 {
                    s.field.replace_text(t.to_owned());
                    s.field.anchor = 0;
                    s.field.focus = s.field.text.len();
                }
            });
        }
    });
    h.show();
    h.focus();
}

pub fn poll(s: &mut Capture, _: f64, tx: &mut Tx) -> bool {
    if let Phase::Saved(_) = s.phase {
        // The confirmation has shown; put the panel away.
        s.phase = Phase::Writing;
        tx.close_window();
        return true;
    }
    false
}

pub fn view(s: &Capture, cx: &mut Cx<Capture>) -> El {
    let k: &Kit = theme::kit(cx.system_dark());
    let kind = s.kind();
    let (glyph, tint, label) = match &kind {
        Kind::Link(url) => (&icons::BOOKMARK, theme::Tint::Bookmark, format!("Bookmark · {}", format::domain(url))),
        Kind::Note => (&icons::NOTE, theme::Tint::Note, "Note".to_owned()),
        Kind::Empty => (&icons::BOLT, theme::Tint::Bookmark, "Quick capture".to_owned()),
    };
    let field = TextArea::new(&s.field)
        .width(None)
        .auto_grow(1, 7)
        .placeholder("Paste a link to save it, or jot down a note…")
        .label("Capture")
        .on_edit(|s: &mut Capture| &mut s.field)
        .on_change(|s: &mut Capture, _| {
            if !matches!(s.phase, Phase::Saving) {
                s.phase = Phase::Writing;
            }
        })
        .on_key(|s: &mut Capture, ev, tx| match ev.key {
            Key::Enter if !ev.mods.shift && !ev.mods.alt => {
                s.save(tx);
                true
            }
            Key::Escape => {
                tx.close_window();
                true
            }
            _ => false,
        })
        .build(k)
        .key("capture-field");
    let status: El = match &s.phase {
        Phase::Writing => row()
            .gap(10.0)
            .center_y()
            .child(row().gap(4.0).center_y().child(widgets::keycap("enter", k)).child(text("save").font(k.ty.small).color(k.c.text_faint)))
            .child(row().gap(4.0).center_y().child(widgets::keycap("shift+enter", k)).child(text("new line").font(k.ty.small).color(k.c.text_faint)))
            .child(row().gap(4.0).center_y().child(widgets::keycap("esc", k)).child(text("close").font(k.ty.small).color(k.c.text_faint))),
        Phase::Saving => row().gap(6.0).center_y().child(rune_kit::Spinner::new().build(k)).child(text("Saving…").font(k.ty.small).color(k.c.text_muted)),
        Phase::Saved(name) => row()
            .gap(6.0)
            .center_y()
            .child(icon(&icons::CHECK_CIRCLE, 14.0).color(theme::Tint::Code.color(k.dark)))
            .child(text(format!("Saved “{name}”")).font(k.ty.small_medium).color(k.c.text_base).truncate().max_w(380.0))
            .transition(rune::Transition::scale(0.9)),
        Phase::Failed(e) => text(e.clone()).font(k.ty.small).color(k.c.danger.fg).truncate().max_w(420.0),
    };
    let card = col()
        .key("capture")
        .w_full()
        .padding(14.0)
        .gap(12.0)
        .radius(16.0)
        .fill(k.c.surface_raised)
        .border(1.0, k.c.border_base)
        .child(row().w_full().gap(8.0).center_y().child(widgets::tile(glyph, tint, k.dark, 22.0)).child(text(label).font(k.ty.ui_medium).color(k.c.text_base).truncate()).child(rune::spacer()).child(text("Turbodoc").font(k.ty.small).color(k.c.text_faint)))
        .child(field)
        .child(row().w_full().justify_between().center_y().child(status).children((!s.auth.signed_in()).then(|| text("Not signed in").font(k.ty.small).color(k.c.warning.fg))));
    el().w_full().child(card)
}
