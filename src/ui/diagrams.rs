//! Diagrams: Mermaid drawn natively by rune-mermaid (no browser, no JavaScript) and redrawn as
//! you edit its source, and canvas diagrams drawn from their shapes and connections.
use rune::{Cx, El, LineCap, ScrollSpec, Shape, TextStyle, canvas, col, el, row, text, text_input};
use rune_kit::{Button, ButtonVariant, Kit, MarkdownStyle};

use super::theme::Tint;
use super::{icons, widgets};
use crate::api::models::Diagram;
use crate::app::{App, DiagramEditor};
use crate::format;

const LIST_W: f32 = 300.0;
const SOURCE: TextStyle = TextStyle { family: rune::Family::Mono, ..TextStyle::new(12.5, 420.0).lh(20.0) };

pub fn view(s: &App, k: &Kit, cx: &mut Cx<App>) -> El {
    if s.loaded && s.lib.diagrams.is_empty() {
        let action = Button::new("New diagram").icon(&icons::PLUS).variant(ButtonVariant::Contrast).on_press(|s: &mut App| s.new_diagram_dialog()).build(k).key("empty-new");
        return widgets::empty(&icons::DIAGRAM, Tint::Diagram, "Draw your first diagram", "Write Mermaid (flowcharts, sequence and state diagrams) and see it drawn as you type.", Some(action), k);
    }
    let q = s.search.text.trim();
    let fold = rune_kit::search::Fold::default();
    let shown: Vec<&Diagram> = s.lib.diagrams.iter().filter(|d| q.is_empty() || fold.contains(&d.title, q) || d.mermaid_source.as_deref().is_some_and(|m| fold.contains(m, q))).collect();
    let width = cx.layout_size("diagrams-pane").map_or(900.0, |sz| sz[0]);
    let rows = col().w_full().pb(16.0).children(shown.iter().map(|d| diagram_row(s, d, k)));
    let add = widgets::add_button("New diagram", k, |s, _| s.new_diagram_dialog());
    let list = widgets::list_pane("diagram-list", "Diagrams", &format::count(shown.len(), "diagram", "diagrams"), Some(add), rows, LIST_W, k);
    let detail = match &s.diagram {
        Some(ed) => detail(s, ed, k, width - LIST_W),
        None => col().grow(1.0).h_full().center().child(text("Select a diagram").font(k.ty.ui).color(k.c.text_faint)),
    };
    row().key("diagrams-pane").w_full().h_full().min_h(0.0).child(list).child(detail)
}

fn diagram_row(s: &App, d: &Diagram, k: &Kit) -> El {
    let selected = s.diagram.as_ref().is_some_and(|e| e.id == d.id);
    let id = d.id.clone();
    let kind = if d.is_mermaid() { "Mermaid" } else { "Canvas" };
    let details = format!("{kind} · {}", format::ago_inline(d.updated_at.as_deref().unwrap_or("")));
    widgets::list_item(widgets::ListItem { key: format!("dia-{}", d.id), title: d.title.clone(), preview: String::new(), details, lead: None, selected }, d.id.clone(), move |s: &mut App, _| s.open_diagram(&id), k)
}

