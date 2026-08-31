//! Parser checks against a checked-in fixture workspace.
//!
//! Replaces the old `live_repo.rs`, which resolved the repo root three levels up
//! and only worked while the app lived inside the workspace it managed. The
//! fixture carries the same awkward shapes that made those tests worth having --
//! a Slack-sourced item, an undated one, an unregistered project folder, nested
//! artifacts, dotfiles -- without naming anyone's real projects.

use std::fs;
use std::path::PathBuf;

use deskwork_lib::{inbox, projects};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace")
}

#[test]
fn inbox_lists_items_with_edge_cases() {
    let root = fixture();
    let items = inbox::list_inbox_at(&root).unwrap();
    assert_eq!(items.len(), 3, "only .md files are tasks");

    // Only .md files; directories and dotfiles are never tasks. The fixture
    // includes `.hidden-draft.md`, which is dotted *and* .md, so the dotfile
    // check has to win.
    assert!(root.join("inbox/.hidden-draft.md").is_file(), "fixture dotfile must be present");
    assert!(items.iter().all(|i| i.filename.ends_with(".md")));
    assert!(items.iter().all(|i| !i.filename.starts_with('.')), "dotted .md is not a task");

    // The undated item reports no date and sorts last.
    let undated = items.iter().find(|i| i.filename == "partner-sync-architecture.md").unwrap();
    assert_eq!(undated.added, None);
    assert_eq!(items.last().unwrap().filename, "partner-sync-architecture.md");

    // Dated items sort newest-first ahead of undated ones.
    let dates: Vec<&String> = items.iter().filter_map(|i| i.added.as_ref()).collect();
    assert!(dates.windows(2).all(|w| w[0] >= w[1]), "items should sort newest first");

    // The Slack flag agrees with the file on disk for every item.
    for item in &items {
        let content = fs::read_to_string(root.join("inbox").join(&item.filename)).unwrap();
        assert_eq!(
            item.is_slack,
            content.contains("**Source:**"),
            "{}: is_slack disagrees with its content",
            item.filename
        );
    }
    assert!(items.iter().any(|i| i.is_slack), "fixture includes a Slack-sourced item");
}

#[test]
fn inbox_extras_surface_non_task_entries() {
    let extras = inbox::list_inbox_extras_at(&fixture()).unwrap();
    assert!(extras.iter().all(|e| !e.name.starts_with('.')), "dotfiles must be filtered");
    let dir = extras.iter().find(|e| e.name == "attachments").expect("directory is an extra");
    assert!(dir.is_dir);
}

#[test]
fn registry_parses_and_covers_every_project_folder() {
    let root = fixture();
    let listed = projects::list_projects_at(&root).unwrap();

    // Every folder on disk appears exactly once -- registered from the table, or
    // picked up by the unregistered sweep.
    let mut on_disk: Vec<String> = fs::read_dir(root.join("projects"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    on_disk.sort();
    let mut names: Vec<String> = listed.iter().map(|p| p.dir.clone()).collect();
    names.sort();
    assert_eq!(names, on_disk, "every project folder should be listed exactly once");

    let widget = listed.iter().find(|p| p.dir == "widget-redesign").unwrap();
    assert_eq!(widget.priority, Some(1));
    assert!(!widget.on_hold);
    assert!(!widget.unregistered);
    assert_eq!(widget.file_status.as_deref(), Some("active"));

    assert!(listed.iter().find(|p| p.dir == "surveys").unwrap().on_hold);
    assert!(listed.iter().find(|p| p.dir == "unregistered-work").unwrap().unregistered);

    // Ranked actives come first, sorted by priority.
    let ranked: Vec<u32> = listed
        .iter()
        .take_while(|p| !p.on_hold && !p.unregistered && p.priority.is_some())
        .filter_map(|p| p.priority)
        .collect();
    assert!(ranked.windows(2).all(|w| w[0] <= w[1]), "ranked projects sort by priority");
}

#[test]
fn project_detail_reads_metadata_and_artifacts() {
    let root = fixture();
    let detail = projects::get_project_at(&root, "widget-redesign".into()).unwrap();
    assert_eq!(detail.name, "Widget Redesign");
    assert!(detail.metadata.iter().any(|m| m.key == "status"));
    assert!(detail.artifacts.iter().any(|a| a.rel_path == "design-notes.md" && a.kind == "md"));
    // Nested html is found; dotted paths never are.
    assert!(detail.artifacts.iter().any(|a| a.kind == "html"));

    // A dotted directory exists inside the project; nothing under it may surface.
    assert!(
        root.join("projects/widget-redesign/.internal/scratch.md").is_file(),
        "fixture dotted directory must be present"
    );
    for p in projects::list_projects_at(&root).unwrap() {
        let detail = projects::get_project_at(&root, p.dir.clone()).unwrap();
        assert!(
            detail.artifacts.iter().all(|a| !a.rel_path.split('/').any(|c| c.starts_with('.'))),
            "{}: dotfiles must not be listed as artifacts",
            p.dir
        );
    }
}
