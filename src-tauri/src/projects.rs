use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::model::{Artifact, MetaEntry, ProjectDetail, ProjectSummary};
use crate::repo::{confine_rel, require_root, RepoState};

// ---------- registry (projects/README.md) parsing ----------

struct RegistryRow {
    dir: String,
    name: String,
    priority: Option<u32>,
    status: Option<String>,
    stakeholders: Option<String>,
    target: Option<String>,
    on_hold: bool,
}

fn split_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn is_separator_row(cells: &[String]) -> bool {
    cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

fn cell(cells: &[String], idx: Option<usize>) -> Option<String> {
    let v = cells.get(idx?)?.trim().to_string();
    if v.is_empty() || v == "—" || v == "-" {
        None
    } else {
        Some(v)
    }
}

/// Extract the folder name from a `[label](dir/README.md)` link cell.
fn dir_from_link(link_cell: &str) -> Option<String> {
    let start = link_cell.find("](")? + 2;
    let end = link_cell[start..].find(')')? + start;
    let target = &link_cell[start..end];
    let target = target.strip_suffix("README.md").unwrap_or(target);
    let target = target.trim_end_matches('/');
    if target.is_empty() {
        None
    } else {
        Some(target.to_string())
    }
}

fn parse_registry(md: &str) -> Vec<RegistryRow> {
    let mut rows = Vec::new();
    let mut on_hold = false;
    let mut header: Option<HashMap<String, usize>> = None;
    for line in md.lines() {
        let trimmed = line.trim();
        if let Some(h) = trimmed.strip_prefix("## ") {
            on_hold = h.to_lowercase().contains("hold");
            header = None;
            continue;
        }
        if !trimmed.starts_with('|') {
            if trimmed.is_empty() {
                header = None;
            }
            continue;
        }
        let cells = split_row(trimmed);
        if is_separator_row(&cells) {
            continue;
        }
        if header.is_none() {
            header = Some(
                cells
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.to_lowercase(), i))
                    .collect(),
            );
            continue;
        }
        let h = header.as_ref().unwrap();
        let col = |name: &str| h.get(name).copied();
        let link = match cell(&cells, col("link")) {
            Some(l) => l,
            None => continue,
        };
        let dir = match dir_from_link(&link) {
            Some(d) => d,
            None => continue,
        };
        rows.push(RegistryRow {
            dir,
            name: cell(&cells, col("project")).unwrap_or_default(),
            priority: cell(&cells, col("priority")).and_then(|p| p.parse().ok()),
            status: cell(&cells, col("status")),
            stakeholders: cell(&cells, col("stakeholders")),
            target: cell(&cells, col("target date")).or_else(|| cell(&cells, col("paused since"))),
            on_hold,
        });
    }
    rows
}

// ---------- project README metadata ----------

fn is_meta_line(line: &str) -> bool {
    match line.split_once(':') {
        Some((key, value)) => {
            !key.is_empty()
                && !value.trim().is_empty()
                && key.chars().next().map_or(false, |c| c.is_ascii_alphabetic())
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '_' || c == '-')
        }
        None => false,
    }
}

/// Post-H1 unfenced `key: value` block, terminated by a blank line or `---`.
/// Mirrors toMetadataTable() in pages-static/build-static.mjs (needs >= 2 entries).
pub fn parse_metadata(md: &str) -> Vec<MetaEntry> {
    let mut lines = md.lines().peekable();
    // Skip until past the H1 (or from the top if there is none).
    let mut seen_h1 = false;
    let mut entries = Vec::new();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if !seen_h1 {
            if t.starts_with('#') {
                seen_h1 = true;
            } else if is_meta_line(t) {
                seen_h1 = true; // metadata with no H1 above it
                entries.push(t.to_string());
            } else if !t.is_empty() {
                return Vec::new(); // body starts before any H1/metadata
            }
            continue;
        }
        if t.is_empty() {
            if entries.is_empty() {
                continue; // blank line(s) between H1 and block
            }
            break;
        }
        if t == "---" {
            break;
        }
        if is_meta_line(t) {
            entries.push(t.to_string());
        } else {
            return Vec::new(); // not a metadata block
        }
    }
    if entries.len() < 2 {
        return Vec::new();
    }
    entries
        .iter()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| MetaEntry {
            key: k.trim().to_string(),
            value: v.trim().to_string(),
        })
        .collect()
}

