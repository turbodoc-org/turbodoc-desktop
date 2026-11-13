//! Bookmarks: a page header, one row of filters, and a grid of preview cards (or a list),
//! virtualised so thousands scroll at frame rate. Selecting a card opens the inspector with
//! the page's summary; double-clicking opens the page.
use std::rc::Rc;

use rune::{Cx, El, GrowthSpec, ScrollSpec, col, el, row, text};
use rune_kit::{Button, ButtonVariant, IconButton, IconButtonVariant, Kit, Markdown, Spinner};

use super::theme::{self, Tint};
use super::{icons, previews, widgets};
use crate::api::models::Bookmark;
use crate::app::{App, BookmarkFilter, Layout};
use crate::format;

const GAP: f32 = 20.0;
const PAD: f32 = 40.0;
const MIN_CARD: f32 = 250.0;

/// The bookmarks on screen, as indices into the library, newest first.
pub fn visible(s: &App) -> Vec<usize> {
    let q = s.search.text.trim();
    let fold = rune_kit::search::Fold::default();
    s.lib
        .bookmarks
        .iter()
        .enumerate()
        .filter(|(_, b)| s.bm_filter.keeps(b))
        .filter(|(_, b)| s.tag.as_ref().is_none_or(|t| b.tag_list().iter().any(|x| x.eq_ignore_ascii_case(t))))
        .filter(|(_, b)| {
            q.is_empty()
                || fold.contains(&b.title, q)
                || fold.contains(&b.url, q)
                || b.tags.as_deref().is_some_and(|t| fold.contains(t, q))
                || b.summary.as_deref().is_some_and(|t| fold.contains(t, q))
        })
        .map(|(i, _)| i)
        .collect()
}

