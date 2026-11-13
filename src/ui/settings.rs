//! Settings: the account, appearance, quick capture, the weekly email digest, and the offline copy.
use rune::{Cx, El, ScrollSpec, col, row, text};
use rune_kit::shell::Page;
use rune_kit::{Button, ButtonVariant, Kit, SegmentedControl, Switch};
use serde_json::json;

use super::theme::{self, Tint};
use super::widgets;
use crate::api::auth::User;
use crate::api::client;
use crate::app::{App, Appearance, ToastKind};
use crate::format;

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn card(title: &str, body: El, k: &Kit) -> El {
    col()
        .w_full()
        .gap(8.0)
        .child(text(title.to_owned()).font(theme::LABEL).color(k.c.text_faint).px(4.0))
        .child(col().w_full().radius(k.radius.lg).fill(k.c.layer_01).border(1.0, k.c.border_weak).child(body))
}

fn line(title: &str, detail: &str, control: El, k: &Kit) -> El {
    row()
        .w_full()
        .px(16.0)
        .py(12.0)
        .gap(16.0)
        .center_y()
        .child(col().grow(1.0).min_w(0.0).gap(2.0).child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_base)).child(text(detail.to_owned()).font(k.ty.small).color(k.c.text_muted).wrap()))
        .child(control)
}

pub fn view(s: &App, user: &User, k: &Kit, cx: &mut Cx<App>, _layer: &mut Vec<El>) -> El {
    let stats = s.lib.stats.clone().unwrap_or_default();
    let email = user.email.clone().unwrap_or_default();
    let since = stats.member_since.as_deref().or(user.created_at.as_deref()).map(|t| format::long_date(t).split(" at ").next().unwrap_or("").to_owned()).unwrap_or_default();
    let initial = email.chars().next().unwrap_or('T').to_ascii_uppercase().to_string();
    let account = row()
        .w_full()
        .padding(16.0)
        .gap(14.0)
        .center_y()
        .child(rune::el().size(44.0, 44.0).radius(22.0).fill(k.c.layer_03).center().child(text(initial).font(theme::HEADING).color(k.c.text_base)))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(email).font(k.ty.body_medium).color(k.c.text_base).truncate())
                .child(text(if since.is_empty() { "Turbodoc account".into() } else { format!("Member since {since}") }).font(k.ty.small).color(k.c.text_muted)),
        )
        .child(Button::new("Sign out").variant(ButtonVariant::Outline).on_press_tx(|s: &mut App, tx| s.sign_out(tx)).build(k).key("sign-out"));
    let counts = row()
        .w_full()
        .px(16.0)
        .pb(14.0)
        .gap(10.0)
        .child(count(s.lib.bookmarks.len(), "bookmarks", Tint::Bookmark, k))
        .child(count(s.lib.notes.len(), "notes", Tint::Note, k))
        .child(count(s.lib.snippets.len(), "snippets", Tint::Code, k))
        .child(count(s.lib.diagrams.len(), "diagrams", Tint::Diagram, k));

    let appearance = SegmentedControl::new(&["System", "Light", "Dark"], Some(match s.appearance {
        Appearance::System => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    }))
    .fit()
    .label("Appearance")
    .on_pick(|s: &mut App, i, tx| {
        s.appearance = [Appearance::System, Appearance::Light, Appearance::Dark][i.min(2)];
        s.set_pref("appearance", &s.appearance.id());
        tx.color_motion(rune::tok::motion::REFLOW);
    })
    .build(k)
    .key("appearance");

    let digest: El = match &s.digest {
        None => line("Weekly digest", "Loading…", rune::el(), k),
        Some(d) => {
            let on = d.enabled;
            let mut c = col().w_full().child(line(
                "Weekly digest",
                "An email with what you saved this week and bookmarks worth revisiting.",
                Switch::new(on).on_toggle(move |s: &mut App| s.set_digest(json!({ "enabled": !on }))).build(k).key("digest-switch"),
                k,
            ));
            if on {
                c = c.child(widgets::hairline(k)).child(line(
                    "Day",
                    &format!("Sent around {} ({})", d.send_time.get(..5).unwrap_or("10:00"), d.timezone),
                    SegmentedControl::new(&DAYS, Some(d.day_of_week.clamp(0, 6) as usize)).fit().label("Day").on_pick(|s: &mut App, i, _| s.set_digest(json!({ "day_of_week": i }))).build(k).key("digest-day"),
                    k,
                ));
            }
            c
        }
    };

    let synced = if s.lib.synced_at > 0 { format!("Last synced {}.", format::ago_from(chrono::DateTime::from_timestamp(s.lib.synced_at, 0).unwrap_or_default(), chrono::Utc::now()).to_lowercase()) } else { "Not synced yet.".into() };
    let storage = col()
        .w_full()
        .child(line(
            "Offline copy",
            &format!("Your library is kept on this Mac so it opens instantly and works offline. {synced}"),
            Button::new("Sync now").variant(ButtonVariant::Neutral).on_press_tx(|s: &mut App, tx| s.refresh(tx)).build(k).key("settings-sync"),
            k,
        ))
        .child(widgets::hairline(k))
        .child(line(
            "Quick capture",
            "Save a link or a thought from any app, without switching to Turbodoc.",
            row().gap(8.0).center_y().child(widgets::keycap(crate::keys::CAPTURE, k)).child(Button::new("Try it").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.open_capture()).build(k)),
            k,
        ));
    let about = col().w_full().child(line(
        &format!("Turbodoc for desktop {}", env!("CARGO_PKG_VERSION")),
        &format!("Native and GPU-rendered with Rune. Connected to {}.", client::base().trim_start_matches("https://")),
        Button::new("turbodoc.ai")
            .variant(ButtonVariant::Ghost)
            .on_press_tx(|_: &mut App, tx| {
                let _ = tx.open_url("https://turbodoc.ai");
            })
            .build(k),
        k,
    ));

    let body = col()
        .key("settings-body")
        .w_full()
        .items_center()
        .padding(28.0)
        .child(
            col()
                .w_full()
                .max_w(620.0)
                .gap(22.0)
                .child(card("Account", col().w_full().child(account).child(counts), k))
                .child(card("Appearance", line("Theme", "Follow the system, or pick one.", appearance, k), k))
                .child(card("Email", digest, k))
                .child(card("This Mac", storage, k))
                .child(card("About", about, k)),
        );
    let content = col().w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body);
    let back = Button::new("Done").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.settings = false).build(k).key("settings-done");
    Page::new(content).title("Settings").leading(back).titlebar_inset(cx.titlebar_inset()).frame_inset(8.0).toolbar_in_panel().build(k)
}

