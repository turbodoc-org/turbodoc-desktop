//! Small pieces every screen shares.
use rune::{Color, El, Icon, PopupItem, PopupMenu, TextStyle, col, el, icon, row, text};
use rune_kit::Kit;

use super::theme::{self, Tint};
use crate::app::App;

/// A rounded tile with an item kind's icon on a wash of its colour.
pub fn tile(glyph: &'static Icon, tint: Tint, dark: bool, side: f32) -> El {
    let r = (side * 0.28).round();
    el().size(side, side)
        .radius(r)
        .fill(tint.wash(dark))
        .center()
        .child(icon(glyph, (side * 0.52).round()).color(tint.color(dark)))
        .a11y_hidden()
}

/// A tag: neutral, as text in a soft capsule. Colour is kept for what kind a thing is.
pub fn tag_chip(tag: &str, k: &Kit, selected: bool) -> El {
    row()
        .h(22.0)
        .px(8.0)
        .radius(11.0)
        .fill(if selected { k.c.text_base } else { k.c.layer_02 })
        .center_y()
        .child(text(tag.to_owned()).font(theme::LABEL).color(if selected { k.c.bg_base } else { k.c.text_muted }).truncate())
}

/// Tags in a wrapping row, at most `max` and a "+N".
pub fn tag_row(tags: &[String], k: &Kit, max: usize, active: Option<&str>) -> El {
    let mut r = row().gap(6.0).flex_wrap();
    for t in tags.iter().take(max) {
        r = r.child(tag_chip(t, k, active.is_some_and(|a| a.eq_ignore_ascii_case(t))));
    }
    if tags.len() > max {
        r = r.child(text(format!("+{}", tags.len() - max)).font(theme::LABEL).color(k.c.text_faint));
    }
    r
}

/// A key cap for a chord ("mod+k" draws "⌘K" on macOS, "Ctrl+K" elsewhere) or a literal ("↑↓").
pub fn keycap(spec: &str, k: &Kit) -> El {
    let label = crate::keys::label(spec);
    row()
        .h(20.0)
        .px(6.0)
        .radius(5.0)
        .border(1.0, k.c.border_base)
        .center()
        .child(text(label.to_owned()).font(k.ty.text_xsmall_medium).color(k.c.text_faint))
}

/// A page's title, with one faint line under it and the page's main action beside it.
pub fn page_header(title: &str, subtitle: &str, action: Option<El>, k: &Kit) -> El {
    row()
        .key("page-header")
        .w_full()
        .items_end()
        .justify_between()
        .gap(16.0)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(text(title.to_owned()).font(theme::H1).color(k.c.text_strong).truncate())
                .children((!subtitle.is_empty()).then(|| text(subtitle.to_owned()).font(k.ty.ui).color(k.c.text_faint).truncate())),
        )
        .children(action)
}

/// A section's title, with an optional faint hint beside it.
pub fn section_title(title: &str, hint: &str, trailing: Option<El>, k: &Kit) -> El {
    row()
        .w_full()
        .items_center()
        .justify_between()
        .gap(12.0)
        .child(
            row()
                .gap(10.0)
                .items_end()
                .child(text(title.to_owned()).font(theme::H2).color(k.c.text_base))
                .children((!hint.is_empty()).then(|| text(hint.to_owned()).font(theme::DETAILS).color(k.c.text_faint).pb(2.0))),
        )
        .children(trailing)
}

