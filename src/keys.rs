//! Shortcuts written once as chords ("mod+n") and drawn the way each platform writes them:
//! "⌘N" on macOS, "Ctrl+N" on Windows and Linux. `mod` is ⌘ on macOS and Ctrl elsewhere.

/// The global quick-capture chord: ⌃⌥Space on macOS, Ctrl+Alt+Space elsewhere.
pub const CAPTURE: &str = "ctrl+alt+space";

/// How `spec` is written on this platform. Text that isn't a chord ("↑↓") is drawn as given.
pub fn label(spec: &str) -> String { label_on(spec, rune::KeyPlatform::current()) }

pub fn label_on(spec: &str, platform: rune::KeyPlatform) -> String {
    match rune::Chord::parse(spec) {
        // "⎋" is AppKit's glyph for Escape, but few people read it; the key says "esc".
        Ok(c) if c.key() == rune::ChordKey::Escape && c.mods_on(platform).is_empty() => "Esc".into(),
        Ok(c) => c.label_on(platform),
        Err(_) => spec.to_owned(),
    }
}

/// A menu's key equivalent.
pub fn shortcut(spec: &str) -> rune::Shortcut { spec.parse().unwrap_or_else(|_| panic!("valid shortcut {spec}")) }

#[cfg(test)]
mod tests {
    use super::*;
    use rune::KeyPlatform::{Linux, Mac, Windows};

    #[test]
    fn shortcuts_read_the_way_each_platform_writes_them() {
        assert_eq!(label_on("mod+n", Mac), "⌘N");
        assert_eq!(label_on("mod+n", Windows), "Ctrl+N");
        assert_eq!(label_on("mod+shift+n", Linux), "Ctrl+Shift+N");
        assert_eq!(label_on(CAPTURE, Mac), "⌃⌥Space");
        assert_eq!(label_on(CAPTURE, Windows), "Ctrl+Alt+Space");
        assert_eq!(label_on("esc", Mac), "Esc");
        assert_eq!(label_on("↑↓", Windows), "↑↓");
    }

    #[test]
    fn menu_shortcuts_use_ctrl_off_macos() {
        let here = shortcut("mod+n").to_string();
        assert_eq!(here, if cfg!(target_os = "macos") { "super+n" } else { "ctrl+n" });
    }
}
