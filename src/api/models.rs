//! The API's resources, field for field (turbodoc-api `database.types.ts`). Unknown fields are
//! ignored and every optional column defaults, so a server that adds columns never breaks us.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    /// Unix seconds.
    pub time_added: i64,
    /// Pipe-separated (`work|read-later`); read leniently with [`tags`].
    pub tags: Option<String>,
    /// `unread`, `read` or `archived`.
    pub status: String,
    pub is_favorite: Option<bool>,
    pub summary: Option<String>,
    pub content_status: Option<String>,
    pub link_status: Option<String>,
    pub link_status_code: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Bookmark {
    pub fn favorite(&self) -> bool { self.is_favorite.unwrap_or(false) }
    pub fn unread(&self) -> bool { self.status.is_empty() || self.status == "unread" }
    pub fn archived(&self) -> bool { self.status == "archived" }
    pub fn broken(&self) -> bool { self.link_status.as_deref() == Some("broken") }
    pub fn tag_list(&self) -> Vec<String> { tags(self.tags.as_deref()) }
    pub fn display_title(&self) -> &str { if self.title.trim().is_empty() { &self.url } else { &self.title } }
    pub fn summarising(&self) -> bool { matches!(self.content_status.as_deref(), Some("pending" | "processing")) }
}

/// A note: the API's v2 document (`markdown` is the body).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub tags: Option<String>,
    pub is_favorite: Option<bool>,
    pub version: i64,
    pub head_revision_id: Option<String>,
    pub last_edited_by_device: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Note {
    pub fn favorite(&self) -> bool { self.is_favorite.unwrap_or(false) }
    pub fn tag_list(&self) -> Vec<String> { tags(self.tags.as_deref()) }
    pub fn display_title(&self) -> String {
        if !self.title.trim().is_empty() {
            return self.title.clone();
        }
        let first = crate::format::first_line(&self.markdown);
        if first.is_empty() { "Untitled".into() } else { first }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Revision {
    pub id: String,
    pub revision_number: Option<i64>,
    pub title: String,
    pub markdown: String,
    pub name: Option<String>,
    pub change_summary: Option<String>,
    pub device_id: Option<String>,
    /// `edit`, `merge`, `restore` or `submitted`.
    pub kind: String,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snippet {
    pub id: String,
    pub title: Option<String>,
    pub code: String,
    pub language: String,
    pub theme: Option<String>,
    pub background_type: Option<String>,
    pub background_value: Option<String>,
    pub padding: Option<i64>,
    pub show_line_numbers: Option<bool>,
    pub font_family: Option<String>,
    pub font_size: Option<i64>,
    pub window_style: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Snippet {
    pub fn display_title(&self) -> &str {
        match self.title.as_deref() {
            Some(t) if !t.trim().is_empty() => t,
            _ => "Untitled snippet",
        }
    }
}

/// The languages the web editor offers, with the names people know them by.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("javascript", "JavaScript"),
    ("typescript", "TypeScript"),
    ("python", "Python"),
    ("rust", "Rust"),
    ("go", "Go"),
    ("swift", "Swift"),
    ("kotlin", "Kotlin"),
    ("java", "Java"),
    ("csharp", "C#"),
    ("ruby", "Ruby"),
    ("php", "PHP"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("sql", "SQL"),
    ("bash", "Shell"),
    ("json", "JSON"),
    ("yaml", "YAML"),
    ("markdown", "Markdown"),
];

pub fn language_name(id: &str) -> &str {
    LANGUAGES.iter().find(|(k, _)| *k == id).map_or(id, |(_, v)| v)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shape {
    pub id: String,
    /// `rectangle`, `circle`, `diamond`, `database`, `text` or `image`.
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: String,
    pub color: Option<String>,
    #[serde(rename = "fontSize")]
    pub font_size: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub id: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diagram {
    pub id: String,
    pub title: String,
    /// `canvas` or `mermaid`.
    pub diagram_type: String,
    pub mermaid_source: Option<String>,
    #[serde(deserialize_with = "lenient_vec")]
    pub shapes: Vec<Shape>,
    #[serde(deserialize_with = "lenient_vec")]
    pub connections: Vec<Connection>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Diagram {
    pub fn is_mermaid(&self) -> bool { self.diagram_type == "mermaid" }
}

/// Reads a JSON array, skipping elements that don't parse, and `null` as empty.
fn lenient_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Array(items) => items.into_iter().filter_map(|i| serde_json::from_value(i).ok()).collect(),
        _ => Vec::new(),
    })
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub bookmark_count: i64,
    pub note_count: i64,
    pub tag_count: i64,
    pub favorite_count: i64,
    pub member_since: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigestPrefs {
    pub enabled: bool,
    pub day_of_week: i64,
    pub send_time: String,
    pub timezone: String,
}

/// Tags from the stored string: pipe-separated, as the clients write them, but commas and JSON
/// arrays are read too (older rows).
pub fn tags(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else { return Vec::new() };
    if raw.starts_with('[')
        && let Ok(list) = serde_json::from_str::<Vec<String>>(raw)
    {
        return clean(list.into_iter());
    }
    clean(raw.split(['|', ',']).map(str::to_owned))
}

fn clean(list: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in list.map(|t| t.trim().trim_start_matches('#').to_owned()).filter(|t| !t.is_empty()) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out
}

/// Tags back to the stored form; `None` when there are none.
pub fn join_tags(tags: &[String]) -> Option<String> {
    let t = clean(tags.iter().cloned());
    if t.is_empty() { None } else { Some(t.join("|")) }
}

/// Parses what someone typed in a tags field: commas, pipes or spaces between tags.
pub fn parse_tag_input(s: &str) -> Vec<String> { clean(s.split(['|', ',', ' ']).map(str::to_owned)) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_read_pipes_commas_and_json() {
        assert_eq!(tags(Some("work|read-later")), vec!["work", "read-later"]);
        assert_eq!(tags(Some("a, b ,#c")), vec!["a", "b", "c"]);
        assert_eq!(tags(Some(r#"["x","y"]"#)), vec!["x", "y"]);
        assert_eq!(tags(Some("A|a")), vec!["A"]);
        assert!(tags(None).is_empty());
        assert_eq!(join_tags(&["a".into(), "b".into()]).as_deref(), Some("a|b"));
        assert_eq!(join_tags(&[]), None);
        assert_eq!(parse_tag_input("rust, ui  web"), vec!["rust", "ui", "web"]);
    }

    #[test]
    fn rows_parse_with_missing_and_extra_fields() {
        let b: Bookmark = serde_json::from_str(r#"{"id":"1","url":"https://x.dev","title":"X","time_added":1700000000,"tags":null,"status":"unread","is_favorite":null,"extra":1}"#).unwrap();
        assert!(b.unread() && !b.favorite());
        let d: Diagram = serde_json::from_str(r#"{"id":"d","title":"T","diagram_type":"canvas","shapes":[{"id":"s","type":"circle","x":1,"y":2,"width":3,"height":4,"label":"L"},{"bad":true,"x":"no"}],"connections":null}"#).unwrap();
        assert_eq!(d.shapes.len(), 1);
        assert!(d.connections.is_empty());
    }
}
