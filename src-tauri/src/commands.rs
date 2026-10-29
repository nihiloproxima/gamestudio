//! The UI's IPC surface. Anything touching git, gh, tmux or the disk runs off the main thread, so a slow `gh` or a
//! large repo never freezes the window.

use tauri::AppHandle;

use crate::{assets, control, github, paths, sessions, store, sync, update, worktrees};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// Data and sync ---------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn sync_status() -> Result<sync::SyncStatus, String> {
    blocking(|| Ok(sync::status())).await
}

#[tauri::command]
pub async fn sync_now() -> Result<sync::SyncStatus, String> {
    blocking(sync::now).await
}


// Projects, milestones, items -------------------------------------------------------------------------------------

#[tauri::command]
pub async fn projects_list() -> Result<Vec<store::Project>, String> {
    blocking(|| Ok(store::projects())).await
}

#[tauri::command]
pub async fn project_save(app: AppHandle, id: String, meta: store::ProjectMeta, vision: String) -> Result<store::Project, String> {
    blocking(move || {
        // What the repo already says (GitHub repo, default branch) need not be typed.
        let mut meta = meta;
        worktrees::fill_from_repo(&mut meta);
        let project = store::save_project(&id, meta, vision)?;
        control::data_changed(&app);
        Ok(project)
    })
    .await
}

/// Adds a game from its repo; its data comes back if the repo already has a `gamestudio` branch.
#[tauri::command]
pub async fn project_attach(app: AppHandle, meta: store::ProjectMeta) -> Result<store::Project, String> {
    blocking(move || {
        let mut meta = meta;
        worktrees::fill_from_repo(&mut meta);
        let id = sync::attach(meta)?;
        control::data_changed(&app);
        store::project(&id)
    })
    .await
}

#[tauri::command]
pub async fn milestones_list(project: String) -> Result<Vec<store::Milestone>, String> {
    blocking(move || Ok(store::milestones(&project))).await
}

#[tauri::command]
pub async fn milestone_save(
    app: AppHandle,
    project: String,
    id: Option<String>,
    meta: store::MilestoneMeta,
    goal: String,
) -> Result<store::Milestone, String> {
    blocking(move || {
        let m = store::save_milestone(&project, id, meta, goal)?;
        control::data_changed(&app);
        Ok(m)
    })
    .await
}

#[tauri::command]
pub async fn milestone_delete(app: AppHandle, project: String, id: String) -> Result<(), String> {
    blocking(move || {
        store::delete_milestone(&project, &id)?;
        control::data_changed(&app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn items_list(project: String) -> Result<Vec<store::Item>, String> {
    blocking(move || Ok(store::items(&project))).await
}

#[tauri::command]
pub async fn item_create(app: AppHandle, project: String, draft: store::ItemDraft) -> Result<store::Item, String> {
    blocking(move || {
        let item = store::create_item(&project, draft)?;
        control::data_changed(&app);
        Ok(item)
    })
    .await
}

#[tauri::command]
pub async fn item_update(app: AppHandle, id: String, patch: store::ItemPatch) -> Result<store::Item, String> {
    blocking(move || {
        let item = store::update_item(&id, patch)?;
        control::data_changed(&app);
        Ok(item)
    })
    .await
}

#[tauri::command]
pub async fn item_delete(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || {
        store::delete_item(&id)?;
        control::data_changed(&app);
        Ok(())
    })
    .await
}

// Sessions and worktrees ------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn sessions_list() -> Result<Vec<sessions::Session>, String> {
    blocking(|| Ok(sessions::list())).await
}

#[tauri::command]
pub async fn session_start(app: AppHandle, req: sessions::StartRequest) -> Result<sessions::Session, String> {
    blocking(move || sessions::start(&app, req)).await
}

/// A Claude session in the project's repo that scans it, asks the user, and fills Gamestudio once they agree.
#[tauri::command]
pub async fn session_setup(app: AppHandle, project: String) -> Result<sessions::Session, String> {
    blocking(move || {
        let req = sessions::StartRequest {
            project,
            item: None,
            agent: "claude".into(),
            place: "main".into(),
            branch: None,
            path: None,
            prompt: sessions::SETUP_PROMPT.into(),
            title: Some("Configuration de Gamestudio".into()),
            parent: None,
            role: None,
        };
        sessions::start(&app, req)
    })
    .await
}

/// Claude Opus on an integration branch, delegating the roadmap's parallel work to agents and merging it
/// (`studio guide orchestrate`).
#[tauri::command]
pub async fn session_orchestrate(app: AppHandle, project: String, parallel: u32, prompt: String) -> Result<sessions::Session, String> {
    blocking(move || sessions::orchestrate(&app, &project, parallel, &prompt)).await
}

/// Runs the game from the main checkout, or from the worktree at `path`; again where it runs, restarts it.
#[tauri::command]
pub async fn session_play(app: AppHandle, project: String, path: Option<String>) -> Result<sessions::Session, String> {
    blocking(move || sessions::play(&app, &project, path.as_deref())).await
}

#[tauri::command]
pub async fn session_resume(app: AppHandle, id: String) -> Result<sessions::Session, String> {
    blocking(move || sessions::resume(&app, &id)).await
}

#[tauri::command]
pub async fn session_stop(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || sessions::stop(&app, &id)).await
}

#[tauri::command]
pub async fn worktrees_list(project: String) -> Result<Vec<worktrees::Worktree>, String> {
    blocking(move || worktrees::list(&project)).await
}

#[tauri::command]
pub async fn worktree_remove(project: String, path: String) -> Result<(), String> {
    blocking(move || worktrees::remove(&project, &path)).await
}

// GitHub, assets, opening things ----------------------------------------------------------------------------------

#[tauri::command]
pub async fn prs_list(project: String) -> Result<Vec<github::PullRequest>, String> {
    blocking(move || github::list(&project)).await
}

#[tauri::command]
pub async fn assets_list(project: String) -> Result<Vec<assets::Asset>, String> {
    blocking(move || assets::list(&project)).await
}

#[tauri::command]
pub async fn path_open(path: String, reveal: bool) -> Result<(), String> {
    blocking(move || paths::open(&path, reveal)).await
}

#[tauri::command]
pub async fn path_vscode(path: String) -> Result<(), String> {
    blocking(move || paths::open_in_vscode(&path)).await
}

// Updates ---------------------------------------------------------------------------------------------------------

#[tauri::command]
pub fn update_status() -> update::UpdateStatus {
    update::status()
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<update::UpdateStatus, String> {
    blocking(move || Ok(update::check(&app))).await
}

/// Relaunches the app, on the update just installed. Terminal views reattach; agents never stopped.
#[tauri::command]
pub fn app_restart(app: AppHandle) {
    app.restart();
}
