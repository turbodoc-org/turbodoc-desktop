//! Return and Tab in the multi-line editors, the way a native text editor handles them.
//!
//! Rune's multi-line field treats a plain Return in a paragraph as "submit" (a chat box's
//! habit), so without this a note, a snippet or a diagram's source takes no new line at all.
//! Here Return breaks the line and carries its indentation; in Markdown it also continues a
//! list or quote, and on an empty item ends the list (or outdents a nested one). Tab and
//! Shift+Tab indent and outdent the line, or every line a selection touches.
//!
//! A line break is sent through the field as typed text, so it autosaves, undoes and scrolls
//! the caret into view like any other typing. Indenting edits the field directly (the caret
//! stays on its line) and reports `Edited` so the caller schedules a save.
use rune::{Key, KeyEvent, TextEdit, Tx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doc {
    /// A note: lists and quotes continue on Return.
    Markdown,
    /// Code or diagram source; `unit` is the indent a Tab adds when the text has none to copy.
    Code { unit: &'static str },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Not ours: the field handles the key as usual.
    Pass,
    /// Handled: any change was typed through the field, which reports it itself.
    Handled,
    /// Handled by editing the field directly: the caller schedules a save.
    Edited,
}

/// What a key does to the text: type `Type` over the selection, or replace a range and then
/// select `select`.
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    Type(String),
    Replace { from: usize, to: usize, with: String, select: (usize, usize) },
    /// Nothing to change (Shift+Tab with no indent), but the key is still taken.
    Keep,
}

pub fn key(edit: &mut TextEdit, ev: &KeyEvent, tx: &mut Tx, doc: Doc) -> Outcome {
    let m = ev.mods;
    if m.cmd || m.ctrl || m.alt || m.logo || edit.preedit().is_some() {
        return Outcome::Pass;
    }
    let range = edit.range();
    let plan = match ev.key {
        // Shift+Return stays a plain line break, as in most editors.
        Key::Enter if !m.shift => Some(on_return(&edit.text, range, doc)),
        Key::Tab => on_tab(&edit.text, range, doc, m.shift),
        _ => None,
    };
    match plan {
        None => Outcome::Pass,
        Some(Plan::Type(s)) => {
            tx.send_key(KeyEvent { key: Key::Text(s), mods: Default::default() });
            Outcome::Handled
        }
        Some(Plan::Keep) => Outcome::Handled,
        Some(Plan::Replace { from, to, with, select }) => {
            edit.set_caret(from);
            edit.focus = to;
            edit.insert(&with);
            edit.set_caret(select.0);
            edit.focus = select.1;
            Outcome::Edited
        }
    }
}

fn on_return(text: &str, (a, b): (usize, usize), doc: Doc) -> Plan {
    let ls = line_start(text, a);
    let le = line_end(text, a);
    let line = &text[ls..le];
    let col = a - ls;
    let indent = leading_ws(line);
    // Inside the indentation, carry only what's left of the caret.
    let carry = &line[..indent.min(col)];
    let plain = || Plan::Type(format!("\n{carry}"));
    match doc {
        Doc::Code { unit } => {
            let opens = text[ls..a].trim_end().ends_with(['{', '[', '(']) && text[b..line_end(text, b)].trim().is_empty();
            Plan::Type(format!("\n{carry}{}", if opens { unit_for(text, unit) } else { "" }))
        }
        Doc::Markdown => {
            if in_fence(text, ls) {
                return plain();
            }
            let Some(p) = prefix(line) else { return plain() };
            if col < p.len {
                return plain();
            }
            let empty = line[p.len..].trim().is_empty() && b <= le;
            if !empty {
                return Plan::Type(format!("\n{}", p.next));
            }
            // An empty item: a nested one moves out a level, a top-level one ends the list.
            if p.list && indent > 0 && !p.quoted {
                let remove = outdent_width(text, ls, indent);
                return Plan::Replace { from: ls, to: ls + remove, with: String::new(), select: (a - remove, a - remove) };
            }
            let keep = if p.list { &line[..p.list_at] } else { &line[..indent] };
            let keep = keep.trim_end_matches(' ').to_owned();
            let at = ls + keep.len();
            Plan::Replace { from: ls, to: le, with: keep, select: (at, at) }
        }
    }
}

fn on_tab(text: &str, (a, b): (usize, usize), doc: Doc, shift: bool) -> Option<Plan> {
    let first = line_start(text, a);
    // A selection that ends at the start of a line doesn't take that line.
    let b_end = if b > a && b == line_start(text, b) { b - 1 } else { b };
    let last = line_end(text, b_end);
    let block = &text[first..last];
    let multi = block.contains('\n');
    let list = |l: &str, at: usize| doc == Doc::Markdown && prefix(l).is_some_and(|p| p.list) && !in_fence(text, at);
    if !shift && !multi && !list(block, first) {
        // A caret in plain text: Tab types an indent, as it does in any editor.
        let unit = match doc {
            Doc::Code { unit } => unit_for(text, unit),
            Doc::Markdown => "    ",
        };
        return Some(Plan::Type(unit.to_owned()));
    }
    struct Line {
        old: usize,
        new: usize,
        removed: usize,
        added: usize,
    }
    let mut out = String::with_capacity(block.len() + 16);
    let mut lines = Vec::new();
    let (mut old, mut new) = (first, first);
    for (i, l) in block.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let (removed, add) = if shift {
            let w = if list(l, old) { outdent_width(text, old, leading_ws(l)) } else { outdent_one(l, unit_width(text, doc)) };
            (w, String::new())
        } else if l.trim().is_empty() && multi {
            (0, String::new())
        } else if list(l, old) {
            (0, " ".repeat(list_unit(text, old, leading_ws(l))))
        } else {
            (0, match doc {
                Doc::Markdown => "    ".to_owned(),
                Doc::Code { unit } => unit_for(text, unit).to_owned(),
            })
        };
        out.push_str(&add);
        out.push_str(&l[removed..]);
        lines.push(Line { old, new, removed, added: add.len() });
        old += l.len() + 1;
        new += l.len() - removed + add.len() + 1;
    }
    if out == block {
        return Some(Plan::Keep);
    }
    // Where a selection end lands: it moves with its line's text, except that a selection
    // starting at a line's very start keeps the new indent inside it.
    let map = |p: usize, start: bool| -> usize {
        if p > last {
            return (p as isize + out.len() as isize - block.len() as isize) as usize;
        }
        let l = lines.iter().rev().find(|l| l.old <= p).expect("p is in the block");
        let off = p - l.old;
        if l.added > 0 {
            if start && a < b && off == 0 { l.new } else { l.new + off + l.added }
        } else {
            l.new + off.saturating_sub(l.removed)
        }
    };
    let select = (map(a, true), map(b, false));
    Some(Plan::Replace { from: first, to: last, with: out, select })
}

