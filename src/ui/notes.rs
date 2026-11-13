//! Notes: the list beside an editor that styles Markdown as you type, with a live preview
//! (split) or a reading view, and autosave that merges with edits from other devices. The
//! page carries the writing; everything else stays out of the way.
use rune::{Cx, El, GrowthSpec, ScrollSpec, TextStyle, col, el, icon, row, text, text_input};
use rune_kit::{Kit, Markdown, SegmentedControl, TextInput};

use super::theme::{self, Tint};
use super::{icons, md_syntax, widgets};
use crate::api::models::Note;
use crate::app::{App, NoteEditor, NoteMode, Save};
use crate::format;

const LIST_W: f32 = 300.0;
pub const BODY: TextStyle = TextStyle::new(15.0, 420.0).lh(25.0);

pub fn view(s: &App, k: &Kit, cx: &mut Cx<App>) -> El {
    let notes = s.visible_notes();
    if s.loaded && s.lib.notes.is_empty() {
        let action = rune_kit::Button::new("New note")
            .icon(&icons::PLUS)
            .variant(rune_kit::ButtonVariant::Contrast)
            .on_press_tx(|s: &mut App, tx| s.new_note(String::new(), tx))
            .build(k)
            .key("empty-new-note");
        return widgets::empty(&icons::NOTE, Tint::Note, "Write your first note", "Notes are Markdown, sync to every device and keep their full history. Drop .md files here to import them.", Some(action), k);
    }
    let mode = s.prefs_mode();
    let width = cx.layout_size("notes-pane").map_or(900.0, |sz| sz[0]);
    let show_list = width > 700.0 || s.note.is_none();
    let list = show_list.then(|| list(s, &notes, k));
    let editor = match &s.note {
        Some(ed) => editor(s, ed, mode, width - if show_list { LIST_W } else { 0.0 }, k),
        None => col()
            .grow(1.0)
            .h_full()
            .center()
            .gap(8.0)
            .child(icon(&icons::NOTE, 22.0).color(k.c.icon_faint))
            .child(text(if notes.is_empty() { "No notes match" } else { "Select a note" }).font(k.ty.ui).color(k.c.text_faint)),
    };
    row().key("notes-pane").w_full().h_full().min_h(0.0).children(list).child(editor)
}

impl App {
    pub fn prefs_mode(&self) -> NoteMode { NoteMode::from_index(self.prefs.get::<usize>("note_mode").unwrap_or(0)) }
}

fn list(s: &App, notes: &[&Note], k: &Kit) -> El {
    let ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
    let ids2 = ids.clone();
    let rows = col().key("note-rows").w_full().growth(GrowthSpec::default()).virtual_rows_version((ids.len(), s.lib.notes.len(), s.search.text.len(), s.tag.clone())).virtual_rows(
        ids.len(),
        96.0,
        move |_: &App, i| format!("note-{}", ids[i]).into(),
        move |s: &App, i| match s.lib.notes.iter().find(|n| n.id == ids2[i]) {
            Some(n) => note_row(s, n, super::theme::kit(s.dark(super::bookmarks::DARK.with(|d| d.get())))),
            None => el(),
        },
    );
    let title = match &s.tag {
        Some(t) => format!("#{t}"),
        None => "Notes".to_owned(),
    };
    let add = widgets::add_button("New note", k, |s, tx| s.new_note(String::new(), tx));
    widgets::list_pane("note-list", &title, &format::count(notes.len(), "note", "notes"), Some(add), rows, LIST_W, k)
}

fn note_row(s: &App, n: &Note, k: &Kit) -> El {
    let selected = s.note.as_ref().is_some_and(|e| e.id == n.id);
    let id = n.id.clone();
    let preview = format::preview(n.markdown.strip_prefix(&format!("# {}", n.title)).unwrap_or(&n.markdown), 140);
    let mut details = format!("Edited {}", format::ago_inline(n.updated_at.as_deref().unwrap_or("")));
    if let Some(t) = n.tag_list().first() {
        details.push_str(&format!(" · {t}"));
    }
    if n.favorite() {
        details = format!("★ {details}");
    }
    widgets::list_item(widgets::ListItem { key: format!("note-{}", n.id), title: n.display_title(), preview, details, lead: None, selected }, n.id.clone(), move |s: &mut App, tx| s.open_note(&id, tx), k)
}

