//! Brief messages at the bottom of the window; a delete's carries Undo.
use rune::{El, col, icon, row, text};
use rune_kit::Kit;

use super::icons;
use crate::app::{App, ToastKind};

pub fn view(s: &App, k: &Kit) -> Option<El> {
    if s.toasts.is_empty() {
        return None;
    }
    let mut stack = col().gap(8.0).items_center();
    for t in s.toasts.iter().rev().take(3) {
        let (glyph, ink) = match t.kind {
            ToastKind::Success => (&icons::CHECK_CIRCLE, super::theme::Tint::Code.color(true)),
            ToastKind::Error => (k.glyph.circle_x, rune::oklch(0.72, 0.17, 25.0)),
            ToastKind::Info => (&icons::SPARKLE, rune::Color::WHITE.alpha(0.7)),
        };
        let mut pill = row()
            .key(format!("toast-{}", t.id))
            .h(38.0)
            .pl(12.0)
            .pr(if t.undo.is_some() { 6.0 } else { 14.0 })
            .gap(9.0)
            .radius(19.0)
            .fill(rune::oklch(0.22, 0.006, 270.0).alpha(0.96))
            .border(1.0, rune::Color::WHITE.alpha(0.08))
            .shadow(10.0, 30.0, rune::Color::BLACK.alpha(0.3))
            .center_y()
            .child(icon(glyph, 14.0).color(ink))
            .child(text(t.text.clone()).font(k.ty.ui_medium).color(rune::Color::WHITE.alpha(0.92)).truncate().max_w(420.0))
            .live_region(rune::LiveRegion::Polite)
            .transition(rune::Transition::offset(0.0, 14.0).combined(rune::Transition::opacity()));
        if let Some(id) = &t.undo {
            let id = id.clone();
            pill = pill.child(
                row()
                    .key(format!("undo-{}", t.id))
                    .h(26.0)
                    .px(10.0)
                    .radius(13.0)
                    .fill(rune::Color::WHITE.alpha(0.12))
                    .hover(rune::Color::WHITE.alpha(0.08))
                    .center()
                    .cursor(rune::Cursor::Pointer)
                    .role("button")
                    .label("Undo")
                    .focusable(true)
                    .child(text("Undo").font(k.ty.ui_medium).color(rune::Color::WHITE))
                    .on_press_tx(move |s: &mut App, tx| s.undo_delete(&id, tx)),
            );
        }
        stack = stack.child(pill);
    }
    Some(col().key("toasts").absolute(0.0, 0.0).w_full().h_full().justify_end().items_center().pb(22.0).child(stack))
}
