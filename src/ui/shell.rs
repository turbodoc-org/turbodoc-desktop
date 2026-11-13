//! The signed-in window: sidebar, toolbar, the section's content, the bookmark inspector, and
//! everything floating over them (dialogs, the palette, toasts).
use rune::{Cx, El, Spring, col, el, icon, row, text};
use rune_kit::shell::{PaneMotion, Sidebar, SidebarPeek, SidebarRow, SidebarToggle, SplitView, Toolbar};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, Spinner, TextInput};

use super::{bookmarks, diagrams, dialogs, home, icons, notes, palette, settings, snippets, toasts, widgets};
use crate::api::auth::User;
use crate::app::{App, Section, Sync};
use crate::format;

pub fn view(s: &App, user: &User, k: &Kit, cx: &mut Cx<App>) -> El {
    let mut layer: Vec<El> = Vec::new();
    if s.settings {
        let page = settings::view(s, user, k, cx, &mut layer);
        return finish(s, page, layer, k);
    }
    let main = match s.section {
        Section::Home => home::view(s, user, k, cx),
        Section::Bookmarks => bookmarks::view(s, k, cx),
        Section::Notes => notes::view(s, k, cx),
        Section::Snippets => snippets::view(s, k, cx),
        Section::Diagrams => diagrams::view(s, k, cx),
    };
    let main = col().key(format!("section-{}", s.section.id())).w_full().h_full().min_h(0.0).child(main);
    let mut split = SplitView::new(main)
        .sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, with_divider(s, sidebar(s, user, k), k))
        .toolbar(toolbar(s, k))
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(8.0)
        .toolbar_in_panel()
        .sidebar_padding(16.0, 0.0)
        .sidebar_toggle(SidebarToggle::new().size(ControlSize::Large).past_lights(16.0).edge(28.0).gap(4.0).in_header())
        .pane_motion(PaneMotion::new().spring(Spring::new(0.3, 0.0)).scale_with_width(480.0, 0.15, 0.3).fade_tail(0.15))
        .sidebar_peek(SidebarPeek::new().edge(16.0, ms(40)).toggle_delay(ms(80)).below(56.0).hold(15.0, ms(100)).spring(Spring::new(0.18, 0.0)))
        .reduced_motion(cx.is_reduced_motion());
    if s.section == Section::Bookmarks && s.selected.is_some() {
        split = split.inspector(&s.inspector, |s: &mut App| &mut s.inspector, with_leading_divider(s, bookmarks::inspector(s, k), k));
    }
    finish(s, split.build(k), layer, k)
}

fn finish(s: &App, base: El, mut layer: Vec<El>, k: &Kit) -> El {
    dialogs::build(s, k, &mut layer);
    if s.palette.is_some() {
        layer.push(palette::view(s, k));
    }
    if let Some(t) = toasts::view(s, k) {
        layer.push(t);
    }
    if s.drop_hover {
        layer.push(drop_veil(k));
    }
    rune_kit::root(base, layer)
        .on_drop_files(|s: &mut App, files, tx| {
            s.drop_hover = false;
            s.import_files(files, tx);
        })
        .accepts_extensions(&["md", "markdown", "txt", "js", "ts", "tsx", "jsx", "py", "rs", "go", "swift", "kt", "java", "cs", "rb", "php", "html", "css", "sql", "sh", "json", "yml", "yaml"])
        .on_drop_hover(|s: &mut App, over, _| s.drop_hover = over)
        .on_paste(|s: &mut App, text, tx| s.paste_anywhere(text, tx))
}

fn drop_veil(k: &Kit) -> El {
    el().key("drop-veil")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .padding(14.0)
        .inert()
        .child(
            col()
                .w_full()
                .h_full()
                .radius(16.0)
                .border(2.0, k.c.border_focus).border_dash(8.0, 6.0)
                .fill(k.c.border_focus.alpha(0.06))
                .center()
                .gap(8.0)
                .child(rune::icon(&icons::DOWNLOAD, 26.0).color(k.c.border_focus))
                .child(text("Drop to import").font(super::theme::HEADING).color(k.c.text_base))
                .child(text("Markdown and text become notes; code files become snippets.").font(k.ty.ui).color(k.c.text_muted)),
        )
        .transition(rune::Transition::opacity())
}

