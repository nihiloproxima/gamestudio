mod assets;
mod cli;
mod commands;
mod control;
mod github;
mod paths;
mod plan;
mod pty;
mod sessions;
mod store;
mod sync;
mod tmux;
mod update;
mod worktrees;

pub fn cli(args: Vec<String>) -> i32 {
    cli::main(args)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if control::already_running() {
        eprintln!("Gamestudio est déjà ouverte.");
        return;
    }
    paths::adopt_login_path();
    for prepared in [tmux::write_conf(), sessions::write_hooks(), sessions::link_cli()] {
        if let Err(e) = prepared {
            eprintln!("{e}");
        }
    }
    sessions::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(pty::Ptys::default())
        .setup(|app| {
            control::serve(app.handle().clone());
            sync::start(app.handle().clone());
            update::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sync_status,
            commands::sync_now,
            commands::projects_list,
            commands::project_save,
            commands::project_attach,
            commands::milestones_list,
            commands::milestone_save,
            commands::milestone_delete,
            commands::items_list,
            commands::item_create,
            commands::item_update,
            commands::item_delete,
            commands::sessions_list,
            commands::session_start,
            commands::session_setup,
            commands::session_resume,
            commands::session_stop,
            commands::worktrees_list,
            commands::worktree_remove,
            commands::prs_list,
            commands::assets_list,
            commands::path_open,
            commands::path_vscode,
            commands::update_status,
            commands::update_check,
            commands::app_restart,
            pty::pty_attach,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_detach,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
