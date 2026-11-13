//! Every screen, built from app state. Views never block and never touch the network.
pub mod bookmarks;
pub mod diagrams;
pub mod dialogs;
pub mod export;
pub mod home;
pub mod icons;
pub mod md_syntax;
pub mod notes;
pub mod palette;
pub mod previews;
pub mod settings;
pub mod shell;
pub mod signin;
pub mod snippets;
pub mod theme;
pub mod toasts;
pub mod widgets;

use rune::{Cx, El};

use crate::app::{App, Phase};

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let system = cx.system_dark();
    bookmarks::DARK.with(|d| d.set(system));
    let k = theme::kit(s.dark(system));
    cx.caption_color(k.c.text_muted);
    let body = match &s.phase {
        // Reading the saved session: a still window, not a sign-in form that may vanish.
        Phase::SignedOut(_) if s.restoring => rune::col()
            .key("launching")
            .w_full()
            .h_full()
            .center()
            .gap(14.0)
            .child(rune::icon(&icons::BOLT, 28.0).color(k.c.text_faint))
            .child(rune::el().w_full().h(52.0).absolute(0.0, 0.0).window_drag()),
        Phase::SignedOut(form) => rune_kit::root(signin::view(s, form, k, cx), Vec::new()),
        Phase::Ready(user) => shell::view(s, user, k, cx),
    };
    body.fill(k.c.bg_base)
}
