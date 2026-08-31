//! Full create -> edit -> complete -> delete round-trip in a sandbox repo.

use std::fs;
use std::path::PathBuf;

use deskwork_lib::inbox;

fn sandbox() -> PathBuf {
    let root = std::env::temp_dir().join(format!("deskwork-sandbox-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("inbox")).unwrap();
    fs::create_dir_all(root.join("projects")).unwrap();
    root
}

#[test]
fn create_edit_complete_delete_roundtrip() {
    let root = sandbox();

    // Create: filename convention, content shape.
    let item = inbox::create_inbox_item_at(&root, "Try the thing: a test", "Some body.").unwrap();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(item.filename, format!("{date}-try-the-thing-a-test.md"));
    let on_disk = fs::read_to_string(root.join("inbox").join(&item.filename)).unwrap();
    assert_eq!(on_disk, format!("# Try the thing: a test\n\nAdded: {date}\n\nSome body.\n"));

    // Same-day duplicate title gets a -2 suffix.
    let dup = inbox::create_inbox_item_at(&root, "Try the thing: a test", "").unwrap();
    assert_eq!(dup.filename, format!("{date}-try-the-thing-a-test-2.md"));

    // Long titles truncate to the scanner's 50-char slug limit.
    let long = inbox::create_inbox_item_at(
        &root,
        "This is an extremely long title that will definitely exceed the fifty character slug limit",
        "",
    )
    .unwrap();
    let slug_len = long.filename.len() - "YYYY-MM-DD-".len() - ".md".len();
    assert!(slug_len <= 50, "slug too long: {}", long.filename);

    // Edit: atomic in-place write, re-parsed metadata.
    let edited = inbox::save_inbox_item_at(
        &root,
        &item.filename,
        &format!("# Renamed title\n\nAdded: {date}\n\nNew body.\n"),
    )
    .unwrap();
    assert_eq!(edited.title, "Renamed title");

    // Complete: moved verbatim into completed/, collision-safe.
    let done = inbox::complete_inbox_item_at(&root, &item.filename).unwrap();
    assert_eq!(done, item.filename);
    assert!(!root.join("inbox").join(&item.filename).exists());
    let moved = fs::read_to_string(root.join("completed").join(&done)).unwrap();
    assert!(moved.starts_with("# Renamed title"));

    // Completing a same-named file again suffixes instead of overwriting.
    fs::write(root.join("inbox").join(&item.filename), &moved).unwrap();
    let done2 = inbox::complete_inbox_item_at(&root, &item.filename).unwrap();
    assert_eq!(done2, item.filename.replace(".md", "-2.md"));

    // Delete removes the file; deleting again errors cleanly.
    inbox::delete_inbox_item_at(&root, &dup.filename).unwrap();
    assert!(!root.join("inbox").join(&dup.filename).exists());
    assert!(inbox::delete_inbox_item_at(&root, &dup.filename).is_err());

    // Nothing outside inbox/ and completed/ was touched.
    assert_eq!(fs::read_dir(root.join("projects")).unwrap().count(), 0);

    let _ = fs::remove_dir_all(&root);
}
