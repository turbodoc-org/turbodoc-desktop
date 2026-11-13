//! Formatting for people: relative times, domains, counts.
use chrono::{DateTime, Local, Utc};

/// Parses the API's timestamps (RFC 3339, with or without a zone; Postgres sometimes drops the
/// `T` and the zone).
pub fn parse_time(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Some(t.with_timezone(&Utc));
    }
    let s = s.replace(' ', "T");
    if let Ok(t) = DateTime::parse_from_rfc3339(&s) {
        return Some(t.with_timezone(&Utc));
    }
    if let Ok(t) = DateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f%#z") {
        return Some(t.with_timezone(&Utc));
    }
    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f").ok().map(|n| n.and_utc())
}

/// "just now", "5m", "3h", "Yesterday", "Tue", "12 Mar", "12 Mar 2024".
pub fn ago(s: &str) -> String {
    let Some(t) = parse_time(s) else { return String::new() };
    ago_from(t, Utc::now())
}

pub fn ago_from(t: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - t).num_seconds();
    if secs < 45 {
        return "just now".into();
    }
    if secs < 3600 {
        return format!("{}m ago", (secs / 60).max(1));
    }
    let local = t.with_timezone(&Local);
    let today = now.with_timezone(&Local).date_naive();
    let days = (today - local.date_naive()).num_days();
    if days == 0 {
        return format!("{}h ago", secs / 3600);
    }
    if days == 1 {
        return "Yesterday".into();
    }
    if days < 7 {
        return local.format("%A").to_string();
    }
    if local.format("%Y").to_string() == now.with_timezone(&Local).format("%Y").to_string() {
        return local.format("%-d %b").to_string();
    }
    local.format("%-d %b %Y").to_string()
}

/// [`ago`] for the middle of a sentence: "updated yesterday", "updated Tuesday".
pub fn ago_inline(s: &str) -> String {
    let a = ago(s);
    if a == "Yesterday" { "yesterday".into() } else { a }
}

/// "12 March 2025 at 14:05".
pub fn long_date(s: &str) -> String {
    parse_time(s).map(|t| t.with_timezone(&Local).format("%-d %B %Y at %H:%M").to_string()).unwrap_or_default()
}

/// The host of a URL without `www.`: "github.com".
pub fn domain(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next().unwrap_or(host);
    host.strip_prefix("www.").unwrap_or(host).to_ascii_lowercase()
}

/// Whether `s` looks like a link someone meant to save.
pub fn looks_like_url(s: &str) -> bool {
    let s = s.trim();
    if s.contains(char::is_whitespace) || s.len() < 4 {
        return false;
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.len() > 10;
    }
    let host = s.split('/').next().unwrap_or(s);
    host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && host.chars().all(|c| c.is_ascii_alphanumeric() || "-.:".contains(c))
}

/// Adds `https://` to a bare host.
pub fn normalize_url(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("http://") || s.starts_with("https://") { s.to_owned() } else { format!("https://{s}") }
}

/// "1 bookmark", "12 bookmarks".
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The first line of Markdown that reads as a title, without `#` marks.
pub fn first_line(md: &str) -> String {
    md.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.trim_start_matches('#').trim().to_owned())
        .unwrap_or_default()
}

/// A plain one-line preview of Markdown: markup stripped, whitespace collapsed.
pub fn preview(md: &str, max: usize) -> String {
    let mut out = String::new();
    for line in md.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("```")) {
        let line = line.trim_start_matches(['#', '>', '-', '*', '+']).trim();
        let line = line.strip_prefix("[ ] ").or_else(|| line.strip_prefix("[x] ")).or_else(|| line.strip_prefix("[X] ")).unwrap_or(line);
        let line: String = line.chars().filter(|c| !matches!(c, '*' | '`')).collect();
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&line);
        if out.chars().count() > max {
            break;
        }
    }
    if out.chars().count() > max {
        let cut: String = out.chars().take(max).collect();
        return format!("{}…", cut.trim_end());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_drop_scheme_www_and_path() {
        assert_eq!(domain("https://www.GitHub.com/anomalyco/rune?x=1"), "github.com");
        assert_eq!(domain("http://user@host.dev:8080/a"), "host.dev");
        assert_eq!(domain("example.org"), "example.org");
    }

    #[test]
    fn urls_are_recognised() {
        assert!(looks_like_url("https://turbodoc.ai/docs"));
        assert!(looks_like_url("news.ycombinator.com"));
        assert!(!looks_like_url("buy milk"));
        assert!(!looks_like_url("hello."));
        assert_eq!(normalize_url("rust-lang.org"), "https://rust-lang.org");
    }

    #[test]
    fn times_parse_in_the_shapes_postgres_sends() {
        assert!(parse_time("2025-03-12T14:05:00.123456+00:00").is_some());
        assert!(parse_time("2025-03-12 14:05:00.123+00").is_some());
        assert!(parse_time("2025-03-12T14:05:00Z").is_some());
        let now = parse_time("2025-03-12T14:05:00Z").unwrap();
        assert_eq!(ago_from(now, now), "just now");
        assert_eq!(ago_from(now - chrono::Duration::minutes(5), now), "5m ago");
    }

    #[test]
    fn previews_strip_markdown() {
        assert_eq!(first_line("\n# Title here\nbody"), "Title here");
        assert_eq!(preview("# T\n\nSome **bold** `code` text", 100), "T Some bold code text");
    }
}
