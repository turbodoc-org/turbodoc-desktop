//! Code snippets: a list and a presentation view (the snippet's gradient, a window frame and
//! syntax colours from OpenCode Desktop's own lexers), or an editor that colours as you type.
use rune::{Cx, El, ScrollSpec, TextStyle, col, el, rich, row, text, text_input};
use rune_kit::syntax::SyntaxTheme;
use rune_kit::{Button, ButtonVariant, Kit};

use super::theme::Tint;
use super::{icons, widgets};
use crate::api::models::{LANGUAGES, Snippet, language_name};
use crate::app::{App, Save, SnippetEditor};
use crate::format;

const LIST_W: f32 = 300.0;
const CODE: TextStyle = TextStyle { family: rune::Family::Mono, ..TextStyle::new(13.0, 420.0).lh(21.0) };

pub fn visible(s: &App) -> Vec<&Snippet> {
    let q = s.search.text.trim();
    let fold = rune_kit::search::Fold::default();
    s.lib
        .snippets
        .iter()
        .filter(|x| q.is_empty() || fold.contains(x.display_title(), q) || fold.contains(&x.code, q) || fold.contains(&x.language, q))
        .collect()
}

pub fn view(s: &App, k: &Kit, cx: &mut Cx<App>) -> El {
    if s.loaded && s.lib.snippets.is_empty() {
        let action = Button::new("New snippet").icon(&icons::PLUS).variant(ButtonVariant::Contrast).on_press_tx(|s: &mut App, tx| s.new_snippet(String::new(), None, tx)).build(k).key("empty-new");
        return widgets::empty(&icons::CODE, Tint::Code, "Keep code you reuse", "Snippets keep their language and colours. Paste code anywhere in this section, or drop source files on the window.", Some(action), k);
    }
    let shown = visible(s);
    let width = cx.layout_size("snippets-pane").map_or(900.0, |sz| sz[0]);
    let rows = col().w_full().pb(16.0).children(shown.iter().map(|x| snippet_row(s, x, k)));
    let add = widgets::add_button("New snippet", k, |s, tx| s.new_snippet(String::new(), None, tx));
    let list = widgets::list_pane("snippet-list", "Snippets", &format::count(shown.len(), "snippet", "snippets"), Some(add), rows, LIST_W, k);
    let detail = match &s.snippet {
        Some(ed) => detail(s, ed, k, width - LIST_W),
        None => col().grow(1.0).h_full().center().child(text("Select a snippet").font(k.ty.ui).color(k.c.text_faint)),
    };
    row().key("snippets-pane").w_full().h_full().min_h(0.0).child(list).child(detail)
}

fn snippet_row(s: &App, x: &Snippet, k: &Kit) -> El {
    let selected = s.snippet.as_ref().is_some_and(|e| e.id == x.id);
    let id = x.id.clone();
    let lines = x.code.lines().count();
    let details = format!("{} · {} · {}", language_name(&x.language), format::count(lines, "line", "lines"), format::ago_inline(x.updated_at.as_deref().unwrap_or("")));
    widgets::list_item(widgets::ListItem { key: format!("snip-{}", x.id), title: x.display_title().to_owned(), preview: String::new(), details, lead: None, selected }, x.id.clone(), move |s: &mut App, _| s.open_snippet(&id), k)
}

/// The two colours of a CSS `linear-gradient(...)`, else the default violet.
fn gradient(css: Option<&str>) -> (rune::Color, rune::Color) {
    let fallback = (rune::color::hex("#667eea").unwrap_or(rune::Color::BLACK), rune::color::hex("#764ba2").unwrap_or(rune::Color::BLACK));
    let Some(css) = css else { return fallback };
    let hexes: Vec<rune::Color> = css.split(|c: char| !(c.is_ascii_hexdigit() || c == '#')).filter(|t| t.starts_with('#') && (t.len() == 7 || t.len() == 4)).filter_map(rune::color::hex).collect();
    match hexes.as_slice() {
        [a, b, ..] => (*a, *b),
        [a] => (*a, *a),
        [] => fallback,
    }
}

