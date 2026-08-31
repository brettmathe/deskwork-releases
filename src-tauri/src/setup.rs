//! First-run setup: find an existing workspace, clone one, or scaffold a new one.
//!
//! Everything here runs before a repo root exists, so these commands take
//! explicit paths rather than reading `RepoState` — they *establish* it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use tauri::Manager;

use crate::repo::{is_valid_root, save_root, RepoState};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RepoCandidate {
    pub path: String,
    /// Whether the folder is a git working tree (a workspace need not be one).
    pub has_git: bool,
    pub remote: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tooling {
    pub git: bool,
    pub gh: bool,
    pub gh_authed: bool,
    /// `user.name` and `user.email` are both resolvable, so a commit will succeed.
    pub git_identity: bool,
    /// Suggested parent folder for a clone/create (`~/Projects` if it exists, else `~`).
    pub default_parent: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub path: String,
    pub committed: bool,
    /// Non-fatal note to surface after success (e.g. git identity missing).
    pub warning: Option<String>,
}

// ---------- process helpers ----------

/// Run a command with no controlling terminal and every credential prompt
/// disabled, so a missing credential fails fast instead of hanging the GUI.
fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new")
        .env_remove("DISPLAY");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if err.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err
        };
        Err(msg)
    }
}

fn has_binary(program: &str) -> bool {
    run(program, &["--version"], None).is_ok()
}

// ---------- detection ----------

/// Parent folders (relative to home) where a workspace is plausibly checked out.
const SEARCH_PARENTS: &[&str] = &[
    "Projects", "projects", "Developer", "dev", "src", "code", "Code", "repos", "git", "Documents",
    "Documents/Projects", "",
];

const MAX_CANDIDATES: usize = 12;

pub fn detect_at(home: &Path) -> Vec<RepoCandidate> {
    let mut found: Vec<PathBuf> = Vec::new();

    for parent in SEARCH_PARENTS {
        let dir = if parent.is_empty() { home.to_path_buf() } else { home.join(parent) };
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') || !path.is_dir() {
                continue;
            }
            if !is_valid_root(&path) {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or(path);
            if !found.contains(&canonical) {
                found.push(canonical);
            }
        }
        if found.len() >= MAX_CANDIDATES {
            break;
        }
    }

    found.sort();
    found.truncate(MAX_CANDIDATES);
    found
        .into_iter()
        .map(|path| {
            let has_git = path.join(".git").exists();
            let remote = if has_git {
                run("git", &["remote", "get-url", "origin"], Some(&path)).ok().filter(|s| !s.is_empty())
            } else {
                None
            };
            RepoCandidate { path: path.to_string_lossy().to_string(), has_git, remote }
        })
        .collect()
}

// ---------- destination validation ----------

/// A workspace folder name must be a single path component we can safely join.
fn validate_folder_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Folder name is required.".into());
    }
    if name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains('\0')
    {
        return Err(format!("{name:?} isn't a valid folder name."));
    }
    Ok(())
}

fn is_empty_dir(path: &Path) -> bool {
    fs::read_dir(path).map(|mut e| e.next().is_none()).unwrap_or(false)
}

/// Resolve `parent/folder`, creating `parent` if needed. Errors if the
/// destination already exists with anything in it.
fn prepare_dest(parent: &str, folder: &str) -> Result<PathBuf, String> {
    validate_folder_name(folder)?;
    let parent = PathBuf::from(parent);
    if !parent.is_absolute() {
        return Err("Choose an absolute parent folder.".into());
    }
    fs::create_dir_all(&parent)
        .map_err(|e| format!("can't use {}: {e}", parent.display()))?;
    let dest = parent.join(folder.trim());
    if dest.exists() && !is_empty_dir(&dest) {
        return Err(format!("{} already exists and isn't empty.", dest.display()));
    }
    Ok(dest)
}

/// Adopt `root` as the workspace: persist it and update the live state.
fn adopt(
    app: &tauri::AppHandle,
    state: &tauri::State<RepoState>,
    root: &Path,
) -> Result<String, String> {
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    save_root(app, &canonical)?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(canonical.clone());
    Ok(canonical.to_string_lossy().to_string())
}

// ---------- clone ----------