fn editor(s: &App, ed: &NoteEditor, mode: NoteMode, width: f32, k: &Kit) -> El {
    let status = text(match &ed.save {
        Save::Clean if ed.creating => "New note".to_owned(),
        Save::Clean => s.lib.notes.iter().find(|n| n.id == ed.id).map(|n| format!("Edited {}", format::ago_inline(n.updated_at.as_deref().unwrap_or("")))).unwrap_or_default(),
        other => other.label().to_owned(),
    })
    .key("save-status")
    .font(theme::DETAILS)
    .color(if matches!(ed.save, Save::Failed { .. }) { k.c.danger.fg } else { k.c.text_faint })
    .help(match &ed.save {
        Save::Failed { message } => message.clone(),
        _ => "Saves a moment after you stop typing. Every save keeps a version.".into(),
    });
    let modes = SegmentedControl::new(&["Write", "Split", "Read"], Some(mode.index())).fit().label("Editor layout").on_pick(|s: &mut App, i, _| s.set_pref("note_mode", &i)).build(k).key("note-mode");
    let more = {
        let id = ed.id.clone();
        widgets::bar_button(&icons::MORE, "More")
            .on_press_tx(move |s: &mut App, tx| {
                let menu = widgets::item_menu(s, &id);
                let id = id.clone();
                tx.popup_menu(menu, "note-more", move |s: &mut App, p, tx| widgets::item_menu_pick(s, &id, p, tx));
            })
            .build(k)
            .key("note-more")
    };
    let bar = row().key("note-bar").w_full().h(52.0).px(24.0).gap(12.0).center_y().child(status).child(rune::spacer()).child(modes).child(more);

    let title = text_input(&ed.title, theme::TITLE)
        .key("note-title")
        .single_line()
        .placeholder("Untitled", k.c.text_faint)
        .color(k.c.text_strong)
        .w_full()
        .label("Title")
        .on_edit(|s: &mut App| &mut s.note.as_mut().expect("open note").title)
        .on_input_change(|s: &mut App, tx| {
            if let Some(ed) = &mut s.note {
                ed.touch(tx);
            }
        })
        .on_input_key(|s: &mut App, ev, tx| {
            if matches!(ev.key, rune::Key::Enter | rune::Key::Down) {
                if let Some(ed) = &mut s.note {
                    ed.body.focused = true;
                    ed.body.set_caret(0);
                }
                tx.focus("note-body", false);
                return true;
            }
            false
        });
    let tags = row().w_full().gap(8.0).center_y().child(icon(&icons::TAG, 13.0).color(k.c.icon_faint)).child(
        col().grow(1.0).min_w(0.0).child(
            TextInput::new(&ed.tags)
                .plain()
                .width(None)
                .placeholder("Add tags")
                .label("Tags")
                .on_edit(|s: &mut App| &mut s.note.as_mut().expect("open note").tags)
                .on_change(|s: &mut App, tx| {
                    if let Some(ed) = &mut s.note {
                        ed.touch(tx);
                    }
                })
                .build(k)
                .key("note-tags"),
        ),
    );
    let ink = md_syntax::Inks::from_kit(k);
    let body = text_input(&ed.body, BODY)
        .key("note-body")
        .placeholder("Start writing. Markdown works: # headings, **bold**, - lists, `code`.", k.c.text_faint)
        .color(k.c.text_base)
        .w_full()
        .min_h(360.0)
        .label("Note")
        .spellcheck(true)
        .decorations(md_syntax::spans(&ed.body.text, &ink))
        .selection_color(k.selection())
        .on_edit(|s: &mut App| &mut s.note.as_mut().expect("open note").body)
        .on_input_change(|s: &mut App, tx| {
            if let Some(ed) = &mut s.note {
                ed.touch(tx);
            }
        });
    let split = mode == NoteMode::Split;
    let side = if split { width / 2.0 } else { width };
    let measure = (side - 96.0).clamp(280.0, 700.0);
    let writing = col()
        .key("note-write")
        .w_full()
        .items_center()
        .pt(20.0)
        .pb(120.0)
        .px(48.0)
        .child(col().w_full().max_w(measure).gap(10.0).child(title).child(tags).child(el().h(14.0)).child(body));
    let preview_src = if ed.title.text.trim().is_empty() || ed.body.text.trim_start().starts_with("# ") { ed.body.text.clone() } else { format!("# {}\n\n{}", ed.title.text.trim(), ed.body.text) };
    let reading = col().key("note-read").w_full().items_center().pt(20.0).pb(120.0).px(48.0).child(col().w_full().max_w(measure).child(Markdown::new(preview_src).selectable(true).build(k)));
    let scroll = |key: &str, child: El| col().key(key.to_owned()).grow(1.0).min_w(0.0).h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).scroll_edge_fade(16.0).child(child);
    let content = match mode {
        NoteMode::Write => row().w_full().grow(1.0).min_h(0.0).child(scroll("note-write-scroll", writing)),
        NoteMode::Read => row().w_full().grow(1.0).min_h(0.0).child(scroll("note-read-scroll", reading)),
        NoteMode::Split => row().w_full().grow(1.0).min_h(0.0).child(scroll("note-write-scroll", writing)).child(el().w(1.0).h_full().fill(k.c.border_weak)).child(scroll("note-read-scroll", reading)),
    };
    col().key(format!("editor-{}", ed.id)).grow(1.0).min_w(0.0).h_full().min_h(0.0).child(bar).child(content)
}