/// A Markdown line's leading markup: indentation, quote marks and a list marker.
struct Prefix {
    /// Bytes of markup before the content.
    len: usize,
    /// Where the list marker starts, when there is one.
    list_at: usize,
    /// The list marker's width ("- " is 2, "12. " is 4), not counting a task box.
    marker: usize,
    list: bool,
    quoted: bool,
    /// The markup that starts the next line.
    next: String,
}

fn prefix(line: &str) -> Option<Prefix> {
    let indent = leading_ws(line);
    let mut i = indent;
    let b = line.as_bytes();
    let mut quoted = false;
    while i < b.len() && b[i] == b'>' {
        quoted = true;
        i += 1;
        if i < b.len() && b[i] == b' ' {
            i += 1;
        }
    }
    let list_at = i;
    let rest = &line[i..];
    let marker = bullet(rest).map(|n| (n, rest[..n].to_owned())).or_else(|| {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let ok = (1..10).contains(&digits) && matches!(rest.as_bytes().get(digits), Some(b'.' | b')')) && rest.as_bytes().get(digits + 1) == Some(&b' ');
        ok.then(|| {
            let n: u64 = rest[..digits].parse().unwrap_or(0);
            (digits + 2, format!("{}{} ", n + 1, &rest[digits..digits + 1]))
        })
    });
    let Some((n, mut next_marker)) = marker else {
        return quoted.then(|| Prefix { len: i, list_at, marker: 0, list: false, quoted, next: line[..i].to_owned() });
    };
    let mut len = i + n;
    let after = &line[len..];
    if ["[ ] ", "[x] ", "[X] "].iter().any(|t| after.starts_with(t)) {
        len += 4;
        next_marker.push_str("[ ] ");
    } else if ["[ ]", "[x]", "[X]"].contains(&after) {
        len += 3;
        next_marker.push_str("[ ] ");
    }
    Some(Prefix { len, list_at, marker: n, list: true, quoted, next: format!("{}{next_marker}", &line[..i]) })
}

/// "- ", "* " or "+ " (but not a "* * *" rule).
fn bullet(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() < 2 || !matches!(b[0], b'-' | b'*' | b'+') || b[1] != b' ' {
        return None;
    }
    let mark = b[0] as char;
    let rule = s.trim_end().chars().all(|c| c == mark || c == ' ') && s.matches(mark).count() >= 3;
    (!rule).then_some(2)
}

