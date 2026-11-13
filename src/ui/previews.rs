//! What each kind of item shows in a card's preview well: a page's image (or a quiet
//! placeholder), the opening of a note, the top of a snippet, a diagram drawn small.
use rune::{El, TextStyle, col, el, rich, text};
use rune_kit::Kit;

use super::theme;
use crate::api::models::{Bookmark, Diagram, Note, Snippet};
use crate::app::App;
use crate::format;

const NOTE_TITLE: TextStyle = TextStyle::new(12.5, 620.0).lh(17.0);
const NOTE_TEXT: TextStyle = TextStyle::new(11.0, 420.0).lh(17.0);
const CODE: TextStyle = TextStyle { family: rune::Family::Mono, ..TextStyle::new(10.5, 420.0).lh(16.0) };

/// A page's preview image, or the site's initial on a faint wash of its hue with its name.
pub fn bookmark(s: &App, b: &Bookmark, k: &Kit) -> El {
    if let Some(img) = s.preview(&b.url) {
        let e = el().w_full().h_full().pixels_fit(img.pixels(), rune::SurfaceFit::Fill).transition(rune::Transition::opacity());
        return if b.broken() { e.opacity(0.45) } else { e };
    }
    domain_fallback(&format::domain(&b.url), k)
}

pub fn domain_fallback(domain: &str, k: &Kit) -> El {
    let hue = theme::hue_for(domain);
    let letter = domain.chars().next().unwrap_or('•').to_ascii_uppercase().to_string();
    let ink = theme::ink(k.dark);
    col()
        .w_full()
        .h_full()
        .center()
        .gap(8.0)
        .linear_gradient(2.36, &[(0.0, rune::oklcha(0.62, 0.12, hue, if k.dark { 0.14 } else { 0.13 })), (1.0, rune::oklcha(0.62, 0.14, (hue + 50.0) % 360.0, 0.05))])
        .child(
            el().size(36.0, 36.0)
                .radius(10.0)
                .fill(rune::oklch(0.52, 0.11, hue))
                .shadow(1.0, 3.0, rune::Color::BLACK.alpha(0.15))
                .center()
                .child(text(letter).font(TextStyle::new(16.0, 650.0)).color(rune::Color::WHITE)),
        )
        .child(text(domain.to_owned()).font(TextStyle::new(11.5, 520.0)).color(ink.thumb_text).truncate().max_w(200.0))
}

/// The note's title and opening lines, as on a page.
pub fn note(n: &Note, k: &Kit) -> El {
    let ink = theme::ink(k.dark);
    let body = n.markdown.strip_prefix(&format!("# {}", n.title)).unwrap_or(&n.markdown);
    let mut lines = col().gap(1.0);
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("```")).take(6) {
        let plain = format::preview(line, 90);
        if plain.is_empty() {
            continue;
        }
        let heading = line.starts_with('#');
        lines = lines.child(text(plain).font(if heading { TextStyle::new(11.0, 620.0).lh(17.0) } else { NOTE_TEXT }).color(ink.thumb_text).truncate());
    }
    col()
        .w_full()
        .h_full()
        .px(14.0)
        .pt(12.0)
        .gap(6.0)
        .child(text(n.display_title()).font(NOTE_TITLE).color(k.c.text_base).truncate())
        .child(lines)
}

/// The snippet's first lines, coloured as code.
pub fn snippet(x: &Snippet, k: &Kit) -> El {
    let code: String = x.code.lines().take(9).collect::<Vec<_>>().join("\n");
    let ranges = rune_kit::syntax::highlight(Some(super::snippets::lang_for_lexer(&x.language)), &code);
    let spans = rune_kit::syntax::SyntaxTheme::from_kit(k).spans(&ranges, 0);
    col().w_full().h_full().px(14.0).pt(12.0).child(rich(code, spans).font(CODE).color(theme::ink(k.dark).thumb_text))
}

/// A diagram drawn small: Mermaid by rune-mermaid, canvases from their shapes.
pub fn diagram(d: &Diagram, w: f32, k: &Kit) -> El {
    if d.is_mermaid() {
        let style = rune_kit::MarkdownStyle::from_kit(k);
        if let Some(drawing) = d.mermaid_source.as_deref().and_then(|src| rune_kit::mermaid::render(src, &style)) {
            return el().w_full().h_full().center().child(drawing.scale(0.5));
        }
        return col().w_full().h_full().px(14.0).pt(12.0).child(text(d.mermaid_source.clone().unwrap_or_default()).font(CODE).color(theme::ink(k.dark).thumb_text).max_lines(7));
    }
    el().w_full().h_full().center().child(super::diagrams::canvas_drawing(d, k, (w - 40.0).max(80.0)).scale(0.8))
}

/// A row of placeholders while the library loads.
pub fn skeleton_card(i: usize, w: f32, k: &Kit) -> El {
    let well_h = ((w - 16.0) * 10.0 / 16.0).round();
    col()
        .key(format!("card-sk-{i}"))
        .w(w)
        .radius(14.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_base)
        .child(el().w_full().pt(8.0).px(8.0).child(super::widgets::skeleton(None, well_h, k).radius(9.0)))
        .child(col().px(14.0).pt(14.0).pb(16.0).gap(8.0).child(super::widgets::skeleton(Some(w * 0.6), 11.0, k)).child(super::widgets::skeleton(Some(w * 0.35), 9.0, k)))
}
