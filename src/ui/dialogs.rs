//! Modal forms: add or edit a bookmark, start a diagram, and the keyboard shortcut sheet.
use rune::{El, col, row, text};
use rune_kit::{Button, ButtonVariant, Dialog as KitDialog, Kit, SegmentedControl, TextInput};

use super::widgets;
use crate::app::{App, BookmarkForm, Dialog};

fn bm(s: &mut App) -> &mut BookmarkForm {
    match &mut s.dialog {
        Some(Dialog::Bookmark(f)) => f,
        _ => unreachable!("bookmark form is open"),
    }
}

pub fn build(s: &App, k: &Kit, layer: &mut Vec<El>) {
    match &s.dialog {
        Some(Dialog::Bookmark(f)) => bookmark(f, k, layer),
        Some(Dialog::NewDiagram { title, mermaid, busy }) => new_diagram(title, *mermaid, *busy, k, layer),
        Some(Dialog::Shortcuts) => shortcuts(k, layer),
        None => {}
    }
}

fn bookmark(f: &BookmarkForm, k: &Kit, layer: &mut Vec<El>) {
    let editing = f.id.is_some();
    let field = |label: &str, input: El| col().w_full().gap(5.0).child(text(label.to_owned()).font(k.ty.small_medium).color(k.c.text_muted)).child(input);
    let url = TextInput::new(&f.url)
        .width(None)
        .placeholder("https://")
        .label("Link")
        .leading_icon(&super::icons::GLOBE)
        .invalid(f.error.is_some())
        .on_edit(|s: &mut App| &mut bm(s).url)
        .on_submit(|s: &mut App, tx| s.submit_bookmark(tx))
        .build(k)
        .key("bm-url");
    let title = TextInput::new(&f.title)
        .width(None)
        .placeholder(if editing { "Title" } else { "Leave empty to use the page's title" })
        .label("Title")
        .on_edit(|s: &mut App| &mut bm(s).title)
        .on_submit(|s: &mut App, tx| s.submit_bookmark(tx))
        .build(k)
        .key("bm-title");
    let tags = TextInput::new(&f.tags)
        .width(None)
        .placeholder("work, read-later")
        .label("Tags")
        .leading_icon(&super::icons::TAG)
        .on_edit(|s: &mut App| &mut bm(s).tags)
        .on_submit(|s: &mut App, tx| s.submit_bookmark(tx))
        .build(k)
        .key("bm-tags");
    let error = f.error.as_ref().map(|e| text(e.clone()).font(k.ty.small).color(k.c.danger.fg).wrap());
    let buttons = row()
        .w_full()
        .justify_end()
        .gap(8.0)
        .child(Button::new("Cancel").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.dialog = None).build(k).key("bm-cancel"))
        .child(
            Button::new(if f.busy { "Saving…" } else if editing { "Save" } else { "Add bookmark" })
                .variant(if f.busy { ButtonVariant::Loading } else { ButtonVariant::Contrast })
                .disabled(f.busy)
                .shortcut("↩")
                .on_press_tx(|s: &mut App, tx| s.submit_bookmark(tx))
                .build(k)
                .key("bm-save"),
        );
    let body = col()
        .w_full()
        .gap(12.0)
        .child(field("Link", url))
        .child(field("Title", title))
        .child(field("Tags", tags))
        .children(error)
        .children((!editing).then(|| text("Turbodoc reads the page and writes a summary for you.").font(k.ty.small).color(k.c.text_faint)))
        .child(buttons);
    KitDialog::new("bookmark-dialog", if editing { "Edit bookmark" } else { "Add a bookmark" })
        .open(true)
        .fit()
        .width(440.0)
        .child(body.px(16.0).pb(16.0))
        .on_close(|s: &mut App| s.dialog = None)
        .build(k, layer);
}

fn new_diagram(title: &rune::TextEdit, mermaid: bool, busy: bool, k: &Kit, layer: &mut Vec<El>) {
    let input = TextInput::new(title)
        .width(None)
        .placeholder("Untitled Diagram")
        .label("Title")
        .on_edit(|s: &mut App| match &mut s.dialog {
            Some(Dialog::NewDiagram { title, .. }) => title,
            _ => unreachable!("diagram dialog is open"),
        })
        .on_submit(|s: &mut App, tx| s.submit_new_diagram(tx))
        .build(k)
        .key("dia-title");
    let kind = SegmentedControl::new(&["Mermaid", "Canvas"], Some(if mermaid { 0 } else { 1 }))
        .label("Kind")
        .on_pick(|s: &mut App, i, _| {
            if let Some(Dialog::NewDiagram { mermaid, .. }) = &mut s.dialog {
                *mermaid = i == 0;
            }
        })
        .build(k)
        .key("dia-kind");
    let hint = if mermaid { "Write it as text; it's drawn as you type." } else { "Canvas diagrams are drawn in the web app and show here." };
    let body = col()
        .w_full()
        .gap(12.0)
        .child(input)
        .child(kind)
        .child(text(hint).font(k.ty.small).color(k.c.text_faint))
        .child(
            row()
                .w_full()
                .justify_end()
                .gap(8.0)
                .child(Button::new("Cancel").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.dialog = None).build(k))
                .child(Button::new(if busy { "Creating…" } else { "Create" }).variant(ButtonVariant::Contrast).disabled(busy).on_press_tx(|s: &mut App, tx| s.submit_new_diagram(tx)).build(k).key("dia-create")),
        );
    KitDialog::new("diagram-dialog", "New diagram").open(true).fit().width(400.0).child(body.px(16.0).pb(16.0)).on_close(|s: &mut App| s.dialog = None).build(k, layer);
}

pub const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    ("Anywhere", &[("mod+k", "Search and commands"), (crate::keys::CAPTURE, "Quick capture, from any app"), ("mod+n", "New in this section"), ("mod+shift+n", "Add a bookmark"), ("mod+v", "Paste a link to save it, or text to make a note"), ("mod+r", "Sync now"), ("mod+comma", "Settings")]),
    ("Go to", &[("mod+1", "Home"), ("mod+2", "Bookmarks"), ("mod+3", "Notes"), ("mod+4", "Snippets"), ("mod+5", "Diagrams"), ("mod+shift+b", "Show or hide the sidebar")]),
    ("Items", &[("enter", "Open the selected bookmark"), ("backspace", "Delete the selection (Undo for 6 s)"), ("mod+z", "Undo a delete"), ("mod+f", "Search this section"), ("Esc", "Clear the search")]),
];

fn shortcuts(k: &Kit, layer: &mut Vec<El>) {
    let mut body = col().w_full().gap(16.0);
    for (group, keys) in SHORTCUTS {
        let mut list = col().w_full().gap(6.0).child(text((*group).to_owned()).font(super::theme::LABEL).color(k.c.text_faint));
        for (key, what) in keys.iter() {
            list = list.child(row().w_full().justify_between().center_y().child(text((*what).to_owned()).font(k.ty.ui).color(k.c.text_base)).child(widgets::keycap(key, k)));
        }
        body = body.child(list);
    }
    KitDialog::new("shortcuts", "Keyboard shortcuts").open(true).fit().width(420.0).click_outside(true).child(body.px(16.0).pb(18.0)).on_close(|s: &mut App| s.dialog = None).build(k, layer);
}