fn count(n: usize, label: &str, tint: Tint, k: &Kit) -> El {
    col()
        .grow(1.0)
        .padding(10.0)
        .gap(1.0)
        .radius(k.radius.md)
        .fill(k.c.layer_02)
        .child(row().gap(6.0).center_y().child(widgets::dot(tint.color(k.dark), 6.0)).child(text(n.to_string()).font(theme::HEADING).color(k.c.text_base)))
        .child(text(label.to_owned()).font(theme::DETAILS).color(k.c.text_faint))
}

impl App {
    pub fn load_digest(&self) {
        self.run(client::digest, |s, r, _| {
            if let Ok(d) = r {
                s.digest = Some(d);
            }
        });
    }

    pub fn set_digest(&mut self, patch: serde_json::Value) {
        if let (Some(d), Some(p)) = (&mut self.digest, patch.as_object()) {
            if let Some(v) = p.get("enabled").and_then(|v| v.as_bool()) {
                d.enabled = v;
            }
            if let Some(v) = p.get("day_of_week").and_then(|v| v.as_i64()) {
                d.day_of_week = v;
            }
        }
        self.run(
            move |a| client::set_digest(a, patch),
            |s, r, tx| match r {
                Ok(d) => s.digest = Some(d),
                Err(e) => {
                    s.fail(&e, "Couldn't update the digest", tx);
                    s.load_digest();
                }
            },
        );
        let _ = ToastKind::Info;
    }
}