/// How far Tab indents the list item on the line at `ls`: under the content of the item above.
fn list_unit(text: &str, ls: usize, indent: usize) -> usize {
    for l in text[..ls].lines().rev() {
        if l.trim().is_empty() {
            continue;
        }
        let li = leading_ws(l);
        match prefix(l) {
            Some(p) if p.list && li == indent => return p.list_at - li + p.marker,
            Some(p) if p.list && li < indent => break,
            _ if li < indent => break,
            _ => {}
        }
    }
    2
}

/// How far Shift+Tab moves the list item on the line at `ls`: back to its parent's level.
fn outdent_width(text: &str, ls: usize, indent: usize) -> usize {
    for l in text[..ls].lines().rev() {
        if l.trim().is_empty() {
            continue;
        }
        let li = leading_ws(l);
        if li < indent && prefix(l).is_some_and(|p| p.list) {
            return indent - li;
        }
        if li == 0 {
            break;
        }
    }
    indent
}

/// Leading whitespace worth one indent level: a tab, or up to `width` spaces.
fn outdent_one(line: &str, width: usize) -> usize {
    if line.starts_with('\t') {
        return 1;
    }
    line.bytes().take(width).take_while(|c| *c == b' ').count()
}

fn unit_width(text: &str, doc: Doc) -> usize {
    match doc {
        Doc::Markdown => 4,
        Doc::Code { unit } => unit_for(text, unit).len(),
    }
}

/// A tab if the text is indented with tabs, else `spaces`.
fn unit_for(text: &str, spaces: &'static str) -> &'static str {
    let tabs = text.lines().filter(|l| l.starts_with('\t')).count();
    let spaced = text.lines().filter(|l| l.starts_with("  ")).count();
    if tabs > spaced { "\t" } else { spaces }
}

/// Whether the line starting at `ls` is inside a fenced code block (not its opening fence).
fn in_fence(text: &str, ls: usize) -> bool {
    let mut open = false;
    for l in text[..ls].lines() {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            open = !open;
        }
    }
    open
}

fn leading_ws(line: &str) -> usize { line.len() - line.trim_start_matches([' ', '\t']).len() }

fn line_start(text: &str, at: usize) -> usize { text[..at].rfind('\n').map_or(0, |i| i + 1) }