/// Pull `owner/repo` out of a github.com URL in either https or ssh form.
pub fn github_slug(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .strip_prefix("https://github.com/")
        .or_else(|| url.trim().strip_prefix("git@github.com:"))
        .or_else(|| url.trim().strip_prefix("ssh://git@github.com/"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest).trim_end_matches('/');
    let mut parts = rest.split('/');
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let repo = parts.next().filter(|s| !s.is_empty())?;
    if parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

/// Default folder name for a clone destination, derived from the URL.
pub fn folder_from_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let tail = trimmed.rsplit(['/', ':']).next().unwrap_or("");
    let tail = tail.strip_suffix(".git").unwrap_or(tail);
    if tail.is_empty() { "mywork".into() } else { tail.into() }
}

fn gh_authed() -> bool {
    run("gh", &["auth", "status"], None).is_ok()
}

/// Clone `url` into `parent/folder`, preferring `gh` (which carries the user's
/// GitHub auth) and falling back to plain `git clone`.
pub fn clone_into(parent: &str, url: &str, folder: &str) -> Result<PathBuf, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Repository URL is required.".into());
    }
    let dest = prepare_dest(parent, folder)?;
    let dest_str = dest.to_string_lossy().to_string();

    let mut attempts: Vec<String> = Vec::new();

    // `gh repo clone` uses the user's stored GitHub token, so private repos
    // work without a keychain credential helper being primed first.
    if let Some(slug) = github_slug(url) {
        if has_binary("gh") && gh_authed() {
            match run("gh", &["repo", "clone", &slug, &dest_str], None) {
                Ok(_) => return Ok(dest),
                Err(e) => attempts.push(format!("gh repo clone: {e}")),
            }
        }
    }

    match run("git", &["clone", url, &dest_str], None) {
        Ok(_) => return Ok(dest),
        Err(e) => attempts.push(format!("git clone: {e}")),
    }

    // A failed clone can leave a partial directory behind; we created it, so
    // removing it keeps a retry from tripping the "already exists" check.
    let _ = fs::remove_dir_all(&dest);

    let hint = if github_slug(url).is_some() && !gh_authed() {
        "\n\nThis looks like a private GitHub repo. Run `gh auth login` in Terminal, then try again."
    } else {
        ""
    };
    Err(format!("Clone failed.\n\n{}{hint}", attempts.join("\n")))
}

// ---------- create ----------

const SEED_README: &str = r#"# Workspace

Personal work tracking, stored as plain markdown.

- `inbox/` — one file per open task
- `completed/` — finished tasks, moved here verbatim
- `projects/` — one folder per project; the registry table lives in `projects/README.md`

Tasks are named `YYYY-MM-DD-slug.md` and open with a `# Title` heading followed
by an `Added:` line.
"#;

const SEED_PROJECTS_README: &str = r#"# Projects

## Active

| Priority | Project | Status | Stakeholders | Target Date | Link |
| -------- | ------- | ------ | ------------ | ----------- | ---- |

## On Hold

| Priority | Project | Status | Stakeholders | Paused Since | Link |
| -------- | ------- | ------ | ------------ | ------------ | ---- |
"#;

const SEED_TEMPLATE: &str = r#"# Project Name

type: project
status: active
started: YYYY-MM-DD
target: YYYY-MM-DD

---

## Summary

_One-paragraph description of what this project is and why it matters._

## Stakeholders

| Role | Person |
|------|--------|

## Key Dates

| Milestone | Date | Status |
|-----------|------|--------|

## Decisions

| Date | Decision | Context | Decided By |
|------|----------|---------|------------|

## Open Questions

-

## Risks & Dependencies

| Risk/Dependency | Impact | Mitigation | Status |
|-----------------|--------|------------|--------|

## Links

-
"#;

/// Lay down the directory skeleton and seed files. Idempotent: existing files
/// are left alone so this is safe to run against an empty-but-present folder.
pub fn scaffold(dest: &Path) -> Result<(), String> {
    for dir in ["inbox", "completed", "projects"] {
        fs::create_dir_all(dest.join(dir))
            .map_err(|e| format!("creating {dir}/: {e}"))?;
    }

    let seeds: [(&str, &str); 6] = [
        ("README.md", SEED_README),
        (".gitignore", ".DS_Store\n"),
        // Git won't track an empty directory; these keep the layout intact on
        // clone. Both listings skip dotfiles, so they never show in the UI.
        ("inbox/.gitkeep", ""),
        ("completed/.gitkeep", ""),
        ("projects/README.md", SEED_PROJECTS_README),
        ("projects/_template.md", SEED_TEMPLATE),
    ];
    for (rel, body) in seeds {
        let path = dest.join(rel);
        if !path.exists() {
            fs::write(&path, body).map_err(|e| format!("writing {rel}: {e}"))?;
        }
    }
    Ok(())
}

/// True when both `user.name` and `user.email` resolve — i.e. a commit will not
/// fail with "please tell me who you are".
fn has_git_identity(cwd: Option<&Path>) -> bool {
    let name = run("git", &["config", "--get", "user.name"], cwd);
    let email = run("git", &["config", "--get", "user.email"], cwd);
    matches!((name, email), (Ok(n), Ok(e)) if !n.is_empty() && !e.is_empty())
}

