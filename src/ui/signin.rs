//! Signing in: the same Turbodoc account as the web and iOS apps (email and password), plus
//! creating one and resetting a password.
use rune::{Cx, El, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, TextInput};

use super::theme::{self, Tint};
use super::{icons, widgets};
use crate::app::{App, AuthMode, Phase, SignIn};

fn form(s: &mut App) -> &mut SignIn {
    match &mut s.phase {
        Phase::SignedOut(f) => f,
        Phase::Ready(_) => unreachable!("the sign-in form only shows while signed out"),
    }
}

pub fn view(_s: &App, f: &SignIn, k: &Kit, _cx: &mut Cx<App>) -> El {
    let (title, sub, cta) = match f.mode {
        AuthMode::SignIn => ("Welcome back", "Sign in to your Turbodoc account.", "Sign in"),
        AuthMode::SignUp => ("Create your account", "One account for the web, iPhone and this Mac.", "Create account"),
        AuthMode::Reset => ("Reset your password", "We'll email you a link to choose a new one.", "Send reset link"),
    };
    let email = TextInput::new(&f.email)
        .large()
        .width(None)
        .label("Email")
        .placeholder("you@example.com")
        .leading_icon(&icons::MAIL)
        .disabled(f.busy)
        .on_edit(|s: &mut App| &mut form(s).email)
        .on_submit(|s: &mut App, tx| {
            if form(s).mode == AuthMode::Reset {
                s.submit_sign_in(tx);
            } else {
                form(s).password.focused = true;
                tx.focus("password", false);
            }
        })
        .build(k)
        .key("email");
    let password = (f.mode != AuthMode::Reset).then(|| {
        TextInput::new(&f.password)
            .large()
            .width(None)
            .secure()
            .label("Password")
            .placeholder("Password")
            .leading_icon(&icons::LOCK)
            .disabled(f.busy)
            .on_edit(|s: &mut App| &mut form(s).password)
            .on_submit(|s: &mut App, tx| s.submit_sign_in(tx))
            .build(k)
            .key("password")
    });
    let submit = Button::new(if f.busy { "Working…" } else { cta })
        .variant(if f.busy { ButtonVariant::Loading } else { ButtonVariant::Contrast })
        .control_size(ControlSize::Large)
        .disabled(f.busy)
        .on_press_tx(|s: &mut App, tx| s.submit_sign_in(tx))
        .build(k)
        .key("submit")
        .w_full();
    let message = f.error.as_ref().map(|e| banner(e, true, k)).or_else(|| f.info.as_ref().map(|i| banner(i, false, k)));
    let link = |label: &str, mode: AuthMode| {
        text(label.to_owned())
            .font(k.ty.ui_medium)
            .color(k.c.text_accent)
            .cursor(rune::Cursor::Pointer)
            .role("link")
            .focusable(true)
            .on_press(move |s: &mut App| {
                let f = form(s);
                f.mode = mode;
                f.error = None;
                f.info = None;
            })
    };
    let links = match f.mode {
        AuthMode::SignIn => row().w_full().justify_between().child(link("Forgot password?", AuthMode::Reset)).child(link("Create an account", AuthMode::SignUp)),
        AuthMode::SignUp => row().w_full().justify_center().gap(4.0).child(text("Have an account?").font(k.ty.ui).color(k.c.text_muted)).child(link("Sign in", AuthMode::SignIn)),
        AuthMode::Reset => row().w_full().justify_center().child(link("Back to sign in", AuthMode::SignIn)),
    };
    let logo = row()
        .gap(10.0)
        .center_y()
        .child(
            el().size(40.0, 40.0)
                .radius(11.0)
                .linear_gradient(2.4, &[(0.0, rune::color::hex("#3b82f6").unwrap_or(k.c.border_focus)), (1.0, rune::color::hex("#1d4ed8").unwrap_or(k.c.border_focus))])
                .center()
                .shadow(6.0, 16.0, rune::color::hex("#2563eb").unwrap_or(k.c.border_focus).alpha(0.35))
                .child(icon(&icons::BOLT, 20.0).color(rune::Color::WHITE)),
        )
        .child(text("Turbodoc").font(rune::TextStyle::new(20.0, 700.0).tracking(-0.4)).color(k.c.text_strong));
    let card = col()
        .key("signin-card")
        .w(380.0)
        .padding(28.0)
        .gap(18.0)
        .radius(16.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_weak)
        .shadow(20.0, 60.0, rune::Color::BLACK.alpha(if k.dark { 0.5 } else { 0.08 }))
        .child(col().gap(4.0).child(text(title).font(theme::TITLE).color(k.c.text_strong)).child(text(sub).font(k.ty.ui).color(k.c.text_muted)))
        .children(message)
        .child(col().w_full().gap(10.0).child(email).children(password))
        .child(submit)
        .child(links)
        .transition(rune::Transition::scale(0.97));
    let features = row()
        .gap(18.0)
        .child(feature(&icons::BOOKMARK, Tint::Bookmark, "Bookmarks", k))
        .child(feature(&icons::NOTE, Tint::Note, "Notes", k))
        .child(feature(&icons::CODE, Tint::Code, "Snippets", k))
        .child(feature(&icons::DIAGRAM, Tint::Diagram, "Diagrams", k));
    let page = col()
        .key("signin")
        .w_full()
        .h_full()
        .center()
        .gap(26.0)
        .fill(k.c.bg_base)
        .radial_gradient([0.5, 0.0], [760.0, 520.0], &[(0.0, Tint::Bookmark.color(k.dark).alpha(if k.dark { 0.16 } else { 0.10 })), (1.0, rune::Color::TRANSPARENT)])
        .child(logo)
        .child(card)
        .child(features)
        .child(text("Your session is kept in the system keychain.").font(k.ty.small).color(k.c.text_faint));
    el().w_full().h_full().child(page).child(el().absolute(0.0, 0.0).w_full().h(52.0).window_drag())
}

fn feature(glyph: &'static rune::Icon, tint: Tint, label: &str, k: &Kit) -> El {
    row().gap(6.0).center_y().child(widgets::tile(glyph, tint, k.dark, 22.0)).child(text(label.to_owned()).font(k.ty.small_medium).color(k.c.text_muted))
}

fn banner(msg: &str, error: bool, k: &Kit) -> El {
    let (bg, fg) = if error { (k.c.danger.bg, k.c.danger.fg) } else { (k.c.info.bg, k.c.info.fg) };
    row()
        .key("signin-banner")
        .w_full()
        .padding(10.0)
        .gap(8.0)
        .radius(k.radius.md)
        .fill(bg)
        .items_start()
        .child(icon(if error { k.glyph.circle_x } else { &icons::MAIL }, 14.0).color(fg).mt(2.0))
        .child(text(msg.to_owned()).font(k.ty.ui).color(fg).wrap().grow(1.0).min_w(0.0))
        .live_region(rune::LiveRegion::Polite)
        .transition(rune::Transition::opacity())
}