thread_local! {
    /// The system appearance the current view was built in, for virtual row builders.
    pub static DARK: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// The kit's appearance, read again inside virtual rows (which only see state).
fn row_kit(s: &App) -> &'static Kit { theme::kit(s.dark(DARK.with(|d| d.get()))) }

pub fn view(s: &App, k: &Kit, cx: &mut Cx<App>) -> El {
    let b = &s.lib.bookmarks;
    let unread = b.iter().filter(|x| x.unread()).count();
    let title = match (&s.tag, s.bm_filter) {
        (Some(t), _) => format!("#{t}"),
        (None, BookmarkFilter::All) => "Bookmarks".to_owned(),
        (None, f) => f.title().to_owned(),
    };
    let shown = Rc::new(visible(s));
    let subtitle = if !s.loaded {
        String::new()
    } else if s.bm_filter == BookmarkFilter::All && s.tag.is_none() && s.search.text.is_empty() {
        let n = b.iter().filter(|x| !x.archived()).count();
        if unread > 0 { format!("{} · {unread} unread", format::count(n, "bookmark", "bookmarks")) } else { format::count(n, "bookmark", "bookmarks") }
    } else {
        format::count(shown.len(), "bookmark", "bookmarks")
    };
    let head = col()
        .key("bm-top")
        .w_full()
        .px(PAD)
        .pt(20.0)
        .pb(20.0)
        .gap(22.0)
        .child(widgets::page_header(&title, &subtitle, super::shell::new_button(s, k), k))
        .child(filters(s, k, cx.layout_size("bm-top").map_or(900.0, |sz| sz[0]) - 2.0 * PAD));

    let content: El = if !s.loaded {
        loading_grid(k)
    } else if shown.is_empty() {
        empty(s, k)
    } else {
        let width = cx.layout_size("bm-scroll").map_or(900.0, |sz| sz[0]) - 2.0 * PAD;
        let rows = match s.layout {
            Layout::Grid => {
                let cols = (((width + GAP) / (MIN_CARD + GAP)).floor() as usize).clamp(1, 5);
                let card_w = ((width - GAP * (cols as f32 - 1.0)) / cols as f32).floor();
                let n_rows = shown.len().div_ceil(cols);
                let est = ((card_w - 16.0) * 10.0 / 16.0) + 96.0 + GAP;
                let (a, b) = (shown.clone(), shown.clone());
                col().key("bm-rows").w_full().growth(GrowthSpec::default()).virtual_rows_version((cols, shown.len(), s.lib.bookmarks.len(), s.search.text.len(), s.tag.clone())).virtual_rows(
                    n_rows,
                    est,
                    move |s: &App, r| format!("row-{}", s.lib.bookmarks.get(a[r * cols]).map_or("", |b| b.id.as_str())).into(),
                    move |s: &App, r| {
                        let mut line = row().w_full().px(PAD).gap(GAP).pb(GAP).items_stretch();
                        for c in 0..cols {
                            line = match b.get(r * cols + c).and_then(|&i| s.lib.bookmarks.get(i)) {
                                Some(bm) => line.child(card(s, bm, card_w, row_kit(s), "")),
                                None => line.child(el().w(card_w)),
                            };
                        }
                        line
                    },
                )
            }
            Layout::List => {
                let (a, b) = (shown.clone(), shown.clone());
                col().key("bm-rows").w_full().growth(GrowthSpec::default()).virtual_rows_version((shown.len(), s.lib.bookmarks.len(), s.search.text.len(), s.tag.clone())).virtual_rows(
                    shown.len(),
                    72.0,
                    move |s: &App, r| format!("li-{}", s.lib.bookmarks.get(a[r]).map_or("", |b| b.id.as_str())).into(),
                    move |s: &App, r| match s.lib.bookmarks.get(b[r]) {
                        Some(bm) => list_row(s, bm, row_kit(s)),
                        None => el(),
                    },
                )
            }
        };
        col().key("bm-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).scroll_edge_fade(16.0).scroll_anchor().child(rows)
    };
    col().w_full().h_full().min_h(0.0).child(head).child(content)
}

/// Status filters, then the most-used tags, then the layout switch.
fn filters(s: &App, k: &Kit, width: f32) -> El {
    let b = &s.lib.bookmarks;
    let broken = b.iter().filter(|x| x.broken()).count();
    let status = |f: BookmarkFilter, label: &str| {
        widgets::chip(label, s.bm_filter == f && s.tag.is_none(), false, k).key(format!("filter-{f:?}")).on_press(move |s: &mut App| {
            s.bm_filter = f;
            s.tag = None;
            s.selected = None;
        })
    };
    let mut r = row()
        .key("bm-filters")
        .w_full()
        .gap(4.0)
        .center_y()
        .child(status(BookmarkFilter::All, "All"))
        .child(status(BookmarkFilter::Unread, "Unread"))
        .child(status(BookmarkFilter::Favorites, "Favorites"))
        .child(status(BookmarkFilter::Archived, "Archived"));
    if broken > 0 {
        let on = s.bm_filter == BookmarkFilter::Broken;
        r = r.child(widgets::chip(&format!("{broken} broken"), on, true, k).key("filter-Broken").on_press(move |s: &mut App| {
            s.bm_filter = if on { BookmarkFilter::All } else { BookmarkFilter::Broken };
            s.tag = None;
        }));
    }
    let mut tags: Vec<(String, usize)> = Vec::new();
    for t in b.iter().flat_map(|x| x.tag_list()) {
        match tags.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&t)) {
            Some(e) => e.1 += 1,
            None => tags.push((t, 1)),
        }
    }
    tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    // As many tags as fit beside the status filters and the layout switch; the rest in a menu.
    let used = 4.0 * 92.0 + if broken > 0 { 100.0 } else { 0.0 } + 60.0;
    let room = ((width - used - 70.0) / 96.0).floor().max(0.0) as usize;
    let shown_tags = room.min(5).min(tags.len());
    if !tags.is_empty() {
        r = r.child(el().w(1.0).h(18.0).mx(8.0).fill(k.c.border_base));
        for (t, _) in tags.iter().take(shown_tags) {
            let tag = t.clone();
            r = r.child(widgets::chip(t, s.tag.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(t)), false, k).key(format!("tag-{t}")).on_press_tx(move |s: &mut App, tx| s.filter_tag(Some(tag.clone()), tx)));
        }
        if tags.len() > shown_tags {
            let rest: Vec<String> = tags.iter().skip(shown_tags).map(|(t, _)| t.clone()).collect();
            let label = if shown_tags == 0 { "Tags".to_owned() } else { format!("+{}", rest.len()) };
            let active = s.tag.as_ref().is_some_and(|t| rest.iter().any(|r| r.eq_ignore_ascii_case(t)));
            r = r.child(widgets::chip(&label, active, false, k).key("tags-more").on_press_tx(move |s: &mut App, tx| {
                let current = s.tag.clone().unwrap_or_default();
                let menu = rune::PopupMenu::new().items(rest.iter().map(|t| rune::PopupItem::new(t.clone(), t.clone()).checked(*t == current)));
                tx.popup_menu(menu, "tags-more", |s: &mut App, picked, tx| {
                    if let Some(t) = picked {
                        s.filter_tag(Some(t.to_owned()), tx);
                    }
                });
            }));
        }
    }
    let grid = s.layout == Layout::Grid;
    r.child(rune::spacer()).child(
        IconButton::new(if grid { &icons::LIST } else { &icons::GRID })
            .variant(IconButtonVariant::GhostMuted)
            .help(if grid { "Show as list" } else { "Show as grid" })
            .label(if grid { "Show as list" } else { "Show as grid" })
            .on_press(|s: &mut App| {
                s.layout = if s.layout == Layout::Grid { Layout::List } else { Layout::Grid };
                let id = if s.layout == Layout::Grid { "grid" } else { "list" };
                s.set_pref("layout", &id);
            })
            .build(k)
            .key("layout-toggle"),
    )
}

