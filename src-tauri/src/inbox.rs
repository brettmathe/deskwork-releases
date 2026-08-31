use std::fs;

use crate::model::{ExtraEntry, InboxItem, InboxItemFull};
use crate::repo::{atomic_write, confine, dedupe_path, require_root, RepoState};

// ---------- parsing ----------

fn parse_title(content: &str, filename: &str) -> String {
    for line in content.lines() {
        if let Some(t) = line.strip_prefix("# ") {
            let t = t.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    // Fallback: de-slug the filename (strip date prefix and extension).
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    let stem = strip_date_prefix(stem).unwrap_or(stem);
    stem.replace('-', " ").trim().to_string()
}

fn strip_date_prefix(stem: &str) -> Option<&str> {
    let b = stem.as_bytes();
    if b.len() > 11
        && b[..10]
            .iter()
            .enumerate()
            .all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
        && b[10] == b'-'
    {
        Some(&stem[11..])
    } else {
        None
    }
}

fn parse_added(content: &str, filename: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(d) = line.strip_prefix("Added:") {
            let d = d.trim();
            if !d.is_empty() {
                return Some(d.to_string());
            }
        }
    }
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    if strip_date_prefix(stem).is_some() {
        return Some(stem[..10].to_string());
    }
    None
}

fn parse_preview(content: &str) -> String {
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty()
            || t.starts_with('#')
            || t.starts_with("Added:")
            || t.starts_with("**Source:**")
            || t.starts_with("**Channel/Conversation:**")
        {
            continue;
        }
        let t = t
            .strip_prefix("**Original message:**")
            .or_else(|| t.strip_prefix("**Context:**"))
            .unwrap_or(t)
            .trim();
        if t.is_empty() {
            continue;
        }
        let mut p: String = t.chars().take(140).collect();
        if t.chars().count() > 140 {
            p.push('…');
        }
        return p;
    }
    String::new()
}

fn parse_item(filename: &str, content: &str) -> InboxItem {
    InboxItem {
        filename: filename.to_string(),
        title: parse_title(content, filename),
        added: parse_added(content, filename),
        is_slack: content.contains("**Source:**"),
        preview: parse_preview(content),
    }
}

/// Matches the slug convention used by the Slack task scanner, plus trailing-dash trim.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = true; // suppress leading dash
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.truncate(50);
    slug.trim_matches('-').to_string()
}

// ---------- commands ----------

#[tauri::command]
pub fn list_inbox(state: tauri::State<RepoState>) -> Result<Vec<InboxItem>, String> {
    list_inbox_at(&require_root(&state)?)
}

pub fn list_inbox_at(root: &std::path::Path) -> Result<Vec<InboxItem>, String> {
    let mut items = Vec::new();
    for entry in fs::read_dir(root.join("inbox")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !name.ends_with(".md") || !entry.path().is_file() {
            continue;
        }
        let content = fs::read_to_string(entry.path()).unwrap_or_default();
        items.push(parse_item(&name, &content));
    }
    // Newest first; undated last, then by title.
    items.sort_by(|a, b| match (&a.added, &b.added) {
        (Some(x), Some(y)) => y.cmp(x).then_with(|| a.title.cmp(&b.title)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });
    Ok(items)
}

#[tauri::command]
pub fn list_inbox_extras(state: tauri::State<RepoState>) -> Result<Vec<ExtraEntry>, String> {
    list_inbox_extras_at(&require_root(&state)?)
}

pub fn list_inbox_extras_at(root: &std::path::Path) -> Result<Vec<ExtraEntry>, String> {
    let mut extras = Vec::new();
    for entry in fs::read_dir(root.join("inbox")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.path().is_dir();
        if is_dir || !name.ends_with(".md") {
            extras.push(ExtraEntry { name, is_dir });
        }
    }
    extras.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(extras)
}

#[tauri::command]
pub fn get_inbox_item(
    state: tauri::State<RepoState>,
    filename: String,
) -> Result<InboxItemFull, String> {
    let root = require_root(&state)?;
    let path = confine(&root, "inbox", &filename)?;
    let content = fs::read_to_string(&path).map_err(|e| format!("{filename}: {e}"))?;
    let item = parse_item(&filename, &content);
    Ok(InboxItemFull {
        filename: item.filename,
        title: item.title,
        added: item.added,
        is_slack: item.is_slack,
        content,
    })
}

#[tauri::command]
pub fn create_inbox_item(
    state: tauri::State<RepoState>,
    title: String,
    body: String,
) -> Result<InboxItem, String> {
    create_inbox_item_at(&require_root(&state)?, &title, &body)
}

pub fn create_inbox_item_at(
    root: &std::path::Path,
    title: &str,
    body: &str,
) -> Result<InboxItem, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("title is required".into());
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let slug = slugify(title);
    if slug.is_empty() {
        return Err("title must contain letters or digits".into());
    }
    let path = dedupe_path(confine(root, "inbox", &format!("{date}-{slug}.md"))?);
    let body = body.trim();
    let content = if body.is_empty() {
        format!("# {title}\n\nAdded: {date}\n")
    } else {
        format!("# {title}\n\nAdded: {date}\n\n{body}\n")
    };
    atomic_write(&path, &content)?;
    let filename = path.file_name().unwrap().to_string_lossy().to_string();
    Ok(parse_item(&filename, &content))
}

