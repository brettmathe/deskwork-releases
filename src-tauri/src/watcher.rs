//! Tells the UI when workspace files or git state change on disk.
//!
//! The window-focus refresh misses edits made while Deskwork stays focused, such
//! as Claude working in the embedded terminal. Events are debounced and sent as
//! a single `workspace-changed` event; the UI re-reads what it shows.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use tauri::{AppHandle, Emitter};

pub const EVENT: &str = "workspace-changed";

/// Content folders, watched recursively.
const CONTENT_DIRS: [&str; 3] = ["inbox", "completed", "projects"];

#[derive(Default)]
pub struct WatcherState(Mutex<Option<Debouncer<RecommendedWatcher>>>);

/// Whether a changed path should refresh the UI. Inside `.git` only the index
/// and HEAD count (commits, staging, checkouts); object and lock churn doesn't.
pub fn is_relevant(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else { return false };
    let mut parts = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string());
    match parts.next().as_deref() {
        Some(".git") => matches!(parts.next().as_deref(), Some("index" | "HEAD")) && parts.next().is_none(),
        Some(first) => CONTENT_DIRS.contains(&first),
        None => false,
    }
}

/// (Re)starts watching `root`, replacing any previous watcher.
pub fn watch(app: &AppHandle, state: &WatcherState, root: PathBuf) -> Result<(), String> {
    let handle = app.clone();
    let filter_root = root.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(400), move |res: DebounceEventResult| {
        if let Ok(events) = res {
            if events.iter().any(|e| is_relevant(&filter_root, &e.path)) {
                let _ = handle.emit(EVENT, ());
            }
        }
    })
    .map_err(|e| e.to_string())?;

    for dir in CONTENT_DIRS {
        let p = root.join(dir);
        if p.is_dir() {
            debouncer.watcher().watch(&p, RecursiveMode::Recursive).map_err(|e| e.to_string())?;
        }
    }
    let git = root.join(".git");
    if git.is_dir() {
        debouncer.watcher().watch(&git, RecursiveMode::NonRecursive).map_err(|e| e.to_string())?;
    }

    *state.0.lock().map_err(|e| e.to_string())? = Some(debouncer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevant_paths() {
        let root = Path::new("/ws");
        assert!(is_relevant(root, Path::new("/ws/projects/x/README.md")));
        assert!(is_relevant(root, Path::new("/ws/inbox/2026-10-09-task.md")));
        assert!(is_relevant(root, Path::new("/ws/completed/a.md")));
        assert!(is_relevant(root, Path::new("/ws/.git/index")));
        assert!(is_relevant(root, Path::new("/ws/.git/HEAD")));
        assert!(!is_relevant(root, Path::new("/ws/.git/index.lock")));
        assert!(!is_relevant(root, Path::new("/ws/.git/objects/ab/cdef")));
        assert!(!is_relevant(root, Path::new("/ws/docs/index.html")));
        assert!(!is_relevant(root, Path::new("/elsewhere/projects/x.md")));
    }
}