fn readme_title(md: &str, fallback: &str) -> String {
    md.lines()
        .find_map(|l| l.trim().strip_prefix("# ").map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| fallback.replace('-', " "))
}

fn file_status(project_dir: &Path) -> Option<String> {
    let md = fs::read_to_string(project_dir.join("README.md")).ok()?;
    parse_metadata(&md)
        .into_iter()
        .find(|e| e.key.eq_ignore_ascii_case("status"))
        .map(|e| e.value)
}

// ---------- artifact walker ----------

fn walk_artifacts(base: &Path, rel: &str, depth: u32, out: &mut Vec<Artifact>) {
    if depth > 3 {
        return;
    }
    let dir = if rel.is_empty() { base.to_path_buf() } else { base.join(rel) };
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let rel_path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let path = entry.path();
        if path.is_dir() {
            walk_artifacts(base, &rel_path, depth + 1, out);
        } else if rel_path != "README.md" {
            let kind = match path.extension().and_then(|e| e.to_str()) {
                Some("md") => "md",
                Some("html") => "html",
                _ => "other",
            };
            out.push(Artifact { rel_path, kind: kind.to_string() });
        }
    }
}

// ---------- commands ----------

#[tauri::command]
pub fn list_projects(state: tauri::State<RepoState>) -> Result<Vec<ProjectSummary>, String> {
    list_projects_at(&require_root(&state)?)
}

pub fn list_projects_at(root: &Path) -> Result<Vec<ProjectSummary>, String> {
    let projects_dir = root.join("projects");
    let registry_md = fs::read_to_string(projects_dir.join("README.md")).unwrap_or_default();
    let rows = parse_registry(&registry_md);

    let mut summaries: Vec<ProjectSummary> = rows
        .iter()
        .filter(|r| projects_dir.join(&r.dir).is_dir())
        .map(|r| ProjectSummary {
            dir: r.dir.clone(),
            name: if r.name.is_empty() { r.dir.replace('-', " ") } else { r.name.clone() },
            priority: r.priority,
            registry_status: r.status.clone(),
            stakeholders: r.stakeholders.clone(),
            target: r.target.clone(),
            file_status: file_status(&projects_dir.join(&r.dir)),
            on_hold: r.on_hold,
            unregistered: false,
        })
        .collect();

    // Folders present on disk but missing from the registry table.
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }
            if summaries.iter().any(|s| s.dir == name) {
                continue;
            }
            let readme = fs::read_to_string(entry.path().join("README.md")).unwrap_or_default();
            summaries.push(ProjectSummary {
                name: readme_title(&readme, &name),
                dir: name.clone(),
                priority: None,
                registry_status: None,
                stakeholders: None,
                target: None,
                file_status: file_status(&entry.path()),
                on_hold: false,
                unregistered: true,
            });
        }
    }

    // Active ranked first (priority asc, ties by name), then active unranked,
    // then unregistered, then on-hold.
    summaries.sort_by(|a, b| {
        let group = |s: &ProjectSummary| match (s.on_hold, s.unregistered, s.priority) {
            (false, false, Some(_)) => 0,
            (false, false, None) => 1,
            (false, true, _) => 2,
            (true, _, _) => 3,
        };
        group(a)
            .cmp(&group(b))
            .then_with(|| a.priority.unwrap_or(u32::MAX).cmp(&b.priority.unwrap_or(u32::MAX)))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(summaries)
}

#[tauri::command]
pub fn get_project(state: tauri::State<RepoState>, dir: String) -> Result<ProjectDetail, String> {
    get_project_at(&require_root(&state)?, dir)
}

pub fn get_project_at(root: &Path, dir: String) -> Result<ProjectDetail, String> {
    let base = confine_rel(&root.join("projects"), &dir)?;
    if !base.is_dir() {
        return Err(format!("project {dir:?} not found"));
    }
    let readme = fs::read_to_string(base.join("README.md"))
        .unwrap_or_else(|_| format!("# {dir}\n\n_No README.md in this project folder._"));
    let mut artifacts = Vec::new();
    walk_artifacts(&base, "", 0, &mut artifacts);
    let kind_rank = |k: &str| match k {
        "md" => 0,
        "html" => 1,
        _ => 2,
    };
    artifacts.sort_by(|a, b| {
        kind_rank(&a.kind)
            .cmp(&kind_rank(&b.kind))
            .then_with(|| a.rel_path.to_lowercase().cmp(&b.rel_path.to_lowercase()))
    });
    Ok(ProjectDetail {
        name: readme_title(&readme, &dir),
        metadata: parse_metadata(&readme),
        dir,
        readme,
        artifacts,
    })
}

#[tauri::command]
pub fn read_artifact(
    state: tauri::State<RepoState>,
    dir: String,
    rel_path: String,
) -> Result<String, String> {
    let root = require_root(&state)?;
    let base = confine_rel(&root.join("projects"), &dir)?;
    let path = confine_rel(&base, &rel_path)?;
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("only markdown artifacts can be read in-app".into());
    }
    fs::read_to_string(&path).map_err(|e| format!("{rel_path}: {e}"))
}

