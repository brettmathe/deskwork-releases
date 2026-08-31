pub mod git;
pub mod inbox;
pub mod model;
pub mod projects;
pub mod repo;
pub mod setup;

use std::path::PathBuf;
use std::sync::Mutex;

use repo::RepoState;
use tauri::Manager;

#[tauri::command]
fn get_repo_root(state: tauri::State<RepoState>) -> Result<Option<String>, String> {
    Ok(state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
fn set_repo_root(
    app: tauri::AppHandle,
    state: tauri::State<RepoState>,
    path: String,
) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !repo::is_valid_root(&p) {
        return Err(format!(
            "{path} doesn't look like a workspace (needs inbox/ and projects/ directories)"
        ));
    }
    let canonical = p.canonicalize().map_err(|e| e.to_string())?;
    repo::save_root(&app, &canonical)?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(canonical.clone());
    Ok(canonical.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .setup(|app| {
            let root = repo::resolve_root(app.handle());
            app.manage(RepoState(Mutex::new(root)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_repo_root,
            set_repo_root,
            inbox::list_inbox,
            inbox::list_inbox_extras,
            inbox::get_inbox_item,
            inbox::create_inbox_item,
            inbox::save_inbox_item,
            inbox::complete_inbox_item,
            inbox::delete_inbox_item,
            inbox::open_inbox_extra,
            projects::list_projects,
            projects::get_project,
            projects::read_artifact,
            projects::open_project_file,
            projects::reveal_project_file,
            git::git_status,
            git::git_pull,
            git::git_commit_push,
            setup::detect_repos,
            setup::check_tooling,
            setup::clone_repo,
            setup::create_repo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
