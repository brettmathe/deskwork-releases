use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::repo::{require_root, RepoState};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitChange {
    pub status: String, // "added" | "modified" | "deleted" | "renamed" | "untracked" | "other"
    pub path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub branch: String,
    pub has_upstream: bool,
    pub ahead: u32,
    pub behind: u32,
    /// Uncommitted changes under inbox/ and completed/ (what Deskwork manages).
    pub task_changes: Vec<GitChange>,
    /// Count of uncommitted changes elsewhere in the repo (informational).
    pub other_changes: u32,
    /// Set when `fetch` was requested but failed (offline etc.); status is still local-only valid.
    pub fetch_error: Option<String>,
}

fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if err.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err
        };
        Err(format!("git {}: {msg}", args.first().unwrap_or(&"")))
    }
}

pub fn parse_ahead_behind(counts: &str) -> (u32, u32) {
    let mut parts = counts.split_whitespace();
    let ahead = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let behind = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

pub fn parse_porcelain_line(line: &str) -> Option<GitChange> {
    if line.len() < 4 {
        return None;
    }
    let code = &line[..2];
    let rest = line[3..].trim();
    // For renames the format is "old -> new"; show the new path.
    let path = rest.split(" -> ").last().unwrap_or(rest).trim_matches('"').to_string();
    let status = match code {
        c if c.contains('R') => "renamed",
        "??" => "untracked",
        c if c.contains('D') => "deleted",
        c if c.contains('A') => "added",
        c if c.contains('M') => "modified",
        _ => "other",
    };
    Some(GitChange { status: status.to_string(), path })
}

pub fn status_at(root: &Path, fetch: bool) -> Result<GitStatus, String> {
    let fetch_error = if fetch {
        run_git(root, &["fetch", "--quiet"]).err()
    } else {
        None
    };

    // `rev-parse HEAD` fails on a repo with no commits yet; `symbolic-ref` still
    // reports the branch a first commit would land on.
    let branch = match run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(b) => b,
        Err(e) => run_git(root, &["symbolic-ref", "--short", "HEAD"]).map_err(|_| e)?,
    };
    let upstream = run_git(root, &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok();
    let (ahead, behind) = match &upstream {
        Some(_) => {
            let counts = run_git(root, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])?;
            parse_ahead_behind(&counts)
        }
        None => (0, 0),
    };

    let task_porcelain = run_git(root, &["status", "--porcelain", "--", "inbox", "completed"])?;
    let task_changes: Vec<GitChange> =
        task_porcelain.lines().filter_map(parse_porcelain_line).collect();
    let all_porcelain = run_git(root, &["status", "--porcelain"])?;
    let total = all_porcelain.lines().filter(|l| l.len() >= 4).count() as u32;
    let other_changes = total.saturating_sub(task_changes.len() as u32);

    Ok(GitStatus {
        branch,
        has_upstream: upstream.is_some(),
        ahead,
        behind,
        task_changes,
        other_changes,
        fetch_error,
    })
}

/// Rebase-style pull with autostash; aborts a failed rebase so the repo is never
/// left mid-operation.
pub fn pull_at(root: &Path) -> Result<String, String> {
    match run_git(root, &["pull", "--rebase", "--autostash"]) {
        Ok(out) => Ok(if out.is_empty() { "Already up to date.".into() } else { out }),
        Err(e) => {
            let _ = run_git(root, &["rebase", "--abort"]);
            Err(format!("Pull failed (rebase aborted, repo unchanged): {e}"))
        }
    }
}

/// Stage inbox/ + completed/, commit with `message`, rebase onto upstream if
/// behind, then push. Safe to call with nothing staged but commits ahead.
pub fn commit_push_at(root: &Path, message: &str) -> Result<String, String> {
    run_git(root, &["add", "-A", "--", "inbox", "completed"])?;

    let staged_empty = run_git(root, &["diff", "--cached", "--quiet"]).is_ok();
    let mut actions = Vec::new();
    if !staged_empty {
        let message = message.trim();
        if message.is_empty() {
            return Err("commit message is required".into());
        }
        run_git(root, &["commit", "-m", message])?;
        actions.push("committed");
    }

    let status = status_at(root, true)?;
    if !status.has_upstream {
        return Err(format!("branch {} has no upstream to push to", status.branch));
    }
    if status.ahead == 0 && actions.is_empty() {
        return Err("nothing to commit or push".into());
    }
    if status.behind > 0 {
        pull_at(root)?;
        actions.push("pulled");
    }
    run_git(root, &["push"])?;
    actions.push("pushed");
    Ok(actions.join(", "))
}

// ---------- commands ----------

#[tauri::command]
pub fn git_status(state: tauri::State<RepoState>, fetch: bool) -> Result<GitStatus, String> {
    status_at(&require_root(&state)?, fetch)
}

#[tauri::command]
pub fn git_pull(state: tauri::State<RepoState>) -> Result<String, String> {
    pull_at(&require_root(&state)?)
}

#[tauri::command]
pub fn git_commit_push(state: tauri::State<RepoState>, message: String) -> Result<String, String> {
    commit_push_at(&require_root(&state)?, &message)
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ahead_behind_counts() {
        assert_eq!(parse_ahead_behind("1\t2"), (1, 2));
        assert_eq!(parse_ahead_behind("0\t0"), (0, 0));
        assert_eq!(parse_ahead_behind(""), (0, 0));
        assert_eq!(parse_ahead_behind("3"), (3, 0));
    }

    #[test]
    fn parses_porcelain_lines() {
        let c = parse_porcelain_line("?? inbox/2026-08-08-new-task.md").unwrap();
        assert_eq!((c.status.as_str(), c.path.as_str()), ("untracked", "inbox/2026-08-08-new-task.md"));
        let c = parse_porcelain_line(" M inbox/task.md").unwrap();
        assert_eq!(c.status, "modified");
        let c = parse_porcelain_line("R  inbox/a.md -> completed/a.md").unwrap();
        assert_eq!((c.status.as_str(), c.path.as_str()), ("renamed", "completed/a.md"));
        let c = parse_porcelain_line(" D inbox/gone.md").unwrap();
        assert_eq!(c.status, "deleted");
        assert!(parse_porcelain_line("").is_none());
    }
}
