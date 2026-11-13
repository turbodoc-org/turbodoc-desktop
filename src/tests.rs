//! The app driven as a person would, through Rune's headless runtime: real views, layout,
//! hit testing and handlers, on the demo library, with no network (no `AppHandle`).
use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::{App, BookmarkFilter, Dialog, Layout, Phase, Save, Section};
use crate::{demo, ui};

fn runtime(scene: fn(&mut App)) -> Runtime<App> {
    let mut s = App::new(None);
    scene(&mut s);
    let mut rt = Runtime::new(s, ui::view, (1280.0, 800.0), Env { system_dark: true, scale: 2.0 });
    rt.on_key = Some(crate::on_key);
    rt.frame(0.0);
    rt
}

fn settle(rt: &mut Runtime<App>) {
    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
}

fn click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn key(rt: &mut Runtime<App>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    settle(rt);
}

fn typed(rt: &mut Runtime<App>, text: &str) {
    for c in text.chars() {
        rt.key(KeyEvent { key: Key::Text(c.to_string()), mods: Mods::default() });
    }
    settle(rt);
}

#[test]
fn the_sidebar_moves_between_sections() {
    let mut rt = runtime(demo::sign_in);
    assert_eq!(rt.state.section, Section::Home);
    click(&mut rt, "nav-notes");
    assert_eq!(rt.state.section, Section::Notes);
    assert!(rt.state.note.is_some(), "the first note opens");
    click(&mut rt, "nav-diagrams");
    assert_eq!(rt.state.section, Section::Diagrams);
    assert_eq!(rt.state.diagram.as_ref().map(|d| d.id.as_str()), Some("d1"));
    click(&mut rt, "nav-bookmarks");
    click(&mut rt, "filter-Unread");
    assert_eq!((rt.state.section, rt.state.bm_filter), (Section::Bookmarks, BookmarkFilter::Unread));
    assert!(ui::bookmarks::visible(&rt.state).iter().all(|&i| rt.state.lib.bookmarks[i].unread()));
}

#[test]
fn a_card_selects_and_its_inspector_shows_it() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Bookmarks;
    });
    click(&mut rt, "card-b3");
    assert_eq!(rt.state.selected.as_deref(), Some("b3"));
    assert!(rt.find("inspector-b3").is_some(), "the inspector shows the selected bookmark");
}

#[test]
fn searching_filters_bookmarks_as_you_type() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Bookmarks;
        s.layout = Layout::List;
        s.search.focused = true;
    });
    settle(&mut rt);
    typed(&mut rt, "postgres");
    assert_eq!(rt.state.search.text, "postgres");
    assert_eq!(ui::bookmarks::visible(&rt.state).len(), 1);
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key.clone()).filter(|k| k.starts_with("li-")).collect();
    assert_eq!(keys, vec!["li-b4".to_owned()]);
}

#[test]
fn deleting_waits_out_undo() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Bookmarks;
        s.selected = Some("b2".into());
    });
    let before = rt.state.lib.bookmarks.len();
    key(&mut rt, Key::Backspace, Mods::default());
    assert_eq!(rt.state.lib.bookmarks.len(), before - 1, "gone from the screen at once");
    assert_eq!(rt.state.deletes.len(), 1, "but held for Undo");
    let undo = rt.state.toasts.iter().find(|t| t.undo.is_some()).map(|t| format!("undo-{}", t.id)).expect("an undo toast");
    click(&mut rt, &undo);
    assert_eq!(rt.state.lib.bookmarks.len(), before);
    assert!(rt.state.deletes.is_empty());
    assert_eq!(rt.state.lib.bookmarks[1].id, "b2", "back where it was");
}

#[test]
fn undo_is_also_cmd_z() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Bookmarks;
        s.selected = Some("b5".into());
    });
    key(&mut rt, Key::Delete, Mods::default());
    assert!(rt.state.lib.bookmarks.iter().all(|b| b.id != "b5"));
    key(&mut rt, Key::Command('z'), Mods { cmd: true, logo: true, ..Default::default() });
    assert!(rt.state.lib.bookmarks.iter().any(|b| b.id == "b5"));
}

#[test]
fn the_palette_finds_items_and_runs_them() {
    let mut rt = runtime(demo::sign_in);
    rt.update(|s, _| s.open_palette());
    settle(&mut rt);
    typed(&mut rt, "ramen");
    let first = ui::palette::entries(&rt.state).into_iter().next().map(|e| e.title);
    assert_eq!(first.as_deref(), Some("Recipe: weeknight ramen"));
    key(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.palette.is_none());
    assert_eq!(rt.state.section, Section::Notes);
    assert_eq!(rt.state.note.as_ref().map(|n| n.id.as_str()), Some("n3"));
}

