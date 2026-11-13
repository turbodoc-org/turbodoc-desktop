//! Turbodoc for the desktop: bookmarks, notes, code snippets and diagrams from the Turbodoc API,
//! in a native, GPU-rendered window (Rune), with a global quick-capture panel, native menus,
//! an offline copy of the library and the session in the system keychain.
//!
//! `cargo run --release` opens the app. Headless, for checking screens without an account:
//! `turbodoc --snap out.png --scene bookmarks --size 1280x800` (`--list-scenes` for the rest).
mod api;
mod app;
mod capture;
mod demo;
mod format;
mod keys;
mod net;
mod store;
mod ui;

use std::sync::OnceLock;

use rune::{AppHandle, AppSpec, Chrome, KeyEvent, MenuItem, Tx, WindowOptions, WindowSpec};

use crate::app::{App, BookmarkFilter, Dialog, Section};

pub const APP_ID: &str = "ai.turbodoc.desktop";

/// The quick-capture panel's handle, for opening it from the main window.
pub static CAPTURE: OnceLock<AppHandle<capture::Capture>> = OnceLock::new();

fn sc(s: &str) -> rune::Shortcut { keys::shortcut(s) }

fn poll(s: &mut App, clock: f64, tx: &mut Tx) -> bool { s.tick(clock, tx) }

fn setup(s: &mut App, waker: rune::Waker) { s.previews.cache = Some(rune::ImageCache::new(waker)); }

/// Keys no control took: shortcuts that act on the selection.
pub(crate) fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    use rune::Key;
    if rune_kit::shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
        let w = s.sidebar.width();
        s.set_pref("sidebar_width", &w);
        return true;
    }
    if s.user().is_none() {
        return false;
    }
    if ev.key == Key::Escape {
        if s.palette.is_some() {
            s.palette = None;
            return true;
        }
        if s.settings {
            s.settings = false;
            return true;
        }
        if !s.search.text.is_empty() {
            s.search.clear();
            return true;
        }
        if s.section == Section::Bookmarks && s.selected.take().is_some() {
            return true; // closes the inspector
        }
        return false;
    }
    if tx.focused().is_some() {
        return false;
    }
    match ev.key {
        Key::Text(ref t) if t == "/" && !ev.mods.cmd => {
            s.search.focused = true;
            tx.focus("search", true);
            true
        }
        Key::Backspace | Key::Delete if !ev.mods.cmd => {
            if let Some(id) = s.selected.clone()
                && s.section == Section::Bookmarks
            {
                s.delete(&id, tx);
                return true;
            }
            false
        }
        Key::Enter => {
            if let Some(id) = s.selected.clone()
                && s.section == Section::Bookmarks
            {
                s.open_bookmark(&id, tx);
                return true;
            }
            false
        }
        Key::Command('z') if !ev.mods.shift => {
            if let Some(id) = s.deletes.last().map(|d| d.item.id().to_owned()) {
                s.undo_delete(&id, tx);
                return true;
            }
            false
        }
        _ => false,
    }
}