fn loading_grid(k: &Kit) -> El {
    let w = 260.0;
    col().key("bm-loading").w_full().px(PAD).gap(GAP).child(row().gap(GAP).children((0..3).map(|i| previews::skeleton_card(i, w, k))))
}

fn empty(s: &App, k: &Kit) -> El {
    let capture = format!("Paste a link anywhere in this window, or press {} in any app.", crate::keys::label(crate::keys::CAPTURE));
    let (title, body): (&str, &str) = if !s.search.text.trim().is_empty() {
        ("No matches", "Try a different search or tag. Titles, links, tags and summaries are all searched.")
    } else {
        match s.bm_filter {
            BookmarkFilter::Unread => ("You're all caught up", "Everything you saved has been read. New bookmarks land here first."),
            BookmarkFilter::Favorites => ("No favorites yet", "Star a bookmark to keep it here."),
            BookmarkFilter::Archived => ("Nothing archived", "Archive bookmarks you're done with to keep the list short."),
            BookmarkFilter::Broken => ("No broken links", "Turbodoc checks every link each week."),
            BookmarkFilter::All => ("Save your first link", &capture),
        }
    };
    let action = (s.bm_filter == BookmarkFilter::All && s.search.text.is_empty() && s.tag.is_none())
        .then(|| Button::new("Add bookmark").icon(&icons::PLUS).variant(ButtonVariant::Contrast).on_press(|s: &mut App| s.new_bookmark_dialog(None)).build(k).key("empty-add"));
    widgets::empty(&icons::BOOKMARK, Tint::Bookmark, title, body, action, k)
}

/// One bookmark as a card. `prefix` keeps keys unique where a bookmark shows twice (Home).
pub fn card(s: &App, b: &Bookmark, w: f32, k: &Kit, prefix: &str) -> El {
    let selected = prefix.is_empty() && s.selected.as_deref() == Some(b.id.as_str());
    let domain = format::domain(&b.url);
    let saved = format::ago_from(chrono::DateTime::from_timestamp(b.time_added, 0).unwrap_or_default(), chrono::Utc::now()).replace("Yesterday", "yesterday");
    let (details, details_color) = if b.broken() { (format!("Link broken · {domain}"), Some(k.c.danger.fg)) } else { (format!("{domain} · Saved {saved}"), None) };
    let badge = b.archived().then(|| widgets::badge("Archived", None));
    let dot = (b.unread() && !b.broken()).then(|| Tint::Bookmark.color(k.dark));
    let (fav_id, open_id) = (b.id.clone(), b.id.clone());
    let actions = vec![
        widgets::card_action(if b.favorite() { &icons::STAR_FILL } else { &icons::STAR }, if b.favorite() { "Remove from favorites" } else { "Add to favorites" }, k, move |s, tx| s.toggle_favorite_bookmark(&fav_id, tx)),
        widgets::card_action(&icons::EXTERNAL, "Open in browser", k, move |s, tx| s.open_bookmark(&open_id, tx)),
    ];
    let (id, id_open, url_vis) = (b.id.clone(), b.id.clone(), b.url.clone());
    let link = b.url.clone();
    let c = widgets::card(
        widgets::Card { key: format!("{prefix}card-{}", b.id), tint: Tint::Bookmark, glyph: &icons::BOOKMARK, title: b.display_title().to_owned(), details, details_color, dot, preview: previews::bookmark(s, b, k), badge, actions, selected, width: w },
        k,
    )
    .help(b.url.clone())
    .on_press(move |s: &mut App| s.select_bookmark(&id))
    .on_double_press_tx(move |s: &mut App, tx| s.open_bookmark(&id_open, tx))
    .on_visible(move |s: &mut App, on, _| {
        if on {
            s.want_preview(&url_vis);
        }
    })
    .drag_out(move |_: &App| Some(rune::DragItem::new().text(link.clone())));
    widgets::with_menu(c, b.id.clone())
}