/// A filter chip: quiet until chosen, then solid.
pub fn chip(label: &str, active: bool, danger: bool, k: &Kit) -> El {
    let (fill, ink, border) = match (active, danger) {
        (true, false) => (k.c.text_base, k.c.bg_base, rune::Color::TRANSPARENT),
        (true, true) => (k.c.danger.fg, rune::Color::WHITE, rune::Color::TRANSPARENT),
        (false, true) => (rune::Color::TRANSPARENT, k.c.danger.fg, k.c.danger.fg.alpha(0.4)),
        (false, false) => (rune::Color::TRANSPARENT, k.c.text_muted, rune::Color::TRANSPARENT),
    };
    row()
        .h(30.0)
        .px(12.0)
        .radius(15.0)
        .fill(fill)
        .border(1.0, border)
        .center()
        .cursor(rune::Cursor::Pointer)
        .hover(if active { fill } else { k.c.hover })
        .role("button")
        .label(label.to_owned())
        .checked(active)
        .focusable(true)
        .child(text(label.to_owned()).font(if active { k.ty.ui_medium } else { k.ty.ui }).color(ink))
}

/// A small text link: "See all →".
pub fn link(label: &str, k: &Kit) -> El {
    row()
        .gap(4.0)
        .center_y()
        .cursor(rune::Cursor::Pointer)
        .role("link")
        .label(label.to_owned())
        .focusable(true)
        .hover_group()
        .child(text(label.to_owned()).font(k.ty.ui_medium).color(k.c.text_muted).group_color(k.c.text_base))
        .child(icon(&super::icons::ARROW_RIGHT, 13.0).color(k.c.text_muted).group_color(k.c.text_base))
}

/// The thumbnail card every grid uses (the web app's `ContentCard`): an inset preview well
/// above the kind's icon, the title and a single line of details. It lifts under the pointer;
/// its actions appear only then.
pub struct Card {
    pub key: String,
    pub tint: Tint,
    pub glyph: &'static Icon,
    pub title: String,
    pub details: String,
    pub details_color: Option<Color>,
    /// A dot before the details: unread.
    pub dot: Option<Color>,
    pub preview: El,
    pub badge: Option<El>,
    pub actions: Vec<El>,
    pub selected: bool,
    /// The card's width; the preview is 16:10 inside it.
    pub width: f32,
}

pub fn card(c: Card, k: &Kit) -> El {
    let ink = theme::ink(k.dark);
    let well_w = c.width - 16.0;
    let well_h = (well_w * 10.0 / 16.0).round();
    let well = el()
        .w(well_w)
        .h(well_h)
        .radius(9.0)
        .clip_rounded(9.0)
        .fill(ink.thumb)
        .border(1.0, ink.thumb_line)
        .child(c.preview)
        .children(c.badge.map(|b| b.absolute(10.0, 10.0)));
    let actions = (!c.actions.is_empty()).then(|| row().absolute(c.width - 16.0 - 30.0 * c.actions.len() as f32 + 4.0, 16.0).gap(4.0).group_opacity(0.0, 1.0).children(c.actions));
    let meta = col()
        .w_full()
        .px(14.0)
        .pt(12.0)
        .pb(14.0)
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .items_start()
                .child(icon(c.glyph, 14.0).color(c.tint.color(k.dark)).mt(3.0).a11y_hidden())
                .child(text(c.title.clone()).font(theme::CARD_TITLE).color(k.c.text_base).max_lines(2).grow(1.0).min_w(0.0)),
        )
        .children((!c.details.is_empty()).then(|| {
            row()
                .w_full()
                .pl(22.0)
                .gap(6.0)
                .center_y()
                .children(c.dot.map(|d| dot(d, 6.0)))
                .child(text(c.details.clone()).font(theme::DETAILS).color(c.details_color.unwrap_or(k.c.text_faint)).truncate().shrink())
        }));
    let border = if c.selected { k.c.border_focus } else { k.c.border_base };
    // A stronger border and a soft drop shadow fade in under the pointer, as the web's cards lift.
    let lift = el()
        .absolute(0.0, 0.0)
        .w(c.width)
        .h_full()
        .radius(14.0)
        .border(1.0, if c.selected { k.c.border_focus } else { ink.line_strong })
        .shadow(12.0, 28.0, ink.lift)
        .group_opacity(0.0, 1.0)
        .inert()
        .a11y_hidden();
    col()
        .key(c.key)
        .w(c.width)
        .radius(14.0)
        .fill(k.c.layer_01)
        .border(if c.selected { 1.5 } else { 1.0 }, border)
        .hover_group()
        .hover_motion(rune::tok::motion::SNAP, rune::tok::motion::SETTLE)
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(c.title)
        .focusable(true)
        .child(lift)
        .child(el().w_full().pt(8.0).px(8.0).child(well))
        .child(meta)
        .children(actions)
}

