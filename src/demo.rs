//! A signed-in library made up for headless renders (`--scene demo`) and tests, so every screen
//! can be drawn and checked without an account or a network.
use crate::api::auth::User;
use crate::api::models::{Bookmark, Connection, Diagram, Note, Shape, Snippet, Stats};
use crate::app::{App, Phase, Section};
use crate::store::cache::Library;

fn iso(days_ago: f64) -> String { (chrono::Utc::now() - chrono::Duration::seconds((days_ago * 86400.0) as i64)).to_rfc3339() }
fn unix(days_ago: f64) -> i64 { (chrono::Utc::now() - chrono::Duration::seconds((days_ago * 86400.0) as i64)).timestamp() }

fn bm(i: usize, title: &str, url: &str, tags: &str, status: &str, fav: bool, days: f64, summary: &str) -> Bookmark {
    Bookmark {
        id: format!("b{i}"),
        title: title.into(),
        url: url.into(),
        time_added: unix(days),
        tags: (!tags.is_empty()).then(|| tags.into()),
        status: status.into(),
        is_favorite: Some(fav),
        summary: (!summary.is_empty()).then(|| summary.into()),
        content_status: Some("succeeded".into()),
        link_status: Some(if i == 7 { "broken" } else { "ok" }.into()),
        created_at: Some(iso(days)),
        updated_at: Some(iso(days)),
        ..Default::default()
    }
}

pub fn library() -> Library {
    let bookmarks = vec![
        bm(1, "Rune: a GPU-rendered Rust UI framework", "https://github.com/anomalyco/rune", "rust|ui", "unread", true, 0.02, "Rune is a **native GPU-rendered** Rust UI framework for macOS, Windows, Linux and the web, with its own renderer, text, layout and springs.\n\n- Headless rendering for agents\n- 120 Hz motion on analytic springs"),
        bm(2, "Designing calm interfaces", "https://www.calmtech.com/", "design", "unread", false, 0.4, "Principles for technology that informs without demanding attention."),
        bm(3, "The Rust Programming Language", "https://doc.rust-lang.org/book/", "rust|learning", "read", true, 1.2, "The official book: ownership, borrowing, traits and fearless concurrency."),
        bm(4, "Postgres row-level security in practice", "https://supabase.com/docs/guides/auth/row-level-security", "postgres|security", "unread", false, 2.5, ""),
        bm(5, "Three-way merges, explained", "https://blog.jcoglan.com/2017/05/08/merging-with-diff3/", "algorithms", "read", false, 4.0, "How diff3 finds stable chunks between a base and two edits, and where conflicts come from."),
        bm(6, "Cloudflare Workers: Durable Objects", "https://developers.cloudflare.com/durable-objects/", "cloudflare|backend", "unread", false, 6.0, ""),
        bm(7, "An old link that moved", "https://example.com/gone", "", "unread", false, 9.0, ""),
        bm(8, "Oklch colour picker", "https://oklch.com", "design|color", "archived", false, 20.0, "Pick colours in a perceptually uniform space."),
        bm(9, "Spring animations, the physics", "https://www.joshwcomeau.com/animation/a-friendly-introduction-to-spring-physics/", "design|motion", "unread", true, 0.9, ""),
    ];
    let note = |i: usize, title: &str, md: &str, tags: &str, fav: bool, days: f64| Note {
        id: format!("n{i}"),
        title: title.into(),
        markdown: md.into(),
        tags: (!tags.is_empty()).then(|| tags.into()),
        is_favorite: Some(fav),
        version: 3,
        head_revision_id: Some(format!("r{i}")),
        created_at: Some(iso(days + 3.0)),
        updated_at: Some(iso(days)),
        ..Default::default()
    };
    let notes = vec![
        note(1, "Desktop app launch plan", "## Goals\n\nShip a **native** Turbodoc for the Mac that feels instant.\n\n- [x] Sign in with the same account\n- [x] Offline copy of the library\n- [ ] Quick capture from any app\n\n> Fast is a feature.\n\n```rust\nfn main() {\n    rune::run(spec);\n}\n```\n\nSee [the docs](https://turbodoc.ai/docs) for the API.", "launch|desktop", true, 0.01),
        note(2, "Reading notes: calm technology", "Technology should require the smallest possible amount of attention.\n\n1. Inform without overburdening\n2. Work even when it fails", "design", false, 0.6),
        note(3, "Recipe: weeknight ramen", "# Weeknight ramen\n\n- 2 nests noodles\n- 1 tbsp miso\n- soft egg, scallions", "", false, 3.0),
        note(4, "Meeting with the API team", "Notes v2 merges with *diff3*. Send `base_revision_id` with every save.", "api|work", false, 5.0),
    ];
    let snippets = vec![
        Snippet { id: "s1".into(), title: Some("Debounce a function".into()), language: "typescript".into(), code: "export function debounce<T extends (...args: any[]) => void>(fn: T, ms = 300) {\n  let timer: ReturnType<typeof setTimeout> | undefined;\n  return (...args: Parameters<T>) => {\n    clearTimeout(timer);\n    timer = setTimeout(() => fn(...args), ms);\n  };\n}\n".into(), background_value: Some("linear-gradient(135deg, #667eea 0%, #764ba2 100%)".into()), updated_at: Some(iso(0.3)), created_at: Some(iso(2.0)), ..Default::default() },
        Snippet { id: "s2".into(), title: Some("Retry with backoff".into()), language: "rust".into(), code: "pub fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>, tries: u32) -> Result<T, E> {\n    let mut delay = std::time::Duration::from_millis(100);\n    for _ in 1..tries {\n        match f() {\n            Ok(v) => return Ok(v),\n            Err(_) => std::thread::sleep(delay),\n        }\n        delay *= 2;\n    }\n    f()\n}\n".into(), background_value: Some("linear-gradient(135deg, #f093fb 0%, #f5576c 100%)".into()), updated_at: Some(iso(1.5)), created_at: Some(iso(4.0)), ..Default::default() },
        Snippet { id: "s3".into(), title: Some("Find large files".into()), language: "bash".into(), code: "find . -type f -size +50M -exec ls -lh {} \\; | sort -k5 -h\n".into(), updated_at: Some(iso(6.0)), ..Default::default() },
    ];
    let shape = |id: &str, kind: &str, x: f32, y: f32, label: &str, color: &str| Shape { id: id.into(), kind: kind.into(), x, y, width: 150.0, height: 70.0, label: label.into(), color: Some(color.into()), font_size: Some(14.0) };
    let diagrams = vec![
        Diagram { id: "d1".into(), title: "Sync flow".into(), diagram_type: "mermaid".into(), mermaid_source: Some("flowchart LR\n  Edit[Edit in app] --> Save{Saved?}\n  Save -->|yes| Revision[New revision]\n  Save -->|offline| Queue[Keep the draft]\n  Queue --> Save\n  Revision --> Merge[Merge on server]\n".into()), updated_at: Some(iso(0.2)), ..Default::default() },
        Diagram {
            id: "d2".into(),
            title: "System overview".into(),
            diagram_type: "canvas".into(),
            shapes: vec![shape("a", "rectangle", 0.0, 0.0, "Desktop app", "#3b82f6"), shape("b", "diamond", 260.0, 0.0, "API", "#a78bfa"), shape("c", "database", 520.0, 0.0, "Postgres", "#22c55e"), shape("d", "circle", 260.0, 160.0, "Auth", "#f5a524")],
            connections: vec![Connection { id: "1".into(), from: "a".into(), to: "b".into() }, Connection { id: "2".into(), from: "b".into(), to: "c".into() }, Connection { id: "3".into(), from: "b".into(), to: "d".into() }],
            updated_at: Some(iso(2.0)),
            ..Default::default()
        },
        Diagram { id: "d3".into(), title: "Sign-in sequence".into(), diagram_type: "mermaid".into(), mermaid_source: Some("sequenceDiagram\n  App->>Supabase: email + password\n  Supabase-->>App: session\n  App->>API: Bearer token\n  API-->>App: library\n".into()), updated_at: Some(iso(8.0)), ..Default::default() },
    ];
    Library { bookmarks, notes, snippets, diagrams, stats: Some(Stats { bookmark_count: 9, note_count: 4, tag_count: 12, favorite_count: 4, member_since: Some(iso(400.0)) }), og: Vec::new(), synced_at: unix(0.01) }
}