fn list_row(s: &App, b: &Bookmark, k: &Kit) -> El {
    let selected = s.selected.as_deref() == Some(b.id.as_str());
    let ink = theme::ink(k.dark);
    let (id, id_open, url_vis, link) = (b.id.clone(), b.id.clone(), b.url.clone(), b.url.clone());
    let domain = format::domain(&b.url);
    let saved = format::ago_from(chrono::DateTime::from_timestamp(b.time_added, 0).unwrap_or_default(), chrono::Utc::now()).replace("Yesterday", "yesterday");
    let thumb = el().size(72.0, 46.0).radius(7.0).clip_rounded(7.0).fill(ink.thumb).border(1.0, ink.thumb_line).child(previews::bookmark(s, b, k));
    let (fav_id, open2) = (b.id.clone(), b.id.clone());
    let actions = row()
        .gap(6.0)
        .group_opacity(0.0, 1.0)
        .child(widgets::card_action(if b.favorite() { &icons::STAR_FILL } else { &icons::STAR }, "Favorite", k, move |s, tx| s.toggle_favorite_bookmark(&fav_id, tx)))
        .child(widgets::card_action(&icons::EXTERNAL, "Open in browser", k, move |s, tx| s.open_bookmark(&open2, tx)));
    let details = if b.broken() { format!("Link broken · {domain}") } else { format!("{domain} · Saved {saved}") };
    let r = row()
        .key(format!("li-{}", b.id))
        .w_full()
        .px(PAD - 12.0)
        .child(
            row()
                .w_full()
                .h(68.0)
                .px(12.0)
                .gap(14.0)
                .radius(10.0)
                .center_y()
                .fill(if selected { k.c.border_focus.alpha(0.10) } else { rune::Color::TRANSPARENT })
                .hover(k.c.hover)
                .hover_group()
                .cursor(rune::Cursor::Pointer)
                .role("button")
                .label(b.display_title().to_owned())
                .focusable(true)
                .child(thumb)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(3.0)
                        .child(row().gap(8.0).center_y().child(text(b.display_title().to_owned()).font(theme::CARD_TITLE).color(k.c.text_base).truncate().shrink()).children(b.unread().then(|| widgets::dot(Tint::Bookmark.color(k.dark), 6.0))))
                        .child(text(details).font(theme::DETAILS).color(if b.broken() { k.c.danger.fg } else { k.c.text_faint }).truncate()),
                )
                .child(actions)
                .help(b.url.clone())
                .on_press(move |s: &mut App| s.select_bookmark(&id))
                .on_double_press_tx(move |s: &mut App, tx| s.open_bookmark(&id_open, tx))
                .drag_out(move |_: &App| Some(rune::DragItem::new().text(link.clone())))
                .pipe_menu(b.id.clone()),
        )
        .on_visible(move |s: &mut App, on, _| {
            if on {
                s.want_preview(&url_vis);
            }
        });
    r
}

