//! Home, as the web app's dashboard: a greeting, the four kinds of item, where you left off,
//! and what's waiting to be read. One idea per section, and room around each.
use chrono::Timelike;
use rune::{Cx, El, Icon, ScrollSpec, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, Kit};

use super::theme::{self, Tint};
use super::{icons, previews, widgets};
use crate::api::auth::User;
use crate::app::{App, BookmarkFilter, Section};
use crate::format;

const GAP: f32 = 16.0;
const PAD: f32 = 40.0;
const MAX_W: f32 = 1120.0;

pub fn view(s: &App, user: &User, k: &Kit, cx: &mut Cx<App>) -> El {
    let outer = cx.layout_size("home-scroll").map_or(980.0, |sz| sz[0]);
    let width = (outer - 2.0 * PAD).min(MAX_W).max(320.0);
    let cols = (((width + GAP) / (220.0 + GAP)).floor() as usize).clamp(1, 4);
    let card_w = ((width - GAP * (cols as f32 - 1.0)) / cols as f32).floor();

    let part = match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    };
    let name = user.first_name();
    let greeting = if name.is_empty() { part.to_owned() } else { format!("{part}, {name}") };
    let unread = s.lib.bookmarks.iter().filter(|b| b.unread()).count();
    let mut subtitle = chrono::Local::now().format("%A, %-d %B").to_string();
    if s.loaded && unread > 0 {
        subtitle.push_str(&format!(" · {} to read", format::count(unread, "bookmark", "bookmarks")));
    }
    let new = Button::new("New")
        .icon(&icons::PLUS)
        .variant(ButtonVariant::Contrast)
        .on_press_tx(|_: &mut App, tx| {
            let item = |id: &str, title: &str, symbol: &str| rune::PopupItem::new(id, title).symbol(symbol);
            let menu = rune::PopupMenu::new()
                .item(item("note", "Note", "doc.text").key_equivalent(crate::keys::shortcut("mod+n")))
                .item(item("bookmark", "Bookmark", "bookmark").key_equivalent(crate::keys::shortcut("mod+shift+n")))
                .item(item("snippet", "Code snippet", "chevron.left.forwardslash.chevron.right"))
                .item(item("diagram", "Diagram", "point.3.connected.trianglepath.dotted"));
            tx.popup_menu(menu, "home-new", |s: &mut App, picked, tx| match picked {
                Some("note") => s.new_note(String::new(), tx),
                Some("bookmark") => s.new_bookmark_dialog(None),
                Some("snippet") => s.new_snippet(String::new(), None, tx),
                Some("diagram") => s.new_diagram_dialog(),
                _ => {}
            });
        })
        .build(k)
        .key("home-new");

    let b = &s.lib.bookmarks;
    let tiles = row()
        .key("modules")
        .w_full()
        .gap(GAP)
        .child(tile(s, Section::Bookmarks, Tint::Bookmark, &icons::BOOKMARK, stats(s.loaded, b.iter().filter(|x| !x.archived()).count(), "bookmark", "bookmarks", b.iter().filter_map(|x| x.created_at.as_deref())), k))
        .child(tile(s, Section::Notes, Tint::Note, &icons::NOTE, stats(s.loaded, s.lib.notes.len(), "note", "notes", s.lib.notes.iter().filter_map(|x| x.updated_at.as_deref())), k))
        .child(tile(s, Section::Snippets, Tint::Code, &icons::CODE, stats(s.loaded, s.lib.snippets.len(), "snippet", "snippets", s.lib.snippets.iter().filter_map(|x| x.updated_at.as_deref())), k))
        .child(tile(s, Section::Diagrams, Tint::Diagram, &icons::DIAGRAM, stats(s.loaded, s.lib.diagrams.len(), "diagram", "diagrams", s.lib.diagrams.iter().filter_map(|x| x.updated_at.as_deref())), k));

    let mut body = col()
        .key("home-body")
        .w(width)
        .pt(24.0)
        .pb(56.0)
        .gap(40.0)
        .child(widgets::page_header(&greeting, &subtitle, Some(new), k))
        .child(tiles);

    let recent = recent(s, cols, card_w, k);
    if s.loaded && recent.is_empty() {
        body = body.child(widgets::empty(&icons::SPARKLE, Tint::Bookmark, "Your workspace is empty", "Save a link, write a note, or drop files on this window to import them.", None, k).h(240.0));
    } else {
        body = body.child(col().w_full().gap(16.0).child(widgets::section_title("Jump back in", "Recently edited", None, k)).child(row().w_full().gap(GAP).items_stretch().children(recent)));
    }
    let reading: Vec<_> = s.lib.bookmarks.iter().filter(|b| b.unread()).take(cols).collect();
    if s.loaded && !reading.is_empty() {
        let see_all = widgets::link("See all", k).key("see-all").on_press_tx(|s: &mut App, tx| {
            s.go(Section::Bookmarks, tx);
            s.bm_filter = BookmarkFilter::Unread;
        });
        let cards = reading.into_iter().map(|bm| super::bookmarks::card(s, bm, card_w, k, "home-"));
        body = body.child(
            col()
                .w_full()
                .gap(16.0)
                .child(widgets::section_title("Reading list", &format!("{unread} unread"), Some(see_all), k))
                .child(row().w_full().gap(GAP).items_stretch().children(cards)),
        );
    }
    col().key("home-scroll").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).scroll_edge_fade(16.0).items_center().child(body)
}