/// The sidebar with a hairline down its trailing edge, on the kit's resize grip, so the edge
/// reads as something to drag. It takes the accent while a drag resizes the sidebar.
fn with_divider(s: &App, sidebar: El, k: &Kit) -> El {
    let line = if s.sidebar.is_resizing() { k.c.border_focus } else { k.c.border_base };
    row()
        .key("sidebar-frame")
        .w_full()
        .h_full()
        .child(col().grow(1.0).min_w(0.0).h_full().pr(10.0).child(sidebar))
        .child(el().key("sidebar-divider").w(1.0).h_full().fill(line).a11y_hidden())
}

/// The inspector with the same hairline down its leading edge, on its resize grip.
fn with_leading_divider(s: &App, inspector: El, k: &Kit) -> El {
    let line = if s.inspector.is_resizing() { k.c.border_focus } else { k.c.border_base };
    row()
        .key("inspector-frame")
        .w_full()
        .h_full()
        .child(el().key("inspector-divider").w(1.0).h_full().fill(line).a11y_hidden())
        .child(col().grow(1.0).min_w(0.0).h_full().child(inspector))
}

fn ms(n: u64) -> std::time::Duration { std::time::Duration::from_millis(n) }

fn nav(s: &App, section: Section, glyph: &'static rune::Icon, k: &Kit) -> El {
    SidebarRow::new(section.title())
        .icon(glyph)
        .height(34.0)
        .selected(s.section == section && !s.settings)
        .on_select(move |s: &mut App, tx| s.go(section, tx))
        .build(k)
        .key(format!("nav-{}", section.id()))
}

/// Five places, as the web app's navigation has them. Filters and tags live on their pages,
/// where they apply; counts live in page subtitles.
fn sidebar(s: &App, user: &User, k: &Kit) -> El {
    Sidebar::new()
        .label("Turbodoc")
        .inset(0.0)
        .header_gap(2.0)
        .header_padding(6.0, 0.0)
        .row_gap(2.0)
        .header(nav(s, Section::Home, &icons::GRID, k))
        .header(nav(s, Section::Bookmarks, &icons::BOOKMARK, k))
        .header(nav(s, Section::Notes, &icons::NOTE, k))
        .header(nav(s, Section::Snippets, &icons::CODE, k))
        .header(nav(s, Section::Diagrams, &icons::DIAGRAM, k))
        .footer(account_row(s, user, k))
        .build(k)
}

fn account_row(s: &App, user: &User, k: &Kit) -> El {
    let name = user.email.clone().unwrap_or_else(|| "Account".into());
    let initial = name.chars().next().unwrap_or('T').to_ascii_uppercase().to_string();
    let avatar = row()
        .size(24.0, 24.0)
        .radius(12.0)
        .center()
        .fill(k.c.layer_03)
        .child(text(initial).font(k.ty.small_medium).color(k.c.text_base));
    let status = match &s.sync {
        Sync::Running { .. } => "Syncing…".to_owned(),
        Sync::Offline { .. } => "Offline".to_owned(),
        Sync::Idle if s.lib.synced_at > 0 => format!("Synced {}", format::ago_from(chrono::DateTime::from_timestamp(s.lib.synced_at, 0).unwrap_or_default(), chrono::Utc::now()).replace("Yesterday", "yesterday")),
        Sync::Idle => String::new(),
    };
    SidebarRow::new(name)
        .avatar(avatar)
        .subtitle(status)
        .height(46.0)
        .selected(s.settings)
        .on_select(|s: &mut App, _| s.open_settings())
        .build(k)
        .key("nav-account")
}

