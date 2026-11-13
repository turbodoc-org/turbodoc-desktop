//! The command palette (⌘K): one field that finds anything in the library and runs any command.
//! Typing a link offers to save it.
use rune::{El, Icon, Key, ScrollSpec, Tx, col, el, row, text};
use rune_kit::{Kit, TextInput};

use super::theme::Tint;
use super::{icons, widgets};
use crate::app::{App, Section};
use crate::format;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Go(Section),
    NewNote,
    NewBookmark,
    NewSnippet,
    NewDiagram,
    SaveUrl(String),
    Capture,
    Sync,
    Settings,
    Shortcuts,
    ToggleTheme,
    SignOut,
    Bookmark(String),
    Note(String),
    Snippet(String),
    Diagram(String),
    Tag(String),
}

pub struct Entry {
    pub key: String,
    pub title: String,
    pub sub: String,
    pub glyph: &'static Icon,
    pub tint: Option<Tint>,
    pub keys: &'static str,
    pub action: Action,
    score: i32,
}

fn score(hay: &str, q: &str) -> Option<i32> {
    if q.is_empty() {
        return Some(0);
    }
    let h = hay.to_lowercase();
    let q = q.to_lowercase();
    if h.starts_with(&q) {
        return Some(300 - h.len().min(200) as i32);
    }
    if let Some(p) = h.find(&format!(" {q}")) {
        return Some(200 - p.min(150) as i32);
    }
    if let Some(p) = h.find(&q) {
        return Some(120 - p.min(100) as i32);
    }
    // Every query character in order (fuzzy), for typos of omission.
    let mut it = h.chars();
    q.chars().all(|c| it.any(|x| x == c)).then_some(20)
}