fn stats<'a>(loaded: bool, n: usize, one: &str, many: &str, times: impl Iterator<Item = &'a str>) -> Option<String> {
    if !loaded {
        return None;
    }
    if n == 0 {
        return Some("Nothing yet".into());
    }
    let latest = times.max().map(format::ago_inline).unwrap_or_default();
    Some(if latest.is_empty() { format::count(n, one, many) } else { format!("{} · {}", format::count(n, one, many), latest) })
}

fn tile(s: &App, section: Section, tint: Tint, glyph: &'static Icon, stats: Option<String>, k: &Kit) -> El {
    let ink = theme::ink(k.dark);
    let add = el()
        .key(format!("tile-add-{}", section.id()))
        .size(28.0, 28.0)
        .radius(8.0)
        .border(1.0, k.c.border_base)
        .center()
        .cursor(rune::Cursor::Pointer)
        .hover(k.c.hover)
        .role("button")
        .label(format!("New in {}", section.title()))
        .help(format!("New in {}", section.title()))
        .child(icon(&icons::PLUS, 13.0).color(k.c.text_muted))
        .group_opacity(0.0, 1.0)
        .on_press_tx(move |s: &mut App, tx| {
            s.go(section, tx);
            s.new_in_section(tx);
        });
    let stat: El = match stats {
        Some(t) => text(t).font(theme::DETAILS).color(k.c.text_faint).truncate(),
        None => widgets::skeleton(Some(96.0), 10.0, k).mt(3.0),
    };
    let _ = s;
    col()
        .key(format!("stat-{}", section.id()))
        .grow(1.0)
        .min_w(0.0)
        .padding(16.0)
        .gap(14.0)
        .radius(14.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_base)
        .hover_group()
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(section.title())
        .focusable(true)
        .child(el().absolute(0.0, 0.0).w_full().h_full().radius(14.0).border(1.0, ink.line_strong).shadow(10.0, 26.0, ink.lift).group_opacity(0.0, 1.0).inert().a11y_hidden())
        .child(row().w_full().justify_between().center_y().child(widgets::tile(glyph, tint, k.dark, 36.0)).child(add))
        .child(col().gap(3.0).child(text(section.title()).font(theme::HEADING).color(k.c.text_base)).child(stat))
        .on_press_tx(move |s: &mut App, tx| s.go(section, tx))
}

/// The latest edits across every kind, as cards.
fn recent(s: &App, cols: usize, w: f32, k: &Kit) -> Vec<El> {
    if !s.loaded {
        return (0..cols).map(|i| previews::skeleton_card(i, w, k)).collect();
    }
    enum R<'a> {
        Note(&'a crate::api::models::Note),
        Snippet(&'a crate::api::models::Snippet),
        Diagram(&'a crate::api::models::Diagram),
        Bookmark(&'a crate::api::models::Bookmark),
    }
    let mut items: Vec<(&str, R)> = Vec::new();
    items.extend(s.lib.notes.iter().map(|n| (n.updated_at.as_deref().unwrap_or(""), R::Note(n))));
    items.extend(s.lib.snippets.iter().map(|x| (x.updated_at.as_deref().unwrap_or(""), R::Snippet(x))));
    items.extend(s.lib.diagrams.iter().map(|d| (d.updated_at.as_deref().unwrap_or(""), R::Diagram(d))));
    items.extend(s.lib.bookmarks.iter().filter(|b| !b.unread()).take(20).map(|b| (b.created_at.as_deref().unwrap_or(""), R::Bookmark(b))));
    items.sort_by(|a, b| b.0.cmp(a.0));
    items
        .into_iter()
        .take(cols)
        .map(|(when, item)| match item {
            R::Bookmark(b) => super::bookmarks::card(s, b, w, k, "recent-"),
            R::Note(n) => {
                let id = n.id.clone();
                let details = format!("Edited {}", format::ago_inline(when));
                widgets::with_menu(
                    widgets::card(widgets::Card { key: format!("recent-{}", n.id), tint: Tint::Note, glyph: &icons::NOTE, title: n.display_title(), details, details_color: None, dot: None, preview: previews::note(n, k), badge: None, actions: Vec::new(), selected: false, width: w }, k)
                        .on_press_tx(move |s: &mut App, tx| {
                            s.go(Section::Notes, tx);
                            s.open_note(&id, tx);
                        }),
                    n.id.clone(),
                )
            }
            R::Snippet(x) => {
                let id = x.id.clone();
                let details = format!("{} · {}", crate::api::models::language_name(&x.language), format::ago_inline(when));
                widgets::with_menu(
                    widgets::card(widgets::Card { key: format!("recent-{}", x.id), tint: Tint::Code, glyph: &icons::CODE, title: x.display_title().to_owned(), details, details_color: None, dot: None, preview: previews::snippet(x, k), badge: None, actions: Vec::new(), selected: false, width: w }, k)
                        .on_press_tx(move |s: &mut App, tx| {
                            s.go(Section::Snippets, tx);
                            s.open_snippet(&id);
                        }),
                    x.id.clone(),
                )
            }
            R::Diagram(d) => {
                let id = d.id.clone();
                let details = format!("{} · {}", if d.is_mermaid() { "Mermaid" } else { "Canvas" }, format::ago_inline(when));
                widgets::with_menu(
                    widgets::card(widgets::Card { key: format!("recent-{}", d.id), tint: Tint::Diagram, glyph: &icons::DIAGRAM, title: d.title.clone(), details, details_color: None, dot: None, preview: previews::diagram(d, w, k), badge: None, actions: Vec::new(), selected: false, width: w }, k)
                        .on_press_tx(move |s: &mut App, tx| {
                            s.go(Section::Diagrams, tx);
                            s.open_diagram(&id);
                        }),
                    d.id.clone(),
                )
            }
        })
        .collect()
}