/// `git init` the folder and make the initial commit. Returns whether the
/// commit happened; a missing git identity is reported, not fatal.
pub fn init_git(dest: &Path) -> Result<(bool, Option<String>), String> {
    if dest.join(".git").exists() {
        return Ok((false, None));
    }
    // `-b` needs git 2.28+; fall back for older toolchains.
    if run("git", &["init", "-b", "main", "."], Some(dest)).is_err() {
        run("git", &["init", "."], Some(dest)).map_err(|e| format!("git init: {e}"))?;
        let _ = run("git", &["symbolic-ref", "HEAD", "refs/heads/main"], Some(dest));
    }

    if !has_git_identity(Some(dest)) {
        return Ok((
            false,
            Some(
                "Git has no user.name / user.email configured, so the initial commit was skipped. \
                 Set them with `git config --global user.name` and `user.email`, then commit."
                    .into(),
            ),
        ));
    }

    run("git", &["add", "-A"], Some(dest)).map_err(|e| format!("git add: {e}"))?;
    run("git", &["commit", "-m", "Initial workspace"], Some(dest))
        .map_err(|e| format!("git commit: {e}"))?;
    Ok((true, None))
}

// ---------- commands ----------

#[tauri::command]
pub fn detect_repos(app: tauri::AppHandle) -> Result<Vec<RepoCandidate>, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    Ok(detect_at(&home))
}