pub fn entries(s: &App) -> Vec<Entry> {
    let q = s.palette.as_ref().map_or(String::new(), |p| p.query.text.trim().to_owned());
    let mut out: Vec<Entry> = Vec::new();
    if format::looks_like_url(&q) {
        let url = format::normalize_url(&q);
        out.push(Entry { key: "save-url".into(), title: format!("Save {}", format::domain(&url)), sub: url.clone(), glyph: &icons::BOOKMARK, tint: Some(Tint::Bookmark), keys: "↩", action: Action::SaveUrl(url), score: 10_000 });
    }
    let mut push = |key: String, title: String, sub: String, glyph: &'static Icon, tint: Option<Tint>, keys: &'static str, action: Action, boost: i32| {
        if let Some(sc) = score(&title, &q).or_else(|| (!sub.is_empty()).then(|| score(&sub, &q).map(|x| x / 3)).flatten()) {
            out.push(Entry { key, title, sub, glyph, tint, keys, action, score: sc + boost });
        }
    };
    let commands: [(&str, &str, &'static Icon, &'static str, Action); 14] = [
        ("New note", "Create", &icons::NOTE, "mod+n", Action::NewNote),
        ("Add bookmark", "Create", &icons::BOOKMARK, "mod+shift+n", Action::NewBookmark),
        ("New snippet", "Create", &icons::CODE, "", Action::NewSnippet),
        ("New diagram", "Create", &icons::DIAGRAM, "", Action::NewDiagram),
        ("Quick capture", "Save from any app", &icons::BOLT, crate::keys::CAPTURE, Action::Capture),
        ("Go to Home", "Navigate", &icons::GRID, "mod+1", Action::Go(Section::Home)),
        ("Go to Bookmarks", "Navigate", &icons::BOOKMARK, "mod+2", Action::Go(Section::Bookmarks)),
        ("Go to Notes", "Navigate", &icons::NOTE, "mod+3", Action::Go(Section::Notes)),
        ("Go to Snippets", "Navigate", &icons::CODE, "mod+4", Action::Go(Section::Snippets)),
        ("Go to Diagrams", "Navigate", &icons::DIAGRAM, "mod+5", Action::Go(Section::Diagrams)),
        ("Sync now", "Fetch the latest from Turbodoc", &icons::REFRESH, "mod+r", Action::Sync),
        ("Settings", "Account, appearance and digest", &icons::SETTINGS, "mod+comma", Action::Settings),
        ("Keyboard shortcuts", "Help", &icons::KEYBOARD, "mod+slash", Action::Shortcuts),
        ("Switch light / dark", "Appearance", &icons::EYE, "", Action::ToggleTheme),
    ];
    for (i, (title, sub, glyph, keys, action)) in commands.into_iter().enumerate() {
        push(format!("cmd-{i}"), title.into(), sub.into(), glyph, None, keys, action, if q.is_empty() { 500 - i as i32 } else { 40 });
    }
    push("cmd-signout".into(), "Sign out".into(), "Account".into(), &icons::SIGN_OUT, None, "", Action::SignOut, if q.is_empty() { -1000 } else { 0 });
    if q.is_empty() {
        for n in s.lib.notes.iter().take(5) {
            push(format!("n-{}", n.id), n.display_title(), "Recent note".into(), &icons::NOTE, Some(Tint::Note), "", Action::Note(n.id.clone()), 100);
        }
    } else {
        for b in &s.lib.bookmarks {
            push(format!("b-{}", b.id), b.display_title().to_owned(), format::domain(&b.url), &icons::BOOKMARK, Some(Tint::Bookmark), "", Action::Bookmark(b.id.clone()), 60);
        }
        for n in &s.lib.notes {
            let mut e_sub = format::preview(&n.markdown, 70);
            if e_sub.is_empty() {
                e_sub = "Note".into();
            }
            push(format!("n-{}", n.id), n.display_title(), e_sub, &icons::NOTE, Some(Tint::Note), "", Action::Note(n.id.clone()), 70);
        }
        for x in &s.lib.snippets {
            push(format!("s-{}", x.id), x.display_title().to_owned(), crate::api::models::language_name(&x.language).to_owned(), &icons::CODE, Some(Tint::Code), "", Action::Snippet(x.id.clone()), 60);
        }
        for d in &s.lib.diagrams {
            push(format!("d-{}", d.id), d.title.clone(), "Diagram".into(), &icons::DIAGRAM, Some(Tint::Diagram), "", Action::Diagram(d.id.clone()), 60);
        }
        for (t, n) in s.all_tags().into_iter().take(40) {
            push(format!("t-{t}"), format!("#{t}"), format::count(n, "item", "items"), &icons::HASH, None, "", Action::Tag(t), 50);
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out.truncate(50);
    out
}

pub fn run(s: &mut App, action: Action, tx: &mut Tx) {
    s.palette = None;
    match action {
        Action::Go(sec) => s.go(sec, tx),
        Action::NewNote => s.new_note(String::new(), tx),
        Action::NewBookmark => s.new_bookmark_dialog(None),
        Action::NewSnippet => s.new_snippet(String::new(), None, tx),
        Action::NewDiagram => s.new_diagram_dialog(),
        Action::SaveUrl(url) => s.new_bookmark_dialog(Some(url)),
        Action::Capture => s.open_capture(),
        Action::Sync => s.refresh(tx),
        Action::Settings => s.settings = true,
        Action::Shortcuts => s.dialog = Some(crate::app::Dialog::Shortcuts),
        Action::ToggleTheme => {
            let dark = s.dark(super::bookmarks::DARK.with(|d| d.get()));
            s.appearance = if dark { crate::app::Appearance::Light } else { crate::app::Appearance::Dark };
            s.set_pref("appearance", &s.appearance.id());
            tx.color_motion(rune::tok::motion::REFLOW);
        }
        Action::SignOut => s.sign_out(tx),
        Action::Bookmark(id) => {
            s.go(Section::Bookmarks, tx);
            s.bm_filter = if s.lib.bookmarks.iter().any(|b| b.id == id && b.archived()) { crate::app::BookmarkFilter::Archived } else { crate::app::BookmarkFilter::All };
            s.select_bookmark(&id);
            tx.scroll_into_view(format!("card-{id}"));
        }
        Action::Note(id) => {
            s.go(Section::Notes, tx);
            s.open_note(&id, tx);
        }
        Action::Snippet(id) => {
            s.go(Section::Snippets, tx);
            s.open_snippet(&id);
        }
        Action::Diagram(id) => {
            s.go(Section::Diagrams, tx);
            s.open_diagram(&id);
        }
        Action::Tag(t) => s.filter_tag(Some(t), tx),
    }
}

pub fn view(s: &App, k: &Kit) -> El {
    let Some(p) = &s.palette else { return el() };
    let list = entries(s);
    let active = p.active.min(list.len().saturating_sub(1));
    let field = TextInput::new(&p.query)
        .plain()
        .width(None)
        .large()
        .placeholder("Search everything, or type a command or a link…")
        .leading_icon(&icons::SEARCH)
        .label("Search")
        .on_edit(|s: &mut App| &mut s.palette.as_mut().expect("palette is open").query)
        .on_change(|s: &mut App, _| {
            if let Some(p) = &mut s.palette {
                p.active = 0;
            }
        })
        .on_key(|s: &mut App, ev, tx| {
            let n = entries(s).len();
            let Some(p) = &mut s.palette else { return false };
            match ev.key {
                Key::Down => {
                    p.active = if n == 0 { 0 } else { (p.active + 1) % n };
                    tx.scroll_into_view(format!("pal-row-{}", p.active));
                    true
                }
                Key::Up => {
                    p.active = if n == 0 { 0 } else { (p.active + n - 1) % n };
                    tx.scroll_into_view(format!("pal-row-{}", p.active));
                    true
                }
                Key::Enter => {
                    let i = p.active;
                    if let Some(e) = entries(s).into_iter().nth(i) {
                        run(s, e.action, tx);
                    }
                    true
                }
                Key::Escape => {
                    s.palette = None;
                    true
                }
                _ => false,
            }
        })
        .build(k)
        .key("palette-input");
    let rows: Vec<El> = list
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let lead = match e.tint {
                Some(t) => widgets::tile(e.glyph, t, k.dark, 26.0),
                None => el().size(26.0, 26.0).radius(7.0).fill(k.c.layer_02).center().child(rune::icon(e.glyph, 14.0).color(k.c.icon_muted)),
            };
            let action = e.action.clone();
            let mut r = row()
                .key(format!("pal-row-{i}"))
                .w_full()
                .h(42.0)
                .px(8.0)
                .gap(10.0)
                .radius(k.radius.md)
                .center_y()
                .fill(if i == active { k.c.border_focus.alpha(0.14) } else { rune::Color::TRANSPARENT })
                .hover(k.c.hover)
                .cursor(rune::Cursor::Pointer)
                .role("option")
                .label(e.title.clone())
                .child(lead)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(1.0)
                        .child(text(e.title.clone()).font(k.ty.ui_medium).color(k.c.text_base).truncate())
                        .children((!e.sub.is_empty()).then(|| text(e.sub.clone()).font(k.ty.small).color(k.c.text_faint).truncate())),
                )
                .on_press_tx(move |s: &mut App, tx| run(s, action.clone(), tx));
            if !e.keys.is_empty() {
                r = r.child(widgets::keycap(e.keys, k));
            }
            let _ = &e.key;
            r
        })
        .collect();
    let results: El = if rows.is_empty() {
        col().w_full().padding(24.0).center().child(text("No results").font(k.ty.ui).color(k.c.text_faint))
    } else {
        col().key("palette-results").w_full().max_h(380.0).scroll(ScrollSpec { stick_bottom: false }).scroll_edge_fade(8.0).child(col().w_full().padding(6.0).gap(1.0).children(rows))
    };
    let footer = row()
        .w_full()
        .h(32.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak)
        .child(row().gap(4.0).center_y().child(widgets::keycap("↑↓", k)).child(text("move").font(k.ty.small).color(k.c.text_faint)))
        .child(row().gap(4.0).center_y().child(widgets::keycap("enter", k)).child(text("open").font(k.ty.small).color(k.c.text_faint)))
        .child(row().gap(4.0).center_y().child(widgets::keycap("esc", k)).child(text("close").font(k.ty.small).color(k.c.text_faint)));
    let panel = col()
        .key("palette")
        .w(600.0)
        .radius(14.0)
        .fill(k.c.surface_raised)
        .border(1.0, k.c.border_base)
        .shadow(24.0, 64.0, rune::Color::BLACK.alpha(if k.dark { 0.55 } else { 0.18 }))
        .clip_rounded(14.0)
        .child(row().w_full().px(10.0).py(6.0).border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak).child(field))
        .child(results)
        .child(footer)
        .transition(rune::Transition::scale(0.97).combined(rune::Transition::opacity()));
    col()
        .key("palette-scrim")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .items_center()
        .pt(90.0)
        .fill(k.c.scrim.alpha(0.35))
        .transition(rune::Transition::opacity())
        .modal(true, |s: &mut App| s.palette = None)
        .child(panel)
}
