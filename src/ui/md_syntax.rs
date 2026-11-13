//! Markdown styled as you type it: headings large and bold, emphasis shown, code in mono, links
//! coloured, and the markup characters themselves faint. Spans decorate the editor's text and
//! never change it, so what's saved is exactly what's typed.
use rune::{Color, Span};
use rune_kit::Kit;

pub struct Inks {
    pub faint: Color,
    pub muted: Color,
    pub accent: Color,
    pub code: Color,
    pub code_bg: Color,
    pub heading: Color,
}

impl Inks {
    pub fn from_kit(k: &Kit) -> Inks {
        Inks {
            faint: k.c.text_faint,
            muted: k.c.text_muted,
            accent: k.c.text_accent,
            code: super::theme::Tint::Code.color(k.dark),
            code_bg: k.c.layer_02,
            heading: k.c.text_strong,
        }
    }
}

pub fn spans(src: &str, ink: &Inks) -> Vec<Span> {
    let mut out = Vec::new();
    let mut fence = false;
    let mut at = 0usize;
    for line in src.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let body = line.trim_end_matches(['\n', '\r']);
        let end = start + body.len();
        let trimmed = body.trim_start();
        let indent = body.len() - trimmed.len();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
            out.push(Span::new(start, end).mono().color(ink.faint));
            continue;
        }
        if fence {
            out.push(Span::new(start, end).mono().color(ink.code));
            continue;
        }
        if let Some(level) = heading_level(trimmed) {
            let marks = start + indent + level;
            out.push(Span::new(start, (marks + 1).min(end)).color(ink.faint));
            let size = match level {
                1 => Some(24.0),
                2 => Some(19.0),
                3 => Some(16.0),
                _ => None,
            };
            let mut s = Span::new((marks + 1).min(end), end).weight(if level <= 2 { 680.0 } else { 620.0 }).color(ink.heading);
            if let Some(sz) = size {
                s = s.size(sz);
            }
            out.push(s);
            continue;
        }
        if trimmed.starts_with('>') {
            out.push(Span::new(start + indent, start + indent + 1).color(ink.accent));
            out.push(Span::new(start + indent + 1, end).italic().color(ink.muted));
            continue;
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push(Span::new(start, end).color(ink.faint));
            continue;
        }
        let mut content = start + indent;
        if let Some(n) = list_marker(trimmed) {
            out.push(Span::new(content, content + n).color(ink.accent).weight(600.0));
            content += n;
            let rest = &src[content..end];
            for box_ in ["[ ] ", "[x] ", "[X] "] {
                if rest.starts_with(box_) {
                    out.push(Span::new(content, content + 3).mono().color(ink.accent));
                    if box_ != "[ ] " {
                        out.push(Span::new(content + 4, end).color(ink.faint));
                    }
                    content += 4;
                    break;
                }
            }
        }
        inline(src, content, end, ink, &mut out);
    }
    out
}

fn heading_level(line: &str) -> Option<usize> {
    let n = line.bytes().take_while(|b| *b == b'#').count();
    ((1..=6).contains(&n) && line[n..].starts_with(' ')).then_some(n)
}

/// The marker's length including its space: "- ", "* ", "12. ".
fn list_marker(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    if b.len() >= 2 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ' {
        return Some(2);
    }
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 4 && b.len() > digits + 1 && matches!(b[digits], b'.' | b')') && b[digits + 1] == b' ' {
        return Some(digits + 2);
    }
    None
}