pub fn sign_in(s: &mut App) {
    s.phase = Phase::Ready(User { id: "demo".into(), email: Some("ada@turbodoc.ai".into()), user_metadata: serde_json::json!({ "first_name": "Ada" }), ..Default::default() });
    s.lib = library();
    s.loaded = true;
    s.section = Section::Home;
}

pub const SCENES: &[(&str, fn(&mut App))] = &[
    ("signin", |s| s.phase = Phase::SignedOut(crate::app::SignIn::new())),
    ("demo", sign_in),
    ("bookmarks", |s| {
        sign_in(s);
        s.section = Section::Bookmarks;
        s.selected = Some("b1".into());
        if !s.inspector.is_open() {
            s.inspector.toggle();
        }
    }),
    ("bookmarks-list", |s| {
        sign_in(s);
        s.section = Section::Bookmarks;
        s.layout = crate::app::Layout::List;
    }),
    ("notes", |s| {
        sign_in(s);
        s.section = Section::Notes;
        let n = s.lib.notes[0].clone();
        s.note = Some(crate::app::NoteEditor::open(&n));
        s.selected = Some(n.id);
    }),
    ("notes-split", |s| {
        sign_in(s);
        s.section = Section::Notes;
        let n = s.lib.notes[0].clone();
        s.note = Some(crate::app::NoteEditor::open(&n));
        let _ = s.prefs.set("note_mode", &1usize);
    }),
    ("snippets", |s| {
        sign_in(s);
        s.section = Section::Snippets;
        s.open_snippet("s1");
    }),
    ("diagrams", |s| {
        sign_in(s);
        s.section = Section::Diagrams;
        s.open_diagram("d1");
    }),
    ("canvas", |s| {
        sign_in(s);
        s.section = Section::Diagrams;
        s.open_diagram("d2");
    }),
    ("palette", |s| {
        sign_in(s);
        s.open_palette();
        if let Some(p) = &mut s.palette {
            p.query.replace_text("ru");
        }
    }),
    ("settings", |s| {
        sign_in(s);
        s.settings = true;
        s.digest = Some(crate::api::models::DigestPrefs { enabled: true, day_of_week: 5, send_time: "10:00:00".into(), timezone: "Europe/London".into() });
    }),
    ("add-bookmark", |s| {
        sign_in(s);
        s.section = Section::Bookmarks;
        s.new_bookmark_dialog(Some("https://turbodoc.ai/docs".into()));
    }),
    ("empty", |s| {
        sign_in(s);
        s.lib = Library { synced_at: 1, ..Default::default() };
        s.section = Section::Bookmarks;
    }),
    ("loading", |s| {
        sign_in(s);
        s.lib = Library::default();
        s.loaded = false;
    }),
    ("light", |s| {
        sign_in(s);
        s.appearance = crate::app::Appearance::Light;
        s.section = Section::Bookmarks;
        s.selected = Some("b3".into());
        if !s.inspector.is_open() {
            s.inspector.toggle();
        }
    }),
];