/// A small round button over a card's preview, shown on hover.
pub fn card_action(glyph: &'static Icon, label: &str, k: &Kit, f: impl Fn(&mut App, &mut rune::Tx) + 'static) -> El {
    let ink = theme::ink(k.dark);
    el().size(26.0, 26.0)
        .radius(7.0)
        .fill(if k.dark { rune::Color::BLACK.alpha(0.6) } else { rune::Color::WHITE.alpha(0.92) })
        .border(1.0, ink.line_strong)
        .center()
        .cursor(rune::Cursor::Pointer)
        .hover(k.c.layer_01)
        .role("button")
        .label(label.to_owned())
        .help(label.to_owned())
        .child(icon(glyph, 13.0).color(k.c.text_base))
        .on_press_tx(f)
}

/// The "● Unread" pill pinned to a preview's corner.
pub fn badge(label: &str, dot: Option<Color>) -> El {
    row()
        .h(20.0)
        .px(8.0)
        .gap(5.0)
        .radius(10.0)
        .fill(rune::Color::BLACK.alpha(0.66))
        .center_y()
        .children(dot.map(|d| self::dot(d, 6.0)))
        .child(text(label.to_owned()).font(TextStyle::new(10.5, 600.0).lh(12.0)).color(rune::Color::WHITE))
        .a11y_hidden()
}

/// An empty screen with a call to action: an outlined icon in a ring, a title, one sentence.
pub fn empty(glyph: &'static Icon, _tint: Tint, title: &str, body: &str, action: Option<El>, k: &Kit) -> El {
    let mut c = col()
        .key("empty")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(8.0)
        .padding(40.0)
        .child(el().size(56.0, 56.0).radius(28.0).border(1.0, k.c.border_base).fill(k.c.layer_02).center().child(icon(glyph, 22.0).color(k.c.text_muted)).mb(8.0))
        .child(text(title.to_owned()).font(theme::HEADING).color(k.c.text_base))
        .child(text(body.to_owned()).font(k.ty.ui).color(k.c.text_faint).wrap().max_w(360.0).align(rune::Align::Center));
    if let Some(a) = action {
        c = c.child(el().h(12.0)).child(a);
    }
    c.transition(rune::Transition::opacity())
}

/// A small dot.
pub fn dot(color: Color, side: f32) -> El { el().size(side, side).radius(side / 2.0).fill(color).a11y_hidden() }

/// A horizontal hairline.
pub fn hairline(k: &Kit) -> El { el().w_full().h(1.0).fill(k.c.border_weak).a11y_hidden() }

/// Something loading: shimmering placeholder blocks.
pub fn skeleton(w: Option<f32>, h: f32, k: &Kit) -> El {
    let e = el().h(h).radius(5.0).fill(k.c.layer_02);
    let e = match w {
        Some(w) => e.w(w),
        None => e.w_full(),
    };
    // A slow breathing shimmer, sampled at 20 fps so a loading screen stays cheap.
    e.cycle(rune::Cycle { period: 1.6, phase: 0.0, stops: &[(0.0, 0.5), (0.5, 1.0), (1.0, 0.5)], still: 0.8, fps: 20.0 }).a11y_hidden()
}