/// `turbodoc://` links: `turbodoc://note/<id>`, `turbodoc://bookmark/<id>`, `turbodoc://save?url=…`.
fn on_open_url(s: &mut App, url: &str, tx: &mut Tx) {
    let Some(rest) = url.strip_prefix("turbodoc://") else { return };
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut parts = path.trim_matches('/').splitn(2, '/');
    let (kind, id) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let valid_id = !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    match kind {
        "note" if valid_id => {
            s.go(Section::Notes, tx);
            s.open_note(id, tx);
        }
        "bookmark" if valid_id => {
            s.go(Section::Bookmarks, tx);
            s.select_bookmark(id);
        }
        "snippet" if valid_id => {
            s.go(Section::Snippets, tx);
            s.open_snippet(id);
        }
        "diagram" if valid_id => {
            s.go(Section::Diagrams, tx);
            s.open_diagram(id);
        }
        "save" => {
            let target = query.split('&').find_map(|kv| kv.strip_prefix("url=")).map(decode).unwrap_or_default();
            if format::looks_like_url(&target) {
                s.new_bookmark_dialog(Some(format::normalize_url(&target)));
            }
        }
        _ => {}
    }
    if let Some(h) = &s.handle {
        h.open();
    }
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn menus(app: rune::App<App>) -> rune::App<App> {
    let signed = |f: fn(&mut App, &mut Tx)| f;
    app.menu_item(MenuItem::new("Settings…", |s: &mut App, _| s.open_settings()).id("settings").key_equivalent(sc("mod+comma")))
        .menu_item(MenuItem::new("Sign Out", |s: &mut App, tx| s.sign_out(tx)).id("sign-out"))
        .menu_item(MenuItem::new("New Note", signed(|s, tx| if s.user().is_some() { s.new_note(String::new(), tx) })).id("new-note").menu("File").key_equivalent(sc("mod+n")))
        .menu_item(MenuItem::new("Add Bookmark…", |s: &mut App, _| if s.user().is_some() { s.new_bookmark_dialog(None) }).id("new-bookmark").menu("File").key_equivalent(sc("mod+shift+n")))
        .menu_item(MenuItem::new("New Snippet", |s: &mut App, tx| if s.user().is_some() { s.new_snippet(String::new(), None, tx) }).id("new-snippet").menu("File"))
        .menu_item(MenuItem::new("New Diagram…", |s: &mut App, _| if s.user().is_some() { s.new_diagram_dialog() }).id("new-diagram").menu("File"))
        .menu_item(MenuItem::separator().menu("File"))
        .menu_item(MenuItem::new("Quick Capture", |s: &mut App, _| s.open_capture()).id("capture").menu("File").key_equivalent(sc(keys::CAPTURE)))
        .menu_item(MenuItem::new("Import Files…", |_: &mut App, tx| {
            let options = rune::FileDialogOptions { title: "Import into Turbodoc".into(), ..Default::default() }.filter("Markdown, text and code", &["md", "markdown", "txt", "js", "ts", "py", "rs", "go", "swift", "json", "yaml", "sh", "sql", "html", "css"]);
            tx.open_file(options, |s: &mut App, r, tx| {
                if let Ok(Some(path)) = r {
                    s.import_files(&[path], tx);
                }
            });
        }).id("import").menu("File").key_equivalent(sc("mod+o")))
        .menu_item(MenuItem::new("Sync Now", |s: &mut App, tx| s.refresh(tx)).id("sync").menu("File").key_equivalent(sc("mod+r")))
        .menu_item(MenuItem::new("Find…", |s: &mut App, tx| {
            if s.section == Section::Home {
                s.open_palette();
            } else {
                s.search.focused = true;
                tx.focus("search", true);
            }
        }).id("find").menu("Edit").key_equivalent(sc("mod+f")))
        .menu_item(MenuItem::new("Search Everything…", |s: &mut App, _| if s.user().is_some() { s.open_palette() }).id("palette").menu("View").key_equivalent(sc("mod+k")))
        .menu_item(MenuItem::separator().menu("View"))
        .menu_item(MenuItem::new("Home", |s: &mut App, tx| s.go(Section::Home, tx)).id("go-home").menu("View").key_equivalent(sc("mod+1")))
        .menu_item(MenuItem::new("Bookmarks", |s: &mut App, tx| {
            s.go(Section::Bookmarks, tx);
            s.bm_filter = BookmarkFilter::All;
        }).id("go-bookmarks").menu("View").key_equivalent(sc("mod+2")))
        .menu_item(MenuItem::new("Notes", |s: &mut App, tx| s.go(Section::Notes, tx)).id("go-notes").menu("View").key_equivalent(sc("mod+3")))
        .menu_item(MenuItem::new("Snippets", |s: &mut App, tx| s.go(Section::Snippets, tx)).id("go-snippets").menu("View").key_equivalent(sc("mod+4")))
        .menu_item(MenuItem::new("Diagrams", |s: &mut App, tx| s.go(Section::Diagrams, tx)).id("go-diagrams").menu("View").key_equivalent(sc("mod+5")))
        .menu_item(MenuItem::new("Reading List", |s: &mut App, tx| {
            s.go(Section::Bookmarks, tx);
            s.bm_filter = BookmarkFilter::Unread;
        }).id("go-reading").menu("View").key_equivalent(sc("mod+shift+r")))
        .menu_item(MenuItem::separator().menu("View"))
        .menu_item(MenuItem::new("Toggle Sidebar", |s: &mut App, _| s.sidebar.toggle()).id("sidebar").menu("View"))
        .menu_item(MenuItem::new("Keyboard Shortcuts", |s: &mut App, _| s.dialog = Some(Dialog::Shortcuts)).id("shortcuts").menu("Help").key_equivalent(sc("mod+slash")))
        .menu_item(MenuItem::new("Turbodoc Help", |_: &mut App, tx| {
            let _ = tx.open_url("https://turbodoc.ai/docs");
        }).id("help").menu("Help"))
        .menu_item(MenuItem::new("New Note", |s: &mut App, tx| {
            if s.user().is_some() {
                s.new_note(String::new(), tx);
                if let Some(h) = &s.handle {
                    h.open();
                }
            }
        }).id("dock-note").dock())
        .menu_item(MenuItem::new("Quick Capture", |s: &mut App, _| s.open_capture()).id("dock-capture").dock())
}

impl App {
    pub fn open_settings(&mut self) {
        if self.user().is_none() {
            return;
        }
        self.palette = None;
        self.settings = true;
        if self.digest.is_none() {
            self.load_digest();
        }
    }
}

fn main() {
    if !rune::is_headless() && !rune::single_instance(APP_ID) {
        return; // the running copy took our arguments (a turbodoc:// link) and came forward
    }
    rune_mac::launch();
    // Linux: the id desktops match to turbodoc.desktop, for the window's icon and name.
    rune::set_app_id("turbodoc");
    // Windows: the title bar, taskbar and Alt+Tab icon. macOS takes the bundle's.
    #[cfg(target_os = "windows")]
    let _ = rune::set_app_icon(include_bytes!("../assets/icons/256x256.png").as_slice());
    rune::prewarm_gpu();
    let _ = rune_notify::register(&rune_notify::Identity::new(APP_ID, "Turbodoc"));
    rune_notify::on_click(|_| {
        rune_notify::bring_to_front();
    });
    let headless = rune::is_headless();
    let size = rune_mac::saved_size("Turbodoc").unwrap_or((1240.0, 800.0));

    let spec = AppSpec {
        min_size: Some((780.0, 540.0)),
        fonts: rune_theme_opencode::FONTS,
        families: rune_theme_opencode::FAMILIES,
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        scenes: demo::SCENES,
        ..AppSpec::new("Turbodoc", size, || App::new(None), ui::view)
    };
    let main = rune::App::new(spec);
    let main_handle = main.handle();
    let first = App::new((!headless).then(|| main_handle.clone()));
    let auth = first.auth.clone();

    let capture_spec = AppSpec {
        fonts: rune_theme_opencode::FONTS,
        families: rune_theme_opencode::FAMILIES,
        poll: Some(capture::poll),
        ..AppSpec::new("Quick Capture", (560.0, 150.0), || capture::Capture::new(None, None, Default::default()), capture::view)
    };
    let capture_app = rune::App::new(capture_spec).windowless().fit_content_height();
    let capture_handle = capture_app.handle();
    let _ = CAPTURE.set(capture_handle.clone());
    let (me, main_for_capture) = (capture_handle.clone(), main_handle.clone());
    let capture_app = capture_app
        .init(move || capture::Capture::new((!headless).then_some(me), (!headless).then_some(main_for_capture), auth))
        .global_shortcut(keys::CAPTURE, |s, _| {
            if let Some(me) = s.me.clone() {
                capture::summon(&me);
            }
        })
        .on_focus(|_, focused, tx| {
            if !focused {
                tx.close_window();
            }
        });

    let ready = main_handle.clone();
    let main = menus(main)
        .init(move || first)
        .chrome(Chrome { full_size_content: true, traffic_lights: Some((16.0, 25.0)), drag_regions: true })
        .keep_running()
        .on_open_url(on_open_url)
        .on_focus(|s, focused, tx| s.on_focus(focused, tx))
        .on_quit(|s, _| s.flush_deletes())
        .on_ready(move |_| {
            // After the first frame: the network, and housekeeping.
            ready.update(|s, _| s.restore_session());
            store::images::trim();
        });

    let panel = WindowOptions::launcher_panel().macos(|native| native.corner_radius(16.0));
    rune::run_windows(move |_| -> Vec<rune::AnyWindowSpec> {
        vec![WindowSpec::from_app("main", main).into(), WindowSpec::from_app("capture", capture_app).configured_options(panel).into()]
    });
}

#[cfg(test)]
mod tests;