/// Inline code, bold, italics and links between `from` and `to`.
fn inline(src: &str, from: usize, to: usize, ink: &Inks, out: &mut Vec<Span>) {
    let bytes = src.as_bytes();
    let mut i = from;
    while i < to {
        match bytes[i] {
            b'`' => {
                if let Some(close) = find(src, i + 1, to, "`") {
                    out.push(Span::new(i, i + 1).color(ink.faint));
                    out.push(Span { bg: Some(ink.code_bg), bg_pad: 2.0, ..Span::new(i + 1, close).mono().color(ink.code) });
                    out.push(Span::new(close, close + 1).color(ink.faint));
                    i = close + 1;
                    continue;
                }
            }
            b'*' | b'_' if i + 1 < to && bytes[i + 1] == bytes[i] => {
                let mark = if bytes[i] == b'*' { "**" } else { "__" };
                if let Some(close) = find(src, i + 2, to, mark).filter(|c| *c > i + 2) {
                    out.push(Span::new(i, i + 2).color(ink.faint));
                    out.push(Span::new(i + 2, close).bold());
                    out.push(Span::new(close, close + 2).color(ink.faint));
                    i = close + 2;
                    continue;
                }
            }
            b'*' | b'_' => {
                let left_ok = i == from || !bytes[i - 1].is_ascii_alphanumeric();
                let mark = if bytes[i] == b'*' { "*" } else { "_" };
                if left_ok
                    && let Some(close) = find(src, i + 1, to, mark).filter(|c| *c > i + 1 && !bytes[i + 1].is_ascii_whitespace())
                {
                    out.push(Span::new(i, i + 1).color(ink.faint));
                    out.push(Span::new(i + 1, close).italic());
                    out.push(Span::new(close, close + 1).color(ink.faint));
                    i = close + 1;
                    continue;
                }
            }
            b'~' if i + 1 < to && bytes[i + 1] == b'~' => {
                if let Some(close) = find(src, i + 2, to, "~~") {
                    out.push(Span::new(i, close + 2).color(ink.faint));
                    i = close + 2;
                    continue;
                }
            }
            b'[' => {
                if let Some(mid) = find(src, i + 1, to, "](")
                    && let Some(close) = find(src, mid + 2, to, ")")
                {
                    out.push(Span::new(i, i + 1).color(ink.faint));
                    out.push(Span { underline: Some(ink.accent.alpha(0.5)), ..Span::new(i + 1, mid).color(ink.accent) });
                    out.push(Span::new(mid, close + 1).color(ink.faint));
                    i = close + 1;
                    continue;
                }
            }
            b'h' if src[i..to].starts_with("http://") || src[i..to].starts_with("https://") => {
                let len = src[i..to].find(|c: char| c.is_whitespace() || c == ')' || c == '>').unwrap_or(to - i);
                out.push(Span { underline: Some(ink.accent.alpha(0.4)), ..Span::new(i, i + len).color(ink.accent) });
                i += len;
                continue;
            }
            _ => {}
        }
        i += utf8_len(bytes[i]);
    }
}

fn find(src: &str, from: usize, to: usize, pat: &str) -> Option<usize> { src.get(from..to)?.find(pat).map(|p| p + from) }

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink() -> Inks {
        let c = Color::BLACK;
        Inks { faint: c, muted: c, accent: c, code: c, code_bg: c, heading: c }
    }

    #[test]
    fn spans_stay_on_char_boundaries_and_inside_the_text() {
        let src = "# Título ✨\n\n- [x] done **bold** and `code` ~~gone~~\n> quote [link](https://x.y) é_ok_\n```rust\nfn main() {}\n```\n1. one *it* https://turbodoc.ai";
        for s in spans(src, &ink()) {
            assert!(s.start <= s.end && s.end as usize <= src.len(), "{s:?}");
            assert!(src.is_char_boundary(s.start as usize) && src.is_char_boundary(s.end as usize));
        }
    }

    #[test]
    fn headings_and_lists_are_recognised() {
        assert_eq!(heading_level("## Hi"), Some(2));
        assert_eq!(heading_level("#nope"), None);
        assert_eq!(list_marker("- a"), Some(2));
        assert_eq!(list_marker("12. a"), Some(4));
        assert_eq!(list_marker("1.5 kg"), None);
    }
}