/// The selected bookmark, in full.
pub fn inspector(s: &App, k: &Kit) -> El {
    let Some(b) = s.selected.as_ref().and_then(|id| s.lib.bookmarks.iter().find(|b| &b.id == id)) else {
        return el().key("inspector-empty");
    };
    let ink = theme::ink(k.dark);
    let id = b.id.clone();
    let open = {
        let id = id.clone();
        Button::new("Open").icon(&icons::EXTERNAL).variant(ButtonVariant::Contrast).on_press_tx(move |s: &mut App, tx| s.open_bookmark(&id, tx)).build(k).key("inspector-open")
    };
    let star = {
        let id = id.clone();
        IconButton::new(if b.favorite() { &icons::STAR_FILL } else { &icons::STAR })
            .variant(IconButtonVariant::GhostMuted)
            .label(if b.favorite() { "Remove from favorites" } else { "Add to favorites" })
            .help(if b.favorite() { "Remove from favorites" } else { "Add to favorites" })
            .on_press_tx(move |s: &mut App, tx| s.toggle_favorite_bookmark(&id, tx))
            .build(k)
            .key("inspector-star")
    };
    let read = {
        let id = id.clone();
        IconButton::new(if b.unread() { &icons::CHECK_CIRCLE } else { &icons::CIRCLE })
            .variant(IconButtonVariant::GhostMuted)
            .label(if b.unread() { "Mark as read" } else { "Mark as unread" })
            .help(if b.unread() { "Mark as read" } else { "Mark as unread" })
            .on_press_tx(move |s: &mut App, tx| s.toggle_read(&id, tx))
            .build(k)
    };
    let more = {
        let id = id.clone();
        IconButton::new(&icons::MORE)
            .variant(IconButtonVariant::GhostMuted)
            .label("More")
            .help("More")
            .on_press_tx(move |s: &mut App, tx| {
                let menu = widgets::item_menu(s, &id);
                let id = id.clone();
                tx.popup_menu(menu, "inspector-more", move |s: &mut App, p, tx| widgets::item_menu_pick(s, &id, p, tx));
            })
            .build(k)
            .key("inspector-more")
    };
    let summary: El = match (&b.summary, b.summarising()) {
        (Some(sum), _) if !sum.trim().is_empty() => Markdown::new(sum.clone()).selectable(true).build(k),
        (_, true) => row().gap(8.0).center_y().child(Spinner::new().build(k)).child(text("Reading the page…").font(k.ty.ui).color(k.c.text_faint)),
        _ => text("No summary for this page.").font(k.ty.ui).color(k.c.text_faint),
    };
    let section = |label: &str, body: El| col().w_full().gap(8.0).child(text(label.to_owned()).font(theme::LABEL).color(k.c.text_faint)).child(body);
    let tags = b.tag_list();
    let saved = format::long_date(&chrono::DateTime::from_timestamp(b.time_added, 0).unwrap_or_default().to_rfc3339());
    let status = if b.archived() { "Archived" } else if b.unread() { "Unread" } else { "Read" };
    let link_state = match b.link_status.as_deref() {
        Some("broken") => "Link broken",
        Some("ok") => "Link works",
        _ => "Link not checked yet",
    };
    let url = b.url.clone();
    let body = col()
        .key(format!("inspector-{}", b.id))
        .w_full()
        .gap(24.0)
        .px(20.0)
        .pt(16.0)
        .pb(28.0)
        .children(s.preview(&b.url).is_some().then(|| el().w_full().h(168.0).radius(10.0).clip_rounded(10.0).fill(ink.thumb).border(1.0, ink.thumb_line).child(previews::bookmark(s, b, k))))
        .child(
            row()
                .w_full()
                .gap(12.0)
                .items_start()
                .children(s.preview(&b.url).is_none().then(|| site_tile(&format::domain(&b.url))))
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(4.0)
                        .child(text(b.display_title().to_owned()).font(theme::H2).color(k.c.text_strong).wrap().selectable(true).selection_color(k.selection()))
                        .child(
                            text(format::domain(&b.url))
                                .font(k.ty.ui)
                                .color(k.c.text_faint)
                                .cursor(rune::Cursor::Pointer)
                                .help(b.url.clone())
                                .on_press_tx(move |_: &mut App, tx| {
                                    let _ = tx.open_url(&url);
                                }),
                        ),
                ),
        )
        .child(row().w_full().gap(4.0).center_y().child(open).child(rune::spacer()).child(read).child(star).child(more))
        .child(section("Summary", summary))
        .children((!tags.is_empty()).then(|| section("Tags", widgets::tag_row(&tags, k, 20, None))))
        .child(section("Details", text(format!("Saved {saved}\n{status} · {link_state}")).font(k.ty.ui).color(k.c.text_muted).wrap()))
        .transition(rune::Transition::opacity());
    col().key("inspector-scroll").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)
}

/// A site's initial in its hue: the inspector's mark for a page without a preview image.
fn site_tile(domain: &str) -> El {
    let hue = theme::hue_for(domain);
    el().size(40.0, 40.0)
        .radius(10.0)
        .fill(rune::oklch(0.52, 0.11, hue))
        .center()
        .child(text(domain.chars().next().unwrap_or('•').to_ascii_uppercase().to_string()).font(rune::TextStyle::new(17.0, 650.0)).color(rune::Color::WHITE))
        .a11y_hidden()
}

trait PipeMenu {
    fn pipe_menu(self, id: String) -> Self;
}
impl PipeMenu for El {
    fn pipe_menu(self, id: String) -> Self { widgets::with_menu(self, id) }
}