#[tauri::command]
pub fn check_tooling(app: tauri::AppHandle) -> Result<Tooling, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let projects = home.join("Projects");
    let default_parent = if projects.is_dir() { projects } else { home };
    let gh = has_binary("gh");
    Ok(Tooling {
        git: has_binary("git"),
        gh,
        gh_authed: gh && gh_authed(),
        git_identity: has_git_identity(None),
        default_parent: default_parent.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn clone_repo(
    app: tauri::AppHandle,
    state: tauri::State<RepoState>,
    parent: String,
    url: String,
    folder: String,
) -> Result<SetupResult, String> {
    let dest = clone_into(&parent, &url, &folder)?;
    if !is_valid_root(&dest) {
        return Err(format!(
            "Cloned to {}, but it has no inbox/ and projects/ directories — that isn't a \
             workspace. Check the repository URL.",
            dest.display()
        ));
    }
    let path = adopt(&app, &state, &dest)?;
    Ok(SetupResult { path, committed: false, warning: None })
}

#[tauri::command]
pub fn create_repo(
    app: tauri::AppHandle,
    state: tauri::State<RepoState>,
    parent: String,
    folder: String,
    init_repo: bool,
) -> Result<SetupResult, String> {
    let dest = prepare_dest(&parent, &folder)?;
    fs::create_dir_all(&dest).map_err(|e| format!("creating {}: {e}", dest.display()))?;
    scaffold(&dest)?;

    let (committed, warning) = if init_repo {
        init_git(&dest)?
    } else {
        (false, None)
    };

    let path = adopt(&app, &state, &dest)?;
    Ok(SetupResult { path, committed, warning })
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("deskwork-setup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn github_slug_handles_https_ssh_and_rejects_others() {
        assert_eq!(
            github_slug("https://github.com/acme/notes-workspace.git").as_deref(),
            Some("acme/notes-workspace")
        );
        assert_eq!(
            github_slug("git@github.com:acme/notes-workspace.git").as_deref(),
            Some("acme/notes-workspace")
        );
        assert_eq!(
            github_slug("https://github.com/acme/notes-workspace").as_deref(),
            Some("acme/notes-workspace")
        );
        assert_eq!(github_slug("https://gitlab.com/a/b.git"), None);
        assert_eq!(github_slug("https://github.com/acme"), None);
        // A deeper path isn't a clonable repo root.
        assert_eq!(github_slug("https://github.com/a/b/tree/main"), None);
    }

    #[test]
    fn folder_from_url_strips_git_suffix() {
        assert_eq!(folder_from_url("https://github.com/acme/notes-workspace.git"), "notes-workspace");
        assert_eq!(folder_from_url("git@github.com:acme/notes-workspace.git"), "notes-workspace");
        assert_eq!(folder_from_url("https://example.com/x/mywork/"), "mywork");
        assert_eq!(folder_from_url(""), "mywork");
    }

    #[test]
    fn folder_names_reject_traversal() {
        assert!(validate_folder_name("mywork").is_ok());
        assert!(validate_folder_name("notes-workspace").is_ok());
        assert!(validate_folder_name("").is_err());
        assert!(validate_folder_name("  ").is_err());
        assert!(validate_folder_name("..").is_err());
        assert!(validate_folder_name("a/b").is_err());
        assert!(validate_folder_name("../escape").is_err());
        assert!(validate_folder_name(".hidden").is_err());
    }

    #[test]
    fn prepare_dest_creates_parent_and_guards_nonempty() {
        let base = tmp_dir("dest");
        let parent = base.join("nested/parent");
        let parent_str = parent.to_string_lossy().to_string();

        // Parent is created on demand.
        let dest = prepare_dest(&parent_str, "mywork").unwrap();
        assert_eq!(dest, parent.join("mywork"));
        assert!(parent.is_dir());

        // An existing but empty destination is fine (re-running after a failure).
        fs::create_dir_all(&dest).unwrap();
        assert!(prepare_dest(&parent_str, "mywork").is_ok());

        // Anything inside it and we refuse.
        fs::write(dest.join("file.md"), "x").unwrap();
        let err = prepare_dest(&parent_str, "mywork").unwrap_err();
        assert!(err.contains("isn't empty"), "{err}");

        assert!(prepare_dest("relative/path", "mywork").is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn scaffold_produces_a_valid_root() {
        let dest = tmp_dir("scaffold").join("mywork");
        fs::create_dir_all(&dest).unwrap();
        scaffold(&dest).unwrap();

        assert!(is_valid_root(&dest));
        assert!(dest.join("completed").is_dir());
        assert!(dest.join("inbox/.gitkeep").is_file());
        assert!(dest.join("projects/_template.md").is_file());

        // The seeded registry parses as an (empty) project list rather than erroring.
        let projects = crate::projects::list_projects_at(&dest).unwrap();
        assert!(projects.is_empty(), "{projects:?}", projects = projects.len());

        // The inbox is empty and the .gitkeep is not surfaced as a file or an extra.
        assert!(crate::inbox::list_inbox_at(&dest).unwrap().is_empty());
        assert!(crate::inbox::list_inbox_extras_at(&dest).unwrap().is_empty());

        let _ = fs::remove_dir_all(dest.parent().unwrap());
    }

    #[test]
    fn scaffold_is_idempotent_and_preserves_content() {
        let dest = tmp_dir("scaffold-idem").join("mywork");
        fs::create_dir_all(&dest).unwrap();
        scaffold(&dest).unwrap();
        fs::write(dest.join("README.md"), "hand-edited").unwrap();

        scaffold(&dest).unwrap();
        assert_eq!(fs::read_to_string(dest.join("README.md")).unwrap(), "hand-edited");

        let _ = fs::remove_dir_all(dest.parent().unwrap());
    }

    #[test]
    fn init_git_creates_a_repo_on_main_with_one_commit() {
        let dest = tmp_dir("initgit").join("mywork");
        fs::create_dir_all(&dest).unwrap();
        scaffold(&dest).unwrap();

        let (committed, warning) = init_git(&dest).unwrap();
        assert!(dest.join(".git").exists());

        if committed {
            assert!(warning.is_none());
            let branch = run("git", &["rev-parse", "--abbrev-ref", "HEAD"], Some(&dest)).unwrap();
            assert_eq!(branch, "main");
            let tracked = run("git", &["ls-files"], Some(&dest)).unwrap();
            assert!(tracked.contains("projects/README.md"), "{tracked}");
            assert!(tracked.contains("inbox/.gitkeep"), "{tracked}");
        } else {
            // No git identity on this machine — the warning must explain why.
            assert!(warning.unwrap().contains("user.name"));
        }

        // Second call is a no-op on an already-initialised folder.
        assert_eq!(init_git(&dest).unwrap(), (false, None));

        let _ = fs::remove_dir_all(dest.parent().unwrap());
    }

    #[test]
    fn detect_finds_valid_roots_and_skips_others() {
        let home = tmp_dir("detect");
        let projects = home.join("Projects");

        // A real workspace.
        let good = projects.join("mywork");
        fs::create_dir_all(good.join("inbox")).unwrap();
        fs::create_dir_all(good.join("projects")).unwrap();

        // Missing projects/ — not a workspace.
        fs::create_dir_all(projects.join("other/inbox")).unwrap();
        // Hidden directories are skipped entirely.
        let hidden = projects.join(".cache");
        fs::create_dir_all(hidden.join("inbox")).unwrap();
        fs::create_dir_all(hidden.join("projects")).unwrap();

        let found = detect_at(&home);
        assert_eq!(found.len(), 1, "{found:?}", found = found.iter().map(|c| &c.path).collect::<Vec<_>>());
        assert!(found[0].path.ends_with("mywork"));
        assert!(!found[0].has_git);
        assert!(found[0].remote.is_none());

        let _ = fs::remove_dir_all(&home);
    }
}
