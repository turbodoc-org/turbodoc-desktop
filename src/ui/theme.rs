//! Turbodoc's look as a rune-kit theme, following the web app: zinc neutrals, one blue for
//! action, a colour per kind of item used only on its icon, Inter for text and IBM Plex Mono
//! for code. Restraint is the rule: colour marks what something *is*, never decoration.
use std::sync::OnceLock;

use rune::{Color, TextStyle};
use rune_kit::{Kit, PaletteSeed};

fn hex(s: &str) -> Color { rune::color::hex(s).unwrap_or(Color::BLACK) }

pub fn kit(dark: bool) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    let slot = if dark { &DARK } else { &LIGHT };
    slot.get_or_init(|| build(dark))
}

fn build(dark: bool) -> Kit {
    let (bg, fg, accent) = if dark { ("#0b0b0d", "#f4f4f5", "#3b82f6") } else { ("#fafafa", "#18181b", "#2563eb") };
    let mut k = Kit::from_palette(PaletteSeed { bg: hex(bg), fg: hex(fg), accent: hex(accent) });
    k.radius.sm = 6.0;
    k.radius.md = 8.0;
    k.radius.lg = 12.0;
    k.radius.xl = 14.0;
    k.c.border_focus = hex(accent);
    if dark {
        k.c.layer_01 = hex("#131418");
        k.c.surface_raised = hex("#16171c");
        k.c.border_weak = hex("#1e2026");
        k.c.border_base = hex("#24262c");
        k.c.text_muted = hex("#a1a1aa");
        k.c.text_faint = hex("#80838e");
    } else {
        k.c.layer_01 = hex("#ffffff");
        k.c.surface_raised = hex("#ffffff");
        k.c.border_weak = hex("#ececef");
        k.c.border_base = hex("#e4e4e7");
        k.c.text_muted = hex("#52525b");
        k.c.text_faint = hex("#71717a");
    }
    k.ty.mono = TextStyle { family: rune::Family::Mono, ..TextStyle::new(12.5, 420.0).lh(19.0) };
    k
}

/// The web app's surface tokens that rune-kit's palette has no slot for.
pub struct Ink {
    /// A card's inset preview well, and its hairline.
    pub thumb: Color,
    pub thumb_line: Color,
    /// Text inside a preview well.
    pub thumb_text: Color,
    /// A hovered card's border.
    pub line_strong: Color,
    /// A hovered card's lift.
    pub lift: Color,
}

pub fn ink(dark: bool) -> Ink {
    if dark {
        Ink { thumb: hex("#17181d"), thumb_line: hex("#202228"), thumb_text: hex("#9a9ca6"), line_strong: hex("#363944"), lift: Color::BLACK.alpha(0.55) }
    } else {
        Ink { thumb: hex("#f7f7f8"), thumb_line: hex("#ececef"), thumb_text: hex("#5b5d66"), line_strong: hex("#d4d4d8"), lift: hex("#18181b").alpha(0.12) }
    }
}

/// The four kinds of item and their colours, as the web app draws them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Bookmark,
    Note,
    Code,
    Diagram,
}

impl Tint {
    pub fn color(self, dark: bool) -> Color {
        hex(match (self, dark) {
            (Tint::Note, false) => "#d97706",
            (Tint::Note, true) => "#f5a524",
            (Tint::Bookmark, false) => "#2563eb",
            (Tint::Bookmark, true) => "#4f8ff7",
            (Tint::Code, false) => "#16a34a",
            (Tint::Code, true) => "#22c55e",
            (Tint::Diagram, false) => "#7c3aed",
            (Tint::Diagram, true) => "#a78bfa",
        })
    }
    /// A soft wash of the colour for icon tiles.
    pub fn wash(self, dark: bool) -> Color { self.color(dark).alpha(if dark { 0.14 } else { 0.09 }) }
}

/// A site's hue, from its name: the placeholder of a page without a preview image.
pub fn hue_for(name: &str) -> f32 {
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    (h % 360) as f32
}

/// Page titles.
pub const H1: TextStyle = TextStyle::new(28.0, 650.0).lh(34.0).tracking(-0.55);
/// Section titles within a page.
pub const H2: TextStyle = TextStyle::new(17.0, 600.0).lh(22.0).tracking(-0.17);
/// An open document's title.
pub const TITLE: TextStyle = TextStyle::new(26.0, 650.0).lh(32.0).tracking(-0.5);
pub const HEADING: TextStyle = TextStyle::new(15.0, 600.0).lh(20.0).tracking(-0.15);
pub const CARD_TITLE: TextStyle = TextStyle::new(14.0, 600.0).lh(20.0).tracking(-0.1);
/// The one line of details under a title.
pub const DETAILS: TextStyle = TextStyle::new(12.0, 420.0).lh(16.0);
pub const LABEL: TextStyle = TextStyle::new(12.0, 520.0).lh(16.0);
