//! Exercises git status/pull/commit+push against a real temp repo with a bare
//! "origin", including the behind-after-CI-bot-commit scenario.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use deskwork_lib::git;

fn sh(dir: &Path, args: &[&str]) {
    let out = Command::new(args[0]).args(&args[1..]).current_dir(dir).output().unwrap();
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// origin (bare) + two clones: `local` plays Deskwork, `other` plays the CI bot.
fn setup() -> (PathBuf, PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("deskwork-git-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let origin = base.join("origin.git");
    fs::create_dir_all(&origin).unwrap();
    sh(&origin, &["git", "init", "--bare", "--initial-branch=main", "."]);

    let seed = base.join("seed");
    sh(&base, &["git", "clone", origin.to_str().unwrap(), "seed"]);
    for d in ["inbox", "projects", "completed"] {
        fs::create_dir_all(seed.join(d)).unwrap();
    }
    fs::write(seed.join("inbox/.gitkeep"), "").unwrap();
    fs::write(seed.join("completed/.gitkeep"), "").unwrap();
    fs::write(seed.join("projects/README.md"), "# Projects\n").unwrap();
    config_user(&seed);
    sh(&seed, &["git", "add", "-A"]);
    sh(&seed, &["git", "commit", "-m", "seed"]);
    sh(&seed, &["git", "push", "origin", "main"]);

    let local = base.join("local");
    let other = base.join("other");
    sh(&base, &["git", "clone", origin.to_str().unwrap(), "local"]);
    sh(&base, &["git", "clone", origin.to_str().unwrap(), "other"]);
    config_user(&local);
    config_user(&other);
    (base, local, other)
}

fn config_user(repo: &Path) {
    sh(repo, &["git", "config", "user.email", "test@test.local"]);
    sh(repo, &["git", "config", "user.name", "Test"]);
}

#[test]
fn full_sync_cycle_with_remote_bot_commit() {
    let (base, local, other) = setup();

    // Clean clone: no changes, in sync.
    let s = git::status_at(&local, false).unwrap();
    assert_eq!(s.branch, "main");
    assert!(s.has_upstream);
    assert_eq!((s.ahead, s.behind), (0, 0));
    assert!(s.task_changes.is_empty());

    // New task appears as an untracked task change.
    fs::write(local.join("inbox/2026-08-08-new-task.md"), "# New task\n\nAdded: 2026-08-08\n").unwrap();
    let s = git::status_at(&local, false).unwrap();
    assert_eq!(s.task_changes.len(), 1);
    assert_eq!(s.task_changes[0].status, "untracked");

    // Nothing staged + empty message is rejected before committing.
    assert!(git::commit_push_at(&local, "   ").is_err());

    // Commit & push works and lands on origin.
    let msg = git::commit_push_at(&local, "Add new task").unwrap();
    assert!(msg.contains("committed") && msg.contains("pushed"), "{msg}");
    let s = git::status_at(&local, true).unwrap();
    assert_eq!((s.ahead, s.behind), (0, 0));
    assert!(s.task_changes.is_empty());

    // "CI bot" pushes from elsewhere -> we are behind after fetch.
    sh(&other, &["git", "pull"]);
    fs::write(other.join("docs.txt"), "rebuilt site\n").unwrap();
    sh(&other, &["git", "add", "-A"]);
    sh(&other, &["git", "commit", "-m", "chore: rebuild site"]);
    sh(&other, &["git", "push"]);
    let s = git::status_at(&local, true).unwrap();
    assert_eq!((s.ahead, s.behind), (0, 1));

    // Pull catches up.
    git::pull_at(&local).unwrap();
    let s = git::status_at(&local, false).unwrap();
    assert_eq!((s.ahead, s.behind), (0, 0));
    assert!(local.join("docs.txt").exists());

    // Diverged case: local task commit + remote bot commit -> one button still lands it.
    fs::write(other.join("docs.txt"), "rebuilt again\n").unwrap();
    sh(&other, &["git", "commit", "-am", "chore: rebuild again"]);
    sh(&other, &["git", "push"]);
    fs::write(local.join("inbox/2026-08-08-second-task.md"), "# Second\n\nAdded: 2026-08-08\n").unwrap();
    let msg = git::commit_push_at(&local, "Add second task").unwrap();
    assert!(msg.contains("pulled") && msg.contains("pushed"), "{msg}");
    let s = git::status_at(&local, true).unwrap();
    assert_eq!((s.ahead, s.behind), (0, 0));

    // Changes outside inbox/completed are counted but never committed by the app.
    fs::write(local.join("projects/README.md"), "# Projects\n\nedited\n").unwrap();
    let s = git::status_at(&local, false).unwrap();
    assert_eq!(s.other_changes, 1);
    assert!(s.task_changes.is_empty());
    assert!(git::commit_push_at(&local, "should not commit projects").is_err()); // nothing staged, nothing ahead
    let s = git::status_at(&local, false).unwrap();
    assert_eq!(s.other_changes, 1, "projects/ edit must remain uncommitted");

    let _ = fs::remove_dir_all(&base);
}
