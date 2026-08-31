use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::Manager;

/// Managed state: the resolved repo root (None until resolved/picked).
pub struct RepoState(pub Mutex<Option<PathBuf>>);

/// A directory qualifies as the repo root iff it has both content dirs.
pub fn is_valid_root(p: &Path) -> bool {
    p.join("inbox").is_dir() && p.join("projects").is_dir()
}

fn settings_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

fn load_saved_root(app: &tauri::AppHandle) -> Option<PathBuf> {
    let path = settings_path(app)?;
    let raw = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(PathBuf::from(v.get("repoRoot")?.as_str()?))
}

pub fn save_root(app: &tauri::AppHandle, root: &Path) -> Result<(), String> {
    let path = settings_path(app).ok_or("no app config dir")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body = serde_json::json!({ "repoRoot": root.to_string_lossy() }).to_string();
    atomic_write(&path, &body)
}

/// Resolution chain: env var -> saved settings -> location of this source tree.
pub fn resolve_root(app: &tauri::AppHandle) -> Option<PathBuf> {
    // Escape hatch for working on the first-run wizard, which is otherwise
    // unreachable in dev because the source-tree fallback always resolves.
    if std::env::var_os("DESKWORK_FORCE_SETUP").is_some() {
        return None;
    }
    if let Ok(env_root) = std::env::var("DESKWORK_REPO") {
        let p = PathBuf::from(env_root);
        if is_valid_root(&p) {
            return p.canonicalize().ok();
        }
    }
    if let Some(p) = load_saved_root(app) {
        if is_valid_root(&p) {
            return p.canonicalize().ok();
        }
    }
    // Deskwork used to live inside the workspace it managed and could resolve the
    // root from its own source location. It is now a standalone app, so there is
    // nothing to infer: the wizard asks.
    None
}

pub fn require_root(state: &tauri::State<RepoState>) -> Result<PathBuf, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "No workspace set. Choose your workspace folder in Settings.".into())
}

/// Join `filename` (a bare name, no separators) under `root/subdir`, rejecting traversal.
pub fn confine(root: &Path, subdir: &str, filename: &str) -> Result<PathBuf, String> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.starts_with('.')
    {
        return Err(format!("invalid filename: {filename:?}"));
    }
    Ok(root.join(subdir).join(filename))
}

/// Join a relative path (may contain '/') under `base`, rejecting traversal outside it.
pub fn confine_rel(base: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return Err(format!("invalid path: {rel:?}"));
    }
    for part in rel.split('/') {
        if part.is_empty() || part == ".." || part.starts_with('.') {
            return Err(format!("invalid path component in {rel:?}"));
        }
    }
    Ok(base.join(rel))
}

/// Write via temp file + rename (atomic on APFS).
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("path has no parent")?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let tmp = dir.join(format!(".deskwork-tmp-{nanos}"));
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// If `path` exists, insert -2/-3/... before the extension until it doesn't.
pub fn dedupe_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    for n in 2.. {
        let candidate = dir.join(format!("{stem}-{n}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("deskwork-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn atomic_write_creates_and_replaces() {
        let dir = tmp_dir("aw");
        let f = dir.join("x.md");
        atomic_write(&f, "one").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "one");
        atomic_write(&f, "two").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "two");
        // no temp files left behind
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedupe_appends_suffix() {
        let dir = tmp_dir("dd");
        let f = dir.join("2026-08-08-task.md");
        assert_eq!(dedupe_path(f.clone()), f); // free name unchanged
        fs::write(&f, "x").unwrap();
        assert_eq!(dedupe_path(f.clone()), dir.join("2026-08-08-task-2.md"));
        fs::write(dir.join("2026-08-08-task-2.md"), "x").unwrap();
        assert_eq!(dedupe_path(f), dir.join("2026-08-08-task-3.md"));
        let _ = fs::remove_dir_all(&dir);
    }
}