fn detail(s: &App, ed: &SnippetEditor, k: &Kit, width: f32) -> El {
    let snippet = s.lib.snippets.iter().find(|x| x.id == ed.id);
    // Code always draws on a dark window, as the web's presentation does.
    let code_kit = super::theme::kit(true);
    let syntax = SyntaxTheme::from_kit(code_kit);
    let ranges = rune_kit::syntax::highlight(Some(lang_for_lexer(&ed.language)), &ed.code.text);
    let spans = syntax.spans(&ranges, 0);

    let lang_button = {
        let current = ed.language.clone();
        Button::new(language_name(&ed.language))
            .variant(ButtonVariant::Outline)
            .help("Language")
            .on_press_tx(move |_: &mut App, tx| {
                let menu = rune::PopupMenu::picker(LANGUAGES.iter().map(|(id, name)| (*id, *name)), &current);
                tx.popup_menu(menu, "snippet-lang", |s: &mut App, picked, tx| {
                    if let (Some(p), Some(ed)) = (picked, &mut s.snippet) {
                        ed.language = p.to_owned();
                        ed.touch(tx);
                    }
                });
            })
            .build(k)
            .key("snippet-lang")
    };
    let copy = {
        let code = ed.code.text.clone();
        Button::new("Copy").icon(&icons::COPY).variant(ButtonVariant::Ghost).on_press_tx(move |s: &mut App, tx| {
            tx.copy(code.clone());
            s.toast(crate::app::ToastKind::Success, "Code copied", tx);
        })
        .build(k)
        .key("snippet-copy")
    };
    let edit = Button::new(if ed.editing { "Done" } else { "Edit" })
        .icon(if ed.editing { &icons::CHECK_CIRCLE } else { &icons::EDIT })
        .variant(if ed.editing { ButtonVariant::Contrast } else { ButtonVariant::Ghost })
        .on_press_tx(|s: &mut App, tx| {
            let Some(ed) = &mut s.snippet else { return };
            ed.editing = !ed.editing;
            if ed.editing {
                ed.code.focused = true;
            } else if matches!(ed.save, Save::Pending { .. }) {
                s.save_snippet(tx);
            }
        })
        .build(k)
        .key("snippet-edit");
    let more = {
        let id = ed.id.clone();
        widgets::bar_button(&icons::MORE, "More")
            .on_press_tx(move |s: &mut App, tx| {
                let menu = widgets::item_menu(s, &id);
                let id = id.clone();
                tx.popup_menu(menu, "snippet-more", move |s: &mut App, p, tx| widgets::item_menu_pick(s, &id, p, tx));
            })
            .build(k)
            .key("snippet-more")
    };
    let title: El = if ed.editing {
        text_input(&ed.title, super::theme::TITLE)
            .single_line()
            .key("snippet-title")
            .placeholder("Untitled snippet", k.c.text_faint)
            .color(k.c.text_strong)
            .w_full()
            .label("Title")
            .on_edit(|s: &mut App| &mut s.snippet.as_mut().expect("open snippet").title)
            .on_input_change(|s: &mut App, tx| {
                if let Some(ed) = &mut s.snippet {
                    ed.touch(tx);
                }
            })
    } else {
        text(if ed.title.text.is_empty() { "Untitled snippet".to_owned() } else { ed.title.text.clone() }).font(super::theme::TITLE).color(k.c.text_strong).truncate()
    };
    let saved = if ed.save.unsaved() { format!(" · {}", ed.save.label()) } else { String::new() };
    let meta = snippet.map(|x| format!("{} · Edited {}{saved}", format::count(ed.code.text.lines().count(), "line", "lines"), format::ago_inline(x.updated_at.as_deref().unwrap_or("")))).unwrap_or_default();
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(title).child(text(meta).font(k.ty.ui).color(k.c.text_faint)))
        .child(row().gap(6.0).center_y().child(lang_button).child(copy).child(edit).child(more));

    let lines = ed.code.text.lines().count().max(1);
    let gutter = col().pr(14.0).children((1..=lines).map(|n| text(n.to_string()).font(CODE).color(code_kit.c.text_faint.alpha(0.6)).align(rune::Align::End))).a11y_hidden();
    let code: El = if ed.editing {
        text_input(&ed.code, CODE)
            .key("snippet-code")
            .color(code_kit.c.text_base)
            .grow(1.0)
            .min_w(0.0)
            .label("Code")
            .decorations(spans)
            .selection_color(code_kit.selection())
            .on_edit(|s: &mut App| &mut s.snippet.as_mut().expect("open snippet").code)
            .on_input_change(|s: &mut App, tx| {
                if let Some(ed) = &mut s.snippet {
                    ed.touch(tx);
                }
            })
    } else {
        rich(ed.code.text.clone(), spans).font(CODE).color(code_kit.c.text_base).selectable(true).selection_color(code_kit.selection()).key("snippet-code-view")
    };
    let (a, b) = gradient(snippet.and_then(|x| x.background_value.as_deref()));
    let lights = row().gap(7.0).child(widgets::dot(rune::oklch(0.68, 0.19, 25.0), 11.0)).child(widgets::dot(rune::oklch(0.82, 0.15, 85.0), 11.0)).child(widgets::dot(rune::oklch(0.74, 0.17, 145.0), 11.0));
    let window = col()
        .w_full()
        .radius(12.0)
        .fill(rune::color::hex("#16171d").unwrap_or(rune::Color::BLACK))
        .border(1.0, rune::Color::WHITE.alpha(0.08))
        .shadow(18.0, 48.0, rune::Color::BLACK.alpha(0.45))
        .child(row().w_full().h(36.0).px(14.0).center_y().child(lights).child(rune::spacer()).child(text(language_name(&ed.language).to_owned()).font(k.ty.small).color(rune::Color::WHITE.alpha(0.4))))
        .child(row().w_full().px(18.0).pb(18.0).pt(4.0).items_start().child(gutter).child(code).scroll_x());
    let stage = col()
        .key("snippet-stage")
        .w_full()
        .padding(if width > 700.0 { 40.0 } else { 18.0 })
        .radius(k.radius.xl)
        .linear_gradient(2.36, &[(0.0, a), (1.0, b)])
        .child(window);
    col()
        .key(format!("snippet-{}", ed.id))
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(col().w_full().px(40.0).pt(24.0).pb(40.0).gap(28.0).child(head).child(stage))
        .child(el().h(12.0))
}

/// Our language ids, as the highlighter names them.
pub fn lang_for_lexer(id: &str) -> &str {
    match id {
        "bash" => "shell",
        "csharp" => "cs",
        other => other,
    }
}
