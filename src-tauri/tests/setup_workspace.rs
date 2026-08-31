//! A workspace created by the wizard must be immediately usable: the inbox
//! commands work against it and the Repository panel can read its git status —
//! including before the first commit exists.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use deskwork_lib::{git, inbox, projects, setup};

fn sh(dir: &Path, args: &[&str]) {
    let out = Command::new(args[0]).args(&args[1..]).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("deskwork-ws-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Local identity so the test doesn't depend on the machine's global git config.
fn config_user(dir: &Path) {
    sh(dir, &["git", "config", "user.email", "test@example.com"]);
    sh(dir, &["git", "config", "user.name", "Deskwork Test"]);
}

#[test]
fn created_workspace_supports_the_full_task_lifecycle() {
    let base = tmp("lifecycle");
    let root = base.join("mywork");
    fs::create_dir_all(&root).unwrap();
    setup::scaffold(&root).unwrap();

    // Starts empty but valid.
    assert!(inbox::list_inbox_at(&root).unwrap().is_empty());
    assert!(projects::list_projects_at(&root).unwrap().is_empty());

    let created = inbox::create_inbox_item_at(&root, "Ship the installer", "Notes here.").unwrap();
    assert!(created.filename.ends_with("-ship-the-installer.md"));
    assert_eq!(inbox::list_inbox_at(&root).unwrap().len(), 1);

    let moved = inbox::complete_inbox_item_at(&root, &created.filename).unwrap();
    assert!(root.join("completed").join(&moved).is_file());
    assert!(inbox::list_inbox_at(&root).unwrap().is_empty());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn git_status_works_on_a_freshly_initialised_workspace() {
    let base = tmp("gitinit");
    let root = base.join("mywork");
    fs::create_dir_all(&root).unwrap();
    setup::scaffold(&root).unwrap();

    // Init without a commit — HEAD is unborn, the state right after `git init`.
    sh(&root, &["git", "init", "-b", "main", "."]);
    config_user(&root);

    let status = git::status_at(&root, false).unwrap();
    assert_eq!(status.branch, "main");
    assert!(!status.has_upstream);
    assert_eq!((status.ahead, status.behind), (0, 0));
    // Everything is untracked at this point, including the seeded task dirs.
    assert!(status.task_changes.iter().any(|c| c.status == "untracked"));

    // After the first commit the tree is clean.
    sh(&root, &["git", "add", "-A"]);
    sh(&root, &["git", "commit", "-m", "Initial workspace"]);
    let status = git::status_at(&root, false).unwrap();
    assert_eq!(status.branch, "main");
    assert!(status.task_changes.is_empty());
    assert_eq!(status.other_changes, 0);

    // A new task shows up as a task change, not an "outside tasks" change.
    inbox::create_inbox_item_at(&root, "First task", "").unwrap();
    let status = git::status_at(&root, false).unwrap();
    assert_eq!(status.task_changes.len(), 1);
    assert_eq!(status.other_changes, 0);

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_git_leaves_a_committed_repo_on_main() {
    let base = tmp("initcommit");
    let root = base.join("mywork");
    fs::create_dir_all(&root).unwrap();
    setup::scaffold(&root).unwrap();

    let (committed, warning) = setup::init_git(&root).unwrap();
    assert!(root.join(".git").exists());

    // On a machine with no git identity the commit is skipped, not failed.
    if !committed {
        assert!(warning.unwrap().contains("user.name"));
        let _ = fs::remove_dir_all(&base);
        return;
    }

    let status = git::status_at(&root, false).unwrap();
    assert_eq!(status.branch, "main");
    assert!(status.task_changes.is_empty());
    assert_eq!(status.other_changes, 0);

    let _ = fs::remove_dir_all(&base);
}