#[tauri::command]
pub fn open_project_file(
    state: tauri::State<RepoState>,
    dir: String,
    rel_path: String,
) -> Result<(), String> {
    let root = require_root(&state)?;
    let base = confine_rel(&root.join("projects"), &dir)?;
    let path = confine_rel(&base, &rel_path)?;
    if !path.exists() {
        return Err(format!("{rel_path} does not exist"));
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_project_file(
    state: tauri::State<RepoState>,
    dir: String,
    rel_path: String,
) -> Result<(), String> {
    let root = require_root(&state)?;
    let base = confine_rel(&root.join("projects"), &dir)?;
    let path = confine_rel(&base, &rel_path)?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &str = "# Projects\n\n## Active\n\n| Priority | Project | Status | Stakeholders | Target Date | Link |\n| -------- | ------- | ------ | ------------ | ----------- | ---- |\n| 1 | Widget Redesign | Active | — | 2026-Q3 (end of quarter) | [widget-redesign/README.md](widget-redesign/README.md) |\n| — | Hiring | Active | Sam (DRI) | 2026-Q3 | [hiring/README.md](hiring/README.md) |\n\n## On Hold\n\n| Project | Status | Stakeholders | Paused Since | Link |\n| ------- | ------ | ------------ | ------------ | ---- |\n| Surveys | Hold | Sam | 3/9/2026 | [surveys/README.md](surveys/README.md) |\n";

    #[test]
    fn parses_registry_tables() {
        let rows = parse_registry(REGISTRY);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].dir, "widget-redesign");
        assert_eq!(rows[0].priority, Some(1));
        assert!(!rows[0].on_hold);
        assert_eq!(rows[1].priority, None); // — is unranked
        assert_eq!(rows[1].stakeholders.as_deref(), Some("Sam (DRI)"));
        assert!(rows[2].on_hold);
        assert_eq!(rows[2].target.as_deref(), Some("3/9/2026")); // Paused Since column
    }

    #[test]
    fn parses_unfenced_metadata() {
        let md = "# Widget Redesign\n\ntype: project\nstatus: active\nstarted: 2026-03-05\ntarget: TBD\n\n---\n\n## Summary\n";
        let meta = parse_metadata(md);
        assert_eq!(meta.len(), 4);
        assert_eq!(meta[1].key, "status");
        assert_eq!(meta[1].value, "active");
    }

    #[test]
    fn metadata_requires_two_entries_and_rejects_prose() {
        assert!(parse_metadata("# Title\n\nJust a paragraph of text.\n").is_empty());
        assert!(parse_metadata("# Title\n\nstatus: active\n\nBody.\n").is_empty());
        assert!(parse_metadata("").is_empty());
    }
}