#[test]
fn the_palette_offers_to_save_a_typed_link() {
    let mut rt = runtime(demo::sign_in);
    rt.update(|s, _| s.open_palette());
    settle(&mut rt);
    typed(&mut rt, "news.ycombinator.com");
    key(&mut rt, Key::Enter, Mods::default());
    match &rt.state.dialog {
        Some(Dialog::Bookmark(f)) => assert_eq!(f.url.text, "https://news.ycombinator.com"),
        _ => panic!("the add-bookmark dialog opens with the link"),
    }
}

#[test]
fn typing_in_a_note_schedules_an_autosave() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Notes;
        let n = s.lib.notes[1].clone();
        let mut ed = crate::app::NoteEditor::open(&n);
        ed.body.focused = true;
        ed.body.set_caret(ed.body.text.len());
        s.note = Some(ed);
    });
    settle(&mut rt);
    typed(&mut rt, " More.");
    let ed = rt.state.note.as_ref().expect("open note");
    assert!(ed.body.text.ends_with(" More."));
    assert!(matches!(ed.save, Save::Pending { .. }), "saves after a pause: {:?}", ed.save);
}

#[test]
fn pasting_a_link_anywhere_offers_to_save_it() {
    let mut rt = runtime(demo::sign_in);
    rt.paste("https://turbodoc.ai/mcp");
    settle(&mut rt);
    assert!(matches!(&rt.state.dialog, Some(Dialog::Bookmark(f)) if f.url.text == "https://turbodoc.ai/mcp"));
}

#[test]
fn a_tag_filters_bookmarks_and_notes() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Bookmarks;
    });
    settle(&mut rt);
    click(&mut rt, "tag-rust");
    assert_eq!(rt.state.section, Section::Bookmarks);
    assert_eq!(rt.state.tag.as_deref(), Some("rust"));
    assert!(ui::bookmarks::visible(&rt.state).iter().all(|&i| rt.state.lib.bookmarks[i].tag_list().iter().any(|t| t == "rust")));
    click(&mut rt, "tag-rust");
    assert_eq!(rt.state.tag, None, "a second click clears it");
}

#[test]
fn signed_out_shows_the_form_and_checks_it_before_sending() {
    let mut rt = runtime(|_| {});
    assert!(matches!(rt.state.phase, Phase::SignedOut(_)));
    assert!(rt.find("email").is_some() && rt.find("password").is_some());
    click(&mut rt, "submit");
    match &rt.state.phase {
        Phase::SignedOut(f) => assert!(f.error.as_deref().is_some_and(|e| e.contains("email")), "{:?}", f.error),
        _ => unreachable!(),
    }
}

#[test]
fn every_scene_builds_and_lays_out() {
    for (name, scene) in demo::SCENES {
        let mut rt = runtime(*scene);
        settle(&mut rt);
        assert!(rt.elements().len() > 10, "scene {name} drew almost nothing");
    }
}


#[test]
fn any_row_in_a_list_opens_in_the_detail_pane() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Notes;
        let n = s.lib.notes[0].clone();
        s.note = Some(crate::app::NoteEditor::open(&n));
    });
    settle(&mut rt);
    for id in ["n3", "n2", "n4"] {
        click(&mut rt, &format!("note-{id}"));
        assert_eq!(rt.state.note.as_ref().map(|n| n.id.as_str()), Some(id), "clicking note {id} opens it");
    }
    rt.update(|s, tx| s.go(Section::Snippets, tx));
    settle(&mut rt);
    for id in ["s3", "s2"] {
        click(&mut rt, &format!("snip-{id}"));
        assert_eq!(rt.state.snippet.as_ref().map(|n| n.id.as_str()), Some(id), "clicking snippet {id} opens it");
    }
    rt.update(|s, tx| s.go(Section::Diagrams, tx));
    settle(&mut rt);
    for id in ["d3", "d2"] {
        click(&mut rt, &format!("dia-{id}"));
        assert_eq!(rt.state.diagram.as_ref().map(|n| n.id.as_str()), Some(id), "clicking diagram {id} opens it");
    }
}

#[test]
fn any_bookmark_in_the_list_selects() {
    let mut rt = runtime(|s| {
        demo::sign_in(s);
        s.section = Section::Bookmarks;
        s.layout = Layout::List;
    });
    settle(&mut rt);
    for id in ["b4", "b2", "b9"] {
        click(&mut rt, &format!("li-{id}"));
        assert_eq!(rt.state.selected.as_deref(), Some(id), "clicking bookmark {id} selects it");
    }
    rt.update(|s, _| {
        s.layout = Layout::Grid;
        s.selected = None;
    });
    settle(&mut rt);
    for id in ["b3", "b5", "b9"] {
        rt.update(|_, tx| tx.scroll_into_view(format!("card-{id}")));
        settle(&mut rt);
        click(&mut rt, &format!("card-{id}"));
        assert_eq!(rt.state.selected.as_deref(), Some(id), "clicking card {id} selects it");
    }
}

