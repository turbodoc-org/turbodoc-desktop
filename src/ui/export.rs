//! Exporting to files through the native Save dialog: notes as Markdown, snippets as source.
use rune::{FileDialogOptions, Tx};

use crate::app::{App, ToastKind};

fn file_name(title: &str, ext: &str) -> String {
    let base: String = title.chars().map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '-' }).collect();
    let base = base.trim().trim_matches('-');
    format!("{}.{ext}", if base.is_empty() { "Untitled" } else { base })
}

fn ext_for(language: &str) -> &'static str {
    match language {
        "javascript" => "js",
        "typescript" => "ts",
        "python" => "py",
        "rust" => "rs",
        "go" => "go",
        "swift" => "swift",
        "kotlin" => "kt",
        "java" => "java",
        "csharp" => "cs",
        "ruby" => "rb",
        "php" => "php",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        "bash" => "sh",
        "json" => "json",
        "yaml" => "yaml",
        "markdown" => "md",
        _ => "txt",
    }
}

pub fn export(s: &mut App, id: &str, tx: &mut Tx) {
    let (name, contents, filter) = if let Some(n) = s.lib.notes.iter().find(|n| n.id == id) {
        let body = if n.title.trim().is_empty() || n.markdown.trim_start().starts_with('#') { n.markdown.clone() } else { format!("# {}\n\n{}", n.title.trim(), n.markdown) };
        (file_name(&n.display_title(), "md"), body, ("Markdown", "md"))
    } else if let Some(x) = s.lib.snippets.iter().find(|x| x.id == id) {
        let ext = ext_for(&x.language);
        (file_name(x.display_title(), ext), x.code.clone(), ("Source", ext))
    } else {
        return;
    };
    let options = FileDialogOptions { title: "Export".into(), file_name: Some(name), ..Default::default() }.filter(filter.0, &[filter.1]);
    tx.save_file(options, move |s: &mut App, result, tx| match result {
        Ok(Some(path)) => match std::fs::write(&path, contents.as_bytes()) {
            Ok(()) => s.toast(ToastKind::Success, format!("Exported {}", path.file_name().and_then(|f| f.to_str()).unwrap_or("file")), tx),
            Err(e) => s.toast(ToastKind::Error, format!("Couldn't write the file: {e}"), tx),
        },
        Ok(None) => {}
        Err(e) => s.toast(ToastKind::Error, format!("Couldn't export: {e}"), tx),
    });
}
