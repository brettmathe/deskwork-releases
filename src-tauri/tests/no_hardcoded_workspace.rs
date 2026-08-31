//! Deskwork is published from a public repo, so the shipped bundle must not
//! carry a default pointing at anyone's particular workspace. A prefilled clone
//! URL would disclose a private repo's path to everyone who downloads the app.
//!
//! The old version of this test listed the exact strings to avoid, which meant
//! the guard itself disclosed them. This checks the shape instead: no repository
//! URL literal may appear in shipped frontend source.

use std::fs;
use std::path::{Path, PathBuf};

fn frontend_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("svelte" | "ts" | "js" | "html" | "css")
        ) {
            out.push(path);
        }
    }
}

/// Owner names that are obviously illustrative rather than someone's account.
/// A placeholder in an input field is good UX; a real owner is a disclosure.
const PLACEHOLDER_OWNERS: &[&str] =
    &["owner", "user", "username", "org", "your-org", "your-account", "example", "acme"];

/// A git host URL that names a *real* owner and repo, e.g.
/// `https://github.com/acme-corp/notes` or `git@github.com:acme-corp/notes`. A
/// bare host, a docs link, or an obvious placeholder is fine.
fn looks_like_a_repo_url(line: &str) -> bool {
    const HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org"];
    for host in HOSTS {
        for (i, _) in line.match_indices(host) {
            let rest = line[i + host.len()..].trim_start_matches([':', '/']);
            let path: String =
                rest.chars().take_while(|c| !matches!(c, '"' | '\'' | '`' | ' ')).collect();
            let mut parts = path.split('/').filter(|s| !s.is_empty());
            let (Some(owner), Some(_repo)) = (parts.next(), parts.next()) else {
                continue; // bare host or single segment -- not a repo reference
            };
            if !PLACEHOLDER_OWNERS.contains(&owner.to_ascii_lowercase().as_str()) {
                return true;
            }
        }
    }
    false
}

#[test]
fn frontend_ships_no_hardcoded_repository_url() {
    let root = frontend_src();
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(!files.is_empty(), "expected to scan frontend sources");

    for path in &files {
        for (n, line) in fs::read_to_string(path).unwrap().lines().enumerate() {
            assert!(
                !looks_like_a_repo_url(line),
                "{}:{}: hardcoded repository URL would ship in the public bundle:\n  {}",
                path.strip_prefix(&root).unwrap_or(path).display(),
                n + 1,
                line.trim()
            );
        }
    }
}

#[test]
fn the_detector_recognises_a_repo_url() {
    // Guards against the check silently passing because it matches nothing.
    // A concrete owner that is nobody's placeholder must be flagged.
    assert!(looks_like_a_repo_url(r#"const U = "https://github.com/northwind/widgets.git";"#));
    assert!(looks_like_a_repo_url(r#"const U = "git@github.com:northwind/widgets.git";"#));
    assert!(looks_like_a_repo_url(r#"const U = "https://gitlab.com/northwind/widgets";"#));
    // Placeholders and bare hosts are fine.
    assert!(!looks_like_a_repo_url(r#"placeholder="https://github.com/owner/repo.git""#));
    assert!(!looks_like_a_repo_url(r#"placeholder="https://github.com/your-org/workspace""#));
    assert!(!looks_like_a_repo_url(r#"see github.com for details"#));
    assert!(!looks_like_a_repo_url(r#"const HOST = "github.com";"#));
}