/// The native context menu for an item, with the actions that suit its kind.
pub fn item_menu(s: &App, id: &str) -> PopupMenu {
    let item = |id: &str, title: &str| PopupItem::new(id, title);
    if let Some(b) = s.lib.bookmarks.iter().find(|b| b.id == id) {
        let mut m = PopupMenu::new()
            .item(item("open", "Open in Browser").symbol("safari"))
            .item(item("copy", "Copy Link").symbol("link"))
            .separator()
            .item(item("read", if b.unread() { "Mark as Read" } else { "Mark as Unread" }).symbol(if b.unread() { "checkmark.circle" } else { "circle" }))
            .item(item("favorite", if b.favorite() { "Remove from Favorites" } else { "Add to Favorites" }).symbol(if b.favorite() { "star.slash" } else { "star" }))
            .item(item("archive", if b.archived() { "Unarchive" } else { "Archive" }).symbol("archivebox"))
            .separator()
            .item(item("edit", "Edit…").symbol("pencil"));
        m = m.separator().item(item("delete", "Delete").symbol("trash"));
        return m;
    }
    if let Some(n) = s.lib.notes.iter().find(|n| n.id == id) {
        return PopupMenu::new()
            .item(item("favorite", if n.favorite() { "Remove from Favorites" } else { "Add to Favorites" }).symbol("star"))
            .item(item("history", "Version History…").symbol("clock.arrow.circlepath"))
            .item(item("copy-md", "Copy as Markdown").symbol("doc.on.doc"))
            .item(item("export", "Export…").symbol("square.and.arrow.down"))
            .separator()
            .item(item("delete", "Delete").symbol("trash"));
    }
    if s.lib.snippets.iter().any(|x| x.id == id) {
        return PopupMenu::new()
            .item(item("copy-code", "Copy Code").symbol("doc.on.doc"))
            .item(item("export", "Export…").symbol("square.and.arrow.down"))
            .separator()
            .item(item("delete", "Delete").symbol("trash"));
    }
    PopupMenu::new()
        .item(item("duplicate", "Duplicate").symbol("plus.square.on.square"))
        .item(item("copy-src", "Copy Source").symbol("doc.on.doc"))
        .separator()
        .item(item("delete", "Delete").symbol("trash"))
}

/// Runs what was picked in [`item_menu`].
pub fn item_menu_pick(s: &mut App, id: &str, picked: Option<&str>, tx: &mut rune::Tx) {
    let Some(p) = picked else { return };
    match p {
        "open" => s.open_bookmark(id, tx),
        "copy" => s.copy_link(id, tx),
        "read" => s.toggle_read(id, tx),
        "favorite" if s.lib.bookmarks.iter().any(|b| b.id == id) => s.toggle_favorite_bookmark(id, tx),
        "favorite" => s.toggle_favorite_note(id, tx),
        "archive" => s.toggle_archive(id, tx),
        "edit" => s.edit_bookmark_dialog(id),
        "delete" => s.delete(id, tx),
        "duplicate" => s.duplicate_diagram(id, tx),
        "copy-md" => {
            if let Some(n) = s.lib.notes.iter().find(|n| n.id == id) {
                tx.copy(n.markdown.clone());
                s.toast(crate::app::ToastKind::Success, "Copied as Markdown", tx);
            }
        }
        "copy-code" => {
            if let Some(x) = s.lib.snippets.iter().find(|x| x.id == id) {
                tx.copy(x.code.clone());
                s.toast(crate::app::ToastKind::Success, "Code copied", tx);
            }
        }
        "copy-src" => {
            if let Some(d) = s.lib.diagrams.iter().find(|d| d.id == id) {
                tx.copy(d.mermaid_source.clone().unwrap_or_default());
                s.toast(crate::app::ToastKind::Success, "Source copied", tx);
            }
        }
        "export" => super::export::export(s, id, tx),
        "history" => s.show_history(id),
        _ => {}
    }
}