fn detail(s: &App, ed: &DiagramEditor, k: &Kit, width: f32) -> El {
    let Some(d) = s.lib.diagrams.iter().find(|d| d.id == ed.id) else { return el() };
    let title = text_input(&ed.title, super::theme::TITLE)
        .single_line()
        .key("diagram-title")
        .placeholder("Untitled Diagram", k.c.text_faint)
        .color(k.c.text_strong)
        .w_full()
        .label("Title")
        .on_edit(|s: &mut App| &mut s.diagram.as_mut().expect("open diagram").title)
        .on_input_change(|s: &mut App, tx| {
            if let Some(ed) = &mut s.diagram {
                ed.touch(tx);
            }
        });
    let open_web = {
        let url = format!("https://turbodoc.ai/diagram/{}", d.id);
        Button::new(if d.is_mermaid() { "Open on the web" } else { "Edit on the web" })
            .icon(&icons::EXTERNAL)
            .variant(ButtonVariant::Ghost)
            .on_press_tx(move |_: &mut App, tx| {
                let _ = tx.open_url(&url);
            })
            .build(k)
            .key("diagram-web")
    };
    let more = {
        let id = d.id.clone();
        widgets::bar_button(&icons::MORE, "More")
            .on_press_tx(move |s: &mut App, tx| {
                let menu = widgets::item_menu(s, &id);
                let id = id.clone();
                tx.popup_menu(menu, "diagram-more", move |s: &mut App, p, tx| widgets::item_menu_pick(s, &id, p, tx));
            })
            .build(k)
            .key("diagram-more")
    };
    let kind = if d.is_mermaid() { "Mermaid" } else { "Canvas" };
    let saved = if ed.save.unsaved() { format!(" · {}", ed.save.label()) } else { String::new() };
    let head = row()
        .w_full()
        .gap(16.0)
        .items_end()
        .child(col().grow(1.0).min_w(0.0).gap(4.0).child(title).child(text(format!("{kind} · Edited {}{saved}", format::ago_inline(d.updated_at.as_deref().unwrap_or("")))).font(k.ty.ui).color(k.c.text_faint)))
        .child(row().gap(6.0).center_y().child(open_web).child(more));
    let stage_style = MarkdownStyle::from_kit(k);
    let drawing: El = if d.is_mermaid() {
        match rune_kit::mermaid::render(&ed.source.text, &stage_style) {
            Some(e) => e,
            None if ed.source.text.trim().is_empty() => text("Write Mermaid source to draw it.").font(k.ty.ui).color(k.c.text_faint),
            None => col()
                .gap(4.0)
                .center()
                .child(text("Can't draw this yet").font(k.ty.ui_medium).color(k.c.text_muted))
                .child(text("Flowcharts, sequence and state diagrams draw natively. Open it on the web for the rest.").font(k.ty.small).color(k.c.text_faint).wrap().max_w(320.0).align(rune::Align::Center)),
        }
    } else {
        canvas_drawing(d, k, (width - 96.0).max(200.0))
    };
    // Mermaid draws its own framed surface; a canvas gets a quiet well to sit in.
    let stage = if d.is_mermaid() {
        col().key("diagram-stage").w_full().scroll_x().child(drawing)
    } else {
        col()
            .key("diagram-stage")
            .w_full()
            .min_h(260.0)
            .padding(28.0)
            .radius(14.0)
            .fill(theme_ink(k).thumb)
            .border(1.0, theme_ink(k).thumb_line)
            .center()
            .child(col().w_full().scroll_x().items_center().child(drawing))
    };
    let mut body = col().w_full().px(40.0).pt(24.0).pb(40.0).gap(28.0).child(head).child(stage);
    if d.is_mermaid() {
        let source = text_input(&ed.source, SOURCE)
            .key("diagram-source")
            .color(k.c.text_base)
            .w_full()
            .min_h(160.0)
            .label("Mermaid source")
            .placeholder("flowchart LR\n  A --> B", k.c.text_faint)
            .selection_color(k.selection())
            .on_edit(|s: &mut App| &mut s.diagram.as_mut().expect("open diagram").source)
            .on_input_change(|s: &mut App, tx| {
                if let Some(ed) = &mut s.diagram {
                    ed.touch(tx);
                }
            });
        body = body.child(
            col()
                .w_full()
                .gap(8.0)
                .child(text("Mermaid source").font(super::theme::LABEL).color(k.c.text_faint))
                .child(col().w_full().padding(16.0).radius(12.0).fill(k.c.layer_01).border(1.0, k.c.border_base).child(source)),
        );
    } else {
        body = body.child(text("Canvas diagrams are edited in the web app; they show here as drawn.").font(k.ty.ui).color(k.c.text_faint));
    }
    col().key(format!("diagram-{}", d.id)).grow(1.0).min_w(0.0).h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)
}