#[tauri::command]
pub fn save_inbox_item(
    state: tauri::State<RepoState>,
    filename: String,
    content: String,
) -> Result<InboxItem, String> {
    save_inbox_item_at(&require_root(&state)?, &filename, &content)
}

pub fn save_inbox_item_at(
    root: &std::path::Path,
    filename: &str,
    content: &str,
) -> Result<InboxItem, String> {
    let path = confine(root, "inbox", filename)?;
    if !path.is_file() {
        return Err(format!("{filename} does not exist"));
    }
    atomic_write(&path, content)?;
    Ok(parse_item(filename, content))
}

#[tauri::command]
pub fn complete_inbox_item(
    state: tauri::State<RepoState>,
    filename: String,
) -> Result<String, String> {
    complete_inbox_item_at(&require_root(&state)?, &filename)
}

pub fn complete_inbox_item_at(root: &std::path::Path, filename: &str) -> Result<String, String> {
    let src = confine(root, "inbox", filename)?;
    if !src.is_file() {
        return Err(format!("{filename} does not exist"));
    }
    fs::create_dir_all(root.join("completed")).map_err(|e| e.to_string())?;
    let dest = dedupe_path(confine(root, "completed", filename)?);
    fs::rename(&src, &dest).map_err(|e| e.to_string())?;
    Ok(dest.file_name().unwrap().to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_inbox_item(state: tauri::State<RepoState>, filename: String) -> Result<(), String> {
    delete_inbox_item_at(&require_root(&state)?, &filename)
}

pub fn delete_inbox_item_at(root: &std::path::Path, filename: &str) -> Result<(), String> {
    let path = confine(root, "inbox", filename)?;
    fs::remove_file(&path).map_err(|e| format!("{filename}: {e}"))
}

#[tauri::command]
pub fn open_inbox_extra(state: tauri::State<RepoState>, name: String) -> Result<(), String> {
    let root = require_root(&state)?;
    let path = confine(&root, "inbox", &name)?;
    if !path.exists() {
        return Err(format!("{name} does not exist"));
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_scanner_convention() {
        assert_eq!(slugify("Analyze FIT data: Coros vs RUN"), "analyze-fit-data-coros-vs-run");
        // 50-char truncation, trailing dash trimmed
        let long = "Discuss product scanning and SBOM automation with Mark";
        let s = slugify(long);
        assert!(s.len() <= 50);
        assert!(!s.ends_with('-'));
        assert_eq!(s, "discuss-product-scanning-and-sbom-automation-with");
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify("  Héllo,  Wörld!  "), "h-llo-w-rld");
    }

    #[test]
    fn parses_slack_item() {
        let content = "# Analyze FIT data: Coros vs RUN\n\nAdded: 2026-04-24\n\n**Source:** https://x.slack.com/archives/1\n**Channel/Conversation:** U015\n**Original message:** Compare FIT $task\n**Context:** Something.\n";
        let item = parse_item("2026-04-24-analyze-fit-data-coros-vs-run.md", content);
        assert_eq!(item.title, "Analyze FIT data: Coros vs RUN");
        assert_eq!(item.added.as_deref(), Some("2026-04-24"));
        assert!(item.is_slack);
        assert_eq!(item.preview, "Compare FIT $task");
    }

    #[test]
    fn handles_undated_file() {
        let content = "# Partner Live Sync\n\nSome architecture notes.";
        let item = parse_item("partner-sync-architecture.md", content);
        assert_eq!(item.added, None);
        assert!(!item.is_slack);
        assert_eq!(item.preview, "Some architecture notes.");
    }

    #[test]
    fn handles_malformed_body_and_empty_file() {
        let item = parse_item("2026-05-20-discuss-x.md", "# Discuss X\n\nAdded: 2026-05-20\n\n[object Object]\n");
        assert_eq!(item.preview, "[object Object]");
        let empty = parse_item("2026-05-20-blank-note.md", "");
        assert_eq!(empty.title, "blank note");
        assert_eq!(empty.added.as_deref(), Some("2026-05-20"));
        assert_eq!(empty.preview, "");
    }

    #[test]
    fn confinement_rejects_traversal() {
        let root = std::path::Path::new("/tmp/fake");
        assert!(crate::repo::confine(root, "inbox", "../x.md").is_err());
        assert!(crate::repo::confine(root, "inbox", "a/b.md").is_err());
        assert!(crate::repo::confine(root, "inbox", ".hidden").is_err());
        assert!(crate::repo::confine(root, "inbox", "ok-file.md").is_ok());
    }
}