/// The toolbar holds only what's needed everywhere: search, and the sync state when it isn't
/// simply "up to date".
fn toolbar(s: &App, k: &Kit) -> Toolbar {
    let mut trailing = row().gap(8.0).center_y();
    match s.sync {
        Sync::Running { .. } => trailing = trailing.child(row().key("sync").gap(6.0).center_y().child(Spinner::new().build(k)).child(text("Syncing").font(k.ty.small).color(k.c.text_faint)).transition(rune::Transition::opacity())),
        Sync::Offline { .. } => {
            trailing = trailing.child(
                row()
                    .key("sync")
                    .h(28.0)
                    .px(10.0)
                    .gap(6.0)
                    .radius(14.0)
                    .border(1.0, k.c.border_base)
                    .center_y()
                    .cursor(rune::Cursor::Pointer)
                    .hover(k.c.hover)
                    .help("Changes save once you're back online. Click to try again.")
                    .child(icon(&icons::CLOUD_OFF, 13.0).color(k.c.text_muted))
                    .child(text("Offline").font(k.ty.small_medium).color(k.c.text_muted))
                    .on_press_tx(|s: &mut App, tx| s.refresh(tx))
                    .transition(rune::Transition::opacity()),
            )
        }
        Sync::Idle => {}
    }
    if s.section != Section::Home {
        let placeholder = format!("Search {}", s.section.title().to_lowercase());
        trailing = trailing.child(
            TextInput::new(&s.search)
                .placeholder(placeholder)
                .leading_icon(&icons::SEARCH)
                .width(Some(260.0))
                .clear_button()
                .on_edit(|s: &mut App| &mut s.search)
                .on_key(|s: &mut App, ev, tx| {
                    if ev.key == rune::Key::Escape {
                        s.search.clear();
                        tx.blur();
                        return true;
                    }
                    false
                })
                .label("Search")
                .build(k)
                .key("search"),
        );
    } else {
        trailing = trailing.child(
            row()
                .key("search-all")
                .h(32.0)
                .w(260.0)
                .px(10.0)
                .gap(8.0)
                .radius(k.radius.md)
                .border(1.0, k.c.border_base)
                .center_y()
                .cursor(rune::Cursor::Pointer)
                .hover(k.c.hover)
                .role("button")
                .label("Search everything")
                .child(icon(&icons::SEARCH, 14.0).color(k.c.text_faint))
                .child(text("Search everything").font(k.ty.ui).color(k.c.text_faint).grow(1.0))
                .child(widgets::keycap("mod+k", k))
                .on_press(|s: &mut App| s.open_palette()),
        );
    }
    Toolbar::new().trailing(trailing)
}

/// The main action of a section, for its page header.
pub fn new_button(s: &App, k: &Kit) -> Option<El> {
    let label = match s.section {
        Section::Home => return None,
        Section::Bookmarks => "Add bookmark",
        Section::Notes => "New note",
        Section::Snippets => "New snippet",
        Section::Diagrams => "New diagram",
    };
    Some(Button::new(label).icon(&icons::PLUS).variant(ButtonVariant::Contrast).help(format!("{label} ({})", crate::keys::label("mod+n"))).on_press_tx(|s: &mut App, tx| s.new_in_section(tx)).build(k).key("new-item"))
}

impl App {
    pub fn new_in_section(&mut self, tx: &mut rune::Tx) {
        match self.section {
            Section::Home | Section::Notes => self.new_note(String::new(), tx),
            Section::Bookmarks => self.new_bookmark_dialog(None),
            Section::Snippets => self.new_snippet(String::new(), None, tx),
            Section::Diagrams => self.new_diagram_dialog(),
        }
    }

    pub fn new_diagram_dialog(&mut self) {
        let mut title = rune::TextEdit::new("");
        title.focused = true;
        self.palette = None;
        self.dialog = Some(crate::app::Dialog::NewDiagram { title, mermaid: true, busy: false });
    }

    pub fn open_palette(&mut self) {
        let mut query = rune::TextEdit::new("");
        query.focused = true;
        self.dialog = None;
        self.palette = Some(crate::app::Palette { query, active: 0 });
    }

    pub fn open_capture(&self) {
        if let Some(h) = crate::CAPTURE.get() {
            crate::capture::summon(h);
        }
    }
}