/// A canvas diagram drawn to fit `max_w`: shapes as rounded boxes, circles and diamonds with
/// their labels, connections as lines between their centres.
pub fn canvas_drawing(d: &Diagram, k: &Kit, max_w: f32) -> El {
    if d.shapes.is_empty() {
        return text("This canvas is empty.").font(k.ty.ui).color(k.c.text_faint);
    }
    let min_x = d.shapes.iter().map(|s| s.x).fold(f32::MAX, f32::min);
    let min_y = d.shapes.iter().map(|s| s.y).fold(f32::MAX, f32::min);
    let max_x = d.shapes.iter().map(|s| s.x + s.width).fold(f32::MIN, f32::max);
    let max_y = d.shapes.iter().map(|s| s.y + s.height).fold(f32::MIN, f32::max);
    let (w, h) = ((max_x - min_x).max(1.0), (max_y - min_y).max(1.0));
    let scale = (max_w / w).min(1.0).min(560.0 / h).max(0.2);
    let (cw, ch) = (w * scale, h * scale);
    let accent = Tint::Diagram.color(k.dark);
    let line = k.c.text_faint;
    let mut lines = Shape::new();
    for c in &d.connections {
        let (Some(a), Some(b)) = (d.shapes.iter().find(|s| s.id == c.from), d.shapes.iter().find(|s| s.id == c.to)) else { continue };
        let pa = [(a.x + a.width / 2.0 - min_x) * scale, (a.y + a.height / 2.0 - min_y) * scale];
        let pb = [(b.x + b.width / 2.0 - min_x) * scale, (b.y + b.height / 2.0 - min_y) * scale];
        // From the source's edge to the target's, so lines never cross the shapes they join.
        let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (ux, uy) = (dx / len, dy / len);
        let edge = |w: f32, h: f32| ((w * scale / 2.0) / ux.abs().max(0.01)).min((h * scale / 2.0) / uy.abs().max(0.01));
        let (from_in, to_in) = (edge(a.width, a.height), edge(b.width, b.height) + 2.0);
        let start = [pa[0] + ux * from_in, pa[1] + uy * from_in];
        let tip = [pb[0] - ux * to_in, pb[1] - uy * to_in];
        let back = [tip[0] - ux * 9.0, tip[1] - uy * 9.0];
        if len > from_in + to_in + 10.0 {
            lines = lines.line(start, back, 1.5, LineCap::Round, line);
        }
        lines = lines.fill(vec![tip, [back[0] - uy * 4.5, back[1] + ux * 4.5], [back[0] + uy * 4.5, back[1] - ux * 4.5]], line);
    }
    let mut stage = el().size(cw, ch).child(canvas(lines).absolute(0.0, 0.0).size(cw, ch));
    for sh in &d.shapes {
        let (x, y, sw, shh) = ((sh.x - min_x) * scale, (sh.y - min_y) * scale, sh.width * scale, sh.height * scale);
        let fill = sh.color.as_deref().and_then(rune::color::hex).unwrap_or(accent);
        let label = text(sh.label.clone()).font(TextStyle::new((sh.font_size.unwrap_or(14.0) * scale).clamp(9.0, 18.0), 520.0)).color(k.c.text_base).wrap().align(rune::Align::Center);
        let node = match sh.kind.as_str() {
            "circle" => el().radius(sw.min(shh) / 2.0).fill(fill.alpha(0.16)).border(1.5, fill),
            "diamond" => {
                let poly = vec![[sw / 2.0, 0.0], [sw, shh / 2.0], [sw / 2.0, shh], [0.0, shh / 2.0]];
                el().child(canvas(Shape::new().fill(poly.clone(), fill.alpha(0.16)).stroke(poly.into_iter().chain([[sw / 2.0, 0.0]]).collect::<Vec<_>>(), 1.5, fill)).absolute(0.0, 0.0).size(sw, shh))
            }
            "database" => el().radius(sw.min(shh) * 0.18).fill(fill.alpha(0.16)).border(1.5, fill).child(el().absolute(0.0, shh * 0.22).w(sw).h(1.5).fill(fill)),
            "text" => el(),
            _ => el().radius(8.0).fill(fill.alpha(0.16)).border(1.5, fill),
        };
        stage = stage.child(node.absolute(x, y).size(sw, shh).center().padding(6.0).child(label));
    }
    stage.label(format!("{} shapes", d.shapes.len()))
}

fn theme_ink(k: &Kit) -> super::theme::Ink { super::theme::ink(k.dark) }