/// Opens [`item_menu`] for `id` at the pointer.
pub fn with_menu(e: El, id: String) -> El {
    e.on_secondary_press(move |s: &mut App, press, tx| {
        let menu = item_menu(s, &id);
        let id = id.clone();
        tx.popup_menu(menu, press, move |s: &mut App, picked, tx| item_menu_pick(s, &id, picked, tx));
    })
}

/// The list column of a list-and-detail section: its title and count, a small add button, and
/// the rows under them.
pub fn list_pane(key: &str, title: &str, subtitle: &str, add: Option<El>, rows: El, width: f32, k: &Kit) -> El {
    let head = row()
        .w_full()
        .px(20.0)
        .pt(18.0)
        .pb(14.0)
        .items_end()
        .justify_between()
        .child(
            col()
                .gap(2.0)
                .child(text(title.to_owned()).font(TextStyle::new(20.0, 650.0).lh(26.0).tracking(-0.35)).color(k.c.text_strong))
                .child(text(subtitle.to_owned()).font(theme::DETAILS).color(k.c.text_faint)),
        )
        .children(add);
    col()
        .key(key.to_owned())
        .w(width)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], k.c.border_weak)
        .child(head)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(rune::ScrollSpec { stick_bottom: false }).scroll_edge_fade(12.0).child(rows))
}

/// The small "+" beside a list's title.
pub fn add_button(label: &str, k: &Kit, f: impl Fn(&mut App, &mut rune::Tx) + 'static) -> El {
    rune_kit::IconButton::new(&super::icons::PLUS).variant(rune_kit::IconButtonVariant::GhostMuted).label(label.to_owned()).help(format!("{label} ({})", crate::keys::label("mod+n"))).on_press_tx(f).build(k).key("list-add")
}

/// One row of a list column: a title, up to two faint lines of preview, and one line of
/// details. Selected rows sit on a neutral fill; colour stays on icons.
pub struct ListItem {
    pub key: String,
    pub title: String,
    pub preview: String,
    pub details: String,
    pub lead: Option<El>,
    pub selected: bool,
}

/// `open` runs on a click and `menu_id` names the item for its context menu. Both go on the
/// row that takes the click: a handler on the padding around it would never hear it.
pub fn list_item(i: ListItem, menu_id: String, open: impl Fn(&mut App, &mut rune::Tx) + 'static, k: &Kit) -> El {
    let ink = theme::ink(k.dark);
    let text_col = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(3.0)
        .child(text(i.title.clone()).font(theme::CARD_TITLE).color(k.c.text_base).truncate())
        .children((!i.preview.is_empty()).then(|| text(i.preview.clone()).font(TextStyle::new(13.0, 420.0).lh(18.0)).color(ink.thumb_text).max_lines(2)))
        .children((!i.details.is_empty()).then(|| text(i.details.clone()).font(theme::DETAILS).color(k.c.text_faint).truncate().pt(1.0)));
    row()
        .key(i.key)
        .w_full()
        .px(10.0)
        .pb(2.0)
        .child(
            row()
                .w_full()
                .px(12.0)
                .py(12.0)
                .gap(12.0)
                .items_start()
                .radius(10.0)
                .fill(if i.selected { k.c.layer_02 } else { rune::Color::TRANSPARENT })
                .hover(if i.selected { k.c.layer_02 } else { k.c.hover })
                .cursor(rune::Cursor::Pointer)
                .role("button")
                .label(i.title)
                .focusable(true)
                .children(i.lead)
                .child(text_col)
                .on_press_tx(open)
                .pipe(|row| with_menu(row, menu_id)),
        )
}

trait Pipe: Sized {
    fn pipe(self, f: impl FnOnce(Self) -> Self) -> Self { f(self) }
}
impl Pipe for El {}

/// A quiet icon button for an editor's bar.
pub fn bar_button(glyph: &'static Icon, label: &str) -> rune_kit::IconButton {
    rune_kit::IconButton::new(glyph).variant(rune_kit::IconButtonVariant::GhostMuted).label(label.to_owned()).help(label.to_owned())
}