fn line_end(text: &str, at: usize) -> usize { text[at..].find('\n').map_or(text.len(), |i| at + i) }

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: Doc = Doc::Code { unit: "    " };

    /// Applies `key` to `src`, where `|` marks the caret (or two of them a selection), and
    /// returns the result with the caret marked the same way.
    fn press(src: &str, doc: Doc, enter: bool, shift: bool) -> String {
        let marks: Vec<usize> = src.match_indices('|').map(|(i, _)| i).collect();
        let text = src.replace('|', "");
        let (a, b) = match marks.as_slice() {
            [a] => (*a, *a),
            [a, b] => (*a, *b - 1),
            _ => panic!("mark the caret"),
        };
        let plan = if enter { Some(on_return(&text, (a, b), doc)) } else { on_tab(&text, (a, b), doc, shift) };
        let (out, sel) = match plan.expect("handled") {
            Plan::Type(s) => {
                let mut t = text.clone();
                t.replace_range(a..b, &s);
                (t, (a + s.len(), a + s.len()))
            }
            Plan::Replace { from, to, with, select } => {
                let mut t = text.clone();
                t.replace_range(from..to, &with);
                (t, select)
            }
            Plan::Keep => (text.clone(), (a, b)),
        };
        let mut shown = out.clone();
        if sel.0 == sel.1 {
            shown.insert(sel.0, '|');
        } else {
            shown.insert(sel.1, '|');
            shown.insert(sel.0, '|');
        }
        shown
    }

    fn ret(src: &str, doc: Doc) -> String { press(src, doc, true, false) }
    fn tab(src: &str, doc: Doc) -> String { press(src, doc, false, false) }
    fn untab(src: &str, doc: Doc) -> String { press(src, doc, false, true) }

    #[test]
    fn return_breaks_a_plain_line() {
        assert_eq!(ret("hello|", Doc::Markdown), "hello\n|");
        assert_eq!(ret("hel|lo", Doc::Markdown), "hel\n|lo");
        assert_eq!(ret("|", Doc::Markdown), "\n|");
        assert_eq!(ret("a|bc|d", Doc::Markdown), "a\n|d", "a selection is replaced");
        assert_eq!(ret("Título ✨|", Doc::Markdown), "Título ✨\n|");
    }

    #[test]
    fn return_continues_lists_and_quotes() {
        assert_eq!(ret("- one|", Doc::Markdown), "- one\n- |");
        assert_eq!(ret("* one|", Doc::Markdown), "* one\n* |");
        assert_eq!(ret("  + nested|", Doc::Markdown), "  + nested\n  + |");
        assert_eq!(ret("9. nine|", Doc::Markdown), "9. nine\n10. |");
        assert_eq!(ret("1) one|", Doc::Markdown), "1) one\n2) |");
        assert_eq!(ret("- [x] done|", Doc::Markdown), "- [x] done\n- [ ] |");
        assert_eq!(ret("- [ ] todo|", Doc::Markdown), "- [ ] todo\n- [ ] |");
        assert_eq!(ret("> quoted|", Doc::Markdown), "> quoted\n> |");
        assert_eq!(ret("> - in a quote|", Doc::Markdown), "> - in a quote\n> - |");
        assert_eq!(ret("- sp|lit", Doc::Markdown), "- sp\n- |lit");
    }

    #[test]
    fn return_before_the_marker_is_a_plain_break() {
        assert_eq!(ret("|- one", Doc::Markdown), "\n|- one");
        assert_eq!(ret("-| one", Doc::Markdown), "-\n| one");
    }

    #[test]
    fn return_on_an_empty_item_ends_the_list_or_moves_out() {
        assert_eq!(ret("- one\n- |", Doc::Markdown), "- one\n|");
        assert_eq!(ret("1. one\n2. |", Doc::Markdown), "1. one\n|");
        assert_eq!(ret("- [ ] a\n- [ ] |", Doc::Markdown), "- [ ] a\n|");
        assert_eq!(ret("> a\n> |", Doc::Markdown), "> a\n|");
        assert_eq!(ret("> - a\n> - |", Doc::Markdown), "> - a\n>|");
        assert_eq!(ret("- a\n  - b\n  - |", Doc::Markdown), "- a\n  - b\n- |");
        assert_eq!(ret("1. a\n   - b\n   - |", Doc::Markdown), "1. a\n   - b\n- |");
    }

    #[test]
    fn return_keeps_indentation() {
        assert_eq!(ret("    indented|", Doc::Markdown), "    indented\n    |");
        assert_eq!(ret("  |  half", Doc::Markdown), "  \n  |  half");
        assert_eq!(ret("fn main() {\n    let x = 1;|\n}", CODE), "fn main() {\n    let x = 1;\n    |\n}");
        assert_eq!(ret("\tdeep|", CODE), "\tdeep\n\t|");
    }

    #[test]
    fn return_after_an_opening_bracket_indents_in_code() {
        assert_eq!(ret("fn main() {|", CODE), "fn main() {\n    |");
        assert_eq!(ret("  items = [|", Doc::Code { unit: "  " }), "  items = [\n    |");
        assert_eq!(ret("call(|x)", CODE), "call(\n|x)", "not when the line goes on");
    }

    #[test]
    fn code_fences_in_notes_take_plain_breaks() {
        assert_eq!(ret("```\n- not a list|", Doc::Markdown), "```\n- not a list\n|");
        assert_eq!(ret("```\ncode\n```\n- a list|", Doc::Markdown), "```\ncode\n```\n- a list\n- |");
    }

    #[test]
    fn rules_are_not_bullets() {
        assert_eq!(ret("* * *|", Doc::Markdown), "* * *\n|");
        assert_eq!(ret("- - -|", Doc::Markdown), "- - -\n|");
    }

    #[test]
    fn tab_types_an_indent_in_plain_text() {
        assert_eq!(tab("a|b", CODE), "a    |b");
        assert_eq!(tab("\tx\n|", CODE), "\tx\n\t|");
        assert_eq!(tab("word|", Doc::Markdown), "word    |");
    }

    #[test]
    fn tab_nests_a_list_item_under_the_one_above() {
        assert_eq!(tab("- a\n- b|", Doc::Markdown), "- a\n  - b|");
        assert_eq!(tab("1. a\n2. b|", Doc::Markdown), "1. a\n   2. b|");
        assert_eq!(tab("- [ ] a\n- [ ] |", Doc::Markdown), "- [ ] a\n  - [ ] |");
        assert_eq!(untab("- a\n  - b|", Doc::Markdown), "- a\n- b|");
        assert_eq!(untab("1. a\n   - b|", Doc::Markdown), "1. a\n- b|");
        assert_eq!(untab("- a|", Doc::Markdown), "- a|", "nothing to outdent");
    }

    #[test]
    fn tab_indents_every_selected_line() {
        assert_eq!(tab("|a\nb\nc|", CODE), "|    a\n    b\n    c|");
        assert_eq!(tab("x\n|a\nb\n|c", CODE), "x\n|    a\n    b\n|c", "a selection ending at a line start leaves that line");
        assert_eq!(tab("a|\n\nb|", CODE), "    a|\n\n    b|", "blank lines stay blank");
        assert_eq!(untab("    a|\n  b\nc|", CODE), "a|\nb\nc|");
        assert_eq!(untab("\ta\n\t|b", CODE), "\ta\n|b");
    }
}
